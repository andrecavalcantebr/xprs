# Estado do projeto (memória de continuidade entre sessões)

**Leia este arquivo inteiro antes de fazer qualquer coisa.** Ele existe para que um agente novo,
sem nenhum contexto da conversa anterior, saiba exatamente em que ponto o projeto está e qual é a
próxima ação concreta — sem precisar reconstruir isso lendo todo o histórico do git.

Este arquivo **não** é um relatório histórico (isso é `RELATORIO_PROGRESSO.md`, criado por etapa
concluída conforme `AGENTS.md` Fase 3, e o próprio `git log`). É um ponteiro de **estado atual +
próxima ação**, e deve ser atualizado — não só expandido — cada vez que esse estado muda.

## Status atual

* **Fase 1 (Planejamento) do `AGENTS.md`: CONCLUÍDA.**
* **Fase 2 — Etapa Lexer: CONCLUÍDA.** 52 testes unitários + 12 fixtures golden-file verdes;
  `scripts/run-lexer-tests.sh` sai com `0`.
* **Fase 2 — Etapa Parser: CONCLUÍDA.** 80 testes unitários (subetapas A–J) + 16 fixtures
  golden-file verdes; `scripts/run-parser-tests.sh` sai com `0`.
* **Próxima ação**: iniciar a etapa **Codegen** (`plano-codegen.md`, subetapas A–G) e a CLI
  `xprc` (Seção 8), que é o que falta para `scripts/run-codegen-milestones.sh` passar. Ver
  "Próxima ação concreta" abaixo.

## Inventário do que já existe

* **`xpr-specs.md`** — especificação canônica da linguagem XPR (única). Rust é a linguagem de
  implementação **e** de geração de código (`xprc` traduz XPR para Rust e compila com
  `rustc`/`cargo`).
* **`plano-lexer.md`** e **`plano-parser.md`** (ambos executados), **`plano-codegen.md`** (a
  executar) — esboço de API, subetapas em ordem de TDD, decisões de arquitetura, formato de dump
  golden-file e tabela de fixtures de cada etapa.
* **`plano-testes-codegen.md`** — roteiro **evolutivo** de testes de codegen (milestones M0–M8).
* **`src/lexer/token.rs`** — `Span`, `TokenKind`, `Token`, `LexErrorKind`, `LexError`.
* **`src/lexer/mod.rs`** — `Lexer::new(&str)` + `tokenize() -> Result<Vec<Token>, LexError>`:
  iteração por codepoint, comentários, strings brutas, números, identificadores/keywords,
  operadores e `adjacent_to_next`.
* **`src/parser/ast.rs`** — `Module`, `ImportDecl`, `Definition`, `Initializer`, `FnDef`, `Block`,
  `Stmt`, `Expr` (sem nó `Paren`), `BinOp`, `UnOp`, `BindingPattern`, `ParamPattern`,
  `TypeAnnotation`.
* **`src/parser/dump.rs`** — `dump_module(&Module) -> String` no formato normativo dos `.ast`.
* **`src/parser/mod.rs`** — `Parser::new(&[Token])` + `parse_module() -> Result<Module,
  SyntaxError>`, `SyntaxErrorKind` (enum fechado, 8 variantes) e `decode_escapes` (escapes
  resolvidos nesta etapa).
* **`src/lib.rs`** — exporta `lexer` e `parser`. **`src/main.rs`** ainda é o placeholder
  `xprc: nao implementado ainda`: a CLI entra na etapa Codegen.
* **Testes** — `tests/lexer_tests.rs` (52), `tests/lexer_fixtures.rs` (12 fixtures),
  `tests/parser_tests.rs` (80), `tests/parser_fixtures.rs` (16 fixtures), todas as fixtures em
  `tests/lexer/cases/` e `tests/parser/cases/` (verificadas mecanicamente numa sessão anterior
  contra implementações de referência descartáveis, fora deste repositório).
* **`tests/codegen/milestones/`** (9 diretórios, M0–M8) — `.x` +
  `expected.stdout`/`.exit`/`.exit.nonzero`/`.stderr.contains`, ainda sem nada que os execute.
