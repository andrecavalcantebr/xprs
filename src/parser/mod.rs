//! Parser descendente recursivo da linguagem XPR (ver `MEMORY.md`).
//!
//! Aplica **apenas** regras estruturais e de precedência — nada de tipos, resolução de nomes ou
//! distinção lista/tupla. As restrições puramente sintáticas da gramática que ele **deve**
//! aplicar são: posição de `import`, forma de `const_expr` no nível de módulo, alvos de atribuição
//! válidos, `;` obrigatório em blocos, chamada com um único argumento e a regra de adjacência de
//! sinal unário sobre literal numérico.
//!
//! A compilação para no primeiro erro (Seção 7), então `parse_module` retorna no primeiro
//! [`SyntaxError`].

pub mod ast;
pub mod dump;

pub use ast::*;

use crate::lexer::{Span, Token, TokenKind};

// -------------------------------------------------------------------------------------------
// Erros
// -------------------------------------------------------------------------------------------

/// Enum **fechado** de erros de sintaxe. Os testes golden-file comparam `kind` + `span`, nunca a
/// mensagem (`detail`), que pode evoluir livremente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    /// Token inesperado em uma posição onde a gramática espera outra coisa.
    UnexpectedToken,
    /// Faltou o `;` que encerra uma definição ou um statement de bloco.
    MissingSemicolon,
    /// Alvo de atribuição não é `binding_pattern` (ex.: expressão indexada).
    InvalidAssignmentTarget,
    /// `f(a, b)` — funções XPR têm um único argumento (Seção 4.5).
    MultiArgCall,
    /// `import` fora das primeiras posições permitidas do módulo (Seção 4.1).
    ImportNotAtModuleStart,
    /// `fn_call`/`block`/`iif`/`for`/atribuição/`fn_def` dentro de um `const_expr` de topo.
    InvalidConstExpr,
    /// Sinal unário separado por espaço/comentário do literal numérico que ele precede.
    SignNotAdjacent,
    /// Código fonte sem nenhuma definição (Seção 7).
    EmptyProgram,
}

impl SyntaxErrorKind {
    /// Nome da variante, como aparece nos dumps `.error`.
    pub fn name(self) -> &'static str {
        match self {
            SyntaxErrorKind::UnexpectedToken => "UnexpectedToken",
            SyntaxErrorKind::MissingSemicolon => "MissingSemicolon",
            SyntaxErrorKind::InvalidAssignmentTarget => "InvalidAssignmentTarget",
            SyntaxErrorKind::MultiArgCall => "MultiArgCall",
            SyntaxErrorKind::ImportNotAtModuleStart => "ImportNotAtModuleStart",
            SyntaxErrorKind::InvalidConstExpr => "InvalidConstExpr",
            SyntaxErrorKind::SignNotAdjacent => "SignNotAdjacent",
            SyntaxErrorKind::EmptyProgram => "EmptyProgram",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub kind: SyntaxErrorKind,
    pub span: Span,
    /// Texto livre para humanos; **não** é comparado pelos testes golden-file.
    pub detail: String,
}

// -------------------------------------------------------------------------------------------
// Parser
// -------------------------------------------------------------------------------------------

pub struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
}

/// Token usado como sentinela quando o parser recebe uma slice vazia.
const EMPTY_TOKEN: Token = Token {
    kind: TokenKind::Eof,
    span: Span {
        line_start: 1,
        col_start: 1,
        line_end: 1,
        col_end: 1,
    },
    adjacent_to_next: false,
};

