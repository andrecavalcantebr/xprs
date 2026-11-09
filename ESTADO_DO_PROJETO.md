# Estado do projeto (memória de continuidade entre sessões)

**Leia este arquivo inteiro antes de fazer qualquer coisa.** Ele existe para que um agente novo,
sem nenhum contexto da conversa anterior, saiba exatamente em que ponto o projeto está e qual é a
próxima ação concreta — sem precisar reconstruir isso lendo todo o histórico do git.

Este arquivo **não** é um relatório histórico (isso é `RELATORIO_PROGRESSO.md`, criado por etapa
concluída conforme `AGENTS.md` Fase 3, e o próprio `git log`). É um ponteiro de **estado atual +
próxima ação**, e deve ser atualizado — não só expandido — cada vez que esse estado muda.

## Status atual

* **Fase 1 (Planejamento) do `AGENTS.md`: CONCLUÍDA.**
* **Fase 2 — Etapa Lexer: CONCLUÍDA.** 52 testes unitários (`tests/lexer_tests.rs`, subetapas 1–7)
  + 12 fixtures golden-file (`tests/lexer_fixtures.rs` sobre `tests/lexer/cases/`) verdes.
  `scripts/run-lexer-tests.sh` sai com status `0`. Detalhes e decisões em
  `RELATORIO_PROGRESSO.md`, seção "Etapa 1 — Lexer".
* **Próxima ação**: iniciar a etapa **Parser** (`plano-parser.md`), em TDD, com o mesmo padrão de
  gate do Lexer. Ver "Próxima ação concreta" abaixo.

## Inventário do que já existe

* **`xpr-specs.md`** — especificação canônica da linguagem XPR (única; a cópia duplicada
  `especifica_o_xpr.md` foi removida — ver git log). Já reflete Rust como linguagem de
  implementação **e** de geração de código (`xprc` traduz XPR para Rust e compila com
  `rustc`/`cargo`).
* **`plano-lexer.md`** (executado), **`plano-parser.md`**, **`plano-codegen.md`** — plano de cada
  etapa: esboço de API, subetapas em ordem de TDD, decisões de arquitetura já fixadas, formato de
  dump golden-file e tabela de fixtures.
* **`plano-testes-codegen.md`** — plano de testes **evolutivo** (milestones M0–M8) para a etapa
  Codegen; cresce com o projeto.
* **`src/lexer/token.rs`** — `Span`, `TokenKind`, `Token`, `LexErrorKind`, `LexError`
  (API exatamente como no plano).
* **`src/lexer/mod.rs`** — `Lexer::new(&str)` + `Lexer::tokenize() -> Result<Vec<Token>, LexError>`.
  Iteração por codepoint, comentários, strings brutas, números, identificadores/keywords,
  operadores/pontuação e cálculo de `adjacent_to_next`. Nenhuma dependência externa.
* **`tests/lexer_tests.rs`** — 52 testes unitários das subetapas 1–7 do lexer.
* **`tests/lexer_fixtures.rs`** — runner golden-file do lexer (`[OK]`/`[FAIL]` por fixture).
* **`tests/lexer/cases/`** (12 fixtures: 8 sucesso + 4 erro) e **`tests/parser/cases/`** (16
  fixtures: 11 sucesso + 5 erro) — pares `.x` + dump esperado (`.tokens`/`.ast`/`.error`),
  verificados mecanicamente contra implementações de referência descartáveis fora deste
  repositório. Os 12 do lexer passam com a implementação atual.
* **`tests/codegen/milestones/`** (9 diretórios, M0–M8) — ainda não executáveis (não existe CLI
  `xprc` funcional).
* **`scripts/run-lexer-tests.sh`** (unitários + fixtures do lexer), **`run-parser-tests.sh`** e
  **`run-codegen-milestones.sh`** (este último pressupõe `tests/parser_fixtures.rs` e a CLI),
  **`run-all-tests.sh`** (encadeia as três etapas).
* **`Cargo.toml` / `src/main.rs`** — esqueleto mínimo. `edition = "2021"` (toolchain instalado é
  `cargo`/`rustc` 1.75.0, que não aceita edition 2024). `src/main.rs` ainda imprime
  `xprc: nao implementado ainda` — a CLI só ganha corpo na etapa Codegen.
