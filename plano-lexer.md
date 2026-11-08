# Plano — Etapa Lexer

Fonte normativa: `xpr-specs.md` (único documento de especificação do projeto), Seção 3
(produções marcadas "detectado pelo analisador léxico") e Seção 7 (posições em codepoints).

## Objetivo

Transformar o texto-fonte (UTF-8) de um módulo `.x` em uma sequência de tokens com posição
(linha/coluna, em **codepoints**, não bytes), sem validar semântica de escapes de string nem
faixa de inteiros fora do que a própria produção léxica define.

## Dependências

Nenhuma crate externa. Apenas `std` (iteração de `char` sobre `&str`, que já é por codepoint).

## Estruturas de dados (esboço de API)

```rust
// src/lexer/token.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line_start: u32,  // 1-based
    pub col_start: u32,   // 1-based, em codepoints
    pub line_end: u32,    // 1-based; igual a line_start exceto em strings multilinha
    pub col_end: u32,     // exclusivo, na linha line_end
}
```

**Correção de projeto:** a primeira versão deste plano tinha `Span` com um único campo `line`,
o que não representa um token que cruza linhas — e strings **podem** cruzar linhas (Seção 3,
`chars` aceita qualquer codepoint incluindo controle, "uma string pode abranger várias linhas").
`line_start`/`line_end` resolvem isso; para todo token que não seja uma string multilinha,
`line_start == line_end`.

```rust

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // literais
    IntLit(String),      // texto bruto; validação de faixa i64 ocorre nesta etapa (malformed-number)
    RealLit(String),     // texto bruto; conversão para f64 fica para o parser/sema
    StringLit { raw: String, quote: char }, // conteúdo NÃO decodificado (escapes crus)
    Ident(String),

    // palavras-chave (subconjunto fixo; ver keywords na gramática)
    KwFn, KwIif, KwFor, KwTry, KwImport, KwAnd, KwOr, KwNot,

    // operadores
    Plus, Minus, Star, Slash, Percent,
    Lt, Gt, Le, Ge, EqEq, NotEq, Eq,

    // pontuação
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semi, Colon, Dot,

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// true quando o token é um sinal ('+'/'-') cuja posição é imediatamente
    /// adjacente (sem espaço/comentário) ao próximo token. Consumido pelo parser
    /// para a regra de adjacência de sinal unário (ver plano-parser.md, subetapa F).
    pub adjacent_to_next: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexErrorKind {
    MalformedNumber,      // int_lit fora de i64 (ver Seção 3 comentário "fora do intervalo int64")
    UnterminatedString,
    InvalidCharacter,     // codepoint não-ASCII fora de string/comentário (ver nota sobre identificadores)
    UnterminatedComment,  // "/* ... " sem "*/"
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

// src/lexer/mod.rs
pub struct Lexer<'a> { /* iterador de chars + posição + lookahead */ }

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self;
    /// Tokeniza tudo de uma vez; mais simples para o parser LL consumir.
    /// Erro para no primeiro problema (compilação para no primeiro erro — Seção 7).
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError>;
}
```

## Decisões de arquitetura já fixadas pela especificação

* **Posições em codepoints, não bytes.** Iterar com `char_indices` não basta para coluna;
  manter contador próprio de codepoints por linha.
* **Strings não são validadas aqui.** O lexer apenas localiza as aspas delimitadoras e aceita
  qualquer codepoint, incluindo controle, entre elas (string pode cruzar linhas). A decodificação
  de `\n \t \r \0 \" \' \\` é responsabilidade de uma etapa posterior (parser ou sema) — decisão
  explícita da spec ("o lexer apenas delimita strings e não valida seu conteúdo").
* **`.` é ambíguo.** Um `.` entre identificadores é o token `Dot`; um `.` imediatamente seguido de
  um dígito inicia um `real_lit` (`.5`). O lexer precisa de 1 caractere de lookahead após `.`.
* **Sinal unário vs. binário não é decidido aqui.** O lexer apenas emite `Plus`/`Minus` como
  tokens normais e marca `adjacent_to_next` (sem espaço, nova linha ou comentário até o próximo
  token). A decisão sintática (unário vs. binário) e o erro `syntax-error` de adjacência ficam no
  parser — é a própria spec que pede essa divisão de responsabilidade.
