# MEMORY — handoff do projeto XPR

Leia este arquivo antes de trabalhar. Ele contém estado atual, decisões e lições que evitam
regressões. Atualize as seções existentes ao concluir trabalho; o histórico fica no Git.
`AGENTS.md` define o processo e `xpr-specs.md` é a especificação canônica da linguagem.

## Retomada para outra sessão

* Objetivo desta sessão concluído: consolidar a documentação em `AGENTS.md`, `MEMORY.md`
  e `xpr-specs.md`. Planos executados e relatórios foram removidos depois de extrair suas
  decisões, lições e contratos de fixtures. O histórico anterior permanece no Git.
* Pedido explícito mais recente: usar ai-memory para deixar este arquivo como handoff.
  O handoff do serviço deve apontar para esta memória versionada; valide o checkout antes
  de confiar em um snapshot antigo do serviço.
* Restrição do usuário: **não alterar nem renomear `xpr-specs.md`** nesta consolidação.
  A skill em `skills/` foi preservada por ser ferramenta separada da documentação do projeto.
* As alterações em Rust e scripts nesta etapa são somente comentários e referências a docs;
  a semântica e os testes existentes foram preservados. `git diff --check` sem erros e gate
  completo após a consolidação: `TUDO OK`, 16/16 milestones.
* A correção funcional anterior está no commit `5c9c247`: cache de genéricos preserva o
  resultado de chamadas repetidas e precisão padrão de cinco significativos foi aprovada
  pelo usuário. Não reabrir esses pontos como bugs ainda pendentes.
* Próxima sessão: ler `AGENTS.md` e esta memória, conferir `git status --short` e `git log -3
  --oneline`. A consolidação documental não autoriza começar automaticamente uma nova
  implementação; os candidatos de evolução estão em “Pendências e próxima ação”.

## Estado atual e validação

Lexer, parser e codegen concluídos: XPR → tokens → AST → tipos → Rust → `rustc` → executável.
Rust 1.100.0 fixado em `rust-toolchain.toml`; crates em edition 2024. O Rust emitido usa
edition 2021 deliberadamente, pois `--rustc` pode selecionar outro toolchain.

Última validação funcional: `./scripts/run-all-tests.sh` → **TUDO OK** (exit 0).
265 testes Cargo: 56 biblioteca, 22 runtime, 53 integração, 52 lexer, 80 parser e 2 runners
que verificam 28 fixtures (12 lexer e 16 parser). Mais 16 milestones de ponta a ponta.
Esses números são referência da última validação, não um substituto para executar o gate.

## Mapa do código e comandos

* `src/lexer/`: tokens, spans e lexer; `src/parser/`: AST, parser e dump.
* `src/codegen/types.rs`: inferência, checagem e instanciação; `src/codegen/mod.rs`: emissão.
* `src/main.rs`: CLI, carregamento de módulos, imports e invocação de `rustc`.
* `src/diag/catalog.rs`: fonte única de diagnósticos; `src/diag/mod.rs`: integração.
* `xpr_rt/src/`: `lib.rs` (valores, falhas, formatação e execução), `ops.rs` e `bi.rs`.
* `tests/*_tests.rs` e `tests/*_fixtures.rs`: testes; `tests/codegen/milestones/`: programas e saídas.
* `cargo test --workspace`: testes de todas as crates; `./scripts/run-all-tests.sh`: gate completo.
* Gates específicos: `scripts/run-lexer-tests.sh`, `scripts/run-parser-tests.sh`,
  `scripts/run-codegen-milestones.sh`.
* Exemplo: `cargo run -- tests/codegen/milestones/M4_closures/main.x -o /tmp/xpr-exemplo`;
  `/tmp/xpr-exemplo` imprime `120` e `999`.

## Decisões de arquitetura

* `xprc` traduz XPR para **Rust** e compila com `rustc`, com os fontes do runtime embutidos no Rust emitido. Nunca C,
  nunca bytecode/VM.
* **Catálogo único**: `src/diag/catalog.rs` é compilado pelo compilador **e** pela crate de
  runtime via `#[path = "../../src/diag/catalog.rs"]`. Nada é gerado de um lado para o outro.
* **Runtime**: `try`/`fail` via `Result<Value, Fail>` + `?` (nunca `setjmp`/`longjmp`); valores com
  `Rc` (contagem de referências, Seção 4.8); lista e tupla compartilham `Seq(Rc<[Value]>)` porque a
  distinção é estática.
* **Escopos**: atribuição em posição de statement é emitida **inline** (nunca dentro de um bloco
  `{ }` próprio), senão a variável morreria no fim da expressão (Seção 4.6).
