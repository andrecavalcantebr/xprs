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
* **Próxima ação**: Fase 3 de `AGENTS.md` [3] — gerar `RELATORIO_FINAL.md`. Opcionalmente,
  atacar as limitações listadas abaixo (elas não bloqueiam o gate, mas são lacunas reais em
  relação à spec).

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

Se alguma dessas decisões parecer errada ao implementar, **reporte ao usuário** (AGENTS.md 3.3.1)
em vez de simplesmente revertê-la.

## Limitações conhecidas (documentadas, não mascaradas)

Detalhes em `RELATORIO_PROGRESSO.md`, seção "Etapa 3 — Codegen":

1. `cannot-infer` não é emitido (parâmetro sem anotação vira `Ty::Any`).
2. `assign-to-constant` não é emitido (`pi = 3.14` passa).
3. Genéricos não geram instâncias especializadas (Seção 4.10) — funcionam via `Any`.
4. Neutro do `try` aproximado pela forma sintática da expressão.
5. Formatação científica: `significant < 15` → `significant - 1` casas; padrão 15 → 2 casas.
6. Índice de tupla não é verificado como constante.
7. `_init`/ordem de inicialização entre módulos não implementados.
8. Nomes privados (`_prefixo`) e `import` como statement de bloco sem a distinção exata da spec.

## Próxima ação concreta

1. **Gerar `RELATORIO_FINAL.md`** (AGENTS.md [3]): consolidar arquitetura da linguagem, estrutura
   do compilador, lições aprendidas (incluindo os bugs encontrados pelos milestones: `?` vazando
   do `try`, atribuição em escopo errado, profundidade de chamada, `Indexed` multinível) e status
   final por etapa — lexer, parser, codegen, com números de teste.
2. Commitar (`RELATORIO_FINAL.md` + `ESTADO_DO_PROJETO.md` atualizado).
3. **Opcional, se o usuário quiser fechar lacunas**: escolher um item das "Limitações conhecidas"
   acima, escrever **primeiro** o milestone em `tests/codegen/milestones/` (regra de evolução:
   a suíte só cresce) ou o teste unitário, e só então implementar. Cada lacuna fechada remove uma
   linha da lista de limitações no relatório e neste arquivo.

## Regra de manutenção deste arquivo

Sempre que uma etapa, subetapa ou decisão de arquitetura mudar de estado, **edite este arquivo**
(não apenas acrescente ao final) para que a seção "Próxima ação concreta" nunca fique
desatualizada. Um agente que leia só este arquivo e o `AGENTS.md` deve conseguir continuar o
projeto sem precisar varrer o histórico do git.
