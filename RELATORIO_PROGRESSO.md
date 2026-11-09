# Relatório de progresso — compilador XPR (`xprc`)

Documento criado conforme `AGENTS.md` Fase 3 (fechamento de etapa). Cada etapa concluída ganha uma
seção aqui; o estado atual e a próxima ação concreta ficam em `ESTADO_DO_PROJETO.md`.

---

## Etapa 1 — Lexer

**Status: CONCLUÍDA.** 52 testes unitários + 12 fixtures golden-file, todos verdes.
`scripts/run-lexer-tests.sh` sai com status `0`.

### Atividades realizadas

1. **Testes antes da lógica (TDD).**
   * `tests/lexer_fixtures.rs` — suíte golden-file que itera `tests/lexer/cases/*.x` (par com
     `.tokens`) e `tests/lexer/cases/errors/*.x` (par com `.error`), formata o dump exatamente no
     formato normativo de `plano-lexer.md` (`{posição} {TokenKind}[ adj]`) e imprime `[OK]`/`[FAIL]`
     por fixture, com `panic` e resumo ao final se algo falhar.
   * `tests/lexer_tests.rs` — 52 testes unitários organizados nas 7 subetapas do plano
     (`sub1_…` a `sub7_…`): núcleo (avanço por codepoint, linha/coluna, EOF, espaços,
     comentários), números, strings, identificadores/keywords, operadores/pontuação,
     adjacência de sinal e erros/posições.
2. **Implementação** de `src/lexer/token.rs` (tipos da API) e `src/lexer/mod.rs` (o analisador),
   seguindo o esboço de API do plano.
3. **Script de execução** `scripts/run-lexer-tests.sh` reescrito para rodar as **duas** suítes
   (unitários + fixtures) e só então dar o veredito — antes ele rodava apenas as fixtures, o que
   deixaria as subetapas sem gate.

### Decisões de design/arquitetura tomadas

* **Iteração por `char`, não por bytes.** A fonte é decodificada uma vez em `Vec<char>`; o
  ponteiro de avanço é um índice de codepoint. `\n` reinicia a coluna em 1; qualquer outro
  codepoint (inclusive `\r`) avança a coluna em 1. Isso torna a contagem de colunas equivalente a
  codepoints sem precisar de `char_indices`.
* **`Span` multilinha só pode ocorrer em `StringLit`.** Testado explicitamente
  (`sub7_token_multilinha_so_ocorre_em_string`): todo token não-string tem
  `line_start == line_end`, e um teste de contiguidade verifica que spans de tokens consecutivos
  se encaixam exatamente.
* **Strings: delimitação apenas.** O conteúdo é copiado bruto (escapes **não** são decodificados
  nem validados), mas um `\` consome também o codepoint seguinte — pela gramática (`chars`
  exclui `\`), uma aspa após `\` não pode fechar a string. Escape no fim do arquivo →
  `UnterminatedString`.
* **`adjacent_to_next` calculado depois da tokenização**, comparando `end` do sinal com `start`
  do token seguinte — e não durante o consumo de brancos. Assim o metadado é, por construção,
  "sem espaço, nova linha ou comentário no meio", sem duplicar a lógica de skip. `Eof` nunca
  conta como token seguinte adjacente. Como decidido no plano, um `-` binário colado (`a-1`)
  também recebe a marca: é um fato léxico, não uma interpretação sintática.
* **Número: `e` só inicia expoente se vier `[+|-]? dígito` (lookahead puro).** `3e` tokeniza como
  `IntLit(3)` + `Ident(e)`, fiel à gramática (`exp_part` exige `int_num`). `.` sempre inicia a
  parte fracionária quando o número já começou (`3.` é `RealLit("3.")`), e `.` seguido de dígito
  inicia real (`.5`); `.` seguido de outra coisa é `Dot`.
* **Validação de `int_lit` só quando o token termina sendo inteiro.** `12E+34` é `RealLit` e não
  passa por `parse::<i64>`; `9223372036854775808` (sem parte real/exp) → `MalformedNumber` com o
  span cobrindo o número inteiro.
* **`InvalidCharacter`** é emitido também para não-ASCII **dentro** de um identificador
  (`café` → erro na coluna 4, apontando só o codepoint ofensor) e para os operadores reservados
  (`&`, `|`, `^`, `~`) e símbolos desconhecidos (`@ # $ ?`, `!` isolado).
