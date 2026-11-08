# Estado do projeto (memória de continuidade entre sessões)

**Leia este arquivo inteiro antes de fazer qualquer coisa.** Ele existe para que um agente novo,
sem nenhum contexto da conversa anterior, saiba exatamente em que ponto o projeto está e qual é a
próxima ação concreta — sem precisar reconstruir isso lendo todo o histórico do git.

Este arquivo **não** é um relatório histórico (isso é `RELATORIO_PROGRESSO.md`, criado por etapa
concluída conforme `AGENTS.md` Fase 3, e o próprio `git log`). É um ponteiro de **estado atual +
próxima ação**, e deve ser atualizado — não só expandido — cada vez que esse estado muda.

## Status atual

* **Fase 1 (Planejamento) do `AGENTS.md`: CONCLUÍDA.**
* **Fase 2 (Execução Incremental): NÃO INICIADA.** Nenhuma linha de lógica de lexer, parser ou
  codegen foi escrita. `src/main.rs` é só um placeholder (`println!("xprc: nao implementado
  ainda")`).
* **Próxima ação**: começar a etapa **Lexer**, subetapa 1 ("Núcleo") de `plano-lexer.md`, em TDD.
  Ver "Próxima ação concreta" abaixo para o passo a passo.

## Inventário do que já existe

* **`xpr-specs.md`** — especificação canônica da linguagem XPR (única; a cópia duplicada
  `especifica_o_xpr.md` foi removida — ver git log). Já reflete Rust como linguagem de
  implementação **e** de geração de código (`xprc` traduz XPR para Rust e compila com
  `rustc`/`cargo`). O projeto começou com a spec apontando para C; isso foi migrado por completo,
  não é mais um ponto em aberto.
* **`plano-lexer.md`**, **`plano-parser.md`**, **`plano-codegen.md`** — plano de cada etapa:
  esboço de API, subetapas em ordem de TDD, decisões de arquitetura já fixadas, formato de dump
  golden-file e tabela de fixtures.
* **`plano-testes-codegen.md`** — plano de testes **evolutivo** (milestones M0–M8, mais
  placeholders) para quando a etapa Codegen existir; não é uma lista fechada, cresce com o
  projeto.
* **`tests/lexer/cases/`** (12 fixtures: 8 sucesso + 4 erro) e **`tests/parser/cases/`** (16
  fixtures: 11 sucesso + 5 erro) — pares `.x` + dump esperado (`.tokens`/`.ast`/`.error`).
  Verificados **mecanicamente** (não só à mão) contra implementações de referência descartáveis
  de lexer/parser escritas fora deste repositório, numa sessão em que `cargo`/`rustc` já estavam
  disponíveis — os 28 fixtures bateram exatamente. Essas implementações de referência **não**
  fazem parte do projeto e não devem ser procuradas aqui; servem só para dizer que o formato e os
  valores dos fixtures estão corretos.
* **`tests/codegen/milestones/`** (9 diretórios, M0–M8) — ainda não executáveis (não existe
  `xprc`), mas o conteúdo (`.x` + `expected.stdout`/`.exit`/`.exit.nonzero`/`.stderr.contains`) já
  está escrito e o valor de M1 (aritmética) foi conferido com a semântica de inteiros do próprio
  Rust.
* **`scripts/run-lexer-tests.sh`**, **`run-parser-tests.sh`**, **`run-codegen-milestones.sh`**,
  **`run-all-tests.sh`** — runners OK/NOK em bash puro. Os dois primeiros pressupõem testes de
  integração Rust chamados `tests/lexer_fixtures.rs` e `tests/parser_fixtures.rs` que **ainda não
  existem** — são, na prática, o primeiro código do projeto a ser escrito (antes até da lógica do
  lexer, por TDD).