/// Junta dois spans em um que cobre ambos (assume ordem de leitura).
fn join(start: Span, end: Span) -> Span {
    Span {
        line_start: start.line_start,
        col_start: start.col_start,
        line_end: end.line_end,
        col_end: end.col_end,
    }
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapa A — infraestrutura do cursor
    // ---------------------------------------------------------------------------------------

    fn cur(&self) -> &Token {
        // `tokenize` garante um `Eof` no fim; o índice nunca o ultrapassa. A slice vazia (nunca
        // produzida pela pipeline real, mas possível via `Parser::new(&[])`) é tratada como um
        // programa vazio em 1:1, em vez de estourar o índice.
        if self.tokens.is_empty() {
            return &EMPTY_TOKEN;
        }
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn at_eof(&self) -> bool {
        matches!(self.cur().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if self.tokens.is_empty() {
            return &EMPTY_TOKEN;
        }
        let index = self.pos.min(self.tokens.len() - 1);
        if !matches!(self.tokens[index].kind, TokenKind::Eof) {
            self.pos += 1;
        }
        &self.tokens[index]
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if &self.cur().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error(&self, kind: SyntaxErrorKind, detail: impl Into<String>) -> SyntaxError {
        self.error_at(kind, self.cur().span, detail)
    }

    fn error_at(
        &self,
        kind: SyntaxErrorKind,
        span: Span,
        detail: impl Into<String>,
    ) -> SyntaxError {
        SyntaxError {
            kind,
            span,
            detail: detail.into(),
        }
    }

    fn unexpected(&self, expected: &str) -> SyntaxError {
        self.error(
            SyntaxErrorKind::UnexpectedToken,
            format!("esperava {}, encontrou {:?}", expected, self.cur().kind),
        )
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<Span, SyntaxError> {
        if self.cur().kind == kind {
            let span = self.cur().span;
            self.advance();
            Ok(span)
        } else {
            Err(self.unexpected(expected))
        }
    }

    fn expect_semi(&mut self) -> Result<Span, SyntaxError> {
        if matches!(self.cur().kind, TokenKind::Semi) {
            let span = self.cur().span;
            self.advance();
            Ok(span)
        } else {
            Err(self.error(
                SyntaxErrorKind::MissingSemicolon,
                format!(
                    "esperava ';' ao final da instrução, encontrou {:?}",
                    self.cur().kind
                ),
            ))
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapas I/J — nível de módulo
    // ---------------------------------------------------------------------------------------

    pub fn parse_module(&mut self) -> Result<Module, SyntaxError> {
        let mut imports = Vec::new();
        while matches!(self.cur().kind, TokenKind::KwImport) {
            imports.push(self.parse_import_decl()?);
            self.expect_semi()?;
        }

        let mut definitions = Vec::new();
        loop {
            if self.at_eof() {
                break;
            }
            match self.cur().kind {
                TokenKind::KwImport => {
                    return Err(self.error(
                        SyntaxErrorKind::ImportNotAtModuleStart,
                        "import deve vir antes de qualquer definição do módulo",
                    ))
                }
                _ => {
                    definitions.push(self.parse_definition()?);
                    self.expect_semi()?;
                }
            }
        }

        if definitions.is_empty() {
            let span = self.cur().span;
            return Err(self.error_at(
                SyntaxErrorKind::EmptyProgram,
                span,
                "o programa não contém nenhuma definição",
            ));
        }

        Ok(Module {
            imports,
            definitions,
        })
    }

    fn parse_import_decl(&mut self) -> Result<ImportDecl, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `import`
        self.expect(TokenKind::LParen, "'(' após import")?;
        let (name, _, _) = self.parse_string_literal()?;
        let alias = if self.eat(&TokenKind::Comma) {
            let (alias, _, _) = self.parse_string_literal()?;
            Some(alias)
        } else {
            None
        };
        let end = self.expect(TokenKind::RParen, "')' ao final do import")?;
        Ok(ImportDecl {
            name,
            alias,
            span: join(start, end),
        })
    }

    fn parse_definition(&mut self) -> Result<Definition, SyntaxError> {
        let name_span = self.cur().span;
        let name = match self.cur().kind.clone() {
            TokenKind::Ident(name) => name,
            _ => return Err(self.unexpected("um identificador no início da definição")),
        };
        self.advance();
        self.expect(TokenKind::Eq, "'=' após o nome da definição")?;

        let expr = self.parse_expression()?;
        let end = expr.span();
        let initializer = match expr {
            Expr::FnDef(fn_def) => Initializer::FnDef(*fn_def),
            other => {
                self.validate_const_expr(&other)?;
                Initializer::ConstExpr(other)
            }
        };
        Ok(Definition {
            name,
            initializer,
            span: join(name_span, end),
        })
    }

    /// Subetapa I: `const_expr` não pode conter `fn_call`, `block`, `iif`, `for`, atribuição nem
    /// `fn_def`. O erro aponta para o primeiro nó ofensor (em ordem de percurso).
    fn validate_const_expr(&self, expr: &Expr) -> Result<(), SyntaxError> {
        match expr {
            Expr::FnCall { .. }
            | Expr::Block(_)
            | Expr::Iif { .. }
            | Expr::For { .. }
            | Expr::Assignment { .. }
            | Expr::FnDef(_) => {
                return Err(self.error_at(
                    SyntaxErrorKind::InvalidConstExpr,
                    expr.span(),
                    "esta forma não é permitida em uma expressão constante",
                ))
            }
            _ => {}
        }

        match expr {
            Expr::LogicalOr(operands) | Expr::LogicalAnd(operands) | Expr::BracketLiteral(operands, _) => {
                for operand in operands {
                    self.validate_const_expr(operand)?;
                }
            }
            Expr::LogicalNot(operand, _) | Expr::Unary { operand, .. } | Expr::Try(operand, _) => {
                self.validate_const_expr(operand)?;
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.validate_const_expr(lhs)?;
                self.validate_const_expr(rhs)?;
            }
            Expr::Indexed { base, indices, .. } => {
                self.validate_const_expr(base)?;
                for index in indices {
                    self.validate_const_expr(index)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------------
    // Expressões (subetapas B a H)
    // ---------------------------------------------------------------------------------------

    pub fn parse_expression(&mut self) -> Result<Expr, SyntaxError> {
        // `expression = assignment | fn_def | logical_or | block`; `fn_def` é o único caso que não
        // começa por um operando, então é desviado aqui.
        if matches!(self.cur().kind, TokenKind::KwFn) {
            let fn_def = self.parse_fn_def()?;
            return Ok(Expr::FnDef(Box::new(fn_def)));
        }
        self.parse_assignment()
    }

    /// Subetapa F — `assignment = binding_pattern "=" expression`, associativa à direita.
    fn parse_assignment(&mut self) -> Result<Expr, SyntaxError> {
        let start = self.cur().span;
        let lhs = self.parse_logical_or()?;
        if !matches!(self.cur().kind, TokenKind::Eq) {
            return Ok(lhs);
        }
        let eq_span = self.cur().span;
        let target = match self.expr_to_binding_pattern(&lhs) {
            Some(pattern) => pattern,
            None => {
                return Err(self.error_at(
                    SyntaxErrorKind::InvalidAssignmentTarget,
                    eq_span,
                    "o alvo de uma atribuição deve ser um identificador ou um padrão de lista",
                ))
            }
        };
        self.advance(); // `=`
        let value = self.parse_expression()?;
        let span = join(start, value.span());
        Ok(Expr::Assignment {
            target,
            value: Box::new(value),
            span,
        })
    }

    /// Converte a expressão já parseada em padrão de ligação, se a forma for válida
    /// (identificador simples ou lista recursiva de padrões).
    fn expr_to_binding_pattern(&self, expr: &Expr) -> Option<BindingPattern> {
        match expr {
            Expr::QualifiedName {
                module_alias: None,
                name,
                span,
            } => Some(BindingPattern::Name(name.clone(), *span)),
            Expr::BracketLiteral(items, span) => {
                let mut patterns = Vec::with_capacity(items.len());
                for item in items {
                    patterns.push(self.expr_to_binding_pattern(item)?);
                }
                Some(BindingPattern::List(patterns, *span))
            }
            _ => None,
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, SyntaxError> {
        let first = self.parse_logical_and()?;
        if !matches!(self.cur().kind, TokenKind::KwOr) {
            return Ok(first);
        }
        let mut operands = vec![first];
        while matches!(self.cur().kind, TokenKind::KwOr) {
            self.advance();
            operands.push(self.parse_logical_and()?);
        }
        Ok(Expr::LogicalOr(operands))
    }

    fn parse_logical_and(&mut self) -> Result<Expr, SyntaxError> {
        let first = self.parse_logical_not()?;
        if !matches!(self.cur().kind, TokenKind::KwAnd) {
            return Ok(first);
        }
        let mut operands = vec![first];
        while matches!(self.cur().kind, TokenKind::KwAnd) {
            self.advance();
            operands.push(self.parse_logical_not()?);
        }
        Ok(Expr::LogicalAnd(operands))
    }

    fn parse_logical_not(&mut self) -> Result<Expr, SyntaxError> {
        if matches!(self.cur().kind, TokenKind::KwNot) {
            let start = self.cur().span;
            self.advance();
            let operand = self.parse_logical_not()?;
            let span = join(start, operand.span());
            return Ok(Expr::LogicalNot(Box::new(operand), span));
        }
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> Result<Expr, SyntaxError> {
        let mut lhs = self.parse_relational()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::EqEq => BinOp::EqEq,
                TokenKind::NotEq => BinOp::NotEq,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_relational()?;
            let span = join(lhs.span(), rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_relational(&mut self) -> Result<Expr, SyntaxError> {
        let mut lhs = self.parse_arithmetic()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Ge => BinOp::Ge,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_arithmetic()?;
            let span = join(lhs.span(), rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_arithmetic(&mut self) -> Result<Expr, SyntaxError> {
        let mut lhs = self.parse_term()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_term()?;
            let span = join(lhs.span(), rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    fn parse_term(&mut self) -> Result<Expr, SyntaxError> {
        let mut lhs = self.parse_factor()?;
        loop {
            let op = match self.cur().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.parse_factor()?;
            let span = join(lhs.span(), rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
    }

    /// Subetapa H — `factor = [ "+" | "-" ] indexed`, com a regra de adjacência:
    /// quando o sinal precede diretamente um literal numérico, precisa estar colado a ele
    /// (Seção 4, "Sinais numéricos unários e espaços em branco").
    fn parse_factor(&mut self) -> Result<Expr, SyntaxError> {
        let op = match self.cur().kind {
            TokenKind::Plus => UnOp::Plus,
            TokenKind::Minus => UnOp::Minus,
            _ => return self.parse_indexed(),
        };
        let sign = self.cur().clone();
        let starts_with_number = matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.kind),
            Some(TokenKind::IntLit(_)) | Some(TokenKind::RealLit(_))
        );
        if starts_with_number && !sign.adjacent_to_next {
            return Err(self.error_at(
                SyntaxErrorKind::SignNotAdjacent,
                sign.span,
                "o sinal de um literal numérico deve estar diretamente adjacente a ele",
            ));
        }
        self.advance();
        let operand = self.parse_indexed()?;
        let span = join(sign.span, operand.span());
        Ok(Expr::Unary {
            op,
            operand: Box::new(operand),
            span,
            sign_adjacent: sign.adjacent_to_next,
        })
    }

    /// Subetapa D — `indexed = primary { "[" expression { "," expression } "]" }`.
    /// `v[a, b]` vira **um** `Indexed` com dois índices (o açúcar é resolvido no lowering).
    fn parse_indexed(&mut self) -> Result<Expr, SyntaxError> {
        let mut base = self.parse_primary()?;
        while matches!(self.cur().kind, TokenKind::LBracket) {
            self.advance();
            let mut indices = vec![self.parse_expression()?];
            while self.eat(&TokenKind::Comma) {
                indices.push(self.parse_expression()?);
            }
            let end = self.expect(TokenKind::RBracket, "']' ao final da indexação")?;
            let span = join(base.span(), end);
            base = Expr::Indexed {
                base: Box::new(base),
                indices,
                span,
            };
        }
        Ok(base)
    }

    /// Subetapa C — `primary` (e, portanto, `fn_call` e `qualified_name`).
    fn parse_primary(&mut self) -> Result<Expr, SyntaxError> {
        match self.cur().kind.clone() {
            TokenKind::IntLit(text) => {
                let span = self.cur().span;
                self.advance();
                Ok(Expr::IntLit(text, span))
            }
            TokenKind::RealLit(text) => {
                let span = self.cur().span;
                self.advance();
                Ok(Expr::RealLit(text, span))
            }
            TokenKind::StringLit { .. } => {
                let (decoded, quote, span) = self.parse_string_literal()?;
                Ok(Expr::StringLit(decoded, quote, span))
            }
            TokenKind::Ident(name) => {
                let start = self.cur().span;
                self.advance();
                let (module_alias, name, name_span) = if matches!(self.cur().kind, TokenKind::Dot) {
                    self.advance();
                    match self.cur().kind.clone() {
                        TokenKind::Ident(second) => {
                            let span = self.cur().span;
                            self.advance();
                            (Some(name), second, join(start, span))
                        }
                        _ => return Err(self.unexpected("um identificador após '.'")),
                    }
                } else {
                    (None, name, start)
                };
                let qualified = Expr::QualifiedName {
                    module_alias,
                    name,
                    span: name_span,
                };
                // `fn_call = qualified_name "(" [ expression ] ")"`
                if matches!(self.cur().kind, TokenKind::LParen) {
                    self.advance();
                    let arg = if matches!(self.cur().kind, TokenKind::RParen) {
                        None
                    } else {
                        let arg = self.parse_expression()?;
                        if matches!(self.cur().kind, TokenKind::Comma) {
                            let span = self.cur().span;
                            return Err(self.error_at(
                                SyntaxErrorKind::MultiArgCall,
                                span,
                                "funções XPR recebem um único argumento",
                            ));
                        }
                        Some(Box::new(arg))
                    };
                    let end = self.expect(TokenKind::RParen, "')' ao final da chamada")?;
                    let span = join(start, end);
                    return Ok(Expr::FnCall {
                        target: Box::new(qualified),
                        arg,
                        span,
                    });
                }
                Ok(qualified)
            }
            TokenKind::KwIif => self.parse_iif(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwTry => self.parse_try(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            TokenKind::LBracket => self.parse_bracket_literal(),
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expression()?;
                self.expect(TokenKind::RParen, "')' ao final da expressão entre parênteses")?;
                // Sem nó `Paren`: a forma da árvore já representa o agrupamento.
                Ok(inner)
            }
            _ => Err(self.unexpected("o início de uma expressão")),
        }
    }

    fn parse_iif(&mut self) -> Result<Expr, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `iif`
        self.expect(TokenKind::LParen, "'(' após iif")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::Comma, "',' entre os argumentos de iif")?;
        let then_branch = self.parse_expression()?;
        self.expect(TokenKind::Comma, "',' entre os argumentos de iif")?;
        let else_branch = self.parse_expression()?;
        let end = self.expect(TokenKind::RParen, "')' ao final de iif")?;
        Ok(Expr::Iif {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            span: join(start, end),
        })
    }

    fn parse_for(&mut self) -> Result<Expr, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `for`
        self.expect(TokenKind::LParen, "'(' após for")?;
        let init = self.parse_expression()?;
        self.expect(TokenKind::Semi, "';' após o init de for")?;
        let cond = self.parse_expression()?;
        self.expect(TokenKind::Semi, "';' após a condição de for")?;
        let step = self.parse_expression()?;
        self.expect(TokenKind::RParen, "')' após o passo de for")?;
        let body = self.parse_expression()?;
        let span = join(start, body.span());
        Ok(Expr::For {
            init: Box::new(init),
            cond: Box::new(cond),
            step: Box::new(step),
            body: Box::new(body),
            span,
        })
    }

    fn parse_try(&mut self) -> Result<Expr, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `try`
        self.expect(TokenKind::LParen, "'(' após try")?;
        let inner = self.parse_expression()?;
        let end = self.expect(TokenKind::RParen, "')' ao final de try")?;
        Ok(Expr::Try(Box::new(inner), join(start, end)))
    }

    fn parse_bracket_literal(&mut self) -> Result<Expr, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `[`
        let mut elements = Vec::new();
        if !matches!(self.cur().kind, TokenKind::RBracket) {
            elements.push(self.parse_expression()?);
            while self.eat(&TokenKind::Comma) {
                elements.push(self.parse_expression()?);
            }
        }
        let end = self.expect(TokenKind::RBracket, "']' ao final da lista")?;
        Ok(Expr::BracketLiteral(elements, join(start, end)))
    }

    fn parse_string_literal(&mut self) -> Result<(String, char, Span), SyntaxError> {
        match self.cur().kind.clone() {
            TokenKind::StringLit { raw, quote } => {
                let span = self.cur().span;
                self.advance();
                Ok((decode_escapes(&raw), quote, span))
            }
            _ => Err(self.unexpected("um literal de string")),
        }
    }

    // ---------------------------------------------------------------------------------------
    // Subetapas E/G — padrões de parâmetro, tipos e blocos
    // ---------------------------------------------------------------------------------------

    /// `fn_def = "fn" "(" param_pattern ")" block`
    fn parse_fn_def(&mut self) -> Result<FnDef, SyntaxError> {
        let start = self.cur().span;
        self.advance(); // `fn`
        self.expect(TokenKind::LParen, "'(' após fn")?;
        let param = self.parse_param_pattern()?;
        self.expect(TokenKind::RParen, "')' ao final dos parâmetros de fn")?;
        let body = self.parse_block()?;
        let span = join(start, body.span);
        Ok(FnDef { param, body, span })
    }

    /// `param_pattern = ( identifier [ ":" type ] ) | ( "[" [ param_pattern {"," param_pattern} ] "]" )`
    fn parse_param_pattern(&mut self) -> Result<ParamPattern, SyntaxError> {
        if matches!(self.cur().kind, TokenKind::LBracket) {
            let start = self.cur().span;
            self.advance();
            let mut items = Vec::new();
            if !matches!(self.cur().kind, TokenKind::RBracket) {
                items.push(self.parse_param_pattern()?);
                while self.eat(&TokenKind::Comma) {
                    items.push(self.parse_param_pattern()?);
                }
            }
            let end = self.expect(TokenKind::RBracket, "']' ao final do padrão de parâmetro")?;
            return Ok(ParamPattern::List(items, join(start, end)));
        }

        let start = self.cur().span;
        let name = match self.cur().kind.clone() {
            TokenKind::Ident(name) => name,
            _ => return Err(self.unexpected("um identificador ou '[' no padrão de parâmetro")),
        };
        self.advance();
        let (ty, end) = if matches!(self.cur().kind, TokenKind::Colon) {
            self.advance();
            let (ty, span) = self.parse_type()?;
            (Some(ty), span)
        } else {
            (None, start)
        };
        Ok(ParamPattern::Name {
            name,
            ty,
            span: join(start, end),
        })
    }

    /// `type = "bool" | "int" | "real" | "string" | "fn" | "[]" | "[" type "]" | "[" type,... "]"`
    /// Nomes de tipo só são reconhecidos após `:` (não são palavras reservadas).
    fn parse_type(&mut self) -> Result<(TypeAnnotation, Span), SyntaxError> {
        if matches!(self.cur().kind, TokenKind::LBracket) {
            let start = self.cur().span;
            self.advance();
            if matches!(self.cur().kind, TokenKind::RBracket) {
                let end = self.cur().span;
                self.advance();
                return Ok((TypeAnnotation::AnyList, join(start, end)));
            }
            let (first, _) = self.parse_type()?;
            if self.eat(&TokenKind::Comma) {
                let mut items = vec![first];
                loop {
                    let (item, _) = self.parse_type()?;
                    items.push(item);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RBracket, "']' ao final da tupla de tipos")?;
                return Ok((TypeAnnotation::Tuple(items), join(start, end)));
            }
            let end = self.expect(TokenKind::RBracket, "']' ao final do tipo de lista")?;
            return Ok((
                TypeAnnotation::ListOf(Box::new(first)),
                join(start, end),
            ));
        }

        let span = self.cur().span;
        let ty = match self.cur().kind.clone() {
            TokenKind::KwFn => TypeAnnotation::Fn,
            TokenKind::Ident(name) => match name.as_str() {
                "bool" => TypeAnnotation::Bool,
                "int" => TypeAnnotation::Int,
                "real" => TypeAnnotation::Real,
                "string" => TypeAnnotation::String,
                _ => {
                    return Err(self.error(
                        SyntaxErrorKind::UnexpectedToken,
                        format!("nome de tipo desconhecido: {}", name),
                    ))
                }
            },
            _ => return Err(self.unexpected("um nome de tipo")),
        };
        self.advance();
        Ok((ty, span))
    }

    /// `block = "{" { statement } "}"`, cada `statement` terminado em `;`.
    fn parse_block(&mut self) -> Result<Block, SyntaxError> {
        let start = self.expect(TokenKind::LBrace, "'{' ao início do bloco")?;
        let mut statements = Vec::new();
        loop {
            match self.cur().kind.clone() {
                TokenKind::RBrace => {
                    let end = self.cur().span;
                    self.advance();
                    return Ok(Block {
                        statements,
                        span: join(start, end),
                    });
                }
                TokenKind::Eof => return Err(self.unexpected("'}' fechando o bloco")),
                TokenKind::KwImport => {
                    // `import` como statement só é aceito no início do bloco mais externo do
                    // programa; aqui, o nível de módulo é o único lugar permitido.
                    return Err(self.error(
                        SyntaxErrorKind::ImportNotAtModuleStart,
                        "import só é permitido nas primeiras linhas do módulo",
                    ));
                }
                _ => {
                    let expr = self.parse_expression()?;
                    statements.push(Stmt::Expr(expr));
                    self.expect_semi()?;
                }
            }
        }
    }
}

/// Decodifica as sequências de escape aceitas pela Seção 3 (`\n \t \r \0 \" \' \\`).
///
/// Um escape fora desse conjunto é mantido literalmente (barra invertida + codepoint). Essa
/// escolha é tolerante de propósito: a gramática não define erro léxico para escapes inválidos e
/// a Seção 7 também não prevê um diagnóstico específico; a decisão fica registrada no relatório da
/// etapa.
pub fn decode_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