* **Comentários não aninham** (`/* a /* b */` fecha no primeiro `*/`), conforme a gramática.
  `/*/` é `UnterminatedComment` — verificado em teste.
* **`tokenize()` para no primeiro erro** e descarta os tokens já produzidos, como a Seção 7 exige.

### Correções em testes (divergências encontradas e resolvidas)

Três expectativas manuais minhas estavam erradas e o **lexer estava certo** em todas; nenhum
fixture golden-file precisou ser alterado (os 12 passaram de primeira):

1. `a/**/b` — esperei `b` na coluna 7, mas `a`(1) + `/**/`(2-5) põem `b` na coluna **6**.
2. `/*/` — esperei "comentário fechado e vazio"; na verdade sobra um `/` e falta `*/`, logo
   `UnterminatedComment` (span 1:1-4). Ajustei a expectativa e acrescentei `/**/` como o caso
   realmente vazio.
3. `a /* um\ndois\n/* nao aninha */ b` — `b` fica na coluna **18** (o `*/` fecha em 15-16, espaço
   em 17), não 19.

### Resultado final dos testes

`cargo test`:

```
     Running tests/lexer_fixtures.rs (target/debug/deps/lexer_fixtures-...)
running 1 test
[OK] L01_numeros_int.x
[OK] L02_numeros_real.x
[OK] L03_identificadores_e_palavras_chave.x
[OK] L04_operadores_e_pontuacao.x
[OK] L05_dot_ambiguo.x
[OK] L06_strings.x
[OK] L07_comentarios.x
[OK] L08_sinal_adjacencia.x
[OK] errors/L_ERR1_malformed_number.x
[OK] errors/L_ERR2_unterminated_string.x
[OK] errors/L_ERR3_unterminated_comment.x
[OK] errors/L_ERR4_invalid_character.x
== fixtures do lexer: 12/12 OK ==
test lexer_golden_file_fixtures ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/lexer_tests.rs (target/debug/deps/lexer_tests-...)
running 52 tests
...
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests xprc
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`./scripts/run-lexer-tests.sh` → `OK: etapa Lexer completa (unitários + fixtures).` (exit 0).
Sem warnings de compilação.

---

## Etapa 2 — Parser

**Status: CONCLUÍDA.** 80 testes unitários (subetapas A–J) + 16 fixtures golden-file, todos
verdes. `scripts/run-parser-tests.sh` sai com status `0`.

### Atividades realizadas

1. **Testes antes da lógica (TDD).**
   * `tests/parser_fixtures.rs` — suíte golden-file: tokeniza com o lexer real, parseia e compara
     o dump com `.ast`/`.error`, imprimindo `[OK]`/`[FAIL]` por fixture e resumindo ao final.
   * `tests/parser_tests.rs` — 80 testes unitários nas subetapas A–J do plano (infra do cursor,
     precedência/associatividade, `primary`, `indexed`, padrões e tipos, atribuição/`fn_def`,
     blocos e `;`, adjacência de sinal, nível de módulo/`const_expr`, programa vazio e
     multi-argumento), mais os exemplos literais da Seção 4 e testes de robustez da API.
2. **Implementação**: `src/parser/ast.rs` (árvore), `src/parser/dump.rs` (dump normativo),
   `src/parser/mod.rs` (parser descendente recursivo) e `src/lib.rs` exportando o módulo.
3. **Script**: `scripts/run-parser-tests.sh` roda unitários + fixtures e só então dá o veredito.

### Decisões de design/arquitetura tomadas

* **Escapes decodificados no parser** (recomendação explícita do plano, agora escolha registrada):
  `decode_escapes` resolve `\n \t \r \0 \" \' \\` ao construir `StringLit`. Um escape fora desse
  conjunto é preservado literalmente (`\` + codepoint) — a gramática não define erro léxico para
  isso e a Seção 7 não prevê diagnóstico específico.
* **`param_pattern` é obrigatório**: a gramática não tem `fn()`; `fn(){ … }` é `UnexpectedToken`.
  Os milestones M0–M8 do codegen já usam `fn(args)`, coerente com isso.
* **`fn_call` só tem alvo `qualified_name`**, então `f(1)(2)` não é gramatical (erro no `(`
  seguinte, reportado como `MissingSemicolon` quando aparece como statement). Indexar o resultado
  de uma chamada (`f(1)[0]`) é válido, porque `indexed = primary { "[" … }` e `primary` inclui
  `fn_call`.
* **`MultiArgCall` é detectado na vírgula** — o span do erro é o da vírgula, não o da chamada
  (é isso que o fixture `P_ERR1` exige). `iif`/`for` usam vírgulas legitimamente; só `fn_call` é
  restrito a um argumento.
* **`InvalidAssignmentTarget` é reportado no `=`**, com o alvo já parseado. Como
  `binding_pattern` é só identificador ou lista recursiva, `m.x = 1` (nome qualificado) também é
  inválido — não só a expressão indexada citada na spec.
* **`const_expr` validado por percurso na AST**, apontando o span do primeiro nó ofensor em ordem
  de leitura. `iif`/`for`/`bloco`/chamada/atribuição/`fn_def` são rejeitados; `fn_def` como
  inicializador **direto** de nível de módulo é `Initializer::FnDef` (não é `const_expr`, é a outra
  alternativa da produção).
* **`Or`/`And` são nós `n`-ários** (`Vec<Expr>`): `a or b or c` é um único `Or` com 3 operandos, como
  os dumps esperam. O span é o do primeiro ao último operando.
* **Sem nó `Paren`**, como já fixado no plano: `((1))` produz diretamente `IntLit 1`.
* **`ImportDecl.span`** cobre de `import` até `)`, e `ImportDecl::effective_alias()` devolve o alias
  explícito ou o próprio nome — usado também pelo dump.
* **Sentinela para slice vazia**: `Parser::new` é público; receber `&[]` (impossível pela pipeline,
  que sempre tem `Eof`) reporta `EmptyProgram` em 1:1 em vez de estourar o índice.

### Correções em testes (divergências encontradas e resolvidas)

Nenhum fixture golden-file foi alterado (16/16 passaram logo após a implementação e uma correção
de indentação no dump). As divergências foram todas em **testes unitários meus**, com o parser
correto:

1. **Indentação de filhos rotulados**: minha primeira versão de `labeled_child` indentava os
   descendentes um nível além do devido (`cond: BinOp >` com filhos a 2 níveis). Corrigido no
   `dump.rs` (não no teste): os descendentes ficam exatamente um nível abaixo do rótulo.
2. **`f(1)(2)`** — eu esperava sucesso; a gramática só permite chamar `qualified_name`.
3. **`fn()`** — eu usei `fn(){ … }` em três testes; `param_pattern` é obrigatório.
4. Vários **spans calculados à mão** estavam deslocados (`x[0] = 1` → `=` na coluna 8, vírgula de
   `f(1, 2)` na coluna 18, `import` ocupando 1–7, `Eof` de `// nada aqui\n` na linha 2, etc.).

### Resultado final dos testes

`cargo test`:

```
     Running tests/lexer_fixtures.rs (target/debug/deps/lexer_fixtures-...)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/lexer_tests.rs (target/debug/deps/lexer_tests-...)
running 52 tests
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/parser_fixtures.rs (target/debug/deps/parser_fixtures-...)
[OK] P01_precedencia_aritmetica.x
[OK] P02_logica_curto_circuito.x
[OK] P03_comparacoes.x
[OK] P04_unario_e_indexacao.x
[OK] P05_indexacao_multipla.x
[OK] P06_funcao_e_chamada.x
[OK] P07_bloco_e_atribuicao.x
[OK] P08_padroes_de_lista.x
[OK] P09_iif_e_for.x
[OK] P10_try.x
[OK] P11_modulo_import.x
[OK] errors/P_ERR1_dois_argumentos.x
[OK] errors/P_ERR2_alvo_atribuicao_invalido.x
[OK] errors/P_ERR3_import_fora_de_posicao.x
[OK] errors/P_ERR4_programa_vazio.x
[OK] errors/P_ERR5_const_expr_invalido.x
== fixtures do parser: 16/16 OK ==
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/parser_tests.rs (target/debug/deps/parser_tests-...)
running 80 tests
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`./scripts/run-parser-tests.sh` → `OK: etapa Parser completa (unitários + fixtures).` (exit 0).
`./scripts/run-all-tests.sh` ainda falha, mas **apenas** nos 10 milestones de codegen, que
dependem da CLI `xprc` — etapa seguinte.