* **Git**: histórico completo do planejamento está nos commits (`git log`); não duplicado aqui.

## Decisões de arquitetura já tomadas (não reabrir sem motivo novo)

* `xprc` traduz XPR para **Rust** e compila com `rustc`/`cargo` — nunca para C. Ver
  `plano-codegen.md`, subetapas E/F/G.
* Catálogo de diagnósticos: módulo Rust único (`src/diag/catalog.rs`), compartilhado diretamente
  pelo compilador e pela crate de runtime (`xpr_rt`) — nada é gerado de um lado para o outro.
* Runtime (`xpr_rt`): `try`/`fail` via `Result<Value, Fail>` + `?`, não `setjmp`/`longjmp`.
* AST do parser **não tem** nó `Paren` (parênteses só guiam precedência).
* `Span` (lexer) tem `line_start`/`line_end`/`col_start`/`col_end` — não um único `line` — porque
  uma `StringLit` pode cruzar linhas. Contagem sempre em **codepoints**.
* `SyntaxErrorKind` (parser) é um enum fechado, não uma `String` livre — os testes golden-file
  comparam `kind`+`span`, nunca a mensagem.
* (Lexer, fixado na prática) `Token::adjacent_to_next` só é `true` em `Plus`/`Minus` e é calculado
  comparando o fim do sinal com o início do token seguinte; `Eof` nunca conta. Strings são
  delimitadas sem decodificar escapes; `int_lit` fora de `i64` → `MalformedNumber`.

Se alguma dessas decisões parecer errada ao implementar, **reporte ao usuário** (como qualquer bug
de fixture, por AGENTS.md 3.3.1) em vez de simplesmente revertê-la.

## Próxima ação concreta

1. Ler `plano-parser.md` por completo (AST, esboço de API, subetapas A–F, formato do dump `.ast`).
2. Criar `tests/parser_fixtures.rs` — iterar `tests/parser/cases/*.x` (par com `.ast`) e
   `tests/parser/cases/errors/*.x` (par com `.error`), no formato exato de `plano-parser.md`,
   imprimindo `[OK]`/`[FAIL]` por fixture. Deve existir **antes** da lógica do parser (TDD) e
   consumir `xprc::lexer::Lexer`.
3. Implementar `src/parser/` subetapa por subetapa (A–F), com testes unitários próprios para cada
   uma antes da lógica correspondente, incluindo a regra de adjacência de sinal
   (`adjacent_to_next` → `syntax-error`) que o lexer deliberadamente não decide.
4. Gate de conclusão: `scripts/run-parser-tests.sh` sai com status `0` — testes unitários de todas
   as subetapas **e** os 16 fixtures golden-file a 100%. Sem exceção; fixture que pareça errado é
   reportado, nunca ajustado para mascarar bug (AGENTS.md 3.3.1).
5. Ao concluir: atualizar `RELATORIO_PROGRESSO.md` (nova seção "Etapa 2 — Parser"), commitar e
   **atualizar este arquivo** (status → "Parser concluído, próxima ação: Codegen").
6. Depois: Codegen (`plano-codegen.md`, subetapas A–G: runtime `xpr_rt`, catálogo de diagnósticos,
   geração de Rust, driver CLI com `--lex-dump`/`--parse-dump`/`--emit-rust` etc., mais o roteiro
   evolutivo de `plano-testes-codegen.md` via `scripts/run-codegen-milestones.sh`).
7. Ao final de tudo (Codegen funcional e testado): gerar `RELATORIO_FINAL.md` conforme
   `AGENTS.md` [3].

## Regra de manutenção deste arquivo

Sempre que uma etapa, subetapa ou decisão de arquitetura mudar de estado, **edite este arquivo**
(não apenas acrescente ao final) para que a seção "Próxima ação concreta" nunca fique
desatualizada. Um agente que leia só este arquivo e o `AGENTS.md` deve conseguir continuar o
projeto sem precisar varrer o histórico do git.