* **Captura por cópia**: antes de cada closure `move`, o emissor gera `let __cap_x = x.clone();`
  para cada variável livre — é isso que dá a semântica "congelada na definição" da Seção 4.5.
* **Recursão direta** usa `xpr_rt::recursive_closure` (auto-referência via célula preenchida logo
  após a criação).
* **`v[a, b, c]`** é emitido como `let`s encadeados de `ops::index`, cada um com `?`.
* **Corpo do `try` roda em uma closure** `(|| -> XprResult { … })()` para conter a propagação dos
  `?`.
* **Programa em thread de pilha grande** (512 MB) no ponto de entrada, senão o limite de 10.000
  chamadas vira SIGABRT do sistema em vez de `call-depth-exceeded` capturável.
* **`Span`** do lexer tem `line_start`/`line_end`/`col_start`/`col_end`, sempre em codepoints;
  token multilinha só pode ser `StringLit`.
* AST do parser **não tem** nó `Paren`; `v[a, b]` e `v[a][b]` produzem o mesmo `Indexed`.
* `SyntaxErrorKind` (parser) e `LexErrorKind` (lexer) são enums fechados; os testes comparam
  `kind`+`span`, nunca a mensagem.
* Escapes de string são decodificados **no parser**; escape desconhecido é preservado literalmente.
* `param_pattern` é obrigatório (`fn()` não existe); `fn_call` só chama `qualified_name`;
  `MultiArgCall` aponta para a vírgula; `InvalidAssignmentTarget` aponta para o `=`.
* `edition = "2024"` (toolchain fixada em 1.100.0 por `rust-toolchain.toml`; a edition 2024
  exige 1.85+, e o código gerado pelo `xprc` continua em `--edition=2021`).
* **Especialização de genéricos** (Seção 4.10): `instantiate(nome, tipo_do_argumento)` recheca o
  corpo com o parâmetro no tipo concreto e devolve o tipo de resultado da instância; memoização
  por nome, posição da definição e tipo estrutural, com guarda de reentrância para recursivas
  e tipo de resultado armazenado ao concluir.
* **Privacidade na fronteira de import**: `check_module` devolve todos os nomes (o oráculo dos
  testes unitários depende de `_main`); quem esconde os privados é `exported_members`.
* **Nomes privados** (Seção 4.1): `_` inicial = privado ao módulo; a checagem é no
  `check_name`/`emit_name` do acesso qualificado, e `exported_members` filtra os privados na fronteira de import. `check_module` mantém o mapa completo, inclusive `_main`.
* **`_init`** (Seção 4.1): emitido no `main` gerado na ordem de `modules` (já pós-ordem DFS do
  carregador) e chamado via `xpr_rt::call` + `xpr_rt::force`; nada mudou no runtime.
* **Neutro do `try` por tipo** (Seção 4.9): o checker publica `try_types` (span → Ty) e o
  pipeline (`check_types` → `emit_module` → `Emitter`) o injeta; `neutral_of_ty` resolve o
  neutro e o fallback sintático só cobre `Any`.
* **Índice de tupla** (Seção 4.8): literal int obrigatório; `Diagnostic::render_as(phase)`
  separa a fase de quem reporta (compilação × runtime) da severidade da entrada do catálogo.
* **`cannot-infer` de recursivas** (Seção 4.10): `result_calls_self` faz análise sintática
  conservadora — só a chamada pura, o `iif` com ambas as ramificações dependentes, o `try` e a
  última expressão de bloco contam; operadores e builtins genéricos não (têm tipo próprio/`Any`
  sem relação com recursão).
* **Somente leitura da Seção 6**: `predefined_constant` (types.rs) é a fonte única das constantes
  predefinidas (`true`/`false` bool; `NaN`/`INF`/`pi`/`PI`/`e`/`E` real, ambos os grafões);
  `assign` é *local-first* e aplica `assign-to-constant` a constantes em qualquer escopo e a
  nomes de nível superior quando `scopes.len() > 1` (dentro de função/bloco). Redefinição no
  nível de módulo continua válida (Seção 4.5).


## Lições que devem orientar mudanças

* **Runtime autocontido:** os fontes de `xpr_rt` e do catálogo entram no binário `xprc` por
  `include_str!`; `emit_embedded_runtime` os insere como módulos no Rust emitido. `xpr_rt`
  permanece crate do workspace para desenvolvimento e testes. Distribuir `xprc` e ter `rustc`
  basta para compilar XPR; não restaurar procura de `.rlib`, `--extern` ou cache separado.
