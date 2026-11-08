# Plano — Etapa Codegen

## Nota de arquitetura (leia antes de começar)

O nome "Codegen" em `AGENTS.md` é tratado aqui, deliberadamente, como **tudo que vem depois do
parser**: análise semântica, sistema de módulos, inferência/checagem de tipos, especialização de
genéricos, catálogo de diagnósticos, biblioteca de runtime e emissão de código + driver de build.

Isso não é um desvio da especificação — é a leitura correta dela. `xpr-specs.md` diz
explicitamente: "Tipos estáticos: Todo o programa é analisado sintaticamente e verificado em
termos de tipo **antes que qualquer código seja gerado**" (Seção 1). Como `AGENTS.md` nomeia
apenas três etapas (Lexer, Parser, Codegen) e não há produção de bytecode/VM nesta linguagem —
`xprc` transpila para **Rust** e invoca `rustc`/`cargo` (Seção 8) — toda a análise estática e a
geração de código cabem na etapa "Codegen". Dado o tamanho, ela é dividida nas sete subetapas
abaixo, cada uma tratável como seu próprio ciclo TDD completo (Fase 2 de `AGENTS.md`), na ordem
listada — cada subetapa depende de dados produzidos pela anterior.

**Alvo de geração de código: Rust, não C.** A especificação original (quando o projeto começou em
C) descrevia `xprc` transpilando para C e invocando um compilador C. O projeto todo — compilador
e alvo — agora é Rust: `xpr-specs.md` já foi atualizada para refletir isso (Seção 8, e as notas
sobre nomes de módulo, refcounting e o catálogo de diagnósticos). Isso simplifica várias decisões
que a versão em C exigia:

* **Sem *mangling* manual de nomes.** Cada módulo XPR mapeia para um módulo Rust; o próprio
  sistema de módulos do Rust evita colisões (ver Seção 4.1 atualizada da spec e subetapa B).
* **Sem gerar um cabeçalho separado para o catálogo de diagnósticos.** Compilador e runtime são o
  mesmo idioma: o catálogo é um módulo Rust compartilhado diretamente (crate ou `mod` comum), não
  algo gerado de um lado para o outro (subetapa E).
* **Posições de erro no `.x` original** não dependem de um truque de pré-processador (`#line` do
  C): o código Rust gerado simplesmente embute linha/coluna como literais nas chamadas ao runtime
  de diagnóstico (subetapa G).

Subetapas:

* **A — Resolução de nomes e escopos**
* **B — Sistema de módulos** (import, aliases, nomes privados, busca de caminho, ciclos)
* **C — Inferência e checagem de tipos**
* **D — Genéricos e especialização**
* **E — Catálogo de diagnósticos** (`src/diag/catalog.rs`, módulo Rust compartilhado)
* **F — Biblioteca de runtime em Rust** (crate `xpr_rt`, vinculada ao executável gerado)
* **G — Emissão de Rust e driver de build** (CLI `xprc`, invoca `rustc`/`cargo`)

As subetapas **E** e **F** podem começar em paralelo com **A**-**D** (não dependem de tipos
resolvidos), mas **G** depende de A, B, C, D e E estarem concluídas, e o runtime (F) precisa estar
pronto antes que o Rust emitido por G possa ser compilado e testado de ponta a ponta.

**Testes desta etapa**: ver `plano-testes-codegen.md` para o roteiro evolutivo de milestones
executáveis (compila+roda+compara stdout/exit) e `scripts/run-codegen-milestones.sh` para o
runner. É um documento vivo, não uma lista fechada — ao contrário dos fixtures golden-file do
Lexer/Parser.

## Esboço de API

```rust
// src/sema/mod.rs — tipos resolvidos, por nó de AST
pub enum Ty {
    Bool, Int, Real, String,
    List(Box<Ty>),
    Tuple(Vec<Ty>),
    Fn(Box<Ty>, Box<Ty>), // A -> R
}

pub struct TypedModule {
    pub definitions: Vec<TypedDefinition>,
    // tabela de instâncias geradas por (nome_genérico, tipos_concretos) — ver subetapa D
    pub instances: Vec<FnInstance>,
}

pub struct SemaError { pub code: DiagCode, pub span: Option<Span>, pub detail: String }
```

### A. Resolução de nomes e escopos

* Tabela de escopos aninhados; atribuição a nome existente atualiza a vinculação mais próxima,
  atribuição a nome novo cria variável no escopo atual (Seção 4.6).
* Regra de uso-após-definição dentro de corpos de função, na ordem textual (Seção 4.5), incluindo
  o caso do próprio nome pendente para recursão direta e o caso de cadeias de atribuição
  (`a = b = fn(x){...}`) onde todos os alvos da cadeia tornam-se nomes pendentes antes do valor
  da função existir (Seção 4.5, exemplo do fatorial com `a`/`b`).
