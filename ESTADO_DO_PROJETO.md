# Estado do projeto (memória de continuidade entre sessões)

**Leia este arquivo inteiro antes de fazer qualquer coisa.** Ele existe para que um agente novo,
sem nenhum contexto da conversa anterior, saiba exatamente em que ponto o projeto está e qual é a
próxima ação concreta — sem precisar reconstruir isso lendo todo o histórico do git.

Este arquivo **não** é um relatório histórico (isso é `RELATORIO_PROGRESSO.md`, com uma seção por
etapa concluída, e o próprio `git log`). É um ponteiro de **estado atual + próxima ação**, e deve
ser atualizado — não só expandido — cada vez que esse estado muda.

## Status atual

* **Fase 1 (Planejamento): CONCLUÍDA.**
* **Fase 2 — Lexer: CONCLUÍDA.** 52 testes unitários + 12 fixtures golden-file.
* **Fase 2 — Parser: CONCLUÍDA.** 80 testes unitários (subetapas A–J) + 16 fixtures.
* **Fase 2 — Codegen: CONCLUÍDA.** Pipeline `lex → parse → tipos → Rust → rustc → execução`
  fechado. 20 testes unitários (catálogo + checagem de tipos), 38 de integração (emissão,
  semântica, CLI) e 14 milestones de ponta a ponta.
* **Gate global: `./scripts/run-all-tests.sh` → `TUDO OK` (exit 0).**
* **`RELATORIO_FINAL.md`: GERADO** (AGENTS.md [3]) — arquitetura da linguagem, estrutura do
  compilador, estratégia de testes, lições aprendidas e status final.
* **Toolchain: Rust 1.100.0 via rustup** (era 1.75.0 do `.deb` do Ubuntu, removido com
  `apt remove --purge rustc cargo` + purge dos órfãos `libstd-rust*`). Sem duplicata: `type -a
  rustc cargo` → só `~/.cargo/bin/*`. `edition = "2021"` inalterado (nada no projeto exigia 1.75).
  **Bug de portabilidade corrigido**: `find_rlib` só procurava `debug/deps/libxpr_rt-<hash>.rlib`
  e no cargo ≥ 1.80 o artefato canônico é `debug/libxpr_rt.rlib` (sem `deps/`), então o runtime
  "não era encontrado" (16 e2e falhavam). Agora aceita os dois layouts, prefere o canônico e
  varre a subárvore com limite de profundidade; 4 unitários novos cobrem os layouts.
  Nota operacional: `cargo clean` na raiz **não** limpa `xpr_rt/target/xprc-rt` (cache do
  runtime) — ao trocar de toolchain, remova esse diretório também, senão o `.rlib` antigo gera
  `E0786: invalid metadata`.
* **Pós-Codegen: limitação "especialização de genéricos" FECHADA** (Seção 4.10): `fn(x){ x + 1; }`
  chamada com string agora é `type-mismatch` — o corpo é re-verificado por instância
  (`instantiate`, com o parâmetro ligado ao tipo concreto do argumento) e o **resultado** da
  chamada é o da instância (`f(3)` tem tipo int, não `Any`). Instâncias memoizadas por
  `(nome, tipo)` **antes** de re-checar, senão a recursiva se instancia sem fim. Backend é
  dinâmico, então não há código por instância — a entrega é de verificação.
* **Corrigido o gate**: `run-all-tests.sh` não rodava `cargo test` (só lexer/parser/milestones),
  e foi por isso que uma regressão minha na subetapa de privacidade (`check_module` filtrando
  `_main`, quebrando 8 unitários) passou. O gate agora roda `cargo test` e o filtro de privados
  foi movido para a fronteira de import (`TypeChecker::exported_members`).
* **Pós-Codegen: limitação "nomes privados" FECHADA** (Seção 4.1): nome iniciado por `_` é
  privado ao módulo — `alias._nome` de fora é `undefined-identifier` (checado em `check_name` e,
  defensivamente, em `emit_name`); `check_module` deixa de exportar nomes privados; o uso sem
  qualificação dentro do módulo continua normal. Observação: a Seção 4.1 também diz "definir o
  mesmo nome duas vezes é um erro", o que **conflita** com a Seção 4.5 (exemplo das "variáveis
  salvas": `c = 9; … c = 19;`) e com o milestone M4 — mantido o comportamento da 4.5 e a
  divergência fica registrada como pendência de decisão da spec, não do compilador.
