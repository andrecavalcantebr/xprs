# Plano — Etapa Codegen

## Nota de arquitetura (leia antes de começar)

O nome "Codegen" em `AGENTS.md` é tratado aqui, deliberadamente, como **tudo que vem depois do
parser**: análise semântica, sistema de módulos, inferência/checagem de tipos, especialização de
genéricos, catálogo de diagnósticos, biblioteca de runtime em C e emissão de C + driver de build.

Isso não é um desvio da especificação — é a leitura correta dela. `xpr-specs.md` diz
explicitamente: "Tipos estáticos: Todo o programa é analisado sintaticamente e verificado em
termos de tipo **antes que qualquer código seja gerado**" (Seção 1). Como `AGENTS.md` nomeia
apenas três etapas (Lexer, Parser, Codegen) e não há produção de bytecode/VM nesta linguagem —
`xprc` transpila para C e invoca um compilador C (Seção 8) — toda a análise estática e a geração
de C cabem na etapa "Codegen". Dado o tamanho, ela é dividida nas sete subetapas abaixo, cada uma
tratável como seu próprio ciclo TDD completo (Fase 2 de `AGENTS.md`), na ordem listada — cada
subetapa depende de dados produzidos pela anterior.

Subetapas:

* **A — Resolução de nomes e escopos**
* **B — Sistema de módulos** (import, aliases, nomes privados, busca de caminho, ciclos)
* **C — Inferência e checagem de tipos**
* **D — Genéricos e especialização**
* **E — Catálogo de diagnósticos** (`src/diag/catalog.rs`, fonte única em Rust)
* **F — Biblioteca de runtime em C** (`xprt.h`/`.c`, gerado/acompanhante do executável)
* **G — Emissão de C e driver de build** (CLI `xprc`)

As subetapas **E** e **F** podem começar em paralelo com **A**-**D** (não dependem de tipos
resolvidos), mas **G** depende de A, B, C, D e E estarem concluídas, e o runtime (F) precisa estar
pronto antes que o C emitido por G possa ser compilado e testado de ponta a ponta.

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
* Esquema de *mangling* de nomes: `xm_<len>_<path>__<name>` (Seção 4.1) — útil já aqui para a
  tabela de símbolos, mesmo que a emissão de C só ocorra na subetapa G.

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
  para checar chamadas a `print`, `len`, `cat`, `at`, `apply` etc. antes mesmo de gerar C.
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
  símbolo C mangled), consumida por G.

### E. Catálogo de diagnósticos

A spec (Seção 7) descreve um catálogo único (`xpr_diag.def`) consumido via `#include` tanto pelo
compilador quanto pelo runtime gerado — um idioma de macro X típico de um compilador **escrito em
C**, que era o caso original do projeto. Como `xprc` agora é Rust, esse mecanismo de consumo não
se aplica (o compilador não `#include`ia um `.def`), mas o **requisito** por trás dele continua
valendo integralmente: compilador e programa compilado devem reportar exatamente os mesmos
IDs, mensagens e dicas, a partir de uma única fonte.

Decisão de arquitetura: a fonte canônica passa a ser um módulo Rust, não um arquivo `.def` em C.

* `src/diag/catalog.rs`: uma lista `const` (ou `macro_rules!` que a gera) de `DiagSpec { code,
  public_id, severity, message_template, hint }`, uma entrada por diagnóstico. É a única fonte —
  nada mais duplica código/mensagem/dica manualmente.
* O compilador consome essa lista diretamente como dado Rust (`enum DiagCode` + tabela de busca
  gerada a partir do mesmo `const`, sem macro de pré-processador).
* Na subetapa G, a emissão de C **gera programaticamente**, a partir do mesmo `catalog.rs`, o
  cabeçalho C equivalente (ex.: `xprt_diag.h`, com os `#define` de código e a tabela de
  mensagens/dicas) que o runtime (`xprt.c`, subetapa F) inclui. Gerar o C a partir do Rust —
  em vez de manter os dois sincronizados à mão — é o que garante o requisito "mesmos
  IDs/mensagens/dicas" por construção, e é estritamente mais seguro que a alternativa original de
  dois alvos (`src/xpr.h` em C e `xprt.h`) lendo o mesmo `.def`.
