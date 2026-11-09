#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line_start: u32,
    pub col_start: u32,
    pub line_end: u32,
    pub col_end: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    IntLit(String),
    RealLit(String),
    StringLit { raw: String, quote: char },
    Ident(String),

    KwFn,
    KwIif,
    KwFor,
    KwTry,
    KwImport,
    KwAnd,
    KwOr,
    KwNot,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    NotEq,
    Eq,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    Dot,

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub adjacent_to_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    MalformedNumber,
    UnterminatedString,
    InvalidCharacter,
    UnterminatedComment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}