* **Identificadores são só ASCII**; não-ASCII só é válido dentro de strings e comentários.
* **`true`/`false` não são palavras-chave** — tokenizam como `Ident`, resolvidos depois como
  constantes predefinidas (Seção 6). Mesmo tratamento para `bool`/`int`/`real`/`string`/`fn` como
  nomes de tipo (`fn` É keyword por conta de `fn_def`, mas como *nome de tipo* em anotações ele é
  o mesmo token `KwFn` reinterpretado pelo parser — não precisa de token novo).

## Subetapas (TDD — testes antes da lógica, por sub etapa)

1. **Núcleo**: avanço por codepoint, contagem linha/coluna, fim de arquivo (`Eof`), espaços em
   branco ignorados, comentário de linha `//`, comentário de bloco `/* */` (incl. não fechado →
   `UnterminatedComment`).
2. **Números**: `int_num`, `real_lit` nas 4 formas da gramática (`3.`, `.5`, `4.0`, `3e0`,
   `1.5e-10`), `malformed-number` para inteiro fora de `i64`.
3. **Strings**: aspas simples e duplas, conteúdo bruto preservado, múltiplas linhas dentro da
   string, `UnterminatedString`.
4. **Identificadores e palavras-chave**: tabela fixa de 8 keywords; ASCII-only (não-ASCII fora de
   string/comentário → `InvalidCharacter`).
5. **Operadores e pontuação**, incluindo a resolução do `.` (dot token vs. início de real).
6. **Metadado de adjacência de sinal** (`adjacent_to_next`) para `+`/`-`.
7. **Erros e posições**: todo `LexError` carrega `Span` correto; teste de que a compilação para no
   primeiro erro léxico.

Cada subetapa gera seu arquivo de teste próprio (`tests/lexer_numeros.rs`,
`tests/lexer_strings.rs`, etc.) ou seções dentro de `tests/lexer_tests.rs` — a decidir pelo agente
executor, desde que nenhum teste de uma subetapa posterior preceda a lógica que ele testa. Além
desses testes unitários por subetapa, a etapa inteira é fechada por uma suíte de **fixtures
golden-file** (abaixo), que é o critério de conclusão real.

## Fixtures golden-file: exemplos `.x` e seus dumps esperados

Diretório `tests/lexer/cases/` (casos que devem tokenizar com sucesso) e
`tests/lexer/cases/errors/` (casos que devem falhar com um `LexError` específico). Para cada
`NN_nome.x` existe um arquivo-irmão:

* `NN_nome.tokens` (casos de sucesso) — o dump esperado da sequência de tokens, um por linha.
* `NN_nome.error` (casos de erro) — uma única linha com o erro esperado; `tokenize()` deve
  retornar exatamente esse `Err` para aquele arquivo, independentemente de quantos tokens válidos
  o precederiam (a assinatura `tokenize() -> Result<Vec<Token>, LexError>` descarta os tokens
  parciais em caso de erro).

### Formato do dump (`.tokens`, uma linha por token, nessa ordem, incluindo o `Eof` final)

```
{posição} {representação-do-TokenKind}[ adj]
```

* **Posição**: se `line_start == line_end`, `{line_start}:{col_start}-{col_end}`; senão (token
  multilinha, só acontece em `StringLit`), `{line_start}:{col_start}-{line_end}:{col_end}`.
