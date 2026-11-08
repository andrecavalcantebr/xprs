# Plano — Etapa Lexer

Fonte normativa: `xpr-specs.md`, Seção 3 (produções marcadas "detectado pelo analisador léxico")
e Seção 7 (posições em codepoints). Ver nota de arquitetura em `RELATORIO_PROGRESSO.md` (a criar)
sobre a divergência entre `especifica_o_xpr.md` e `xpr-specs.md`.

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
    pub line: u32,       // 1-based
    pub col_start: u32,  // 1-based, em codepoints
    pub col_end: u32,    // exclusivo
}

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
executor, desde que nenhum teste de uma subetapa posterior preceda a lógica que ele testa.

## Critério de conclusão da etapa

Todas as subetapas acima com testes verdes (`cargo test --test lexer_tests`), incluindo casos de
erro (`malformed-number`, string não terminada, comentário não terminado, caractere inválido) e os
exemplos literais da gramática (Seção 3) usados como fixtures.