* **Testar o programa real:** testes do texto emitido não detectam todos os problemas de
  escopo, propagação de `?`, captura e pilha. Mudanças de semântica precisam de compilação e
  execução com stdout, stderr e status verificados. Um fixture de parsing de curto-circuito
  não comprova curto-circuito durante a execução.
* **O gate precisa incluir tudo:** omitir `cargo test` já deixou passar uma regressão em
  oito unitários de privacidade. Usar `cargo test --workspace` inclui também o runtime.
* **Genéricos precisam conservar o resultado:** cache por nome, posição da definição e tipo
  estrutural do argumento. `None` marca instanciação em andamento (reentrada usa `Any`);
  `Some(Ty)` guarda o resultado concluído, e erro remove a entrada. Devolver `Any` em toda
  chamada repetida deixava operações inválidas passarem. Não usar a apresentação `Ty::name()`
  como identidade estrutural de tipos.
* **`apply` instancia no uso:** para a forma literal `[xs, f]`, a função nomeada ou literal
  é checada com o tipo de elemento de `xs` e o resultado tem tipo `[β]`. `[]` é absorvente,
  sem instanciação. Não reintroduzir `cannot-infer` para esse caso.
* **Operadores recursivos:** decidir soma numérica ou concatenação depois de descer nas
  listas/tuplas. Testar só `[1] + [2]` não protegia `["a"] + ["b"]` ou a tupla mista da spec.
* **Precisão decidida pelo usuário:** formato padrão `[2, 5]`; duas casas na notação fixa e
  cinco significativos na científica (`0.001` → `1.0000e-3`). Formatos explícitos respeitam
  sua precisão. Conflitos entre exemplo, regra e teste exigem decisão explícita antes de
  mudar o oráculo; não voltar ao antigo exemplo inconsistente de três significativos.
* **Posições e gramática:** contar codepoints, não bytes; conferir spans manualmente contra
  o fonte em caso de divergência. `fn()` e `f(1)(2)` não são construções válidas da gramática.
  O lexer só delimita strings e preserva escapes; o parser os decodifica. Expoente só começa
  com dígito após `e` e sinal opcional; validar faixa i64 apenas se o token acabar inteiro.
* **Evitar inferência duplicada:** comunicar tipos do checker ao emissor (como `try_types`),
  em vez de reconstruí-los pela aparência da expressão. Separar fase do diagnóstico de sua
  severidade padrão no catálogo: índice inválido de tupla é erro de compilação, de lista
  pode ser falha de execução.

## Contratos de testes e fixtures

A suíte cresce com regressões positivas, negativas e de fronteira. Não relaxar um fixture para
acomodar bug do compilador. Erro comprovado no teste é reportado ao usuário antes de alterá-lo.
Os nomes `sub1`–`sub7` do lexer e `subA`–`subJ` do parser são agrupamentos históricos dos testes.

* Lexer: `tests/lexer/cases/*.x` + `.tokens`; erros em `cases/errors/*.x` + `.error`.
  Linha de token: `{posição} {TokenKind}[ adj]`, incluindo `Eof`. Posição `L:C-C` ou
  `L:C-L:C` para multilinha, base 1, fim exclusivo. Números/identificadores sem aspas;
  strings `StringLit(dq|sq, <conteúdo bruto com escape Debug do Rust>)`. ` adj` só em sinais
  adjacentes. Erro: `{posição} {LexErrorKind}`. Contrato detalhado no runner
  `tests/lexer_fixtures.rs` e nos arquivos esperados.
* Parser: `tests/parser/cases/*.x` + `.ast`; erros em `cases/errors/*.x` + `.error`.
  Comparar `kind` e span nos erros, não a mensagem humana livre.
* Milestones: um diretório por caso; fonte `main.x` (ou primeiro `.x` em ordem lexical).
  `expected.stdout` verifica saída exata; `expected.exit` verifica status (padrão 0);
  `expected.exit.nonzero` exige status diferente de zero; `expected.stderr.contains` exige
  substring no stderr. Ausência de `expected.stdout` não verifica stdout. O runner exige
  compilação bem-sucedida; erros de compilação são testados na integração Rust.

### Formato do dump de AST

Uma árvore em texto, indentada **2 espaços por nível**, um nó por linha, sem parênteses de
fechamento. Cada linha é `<tag> [payload inline]`; os filhos de um nó vêm nas linhas seguintes,
indentados um nível a mais. **Spans não aparecem neste dump**; são verificados separadamente nos testes.

Convenções de tag, por tipo de nó (`Expr`/`Stmt`/módulo):