* **Representação do `TokenKind`** (notação fixa, não é o `#[derive(Debug)]` literal do Rust —
  mais compacta e desenhada para ser fácil de conferir à mão):
  * `IntLit(<dígitos>)`, `RealLit(<texto bruto>)` — sem aspas, o texto é sempre `[0-9.eE+-]`.
  * `StringLit(dq, <conteúdo>)` ou `StringLit(sq, <conteúdo>)` — `dq`/`sq` indica o delimitador
    original (`"` ou `'`); `<conteúdo>` é o conteúdo **bruto** (não decodificado) formatado com o
    escape padrão do Rust (`{:?}` de uma `&str`) só para tornar a linha do dump legível e
    inambígua — um `\` literal no texto original aparece como `\\`; um codepoint de controle real
    (ex.: uma nova linha dentro da string) aparece como `\n` (uma barra só). Essa é a distinção
    importante: o lexer nunca decodifica `\n` em uma quebra de linha real — se o dump mostra `\n`
    de barra simples, é porque havia um caractere de controle de verdade no fonte.
  * `Ident(<nome>)` — sem aspas (identificadores são só ASCII, sem ambiguidade).
  * Palavras-chave e operadores/pontuação: o nome do variant tal como está em `TokenKind`
    (`KwFn`, `KwIif`, ..., `Plus`, `Minus`, ..., `LParen`, ..., `Dot`).
  * `Eof`.
* **Sufixo ` adj`**: aparece **somente** em tokens `Plus`/`Minus`, quando `adjacent_to_next ==
  true`. Para todos os outros `TokenKind`s, omitido (o campo é irrelevante fora de sinais).

### Formato do erro (`.error`, sempre uma única linha)

```
{posição} {VarianteDeLexErrorKind}
```
(mesma regra de posição acima; variantes = nomes de `LexErrorKind`).

### Exemplos incluídos (seed — a suíte deve crescer, nunca encolher)

| Arquivo | Cobre (subetapa) |
| --- | --- |
| `L01_numeros_int.x` | 2 — inteiros |
| `L02_numeros_real.x` | 2 — reais, as 4 formas da gramática |
| `L03_identificadores_e_palavras_chave.x` | 4 — identificadores vs. as 8 keywords; `true`/`false`/`int`/`real` NÃO são keywords |
| `L04_operadores_e_pontuacao.x` | 5 — todos os operadores/pontuação de um token |
| `L05_dot_ambiguo.x` | 5 — `.` como `Dot` entre identificadores vs. início de `RealLit` |
| `L06_strings.x` | 3 — aspas simples/duplas, escape bruto não decodificado, string multilinha |
| `L07_comentarios.x` | 1 — comentário de linha e de bloco (multilinha), sem gerar token |
| `L08_sinal_adjacencia.x` | 6 — `adjacent_to_next` em `Minus`, com e sem espaço, antes de número e de identificador |
| `errors/L_ERR1_malformed_number.x` | 2/7 — inteiro fora de `i64` |
| `errors/L_ERR2_unterminated_string.x` | 3/7 — string sem aspa de fechamento |
| `errors/L_ERR3_unterminated_comment.x` | 1/7 — `/*` sem `*/` |
| `errors/L_ERR4_invalid_character.x` | 4/7 — codepoint não-ASCII fora de string/comentário |

**Aviso importante**: estes dumps foram derivados **à mão**, linha e coluna por linha e coluna,
sem um compilador Rust disponível neste ambiente para conferi-los mecanicamente (ver nota no
`AGENTS.md`/relatório sobre a ausência de `cargo` nesta máquina). O **formato** acima é a parte
normativa e deve ser seguida à risca; se ao implementar a subetapa 1–7 um fixture específico
der uma coluna diferente da esperada, siga a regra 3.3.1 do `AGENTS.md`: não ajuste o fixture para
mascarar o comportamento do lexer — primeiro re-derive a posição manualmente a partir da gramática
e, se o fixture estiver mesmo errado, reporte e corrija-o abertamente (é um bug no fixture, não uma
licença para pular a verificação).

## Script de execução

`scripts/run-lexer-tests.sh` roda a suíte de fixtures acima via `cargo test --test
lexer_fixtures -- --nocapture` e propaga o veredito OK/NOK (as linhas `[OK]`/`[FAIL]` por fixture
vêm do próprio teste Rust, que as imprime antes de falhar — ver a próxima seção). `scripts/
run-all-tests.sh` encadeia lexer → parser → milestones de codegen. Mesma regra em todas as
etapas: o script sai com status 0 **apenas** se 100% dos casos passaram.

## Critério de conclusão da etapa

A etapa Lexer só é considerada concluída quando **todos** os testes unitários por subetapa E
**todos** os fixtures golden-file acima (sucesso e erro) passarem — isto é, quando
`scripts/run-lexer-tests.sh` (ou `cargo test --test lexer_tests && cargo test --test
lexer_fixtures`) sair com status 0. Não há aprovação parcial: um único fixture falhando mantém a
etapa em aberto. Nenhum fixture pode ser alterado para acomodar um bug do lexer (AGENTS.md,
3.3.1) — bug comprovado no fixture é reportado, não silenciado.
