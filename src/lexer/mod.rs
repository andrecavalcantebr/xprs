//! Analisador léxico da linguagem XPR (ver `plano-lexer.md` e `xpr-specs.md`, Seção 3).
//!
//! Responsabilidades (e apenas elas):
//! * transformar o texto-fonte UTF-8 em uma sequência de [`Token`] com [`Span`] em
//!   **codepoints**;
//! * ignorar espaços em branco e comentários (`//`, `/* */`);
//! * delimitar strings **sem decodificar nem validar** escapes (decisão da spec);
//! * validar o intervalo de `int_lit` (fora de `i64` → [`LexErrorKind::MalformedNumber`]);
//! * marcar `+`/`-` com [`Token::adjacent_to_next`] — a decisão unário/binário é do parser.

pub mod token;
pub use token::*;

use std::marker::PhantomData;

/// Analisador léxico sobre uma fonte emprestada.
///
/// `chars` é a fonte já decodificada em codepoints: iterar por `char` (e não por byte) é o que
/// garante que as colunas contem codepoints, como a Seção 7 da spec exige.
pub struct Lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
    _src: PhantomData<&'a str>,
}

/// Token interno com os deslocamentos brutos (em codepoints) usados só para decidir
/// `adjacent_to_next` — o que interessa ao parser é [`Token`].
#[derive(Debug, Clone)]
struct RawToken {
    kind: TokenKind,
    span: Span,
    start: usize,
    end: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            _src: PhantomData,
        }
    }

    /// Tokeniza a fonte inteira. Retorna `Err` no **primeiro** problema léxico (a compilação para
    /// no primeiro erro, Seção 7); os tokens já produzidos são descartados.
    ///
    /// Em caso de sucesso a sequência **sempre** termina com [`TokenKind::Eof`].
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut raws: Vec<RawToken> = Vec::new();
        loop {
            self.skip_whitespace_and_comments()?;
            if self.pos >= self.chars.len() {
                let span = self.span_from(self.line, self.col);
                raws.push(RawToken {
                    kind: TokenKind::Eof,
                    span,
                    start: self.pos,
                    end: self.pos,
                });
                break;
            }
            let token = self.scan_token()?;
            raws.push(token);
        }
        Ok(finish(raws))
    }

    // ---------------------------------------------------------------------------------------
    // Navegação por codepoints
    // ---------------------------------------------------------------------------------------

    fn cur(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    /// Consome um codepoint, atualizando linha/coluna. `\n` reinicia a coluna em 1; qualquer
    /// outro codepoint (inclusive `\r`) avança a coluna em 1.
    fn bump(&mut self) -> Option<char> {
        let c = self.cur()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    /// Span do token que começou em (`line_start`, `col_start`) até a posição atual.
    fn span_from(&self, line_start: u32, col_start: u32) -> Span {
        Span {
            line_start,
            col_start,
            line_end: self.line,
            col_end: self.col,
        }
    }

    fn error(&self, kind: LexErrorKind, line_start: u32, col_start: u32) -> LexError {
        LexError {
            kind,
            span: self.span_from(line_start, col_start),
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa 1 — espaços em branco e comentários
    // ---------------------------------------------------------------------------------------

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.cur() {
                Some(' ') | Some('\t') | Some('\n') | Some('\r') => {
                    self.bump();
                }
                Some('/') if self.peek(1) == Some('/') => {
                    // `line_comment = "//" { chars } ( "\n" | EOF )` — o `\n` não é consumido
                    // aqui; a volta seguinte do laço o trata como espaço em branco.
                    self.bump();
                    self.bump();
                    while let Some(c) = self.cur() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek(1) == Some('*') => {
                    // `multiline_comment = "/*" { chars } "*/"` — não aninha.
                    let (line_start, col_start) = (self.line, self.col);
                    self.bump();
                    self.bump();
                    let mut closed = false;
                    while self.pos < self.chars.len() {
                        if self.cur() == Some('*') && self.peek(1) == Some('/') {
                            self.bump();
                            self.bump();
                            closed = true;
                            break;
                        }
                        self.bump();
                    }
                    if !closed {
                        return Err(self.error(
                            LexErrorKind::UnterminatedComment,
                            line_start,
                            col_start,
                        ));
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Despacho por primeiro codepoint
    // ---------------------------------------------------------------------------------------

    fn scan_token(&mut self) -> Result<RawToken, LexError> {
        let start = self.pos;
        let (line_start, col_start) = (self.line, self.col);
        let c = self.cur().expect("scan_token chamado sem entrada restante");

        match c {
            '"' | '\'' => self.scan_string(c, start, line_start, col_start),
            '0'..='9' => self.scan_number_from_digits(start, line_start, col_start),
            '.' => {
                if self.peek(1).map_or(false, |n| n.is_ascii_digit()) {
                    self.scan_real_from_dot(start, line_start, col_start)
                } else {
                    self.bump();
                    Ok(self.simple(TokenKind::Dot, start, line_start, col_start))
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(start, line_start, col_start),
            _ => self.scan_operator_or_punct(c, start, line_start, col_start),
        }
    }

    fn simple(&self, kind: TokenKind, start: usize, line_start: u32, col_start: u32) -> RawToken {
        RawToken {
            kind,
            span: self.span_from(line_start, col_start),
            start,
            end: self.pos,
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa 3 — strings (delimitação apenas; conteúdo bruto, sem decodificação)
    // ---------------------------------------------------------------------------------------

    fn scan_string(
        &mut self,
        quote: char,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        self.bump(); // aspas de abertura
        let mut raw = String::new();
        loop {
            match self.cur() {
                None => {
                    return Err(self.error(LexErrorKind::UnterminatedString, line_start, col_start));
                }
                Some(c) if c == quote => {
                    self.bump();
                    return Ok(self.simple(
                        TokenKind::StringLit { raw, quote },
                        start,
                        line_start,
                        col_start,
                    ));
                }
                Some('\\') => {
                    // Escapes NÃO são validados aqui (`\` + qualquer codepoint é apenas texto
                    // bruto), mas precisam ser pulados: o codepoint após `\` não pode ser a aspa
                    // delimitadora, por definição de `chars` na gramática.
                    raw.push('\\');
                    self.bump();
                    match self.cur() {
                        Some(escaped) => {
                            raw.push(escaped);
                            self.bump();
                        }
                        None => {
                            return Err(self.error(
                                LexErrorKind::UnterminatedString,
                                line_start,
                                col_start,
                            ))
                        }
                    }
                }
                Some(c) => {
                    raw.push(c);
                    self.bump();
                }
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa 2 — números
    // ---------------------------------------------------------------------------------------

    /// `int_num ( "." { digit } [ exp_part ] | exp_part )` ou `int_num` sozinho.
    fn scan_number_from_digits(
        &mut self,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        let mut text = String::new();
        while let Some(c) = self.cur() {
            if c.is_ascii_digit() {
                text.push(c);
                self.bump();
            } else {
                break;
            }
        }

        let begins_real = match self.cur() {
            Some('.') => true,
            // `3e` sem dígitos não forma `exp_part`: o `e` pertence a um identificador
            // seguinte, então o número termina aqui.
            Some('e') | Some('E') => self.exponent_ahead(),
            _ => false,
        };

        if !begins_real {
            if text.parse::<i64>().is_err() {
                return Err(self.error(LexErrorKind::MalformedNumber, line_start, col_start));
            }
            return Ok(self.simple(TokenKind::IntLit(text), start, line_start, col_start));
        }

        if self.cur() == Some('.') {
            text.push('.');
            self.bump();
            while let Some(c) = self.cur() {
                if c.is_ascii_digit() {
                    text.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.consume_exponent(&mut text);

        Ok(self.simple(TokenKind::RealLit(text), start, line_start, col_start))
    }

    /// `.` `int_num` `[ exp_part ]` — forma iniciada por ponto.
    fn scan_real_from_dot(
        &mut self,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        let mut text = String::from(".");
        self.bump();
        while let Some(c) = self.cur() {
            if c.is_ascii_digit() {
                text.push(c);
                self.bump();
            } else {
                break;
            }
        }
        self.consume_exponent(&mut text);

        Ok(self.simple(TokenKind::RealLit(text), start, line_start, col_start))
    }

    /// `true` se a posição atual inicia um `exp_part` completo (`e`/`E` `[+|-]` `digit`).
    /// Puro lookahead: não consome nada.
    fn exponent_ahead(&self) -> bool {
        let mut i = self.pos + 1;
        if matches!(self.chars.get(i), Some('+') | Some('-')) {
            i += 1;
        }
        self.chars.get(i).map_or(false, |c| c.is_ascii_digit())
    }

    /// Consome `[ exp_part ]` a partir da posição atual, acrescentando o texto bruto.
    fn consume_exponent(&mut self, text: &mut String) {
        if !matches!(self.cur(), Some('e') | Some('E')) || !self.exponent_ahead() {
            return;
        }
        text.push(self.bump().unwrap()); // e | E
        if matches!(self.cur(), Some('+') | Some('-')) {
            text.push(self.bump().unwrap());
        }
        while let Some(c) = self.cur() {
            if c.is_ascii_digit() {
                text.push(c);
                self.bump();
            } else {
                break;
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa 4 — identificadores e palavras-chave
    // ---------------------------------------------------------------------------------------

    fn scan_identifier(
        &mut self,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        let mut name = String::new();
        while let Some(c) = self.cur() {
            if c.is_ascii_alphanumeric() || c == '_' {
                name.push(c);
                self.bump();
            } else if c.is_ascii() {
                break;
            } else {
                // Não-ASCII nunca é identificador (spec, Seção 3): só strings e comentários.
                // O span do erro cobre apenas o codepoint ofensor, não o identificador inteiro.
                let (bad_line, bad_col) = (self.line, self.col);
                self.bump();
                return Err(self.error(LexErrorKind::InvalidCharacter, bad_line, bad_col));
            }
        }
        let kind = keyword(&name).unwrap_or(TokenKind::Ident(name));
        Ok(self.simple(kind, start, line_start, col_start))
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa 5 — operadores e pontuação
    // ---------------------------------------------------------------------------------------

    fn scan_operator_or_punct(
        &mut self,
        c: char,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        match c {
            '+' => self.punct1(TokenKind::Plus, start, line_start, col_start),
            '-' => self.punct1(TokenKind::Minus, start, line_start, col_start),
            '*' => self.punct1(TokenKind::Star, start, line_start, col_start),
            '/' => self.punct1(TokenKind::Slash, start, line_start, col_start),
            '%' => self.punct1(TokenKind::Percent, start, line_start, col_start),
            '(' => self.punct1(TokenKind::LParen, start, line_start, col_start),
            ')' => self.punct1(TokenKind::RParen, start, line_start, col_start),
            '{' => self.punct1(TokenKind::LBrace, start, line_start, col_start),
            '}' => self.punct1(TokenKind::RBrace, start, line_start, col_start),
            '[' => self.punct1(TokenKind::LBracket, start, line_start, col_start),
            ']' => self.punct1(TokenKind::RBracket, start, line_start, col_start),
            ',' => self.punct1(TokenKind::Comma, start, line_start, col_start),
            ';' => self.punct1(TokenKind::Semi, start, line_start, col_start),
            ':' => self.punct1(TokenKind::Colon, start, line_start, col_start),
            '=' if self.peek(1) == Some('=') => {
                self.punct2(TokenKind::EqEq, start, line_start, col_start)
            }
            '<' if self.peek(1) == Some('=') => {
                self.punct2(TokenKind::Le, start, line_start, col_start)
            }
            '>' if self.peek(1) == Some('=') => {
                self.punct2(TokenKind::Ge, start, line_start, col_start)
            }
            '!' if self.peek(1) == Some('=') => {
                self.punct2(TokenKind::NotEq, start, line_start, col_start)
            }
            '=' => self.punct1(TokenKind::Eq, start, line_start, col_start),
            '<' => self.punct1(TokenKind::Lt, start, line_start, col_start),
            '>' => self.punct1(TokenKind::Gt, start, line_start, col_start),
            _ => {
                // `!` isolado, `&`, `|`, `^`, `~` (reservados, sem operação definida), `@`,
                // `#`, `$`, ... e todo não-ASCII fora de string/comentário.
                self.bump();
                Err(self.error(LexErrorKind::InvalidCharacter, line_start, col_start))
            }
        }
    }

    fn punct1(
        &mut self,
        kind: TokenKind,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        self.bump();
        Ok(self.simple(kind, start, line_start, col_start))
    }

    fn punct2(
        &mut self,
        kind: TokenKind,
        start: usize,
        line_start: u32,
        col_start: u32,
    ) -> Result<RawToken, LexError> {
        self.bump();
        self.bump();
        Ok(self.simple(kind, start, line_start, col_start))
    }
}

/// Tabela fixa das 8 palavras-chave (Seção 3). `true`/`false`/`bool`/`int`/`real`/`string`
/// **não** estão aqui: são identificadores resolvidos em etapas posteriores.
fn keyword(name: &str) -> Option<TokenKind> {
    Some(match name {
        "fn" => TokenKind::KwFn,
        "iif" => TokenKind::KwIif,
        "for" => TokenKind::KwFor,
        "try" => TokenKind::KwTry,
        "import" => TokenKind::KwImport,
        "and" => TokenKind::KwAnd,
        "or" => TokenKind::KwOr,
        "not" => TokenKind::KwNot,
        _ => return None,
    })
}

/// Converte os tokens internos em tokens públicos e calcula `adjacent_to_next`
/// (subetapa 6): um `+`/`-` é adjacente quando o próximo token começa exatamente onde ele
/// termina — sem espaço, nova linha ou comentário no meio.
fn finish(raws: Vec<RawToken>) -> Vec<Token> {
    let count = raws.len();
    let mut tokens = Vec::with_capacity(count);
    for i in 0..count {
        let is_sign = matches!(raws[i].kind, TokenKind::Plus | TokenKind::Minus);
        let adjacent_to_next = is_sign
            && i + 1 < count
            && !matches!(raws[i + 1].kind, TokenKind::Eof)
            && raws[i + 1].start == raws[i].end;
        tokens.push(Token {
            kind: raws[i].kind.clone(),
            span: raws[i].span,
            adjacent_to_next,
        });
    }
    tokens
}