* Captura por cópia: toda variável livre referenciada dentro de uma `fn` é congelada no valor do
  momento da definição (early binding); erro `undefined-identifier` se a variável livre não tiver
  vinculação visível na definição.
* `assign-to-captured`: atribuição (incluindo desestruturação) a um nome capturado de fora da
  função é erro de compilação; parâmetros e variáveis locais sombreiam livremente.
* Padrões de desestruturação em atribuições e parâmetros: alvos repetidos avaliados da esquerda
  para a direita, última atribuição prevalece (`[a,a] = [10,11]` ⇒ `a = 11`).

**Testes-chave**: os exemplos de código dos blocos de `xpr-specs.md` Seção 4.5 (`f9`/`f19`/`c`),
Seção 4.6 (`x` em blocos aninhados), e o exemplo de `helper`/`f` com captura congelada (retorna
`11`, não `101`).

### B. Sistema de módulos

* Busca de `import`: stdlib (prioridade máxima, nunca ocultada) → diretório do arquivo
  importador → diretórios `-I` em ordem (Seção 4.1).
* Alias como namespace separado da tabela de variáveis; `alias.membro` nunca é alvo de
  atribuição; dois imports com o mesmo alias no mesmo arquivo é erro.
* Mesmo arquivo (caminho canônico) importado sob aliases diferentes compartilha uma única
  instância.
* Fechamento de módulo: um módulo só vê builtins + suas próprias definições + aliases dos seus
  próprios imports — nunca variáveis do importador. Nomes de módulo não são transitivos.
* Detecção de ciclo de imports → erro de compilação.
* Nomes privados (`_prefixo`): uso sem qualificação dentro do módulo; `alias._nome` de fora é
  erro.
* `_main`: deve existir no módulo de entrada, um parâmetro, resultado `int` vira status de saída
  (Seção 4.1) — ausência/forma errada é `missing-main`.
* `_init`: ordem de inicialização (busca em profundidade pelos imports; `_init` do módulo de
  entrada executa por último, antes de `_main`).
* Esquema de nomes de módulo: cada arquivo importado mapeia para um módulo Rust próprio; a tabela
  de símbolos desta subetapa já registra, por membro, o caminho de módulo Rust que ele vai ocupar
  (`caminho::nome`), mesmo que a emissão de Rust só ocorra na subetapa G. Resta decidir aqui apenas
  a sanitização injetiva do caminho de arquivo para um nome de módulo Rust válido (Seção 4.1).

### C. Inferência e checagem de tipos

* Unificação int↔real (int promovido a real) em: elementos de lista, ramos de `iif`, atribuições
  múltiplas à mesma variável (independente da ordem de execução, Seção 4.7).
* Decisão lista vs. tupla de um `BracketLiteral`: todos os elementos do mesmo tipo (após
  unificação) ⇒ lista; senão ⇒ tupla. `[]` é lista vazia de tipo a resolver por uso posterior
  (ou "só pode conter a lista vazia" se nunca resolvido).
* Tabela de operadores completa da Seção 5.2 (incluindo as linhas `type-mismatch` e
  `size-mismatch` — tupla×tupla tamanho diferente é erro de **compilação**, lista×lista tamanho
  diferente é falha de **execução**).
* Indexação: índice deve ser `int` (`type-mismatch` senão); índice de tupla deve ser constante
  (`type-mismatch` se não for, `index-out-of-range` em tempo de compilação se fora do limite);
  índice de lista é verificado em tempo de execução.
* Assinaturas dos builtins (tabela da Seção 5) como fatos de tipo a priori — necessário já aqui
  para checar chamadas a `print`, `len`, `cat`, `at`, `apply` etc. antes mesmo de gerar código.
* `try(expr)` ⇒ tipo `[bool, T]`; erro de compilação se `T` não tiver valor neutro (função, ou
  tupla contendo função).
* `fail(msg)` assume o tipo exigido pelo contexto.
* Padrões de parâmetro com anotação (`:tipo`) restringindo tipos aceitos; checagem de
  `size-mismatch` (tupla, tempo de compilação) vs. runtime (lista).

### D. Genéricos e especialização

* Definição de tipo *concreto* vs. função *genérica* (Seção 4.10).
* Para cada nome vinculado a `fn(...)`, rastrear todas as definições que alcançam cada uso
  textual do nome (reatribuição condicional em `iif`/`for` cria múltiplas definições alcançáveis).
* Uso como **chamada direta** (`nome(arg)`, uso alcançado por exatamente uma definição): instancia
  com o tipo do argumento; é inline, sem valor de função em runtime.
