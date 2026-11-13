# Relatório final — compilador XPR (`xprc`)

Documento de encerramento conforme `AGENTS.md` [3]. Consolida a arquitetura da linguagem, a
estrutura do compilador, as decisões de implementação, as lições aprendidas ao longo das três
etapas e o status final do projeto.

*Toolchain de referência*: `rustc`/`cargo` 1.75.0, `edition = "2021"`.
*Data de conclusão*: etapa Lexer → Parser → Codegen, todas com gate verde.

---

## 1. A linguagem XPR em uma página

XPR é uma linguagem **orientada a expressões**: não há instruções, apenas expressões que produzem
valores. Blocos `{ a; b; }`, `iif(c, t, e)` e `for(init; cond; step) corpo` são expressões; o valor
de um bloco é o da última expressão, `iif` avalia só a ramificação escolhida e `for` retorna o
número de iterações concluídas.

| Aspecto | Regra |
| --- | --- |
| Tipos | `int` (i64), `real` (f64), `string` (UTF-8), `bool`, `[T]` (lista), `[T1,…,Tn]` (tupla), `A -> R` (função) |
| Aritmética mista | int com real promove a real; unificar int com real em atribuição/listas/`iif` dá real |
| Funções | sempre **um** argumento e um resultado (`f(a, b)` é erro; use `f([a, b])`) |
| Closures | variável livre é **capturada por cópia e congelada** na definição; atribuir a capturada é erro |
| Recursão | `fat = fn(x){ … fat(x-1) … }` funciona: o nome é "pendente" durante a criação da função |
| Módulos | cada arquivo é um módulo; `import("nome"[, "alias"])` só nas primeiras linhas; nada de carregamento em runtime |
| Falhas | `fail(msg)` e erros de execução são capturáveis por `try(e)` → `[bool, T]` |
| Valores | imutáveis e copiados logicamente; listas/strings compartilham armazenamento (refcount) |
| Sintaxe | `;` obrigatório ao final de cada statement; parênteses só agrupam |

## 2. Arquitetura do compilador

`xprc` é um transpilador: traduz XPR para **Rust** e compila com `rustc`, vinculando a crate de
runtime `xpr_rt`. Não há bytecode nem VM.

```
arquivo.x ──► Lexer ──► Parser ──► TypeChecker ──► Emitter ──► rustc ──► executável
                │         │            │              │           │
             Vec<Token>  Module     tipos por nó   main.rs +   liga xpr_rt
                │         (AST)      (Ty)          mods por        (.rlib)
                └─ Span em codepoints ─┘           módulo XPR
```

### 2.1 Lexer (`src/lexer/`)

* Itera **codepoints** (`Vec<char>` + contador de linha/coluna), nunca bytes: a Seção 7 da spec
  exige colunas em codepoints e uma `StringLit` pode cruzar linhas.
* `Span { line_start, col_start, line_end, col_end }`, 1-based, `col_end` exclusivo.
* Comentários `//` e `/* */` (sem aninhamento); strings apenas **delimitadas**, conteúdo bruto
  (a decodificação de escapes é do parser); `int_lit` fora de i64 → `MalformedNumber`.
* `.` é ambíguo: `Dot` ou início de `RealLit` (`.5`), resolvido com 1 codepoint de lookahead.
* Operadores de 2 caracteres (`==`, `!=`, `<=`, `>=`) e todos os de 1; `& | ^ ~ @ # $ ! ?` →
  `InvalidCharacter`.
* `Token::adjacent_to_next` marca `+`/`-` colados ao token seguinte (comparação de offsets de
  codepoint) — o parser decide o que fazer com isso.

### 2.2 Parser (`src/parser/`)

Descendente recursivo sobre a cadeia completa de precedência da gramática:
`assignment → logical_or → logical_and → logical_not → equality → relational → arithmetic → term →
factor → indexed → primary`.

* `Paren` **não** existe na AST: `((1))` é `IntLit 1`.
* `v[a, b, c]` e `v[a][b][c]` produzem o mesmo `Indexed { base, indices }` — o açúcar é resolvido
  na emissão.