* Cobrir, no mínimo, todos os nomes de erro citados ao longo da especificação: `syntax-error`,
  `undefined-identifier`, `type-mismatch`, `size-mismatch` (compilação e execução), `cannot-infer`,
  `assign-to-captured`, `assign-to-constant`, `missing-main`, `malformed-number`,
  `div-by-zero`, `overflow`, `index-out-of-range`, `invalid-number`, `invalid-size`,
  `end-of-input`, erro de ciclo de import, erro de alias duplicado, erro de limite de profundidade
  de chamada (10.000).
* O documento `diretrizes-diagnosticos.md`, referenciado pela spec, descrevia propagação/limpeza
  de diagnósticos **dentro do compilador em C** (cadeias de `goto` de limpeza, códigos de erro
  subindo manualmente). Em Rust isso é resolvido pelo próprio idioma (`Result<T, SemaError>` + `?`
  + `Drop`, já refletido nas assinaturas de `plano-parser.md`/`plano-codegen.md`) — não recriamos
  esse documento. O que ainda precisa de registro (como cada erro de cada etapa mapeia para um
  `DiagCode` do catálogo) fica anotado no `RELATORIO_PROGRESSO.md` desta subetapa, não em um
  arquivo novo.

### F. Biblioteca de runtime em C (`xprt.h`/`.c`)

* Representações de valor: `int` (int64_t), `real` (double), `string`/`list` com contagem de
  referências (Seção 4.8 — "o compilador usa contagem de referências"), tupla como struct fixo.
* Builtins (tabela da Seção 5) implementados elemento a elemento / posição a posição conforme
  especificado: `print`, `println`, `len`, `input`, `fail`, `string`, `format`, `int`, `real`,
  `floor`, `ceil`, `round`, `frac`, `any`, `all`, `cat`, `at` (as 4 variantes), `fill`, `range`
  (3 formas), `set`, `slice`, `apply`.
* Formatação numérica (Seção 5.1): modo fixo vs. científico pela regra de limiar, arredondamento
  "meio para o par mais próximo" como `printf` de C.
* Mecanismo de `try`/`fail`: um único ponto de falha no runtime (registra diagnóstico; se não há
  `try` ativo, relata no stderr + flush + `exit` ≠ 0, nunca `abort()`; se há `try` ativo, cada
  chamada envolvente retorna até alcançá-lo). Avaliar `setjmp`/`longjmp` vs. threading explícito de
  código de erro pelo retorno de cada função C gerada — **decisão de arquitetura a registrar no
  relatório da etapa**, com os trade-offs (overflow de pilha de `try` vs. custo por chamada).
* Limite de profundidade de chamada (10.000) e sua interação com `try`.
* `main()` em C: constrói a lista `[string]` de `argv` (sem o nome do programa), roda `_init` na
  ordem da Seção 4.1, chama `_main`, usa o `int` retornado como status de saída (ou 0).
* Entregável de documentação: `diretrizes-runtime-c.md` — sucessor do `diretrizes-compilador-c.md`
  original (que descrevia convenções do compilador-em-C; agora o compilador é Rust, então o
  documento passa a descrever apenas o **alvo**: convenções do C gerado/runtime — representação
  de valores, refcounting, esquema de nomes de função/membro de módulo, builtins).

### G. Emissão de C e driver de build

* *Lowering* de `TypedModule` (A-D) para C: expressões, blocos, `iif`, `for`, closures (struct com
  campos = capturas congeladas), chamadas diretas vs. via valor de função.
* Aplicação do esquema de mangling (`xm_<len>_<path>__<name>`) garantido por construção.
* `#line` apontando para o `.x` original (para que falhas em runtime citem o arquivo fonte).
* Driver `xprc`: parsing de CLI (Seção 8) — `<arquivo.x>`, `-o`, `--emit-c`, `--lex-dump`,
  `--parse-dump`, `--type-dump`, `-I` (repetível), `--cc` (default `cc`), `-O` (default `2`),
  `-v`, `-h`/`--help`, `--version`. Nenhum modo interativo; nenhuma leitura de stdin para código.
* Invocação do compilador C configurado, com status de saída propagado; `-v` relata caminho
  resolvido de cada import e o comando do compilador C executado.

## Critério de conclusão da etapa

Cada subetapa (A–G) com sua própria suíte verde. Critério final de ponta a ponta: um conjunto de
programas `.x` de exemplo (extraídos literalmente dos blocos de código da especificação — fatorial
recursivo, `try`/`fail` com `age`, closures `helper`/`f`, módulos com `import`) compilados por
`xprc`, executados, e produzindo exatamente os valores que a especificação afirma.