* **`Cargo.toml` / `src/main.rs`** — esqueleto mínimo. `cargo build`/`cargo run` confirmados
  funcionando com o toolchain instalado (`cargo`/`rustc` 1.75.0). **Importante**: `edition` está
  em `"2021"`, não `"2024"` — 1.75.0 não suporta a edition 2024 (ela só existe a partir do Rust
  1.85). Não mude isso sem conferir a versão do toolchain disponível.
* **Git**: histórico completo do planejamento está nos commits (`git log`); não duplicado aqui.

## Decisões de arquitetura já tomadas (não reabrir sem motivo novo)

* `xprc` traduz XPR para **Rust** e compila com `rustc`/`cargo` — nunca para C. Ver
  `plano-codegen.md`, subetapas E/F/G.
* Catálogo de diagnósticos: módulo Rust único (`src/diag/catalog.rs`), compartilhado
  diretamente pelo compilador e pela crate de runtime (`xpr_rt`) — nada é gerado de um lado para
  o outro.
* Runtime (`xpr_rt`): `try`/`fail` via `Result<Value, Fail>` + `?`, não `setjmp`/`longjmp`.
* AST do parser **não tem** nó `Paren` (parênteses só guiam precedência; a árvore já representa o
  agrupamento pela própria forma).
* `Span` (lexer) tem `line_start`/`line_end`/`col_start`/`col_end` — não um único `line` — porque
  uma `StringLit` pode cruzar linhas.
* `SyntaxErrorKind` (parser) é um enum fechado, não uma `String` livre — os testes golden-file
  comparam `kind`+`span`, nunca a mensagem.

Se uma dessas decisões parecer errada ao implementar, **reporte ao usuário** (como qualquer bug de
fixture, por AGENTS.md 3.3.1) em vez de simplesmente revertê-la.

## Próxima ação concreta

1. Ler `plano-lexer.md` por completo.
2. Criar o teste de integração `tests/lexer_fixtures.rs`: itera `tests/lexer/cases/*.x` (compara
   com `.tokens`) e `tests/lexer/cases/errors/*.x` (compara com `.error`), no formato de dump
   exato descrito em `plano-lexer.md`. Esse teste deve existir **antes** da lógica do lexer
   (TDD) e imprimir `[OK]`/`[FAIL]` por fixture.
3. Implementar `src/lexer/` (`token.rs`, `mod.rs`) seguindo o esboço de API de `plano-lexer.md`,
   subetapa por subetapa (1 a 7 — Núcleo, Números, Strings, Identificadores/keywords,
   Operadores/pontuação, Adjacência de sinal, Erros/posições), cada uma com seus próprios testes
   unitários antes da lógica.
4. Gate de conclusão: `scripts/run-lexer-tests.sh` sai com status `0` — ou seja, os testes
   unitários de todas as subetapas **e** os 12 fixtures golden-file passam a 100%. Sem exceção.
5. Ao concluir: criar/atualizar `RELATORIO_PROGRESSO.md` (seção "Lexer", conforme `AGENTS.md`
   Fase 3), commitar, e **atualizar este arquivo** — status passa para "Lexer concluído, próxima
   ação: Parser", e o inventário/decisões acima são corrigidos se algo mudou durante a
   implementação.
6. Depois do Lexer: Parser (`plano-parser.md`, mesmo padrão de gate). Depois: Codegen
   (`plano-codegen.md`, subetapas A–G, mais o roteiro evolutivo de `plano-testes-codegen.md` via
   `scripts/run-codegen-milestones.sh`).
7. Ao final de tudo (Codegen funcional e testado): gerar `RELATORIO_FINAL.md` conforme `AGENTS.md`
   [3].

## Regra de manutenção deste arquivo

Sempre que uma etapa, subetapa ou decisão de arquitetura mudar de estado, **edite este arquivo**
(não apenas acrescente ao final) para que a seção "Próxima ação concreta" nunca fique
desatualizada. Um agente que leia só este arquivo e o `AGENTS.md` deve conseguir continuar o
projeto sem precisar varrer o histórico do git.