* `BracketLiteral` é lista **ou** tupla: a distinção depende de tipos, não da sintaxe.
* Restrições puramente sintáticas aplicadas aqui: `;` obrigatório, `import` só antes da primeira
  definição, alvo de atribuição precisa ser `binding_pattern`, `f(a, b)` → `MultiArgCall`,
  `const_expr` de topo sem chamada/bloco/`iif`/`for`/atribuição, `fn` unário separado do número →
  `SignNotAdjacent`, programa vazio → `EmptyProgram`.
* `SyntaxErrorKind` é enum fechado (8 variantes); os testes comparam `kind`+`span`, nunca a
  mensagem (que é texto livre para humanos em `detail`).

### 2.3 Checagem de tipos (`src/codegen/types.rs`)

Verificação ética **antes** de qualquer emissão (Seção 1). `Ty` = `Int | Real | Str | Bool |
List(Ty) | Tuple(Vec<Ty>) | Fn(Ty, Ty) | Any`. A unificação aplica int→real; lista e tupla não
unificam; tuplas de tamanhos diferentes geram `size-mismatch` **em tempo de compilação** (listas
só em execução, pois o comprimento é dado do valor). Cobre a tabela de operadores da Seção 5.2,
condições `bool` de `iif`/`for`, tipo único por variável, índice `int`, chamada de não-função, e
`render-function` para funções em `print`/`println`/`string`/`format`.

### 2.4 Emissão (`src/codegen/mod.rs`)

Cada expressão vira um bloco Rust cujo valor é `XprResult = Result<Value, Fail>`; `?` propaga
falhas. Detalhes que custaram iteração e viraram decisão:

* **Atribuições são emitidas inline** em posição de statement. Se fossem embrulhadas em `{ … }`, a
  variável criada morreria ao fim da sub-expressão, violando a Seção 4.6 (`x = 1; { x = 2; }; x`
  deve ser 2).
* **Captura por cópia explícita**: antes de cada closure `move`, o emissor gera
  `let __cap_x = x.clone();` para cada variável livre visível. Isso dá, sem interpretador de
  escopos, exatamente a semântica de "congelado no momento da definição" (`g(5)` = 120 e `fat(5)` =
  999 depois da redefinição).
* **Recursão direta** via `xpr_rt::recursive_closure`: uma célula é preenchida com a própria
  closure logo após a criação; o nome pendente (`fat`, ou os alvos de `a = b = fn …`) resolve para
  `__self` dentro do corpo.
* **Módulos**: um `mod xpr_mod_<nome>` por arquivo XPR, com uma `pub fn` por definição;
  `alias.membro` vira `xpr_mod_<alias>::membro()`. Sem *mangling* manual.
* **`try`** avalia o corpo em uma closure (`(|| -> XprResult { … })()`) para conter a propagação
  dos `?` internos, e devolve `[ok, neutro]`; o neutro é escolhido pela forma sintática da
  expressão.
* **`v[a, b, c]`** vira `let`s encadeados de `ops::index`, cada um com `?`.

### 2.5 Runtime (`xpr_rt/`)

Crate separada (membro do workspace), compilada uma vez e vinculada a todo programa gerado.

* `Value`: `Int(i64)`, `Real(f64)`, `Str(Rc<str>)`, `Bool`, `Seq(Rc<[Value]>)`,
  `Fn(Rc<dyn Fn(Value) -> XprResult>)`.
* `Fail { diag: Diagnostic, position, file }` + `Result` + `?` = `try`/`fail` (sem
  `setjmp`/`longjmp`).
* `ops`: implementação direta da tabela da Seção 5.2, com `checked_*` para `overflow` e
  `div-by-zero` em `/` e `%`.
* `bi`: os 22 builtins da Seção 5 (`print`, `println`, `len`, `input`, `fail`, `string`, `format`,
  `int`, `real`, `floor`, `ceil`, `round`, `frac`, `any`, `all`, `cat`, `at`, `fill`, `range`,
  `set`, `slice`, `apply`), selecionados por tipo na emissão e disponíveis como valores.
* Formatação numérica da Seção 5.1: `decimais = 2`, `alg_significativos = 15`, notação científica
  fora da faixa, `NaN`/`INF`/`-INF` literais.
* `run_main_with`: executa o programa em uma thread com pilha de 512 MB, para que o limite de
  10.000 chamadas da linguagem seja alcançado e vire `call-depth-exceeded` capturável — com a
  pilha padrão o processo estourava com SIGABRT antes disso.