* **Pós-Codegen: limitação "_init / ordem de inicialização" FECHADA** (Seção 4.1): cada módulo
  com `_init` é chamado uma vez com `[]` antes de `_main`, na ordem de dependência (a pós-ordem
  DFS do carregador: imports antes do importador, independentes na ordem textual, entry por
  último); o resultado é descartado e uma falha é relatada e encerra — nunca capturável por
  `try` (usa `xpr_rt::force`, que já existia). Milestone novo M14 (15 no total).
* **Pós-Codegen: limitação "neutro do try por tipo" FECHADA** (Seção 4.9): o `TypeChecker`
  registra o tipo de cada `try` (`try_types`, por span) e o emissor escolhe o neutro por TIPO
  (int→0, real→0.0, string→"", bool→false, lista→[], tupla→posição a posição); a heurística
  sintática anterior devolvia `0` para variáveis — `try(s[9])` com `s` string entregava
  `[false, 0]` em vez de `[false, ""]`. `Any` (genérico) mantém o fallback sintático.
* **Pós-Codegen: limitação "índice de tupla constante" FECHADA** (Seção 4.8): índice de tupla
  precisa ser literal int — calculado é `type-mismatch` em compilação; constante fora do limite
  é `index-out-of-range` em compilação; e `t[i]` devolve o tipo da **posição** indexada (antes
  devolvia o da primeira posição). Para reportar `index-out-of-range` na fase de compilação
  sem quebrar a fonte única, `Diagnostic::render_as(phase)` fixa o prefixo por quem reporta
  (`SemaError` do CLI ⇒ `error[...]`; runtime ⇒ `failure[...]`).