* **`scripts/`** — `run-lexer-tests.sh` e `run-parser-tests.sh` (unitários + fixtures),
  `run-codegen-milestones.sh` (milestones, depende da CLI) e `run-all-tests.sh` (encadeia as três;
  hoje falha só no codegen, como esperado).
* **`Cargo.toml`** — `edition = "2021"` (toolchain instalado: `cargo`/`rustc` 1.75.0; edition 2024
  exigiria 1.85+). Não mude sem conferir a versão disponível.
* **Git**: histórico nos commits (`git log`); não duplicado aqui.

## Decisões de arquitetura já tomadas (não reabrir sem motivo novo)

* `xprc` traduz XPR para **Rust** e compila com `rustc`/`cargo` — nunca para C. Ver
  `plano-codegen.md`, subetapas E/F/G.
* Catálogo de diagnósticos: módulo Rust único (`src/diag/catalog.rs`), compartilhado diretamente
  pelo compilador e pela crate de runtime (`xpr_rt`) — nada é gerado de um lado para o outro.
* Runtime (`xpr_rt`): `try`/`fail` via `Result<Value, Fail>` + `?`, não `setjmp`/`longjmp`.
* `Span` (lexer) tem `line_start`/`line_end`/`col_start`/`col_end`, sempre em **codepoints**;
  token multilinha só pode ser `StringLit`.
* `Token::adjacent_to_next` só é `true` em `Plus`/`Minus`; calculado comparando o fim do sinal com
  o início do token seguinte (`Eof` nunca conta). Strings são delimitadas sem decodificar escapes;
  `int_lit` fora de `i64` → `MalformedNumber`.
* AST do parser **não tem** nó `Paren`; `v[a, b]` e `v[a][b]` produzem o mesmo `Indexed`; lista ×
  tupla não é decidido no parser.
* `SyntaxErrorKind` é enum fechado; os testes comparam `kind`+`span`, nunca a mensagem (`detail`).
* Escapes de string são decodificados **no parser** (`decode_escapes`); escape desconhecido é
  preservado literalmente.
* `param_pattern` é obrigatório (`fn()` não existe na gramática); `fn_call` só chama
  `qualified_name` (logo `f(1)(2)` não é gramatical); `MultiArgCall` aponta para a vírgula;
  `InvalidAssignmentTarget` aponta para o `=`.

Se alguma dessas decisões parecer errada ao implementar, **reporte ao usuário** (como qualquer bug
de fixture, por AGENTS.md 3.3.1) em vez de simplesmente revertê-la.

## Próxima ação concreta

1. Ler `plano-codegen.md` por completo (subetapas A–G) e `plano-testes-codegen.md`, e a Seção 8 de
   `xpr-specs.md` (interface de linha de comando).
2. Seguir as subetapas do plano na ordem de TDD, com testes antes da lógica em cada uma. Pontos já
   fixados que o lexer/parser entregam prontos para consumo: `SyntaxErrorKind`/`LexErrorKind`
   fechados e com `Span`; `Expr::FnCall { target, arg }` com `arg` único; `Expr::Indexed` com lista
   de índices; `Initializer` já distinguindo `FnDef` de `ConstExpr`.
3. Gate de conclusão: `scripts/run-codegen-milestones.sh` sai com status `0` (M0–M8, incluindo
   `M6b_fail_nao_capturado` com `expected.exit.nonzero` e `expected.stderr.contains`).
   `./scripts/run-all-tests.sh` deve então sair com `0` e imprimir `TUDO OK`.
4. Ao concluir: atualizar `RELATORIO_PROGRESSO.md` (nova seção "Etapa 3 — Codegen"), commitar e
   **atualizar este arquivo**.
5. Por último (AGENTS.md [3]): gerar `RELATORIO_FINAL.md` consolidando arquitetura, lições
   aprendidas e status final.

## Regra de manutenção deste arquivo

Sempre que uma etapa, subetapa ou decisão de arquitetura mudar de estado, **edite este arquivo**
(não apenas acrescente ao final) para que a seção "Próxima ação concreta" nunca fique
desatualizada. Um agente que leia só este arquivo e o `AGENTS.md` deve conseguir continuar o
projeto sem precisar varrer o histórico do git.