### 2.6 Catálogo de diagnósticos (`src/diag/catalog.rs`)

Uma única `const CATALOG: &[DiagSpec]` com `code`, `public_id`, `severity`, `message_template` e
`hint`, cobrindo todos os erros citados na spec. O compilador a inclui por `mod`; a crate de
runtime a inclui pelo **mesmo caminho de arquivo** (`#[path = "../../src/diag/catalog.rs"]`).
Resultado: os dois binários relatam IDs, severidades, mensagens e dicas idênticos **por
construção** — o requisito da Seção 7 é satisfeito sem geração nem duplicação.

### 2.7 CLI (`src/main.rs`)

`xprc [opções] <arquivo.x>`, exatamente as formas da Seção 8:

| Invocação | Comportamento |
| --- | --- |
| `xprc prog.x` | compila; executável `prog` no diretório atual |
| `xprc prog.x -o nome` | idem, nomeando a saída |
| `xprc --emit-rust prog.x` | escreve só o Rust (em `-o` ou stdout), sem invocar `rustc` |
| `xprc --lex-dump prog.x` | dump de tokens, **idêntico** ao formato dos fixtures |
| `xprc --parse-dump prog.x` | dump da AST, **idêntico** ao formato dos fixtures |
| `xprc --type-dump prog.x` | lista as definições e seus tipos |
| `xprc -h` / `--help`, `--version` | ajuda (versão primeiro) e versão |

Opções: `-o`, `-I <dir>` (repetível, buscado na ordem), `--rustc <comando>`, `-O 0..3`, `-v`.
Sem arquivo e sem modo: mostra a ajuda e falha (nunca lê stdin).

---

## 3. Estratégia de testes

Três camadas, todas obrigatórias para o gate:

| Camada | Onde | O que garante |
| --- | --- | --- |
| Unitários por subetapa | `tests/lexer_tests.rs` (52), `tests/parser_tests.rs` (80), `src/codegen/{types,diag}` (20) | Cada regra da spec isoladamente, na ordem de TDD das subetapas dos planos |
| Golden-file | `tests/lexer/cases/` (12), `tests/parser/cases/` (16) | Formato e valores exatos de tokens/AST, com `[OK]`/`[FAIL]` por fixture |
| Ponta a ponta | `tests/codegen/milestones/` (14) + `tests/codegen_tests.rs` (38) | Comportamento observável: compilar, executar, comparar stdout/exit/stderr e a superfície da CLI |

**Números finais**, `cargo test`: **190 testes** (20 unitários do binário + 52 lexer + 1 runner de
fixtures + 80 parser + 1 runner de fixtures + 38 codegen) e **28 fixtures** (12 lexer + 16 parser).
`./scripts/run-codegen-milestones.sh`: **14/14 OK**. `./scripts/run-all-tests.sh`: **TUDO OK**.

Os scripts dão o veredito (`OK`/`NOK`, exit status) e nunca mascaram falha: qualquer caso vermelho
derruba o gate da etapa inteira.

---

## 4. Lições aprendidas

### 4.1 Sobre o processo (TDD e golden files)

* **Escrever o runner de fixtures antes da lógica foi decisivo.** Ele virou o contrato de formato
  do dump, e nas três etapas os fixtures passaram quase de primeira — os erros apareceram nos
  testes *unitários* que eu mesmo escrevi, não nos fixtures verificados mecanicamente antes.
* **Os bugs mais valiosos vieram dos milestones, não dos testes unitários.** `?` vazando do `try`,
  atribuição em escopo errado, `Indexed` multinível gerando `?` sobrando, profundidade de chamada
  virando SIGABRT: nenhum seria pego por um teste de unidade que olhasse só o texto gerado. Testar
  **compilando e executando de verdade** é o que revelou cada um.
* **Regra 3.3.1 do `AGENTS.md` salvou o projeto várias vezes.** Quando um teste meu falhava, a
  pergunta certa era sempre "quem está errado: o compilador ou minha expectativa?". Em todos os
  casos desta sessão a resposta foi "minha expectativa" (coluna calculada à mão, `fn()` inexistente
  na gramática, `f(1)(2)` não gramatical, `a/**/b` na coluna 6 e não 7). Corrigir o teste com a
  derivação explícita no comentário evitou "consertar" o compilador para caber num erro meu.