* **Pós-Codegen: limitação `cannot-infer` FECHADA (caso recursivas)** (Seção 4.10, "o tipo de
  resultado de uma função recursiva é determinado pelas ramificações que não chamam a própria
  função"): `f = fn(x){ f(x); };` e `iif(c, f(x), f(x))` são rejeitados em compilação;
  `x * fat(x-1)` e builtins genéricos (`fill`, …) NÃO disparam (análise sintática conservadora
  via `result_calls_self`). Os casos de genéricos — uso como valor sem contexto (linha 725) e
  `apply` com `[]` (linha 816) — seguem abertos, acoplados à especialização (limitação 2).
  Incidente 3.3.1: M10 usava recursão pura (`rec = fn(n){ rec(n+1); };`) para o teste de
  profundidade; ajustado para `iif(n < 0, 0, rec(n + 1))` (válido pela 4.10, mesma saída).
* **Pós-Codegen: limitação `assign-to-constant` FECHADA** (Seção 6). Constantes predefinidas
  (`true`/`false`/`NaN`/`INF`/`pi`/`PI`/`e`/`E`) são somente leitura em qualquer escopo; nomes de
  nível superior são somente leitura dentro de funções (redefinição no nível de módulo continua
  válida, Seção 4.5). Gate segue `TUDO OK`; números atuais: 26 unitários de tipos, 41 de
  integração, 14/14 milestones. Incidente 3.3.1 registrado: M12 usava `e` como variável local e
  foi renomeado para `f`/`xf` (conformidade com a Seção 6 — ver RELATORIO_PROGRESSO.md, seção
  "Pós-Codegen").
* **Próxima ação**: o processo do `AGENTS.md` está completo. Restam apenas as **limitações
  conhecidas** abaixo, que não bloqueiam o gate mas são lacunas reais em relação à spec — fechar
  uma delas exige escrever primeiro o teste/milestone e depois a implementação.

## Como o projeto está montado

```
Cargo.toml            workspace: "." (bin+lib xprc) + "xpr_rt"
src/lib.rs            pub mod codegen, diag, lexer, parser
src/main.rs           CLI xprc (Seção 8): build, --emit-rust, --lex-dump, --parse-dump,
                      --type-dump, -o, -I, --rustc, -O, -v, -h/--help, --version
src/lexer/            token.rs (Span, TokenKind, Token, LexError) + mod.rs (Lexer)
src/parser/           ast.rs (AST) + dump.rs (dump normativo) + mod.rs (Parser recursivo)
src/diag/             catalog.rs (CATALOG: fonte única de diagnósticos) + mod.rs
src/codegen/          mod.rs (emissor de Rust, resolução de escopos, módulos) +
                      types.rs (Ty, TypeChecker — inferência/checagem estática)
xpr_rt/               crate de runtime: lib.rs (Value/Fail/Format/recursive_closure/
                      run_main_with/finish), ops.rs (Seção 5.2), bi.rs (builtins da Seção 5)
tests/lexer/          cases/ (12 fixtures) + fixtures.rs + lexer_tests.rs (52)
tests/parser/         cases/ (16 fixtures) + fixtures.rs + parser_tests.rs (80)
tests/codegen/        milestones/ (14) + codegen_tests.rs (38)
scripts/              run-lexer-tests.sh, run-parser-tests.sh, run-codegen-milestones.sh,
                      run-all-tests.sh
```

Documentos: `xpr-specs.md` (especificação canônica), `plano-lexer.md` e `plano-parser.md`
(executados), `plano-codegen.md` (executado), `plano-testes-codegen.md` (suíte evolutiva),
`RELATORIO_PROGRESSO.md` (uma seção por etapa).

## Decisões de arquitetura já tomadas (não reabrir sem motivo novo)

* `xprc` traduz XPR para **Rust** e compila com `rustc`, vinculando `libxpr_rt.rlib`. Nunca C,
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
* `edition = "2021"` (toolchain instalado: 1.75.0; edition 2024 exigiria 1.85+).
* **Especialização de genéricos** (Seção 4.10): `instantiate(nome, tipo_do_argumento)` recheca o
  corpo com o parâmetro no tipo concreto e devolve o tipo de resultado da instância; memoização
  `(nome, tipo)` **antes** do re-cheque (guarda de reentrância para recursivas).
* **Privacidade na fronteira de import**: `check_module` devolve todos os nomes (o oráculo dos
  testes unitários depende de `_main`); quem esconde os privados é `exported_members`.
* **Nomes privados** (Seção 4.1): `_` inicial = privado ao módulo; a checagem é no
  `check_name`/`emit_name` do acesso qualificado, e `check_module` não exporta privados.
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

Se alguma dessas decisões parecer errada ao implementar, **reporte ao usuário** (AGENTS.md 3.3.1)
em vez de simplesmente revertê-la.

## Limitações conhecidas (documentadas, não mascaradas)

Detalhes em `RELATORIO_PROGRESSO.md`, seção "Etapa 3 — Codegen":

1. `cannot-infer` no caso "uso como valor sem contexto determinar A" (linha 725) e "`apply`
   com `[]` literal" (linha 816): a instanciação cobre chamadas diretas, não o uso do nome como
   valor sem contexto concreto.
2. Formatação científica: `significant < 15` → `significant - 1` casas; padrão 15 → 2 casas
   (o exemplo `0.001 → 1.00e-3` da spec é batido; a regra geral da mantissa é ambígua na própria
   spec — exige decisão antes de mexer).

(Fechadas: `assign-to-constant`, `cannot-infer` de recursivas, índice de tupla constante, neutro
do `try` por tipo, `_init`/ordem de inicialização, nomes privados e especialização de genéricos —
ver seções "Pós-Codegen" do RELATORIO_PROGRESSO.md. Pendência de decisão da spec: "definir o mesmo nome duas vezes é erro"
(Seção 4.1) × redefinição no nível de módulo (Seção 4.5 e milestone M4).)

## Próxima ação concreta

O processo de `AGENTS.md` (Fases 1, 2 e 3) está **completo**. Para continuar evoluindo:

1. Escolher **um** item das "Limitações conhecidas" acima (sugestão de ordem por valor/esforço:
   formatação científica, ou fechar o restante de `cannot-infer` (uso como valor / `apply`
   com `[]`, que já têm a instanciação pronta como base).
2. Escrever **primeiro** o teste — milestone em `tests/codegen/milestones/` (a suíte só cresce) ou
   unitário em `src/codegen/types.rs`/`tests/codegen_tests.rs` — e confirmar que ele falha.
3. Implementar até o gate voltar a `TUDO OK`, remover a linha correspondente da lista de
   limitações em `RELATORIO_FINAL.md` e neste arquivo, e commitar.

## Regra de manutenção deste arquivo

Sempre que uma etapa, subetapa ou decisão de arquitetura mudar de estado, **edite este arquivo**
(não apenas acrescente ao final) para que a seção "Próxima ação concreta" nunca fique
desatualizada. Um agente que leia só este arquivo e o `AGENTS.md` deve conseguir continuar o
projeto sem precisar varrer o histórico do git.