| Nó | Linha | Filhos |
| --- | --- | --- |
| `Module` | `Module` | `Import ...` (sem filhos) e `Def <nome>` (1 filho: o inicializador), em ordem |
| `Import` | `Import name=<nome> alias=<alias>` | nenhum |
| `FnDef` | `FnDef param=<pattern>` | 1 filho: o `Block` do corpo |
| `Block` | `Block` | N filhos, um por `Stmt`, em ordem (um `Stmt::Import` vira uma linha `Import ...` sem filhos, igual ao de módulo) |
| `IntLit`/`RealLit` | `IntLit <texto bruto>` / `RealLit <texto bruto>` | nenhum |
| `StringLit` | `StrLit <string decodificada em `{:?}` do Rust>` | nenhum |
| `QualifiedName` | `Name <nome>` ou `Name <alias>.<nome>` | nenhum |
| `Assignment` | `Assign target=<pattern>` | 1 filho (sem rótulo): o valor |
| `LogicalOr`/`LogicalAnd` | `Or` / `And` | N filhos (sem rótulo), um por operando, em ordem |
| `LogicalNot` | `Not` | 1 filho (sem rótulo): o operando |
| `Binary` | `BinOp <símbolo>` (`+ - * / % < > <= >= == !=`) | 2 filhos rotulados: `lhs: <nó>` e `rhs: <nó>` |
| `Unary` | `Unary <símbolo> adj=<true|false>` | 1 filho (sem rótulo): o operando |
| `Indexed` | `Indexed` | 1 filho rotulado `base: <nó>`, depois um `idx[i]: <nó>` por índice, em ordem |
| `FnCall` | `Call` | `target: <nó>` sempre; `arg: <nó>` só se houver argumento |
| `Block` (como `Expr::Block`) | igual à entrada `Block` acima | — |
| `Iif` | `Iif` | `cond: <nó>`, `then: <nó>`, `else: <nó>` |
| `For` | `For` | `init: <nó>`, `cond: <nó>`, `step: <nó>`, `body: <nó>` |
| `Try` | `Try` | 1 filho (sem rótulo): a expressão |
| `BracketLiteral` | `Bracket` | N filhos (sem rótulo), um por elemento, em ordem |

Quando um filho rotulado (`lhs:`, `cond:`, `base:`, `idx[0]:`, ...) é por sua vez um nó composto,
a tag desse nó fica **na mesma linha**, após o rótulo (ex.: `cond: BinOp >`), e os filhos *dele*
vêm indentados um nível além dessa linha — não dois.

`<pattern>` (usado em `param=`/`target=`) é a forma de superfície da gramática, recursiva:
`x`, `x:int`, `[a, b]`, `[[a,b],[c,d]]`, `[a:int, b:int]`.

### Formato do erro (`.error`, uma única linha)

```
{line}:{col_start}-{col_end} {VarianteDeSyntaxErrorKind}
```
(mesma convenção de posição do lexer — linha dupla `L1:C1-L2:C2` só se o erro cruzar linhas, o
que não deve acontecer para nenhum destes casos).


## Pendências e próxima ação

* Limitação conhecida: `cannot-infer` ao usar função genérica como valor sem contexto que
  determine o tipo; instanciação de chamadas diretas já existe.
* Divergência da spec: Seção 4.1 proíbe definir o mesmo nome duas vezes, mas Seção 4.5 e M4
  permitem redefinições. Manter o comportamento atual da 4.5 até decisão do usuário.
* Cobertura a ampliar: `input`/fim de arquivo, curto-circuito e ordem de efeitos colaterais,
  ciclos de imports, genéricos combinados com capturas/funções como valores, matriz de tipos,
  tamanhos e limites por builtin. Não há medição de cobertura de linhas ou ramificações.
* Próxima ação sugerida: mapear regras da spec a testes positivos, negativos e de fronteira,
  começando pelas lacunas acima. Escrever e confirmar o teste vermelho antes da implementação;
  concluir somente com gate verde e memória atualizada.

## Organização documental

A documentação do projeto fica em `AGENTS.md`, `MEMORY.md` e `xpr-specs.md`.
Planos executados e relatórios foram consolidados aqui; histórico, saídas antigas e APIs
anteriores podem ser consultados no Git. Skills em `skills/` são ferramentas separadas.
A especificação foi mantida sem alterações nesta consolidação; suas referências antigas a
planos e a `diretrizes-runtime.md` permanecem como pendência editorial, a pedido do usuário.
Os detalhes atuais do backend e dos diagnósticos estão neste arquivo e no código.