* Uso **como valor** (passado, retornado, em lista, em ramo de `iif`): instancia com o tipo
  exigido pelo contexto; se o contexto não determina `A`, erro `cannot-infer` (resolvido com
  anotação no parâmetro).
* Quando múltiplas definições alcançam o mesmo uso: todas devem instanciar para o **mesmo** tipo
  concreto `A -> R`, senão `type-mismatch`; a chamada passa a usar um valor de função em runtime.
* Tipo de resultado de função recursiva: determinado pelos ramos que não chamam a própria função;
  se nenhum ramo determina, `cannot-infer`.
* Saída desta subetapa: tabela de instâncias concretas (nome genérico + tipos de argumento →
  caminho/símbolo Rust gerado para aquela instância), consumida por G.

### E. Catálogo de diagnósticos

A spec (Seção 7) pede uma fonte autoritativa única para códigos de diagnóstico, identificadores
públicos, severidades, modelos de mensagem e dicas, compartilhada entre o compilador e o programa
compilado. Na versão original em C isso exigia um arquivo `.def` consumido via `#include` por dois
alvos distintos (compilador em C, runtime em C). Como agora **ambos os lados são Rust**, a
necessidade de "gerar um lado a partir do outro" desaparece: o catálogo pode ser um único módulo
Rust usado diretamente pelos dois.

* `src/diag/catalog.rs`: uma lista `const` de `DiagSpec { code, public_id, severity,
  message_template, hint }`, uma entrada por diagnóstico — a única fonte, sem duplicação.
* O compilador (`xprc`) consome essa lista diretamente como dado Rust (`enum DiagCode` + tabela
  de busca derivada do mesmo `const`).
* O runtime (crate `xpr_rt`, subetapa F) depende do mesmo módulo/crate de catálogo e o usa
  diretamente para formatar o diagnóstico de uma falha não capturada — sem gerar nem duplicar
  nada: é literalmente o mesmo código Rust compilado nos dois binários (o `xprc` e o executável
  gerado). Isso satisfaz "mesmos IDs/mensagens/dicas" por construção, de forma mais direta que a
  alternativa original de dois arquivos de linguagens diferentes lendo o mesmo `.def`.
* Cobrir, no mínimo, todos os nomes de erro citados ao longo da especificação: `syntax-error`,
  `undefined-identifier`, `type-mismatch`, `size-mismatch` (compilação e execução), `cannot-infer`,
  `assign-to-captured`, `assign-to-constant`, `missing-main`, `malformed-number`,
  `div-by-zero`, `overflow`, `index-out-of-range`, `invalid-number`, `invalid-size`,
  `end-of-input`, erro de ciclo de import, erro de alias duplicado, erro de limite de profundidade
  de chamada (10.000).
* O documento `diretrizes-diagnosticos.md`, referenciado pela spec original, descrevia
  propagação/limpeza de diagnósticos **dentro do compilador em C** (cadeias de `goto` de limpeza,
  códigos de erro subindo manualmente). Em Rust isso é resolvido pelo próprio idioma
  (`Result<T, SemaError>` + `?` + `Drop`, já refletido nas assinaturas deste plano) — não
  recriamos esse documento. O que ainda precisa de registro (como cada erro de cada etapa mapeia
  para um `DiagCode` do catálogo) fica anotado no `RELATORIO_PROGRESSO.md` desta subetapa, não em
  um arquivo novo.

### F. Biblioteca de runtime em Rust (crate `xpr_rt`)

Crate separada (workspace member), da qual todo programa compilado por `xprc` depende — análogo
ao que seria uma libruntime estática vinculada ao executável, só que agora é uma dependência
Cargo normal em vez de um `.a`/`.o` em C.

* Representações de valor: `Int = i64`, `Real = f64`. Para `String`/`List` (imutáveis, com
  armazenamento potencialmente compartilhado — Seção 4.8), usar `Rc<str>`/`Rc<[T]>` ou
  equivalente: o requisito da spec é "o compilador usa contagem de referências", e `Rc` é
  exatamente isso em Rust, sem código de refcounting manual. Para `Tuple`, avaliar se o Rust
  nativo (`(T1, T2, ...)`) serve diretamente — a spec já fixa o tamanho e o tipo de cada posição
  em tempo de compilação, que é exatamente o que uma tupla Rust nativa exige; evita ter que gerar
  um `struct` por formato de tupla. **Decisão de arquitetura a registrar no relatório da etapa.**