### 4.2 Sobre a linguagem e a implementação

* **Escopo é o ponto mais delicado de uma linguagem orientada a expressões.** A regra "atribuição
  a nome novo cria variável no escopo corrente" (Seção 4.6) não sobrevive a um emissor que
  embrulha cada expressão em um bloco Rust. Foi preciso distinguir *expressão* de *statement* na
  emissão.
* **Captura por cópia é mais fácil de acertar do que parece** — desde que se copie explicitamente
  antes do `move`. O `move` sozinho captura a variável, não o valor no instante da definição.
* **`Result` + `?` dá `try`/`fail` quase de graça**, mas só se a fronteira do `try` for uma função:
  dentro de um `match` no mesmo corpo, o `?` escapa.
* **O runtime precisa conhecer o limite de recursão do sistema.** Um limite *lógico* de 10.000
  chamadas não significa nada se a pilha do processo suporta menos: a falha vira crash do SO, não
  `try`.
* **Compartilhar o catálogo por `#[path]` é mais simples e mais seguro que qualquer geração de
  código** — e resolve a exigência da spec de "fonte única" sem build script.

### 4.3 Sobre o que ficou de fora

A etapa Codegen é grande (semântica + módulos + tipos + genéricos + catálogo + runtime + emissão +
CLI) e foi entregue com o pipeline fechado e 14 milestones verdes, mas com lacunas conscientes e
documentadas: `cannot-infer` permanece para os casos de genéricos (uso como valor sem contexto,
`apply` com `[]` — dependem da especialização), genéricos não geram instâncias especializadas e
nomes privados não existem. Foram **fechadas depois** (ver "Pós-Codegen" em
`RELATORIO_PROGRESSO.md`): `assign-to-constant` (Seção 6, incluindo os aliases `PI`/`e`),
`cannot-infer` de recursivas sem ramificação determinante (Seção 4.10), o índice de tupla
constante (Seção 4.8, incluindo o tipo da posição indexada), o neutro do `try` por tipo
(Seção 4.9, via `try_types` span→Ty injetado no emissor) e `_init`/ordem de inicialização entre
módulos (Seção 4.1, milestone M14). A lacuna `assign-to-constant` foi **fechada depois** (Seção 6:
constantes predefinidas — agora incluindo os aliases `PI`/`e` — e nomes de nível superior são
somente leitura dentro de funções; ver "Pós-Codegen" em `RELATORIO_PROGRESSO.md`). Nenhuma delas foi mascarada: cada uma
está listada aqui e em `ESTADO_DO_PROJETO.md`, com a indicação de como fechá-la (teste primeiro).

---

## 5. Status final por etapa

| Etapa | Estado | Evidência |
| --- | --- | --- |
| Planejamento | Concluída | `plano-lexer.md`, `plano-parser.md`, `plano-codegen.md`, `plano-testes-codegen.md`, ambiente Cargo + Git |
| Lexer | Concluída | 52 unitários, 12/12 fixtures, `scripts/run-lexer-tests.sh` = 0 |
| Parser | Concluída | 80 unitários (A–J), 16/16 fixtures, `scripts/run-parser-tests.sh` = 0 |
| Codegen | Concluída | 40 unitários + 50 de integração, 15/15 milestones, `scripts/run-all-tests.sh` → `TUDO OK` (números incluem os fechamentos pós-etapa: `assign-to-constant`, `cannot-infer` de recursivas, índice de tupla constante, neutro do `try` por tipo, `_init`/ordem de inicialização) |

O compilador está funcional de ponta a ponta: `xprc testes/codegen/milestones/M4_closures/main.x -o
prog && ./prog` imprime `120` e `999`, como a Seção 4.5 da especificação exige.

### Histórico de commits da implementação

```
Conclui etapa Lexer: testes TDD por subetapa, fixtures golden-file e relatório de progresso
Conclui etapa Parser: AST, dump normativo, parser recursivo e 96 testes
Atualiza ESTADO_DO_PROJETO: Lexer e Parser concluídos, próxima ação é o Codegen
Conclui etapa Codegen: catálogo de diagnósticos, runtime xpr_rt, checagem de tipos e CLI
```