* Builtins (tabela da Seção 5) implementados elemento a elemento / posição a posição conforme
  especificado: `print`, `println`, `len`, `input`, `fail`, `string`, `format`, `int`, `real`,
  `floor`, `ceil`, `round`, `frac`, `any`, `all`, `cat`, `at` (as 4 variantes), `fill`, `range`
  (3 formas), `set`, `slice`, `apply`.
* Formatação numérica (Seção 5.1): modo fixo vs. científico pela regra de limiar, arredondamento
  "meio para o par mais próximo" (round-half-to-even).
* Mecanismo de `try`/`fail`: **cada função gerada retorna `Result<Value, Fail>`** (onde `Fail`
  carrega o `DiagCode` do catálogo — subetapa E — mais a posição no `.x` e detalhes). Falhas se
  propagam pelo próprio operador `?` do Rust, função a função, até o ponto onde `try(expr)` foi
  lowered — não há necessidade de `setjmp`/`longjmp` nem de nenhum mecanismo de controle de fluxo
  manual, o que **elimina** a decisão de arquitetura que a versão em C exigia aqui. `try(expr)`
  lowera para avaliar `expr` (ou a chamada que a representa) e casar o `Result`: `Ok(v) ⇒
  (true, v)`, `Err(_) ⇒ (false, neutro_de_T)`. Quando a falha não é capturada por nenhum `try` até
  `_main`, o runtime relata o diagnóstico no stderr (ID, mensagem, dica do catálogo), descarrega
  os buffers e sai com status ≠ 0 — nunca um `panic!`/abort sem relatar.
* Limite de profundidade de chamada (10.000): um contador (passado por referência ou em uma
  estrutura de contexto de execução) incrementado em cada chamada gerada e checado **antes** de
  recursar, retornando `Err` como qualquer outra falha — evita depender do limite de pilha do SO.
* `main()` do executável gerado: constrói a lista `[string]` de `argv` (sem o nome do programa),
  roda `_init` na ordem da Seção 4.1, chama `_main`, usa o `int` retornado como status de saída
  (ou 0).
* Entregável de documentação: `diretrizes-runtime.md` (referenciado por `xpr-specs.md`),
  descrevendo convenções do código Rust gerado/runtime: representação de valores, uso de `Rc`,
  esquema de nomes de módulo/função, convenções de builtins.

### G. Emissão de Rust e driver de build

* *Lowering* de `TypedModule` (A-D) para Rust: expressões, blocos, `iif`, `for`. Para closures,
  avaliar se o fechamento nativo do Rust (`move |arg| -> Result<Value, Fail> { ... }`, com captura
  por valor) já basta para a semântica "capturado por cópia, congelado na definição" (Seção 4.5),
  em vez de gerar um `struct` de captura manualmente, como seria necessário em C — **decisão de
  arquitetura a registrar no relatório da etapa**. Chamadas diretas (subetapa D) emitem uma
  chamada de função Rust comum; chamadas por valor emitem uma chamada através do valor de closure.
* Cada módulo XPR emitido como seu próprio módulo Rust (`mod`), com os membros em seu caminho —
  sem esquema de mangling manual (ver nota de arquitetura e subetapa B).
* Posições do `.x` original embutidas como literais (inteiros de linha/coluna) nos pontos de
  chamada ao runtime de diagnóstico, para que falhas em tempo de execução citem o arquivo fonte
  `.x` e não o Rust gerado.
* Driver `xprc`: parsing de CLI (Seção 8) — `<arquivo.x>`, `-o`, `--emit-rust`, `--lex-dump`,
  `--parse-dump`, `--type-dump`, `-I` (repetível), `--rustc` (default `rustc`), `-O` (default `2`,
  traduzido para `-C opt-level`), `-v`, `-h`/`--help`, `--version`. Nenhum modo interativo; nenhuma
  leitura de stdin para código.
* Invocação do `rustc` (ou `cargo build --release`, se o agente executor preferir gerar um crate
  completo em vez de um único arquivo — **decisão de arquitetura a registrar no relatório da
  etapa**, considerando que o executável precisa vincular a crate `xpr_rt` da subetapa F), com
  status de saída propagado; `-v` relata caminho resolvido de cada import e o comando do
  compilador Rust executado.

## Critério de conclusão da etapa

Cada subetapa (A–G) com sua própria suíte verde, **e** todos os milestones já registrados em
`plano-testes-codegen.md` (`tests/codegen/milestones/`) passando via
`scripts/run-codegen-milestones.sh`. Diferente do Lexer/Parser, essa suíte não é fixa — ela
cresce a cada subetapa que introduz comportamento observável novo (ver a "Regra de evolução" no
próprio `plano-testes-codegen.md`). Uma subetapa só está concluída quando os milestones que ela
introduziu **e** todos os anteriores continuam verdes: não há regressão aceitável.
