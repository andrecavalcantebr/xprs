//! Inferência e checagem estática de tipos (subetapa C de `plano-codegen.md`; `xpr-specs.md`,
//! Seções 2, 4.7, 4.10 e 5.2).
//!
//! O programa é **inteiramente** verificado antes de qualquer geração de código (Seção 1), e um
//! erro de tipo é relatado mesmo em código que nunca executa (Seção 4.7). Este módulo faz essa
//! verificação em uma passagem sobre a AST:
//!
//! * tipos: `int`, `real`, `string`, `bool`, `[T]` (lista), `[T1, …, Tn]` (tupla) e `A -> R`;
//! * **unificação int→real** (Seção 2): unificar `int` com `real` dá `real`;
//! * a tabela de operadores da Seção 5.2, incluindo `size-mismatch` de **tuplas em tempo de
//!   compilação** e `type-mismatch` para escalar ⊗ lista;
//! * `iif` exige condição `bool` e ramificações do mesmo tipo (com `fail` adaptando-se);
//! * `for` exige condição `bool`;
//! * chamadas exigem alvo do tipo função, com argumento unificável;
//! * `print`/`println`/`string`/`format` rejeitam valores de função (`render-function`).
//!
//! ## Limite conhecido
//!
//! Funções genéricas (sem anotação no parâmetro) recebem o tipo `Any` — um tipo "desconhecido"
//! que unifica com qualquer outro sem gerar erro. Isso mantém programas como `identidade = fn(x){
//! x; }; identidade(1); identidade("abc");` válidos (Seção 4.10 fala em especialização; aqui o
//! caso genérico funciona, mas sem multiplicar instâncias). O mesmo vale para recursão, cujo tipo
//! só é conhecido ao final do corpo. Está registrado no `RELATORIO_PROGRESSO.md`.

use std::collections::HashMap;

use crate::codegen::{SemaError, BUILTINS};
use crate::diag::DiagCode;
use crate::lexer::Span;
use crate::parser::ast::*;

/// Tipo estático de uma expressão XPR.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Real,
    Str,
    Bool,
    /// `[T]` — lista de elementos de tipo único.
    List(Box<Ty>),
    /// `[T1, …, Tn]` — tupla de tamanho e tipos fixos.
    Tuple(Vec<Ty>),
    /// `A -> R`.
    Fn(Box<Ty>, Box<Ty>),
    /// Desconhecido (`[]` vazio ainda não resolvido, função genérica, resultado de `fail`, …).
    /// Unifica com qualquer coisa sem erro.
    Any,
}

impl Ty {
    pub fn name(&self) -> String {
        match self {
            Ty::Int => "int".into(),
            Ty::Real => "real".into(),
            Ty::Str => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::List(inner) => format!("[{}]", inner.name()),
            Ty::Tuple(items) => format!(
                "[{}]",
                items.iter().map(Ty::name).collect::<Vec<_>>().join(", ")
            ),
            Ty::Fn(arg, result) => format!("{} -> {}", arg.name(), result.name()),
            Ty::Any => "?".into(),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Real)
    }

    fn is_any(&self) -> bool {
        matches!(self, Ty::Any)
    }

    /// Contém um valor de função em qualquer profundidade?
    fn contains_fn(&self) -> bool {
        match self {
            Ty::Fn(_, _) => true,
            Ty::List(inner) => inner.contains_fn(),
            Ty::Tuple(items) => items.iter().any(Ty::contains_fn),
            _ => false,
        }
    }
}

/// Resultado da unificação de dois tipos.
enum Unify {
    /// Tipos compatíveis; o tipo resultante.
    Ok(Ty),
    /// Erro de tipo, com o motivo para o diagnóstico.
    Mismatch(String),
    /// Erro de tamanho (tuplas de comprimentos diferentes) — `size-mismatch` em compilação.
    Size(String),
}

/// Unifica dois tipos aplicando a promoção int→real (Seção 2).
fn unify(a: &Ty, b: &Ty) -> Unify {
    match (a, b) {
        (Ty::Any, other) | (other, Ty::Any) => Unify::Ok(other.clone()),
        (Ty::Int, Ty::Int) => Unify::Ok(Ty::Int),
        (Ty::Real, Ty::Real) => Unify::Ok(Ty::Real),
        (Ty::Int, Ty::Real) | (Ty::Real, Ty::Int) => Unify::Ok(Ty::Real),
        (Ty::Str, Ty::Str) => Unify::Ok(Ty::Str),
        (Ty::Bool, Ty::Bool) => Unify::Ok(Ty::Bool),
        (Ty::List(a), Ty::List(b)) => match unify(a, b) {
            Unify::Ok(inner) => Unify::Ok(Ty::List(Box::new(inner))),
            other => other,
        },
        (Ty::Tuple(a), Ty::Tuple(b)) => {
            if a.len() != b.len() {
                return Unify::Size(format!(
                    "tuplas de tamanhos {} e {}",
                    a.len(),
                    b.len()
                ));
            }
            let mut items = Vec::with_capacity(a.len());
            for (left, right) in a.iter().zip(b.iter()) {
                match unify(left, right) {
                    Unify::Ok(item) => items.push(item),
                    other => return other,
                }
            }
            Unify::Ok(Ty::Tuple(items))
        }
        (Ty::List(_), Ty::Tuple(_))
        | (Ty::Tuple(_), Ty::List(_)) => Unify::Mismatch(format!(
            "lista e tupla não são o mesmo tipo ({} e {})",
            a.name(),
            b.name()
        )),
        (Ty::Fn(_, _), Ty::Fn(_, _)) => Unify::Ok(Ty::Any),
        _ => Unify::Mismatch(format!("{} e {}", a.name(), b.name())),
    }
}

/// Unificação entre o tipo **esperado** por um parâmetro e o tipo do argumento.
///
/// Um padrão de lista (`fn([a, b])`) aceita tanto uma lista quanto uma tupla com o mesmo número
/// de elementos no nível desestruturado (Seção 4.5). Como o padrão é representado por `Ty::Tuple`,
/// esta unificação relaxa a regra "lista e tupla são tipos diferentes" **apenas** nessa posição.
fn unify_argument(param: &Ty, arg: &Ty) -> Unify {
    match (param, arg) {
        (Ty::Tuple(pattern), Ty::List(inner)) => {
            for item in pattern {
                match unify(item, inner) {
                    Unify::Ok(_) => {}
                    other => return other,
                }
            }
            Unify::Ok(Ty::List(inner.clone()))
        }
        _ => unify(param, arg),
    }
}

/// Constantes predefinidas da Seção 6 — vinculações somente leitura.
///
/// Atribuir a qualquer uma delas é `assign-to-constant` em tempo de compilação, em qualquer
/// escopo (`pi = 3.14` é o exemplo canônico da spec). Cada uma tem tipo fixo: `true`/`false`
/// são bools; `NaN`, `INF`, `pi`/`PI` e `e`/`E` são reais.
fn predefined_constant(name: &str) -> Option<Ty> {
    match name {
        "true" | "false" => Some(Ty::Bool),
        "NaN" | "INF" | "pi" | "PI" | "e" | "E" => Some(Ty::Real),
        _ => None,
    }
}

/// Verificador de um módulo.
pub struct TypeChecker<'a> {
    file: &'a str,
    /// Tipos das definições do módulo já verificadas (uso após definição).
    globals: HashMap<String, Ty>,
    /// Alias de import → tipos dos membros exportados por aqueles módulos (verificado à parte).
    imported: HashMap<String, HashMap<String, Ty>>,
    /// Pilha de escopos locais.
    scopes: Vec<HashMap<String, Ty>>,
    /// Nome da definição corrente (para recursão direta).
    current: String,
    /// Alvos pendentes de uma atribuição cujo valor é uma `fn_def` (Seção 4.5).
    pending: Vec<String>,
}

/// O **valor de resultado** desta expressão depende de uma chamada à própria função sendo
/// definida (`is_self`)? Seção 4.10: "o tipo de resultado de uma função recursiva é determinado
/// pelas ramificações que não chamam a própria função" — se todas dependem, o tipo não pode ser
/// inferido (`cannot-infer`).
///
/// A análise é sintática e conservadora: conta como "dependente" apenas a chamada pura (o
/// resultado É a chamada), o `iif` cujas **duas** ramificações dependem, o `try` cujo tipo de
/// resultado acompanha o da expressão tentada e o bloco pela última expressão. Operadores têm
/// tipo próprio — `x * fat(x - 1)` é numérico mesmo contendo uma chamada no caminho — e
/// builtins genéricos (`fill`, `at`, `apply`, …) devolvem `Any` sem relação com recursão.
fn result_calls_self(expr: &Expr, is_self: &impl Fn(&str) -> bool) -> bool {
    match expr {
        Expr::FnCall { target, .. } => matches!(
            target.as_ref(),
            Expr::QualifiedName {
                module_alias: None,
                name,
                ..
            } if is_self(name)
        ),
        Expr::Iif {
            then_branch,
            else_branch,
            ..
        } => result_calls_self(then_branch, is_self) && result_calls_self(else_branch, is_self),
        Expr::Try(inner, _) => result_calls_self(inner, is_self),
        Expr::Block(block) => block
            .statements
            .last()
            .is_some_and(|stmt| match stmt {
                Stmt::Expr(expr) => result_calls_self(expr, is_self),
                Stmt::Import(_) => false,
            }),
        _ => false,
    }
}

impl<'a> TypeChecker<'a> {
    pub fn new(file: &'a str) -> Self {
        Self {
            file,
            globals: HashMap::new(),
            imported: HashMap::new(),
            scopes: vec![HashMap::new()],
            current: String::new(),
            pending: Vec::new(),
        }
    }

    /// Registra os membros exportados por um módulo importado sob um alias.
    pub fn register_import(&mut self, alias: &str, members: HashMap<String, Ty>) {
        self.imported.insert(alias.to_string(), members);
    }

    /// Verifica um módulo inteiro e devolve os tipos das suas definições (exportados).
    pub fn check_module(&mut self, module: &Module) -> Result<HashMap<String, Ty>, SemaError> {
        for def in &module.definitions {
            let ty = self.check_definition(def)?;
            self.globals.insert(def.name.clone(), ty);
        }
        Ok(self.globals.clone())
    }

    fn check_definition(&mut self, def: &Definition) -> Result<Ty, SemaError> {
        if predefined_constant(&def.name).is_some() {
            return Err(self.error(
                DiagCode::AssignToConstant,
                def.span,
                format!(
                    "{} é uma constante predefinida (Seção 6) e é somente leitura",
                    def.name
                ),
            ));
        }
        self.current = def.name.clone();
        match &def.initializer {
            Initializer::FnDef(fn_def) => self.check_fn(fn_def, Some(&def.name)),
            Initializer::ConstExpr(expr) => self.check_expr(expr),
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        self.globals.get(name).cloned()
    }

    fn declare(&mut self, name: &str, ty: Ty) {
        self.scopes
            .last_mut()
            .expect("pilha de escopos vazia")
            .insert(name.to_string(), ty);
    }

    /// Registra a atribuição de `ty` a `name`, unificando com o tipo já existente no escopo (uma
    /// variável tem **um único tipo** no seu escopo — Seção 4.7).
    fn assign(&mut self, name: &str, ty: Ty, span: Span) -> Result<Ty, SemaError> {
        // Escopos locais primeiro: uma variável local (parâmetro ou primeira atribuição no
        // corpo — Seção 4.5) pode ser reatribuída livremente, mesmo sombreando um nome externo.
        if let Some(existing) = self.scopes.iter().rev().find_map(|scope| scope.get(name)) {
            return match unify(existing, &ty) {
                Unify::Ok(unified) => {
                    let unified = unified.clone();
                    self.declare(name, unified.clone());
                    Ok(unified)
                }
                Unify::Mismatch(detail) => Err(self.error(
                    DiagCode::TypeMismatch,
                    span,
                    format!("a variável {} não pode ser {} e {} ({})", name, existing.name(), ty.name(), detail),
                )),
                Unify::Size(detail) => Err(self.error(DiagCode::SizeMismatch, span, detail)),
            };
        }
        if predefined_constant(name).is_some() {
            return Err(self.error(
                DiagCode::AssignToConstant,
                span,
                format!("{} é uma constante predefinida (Seção 6) e é somente leitura", name),
            ));
        }
        // `scopes.len() > 1` ⇔ dentro de um corpo de função/bloco: lá, nomes de nível superior
        // são somente leitura (Seção 6); no nível de módulo a redefinição continua válida
        // (Seção 4.5, exemplo das "variáveis salvas").
        if self.scopes.len() > 1 && self.globals.contains_key(name) {
            return Err(self.error(
                DiagCode::AssignToConstant,
                span,
                format!(
                    "{} é uma definição de nível superior, somente leitura dentro de funções (Seção 6)",
                    name
                ),
            ));
        }
        self.declare(name, ty.clone());
        Ok(ty)
    }

    fn error(&self, code: DiagCode, span: Span, detail: impl Into<String>) -> SemaError {
        let detail = detail.into();
        SemaError {
            code,
            span: Some(span),
            detail: format!("{} (em {})", detail, self.file),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Expressões
    // -----------------------------------------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> Result<Ty, SemaError> {
        match expr {
            Expr::IntLit(_, _) => Ok(Ty::Int),
            Expr::RealLit(_, _) => Ok(Ty::Real),
            Expr::StringLit(_, _, _) => Ok(Ty::Str),
            Expr::QualifiedName {
                module_alias,
                name,
                span,
            } => self.check_name(module_alias.as_deref(), name, *span),
            Expr::Assignment { target, value, span } => self.check_assignment(target, value, *span),
            Expr::FnDef(fn_def) => self.check_fn(fn_def, None),
            Expr::LogicalOr(operands) | Expr::LogicalAnd(operands) => {
                for operand in operands {
                    let ty = self.check_expr(operand)?;
                    self.require_bool(&ty, operand.span(), "and/or")?;
                }
                Ok(Ty::Bool)
            }
            Expr::LogicalNot(operand, span) => {
                let ty = self.check_expr(operand)?;
                self.require_bool(&ty, *span, "not")?;
                Ok(Ty::Bool)
            }
            Expr::Binary {
                op,
                lhs,
                rhs,
                span,
            } => self.check_binary(*op, lhs, rhs, *span),
            Expr::Unary {
                op,
                operand,
                span,
                ..
            } => {
                let ty = self.check_expr(operand)?;
                match op {
                    UnOp::Minus | UnOp::Plus => match &ty {
                        Ty::Int => Ok(Ty::Int),
                        Ty::Real => Ok(Ty::Real),
                        Ty::List(inner) if inner.is_numeric() || inner.is_any() => Ok(ty.clone()),
                        Ty::Tuple(items) if items.iter().all(Ty::is_numeric) => Ok(ty.clone()),
                        Ty::Any => Ok(Ty::Any),
                        other => Err(self.error(
                            DiagCode::TypeMismatch,
                            *span,
                            format!("o sinal unário exige valor numérico, recebeu {}", other.name()),
                        )),
                    },
                }
            }
            Expr::Indexed {
                base,
                indices,
                span,
            } => {
                let mut ty = self.check_expr(base)?;
                for index in indices {
                    let index_ty = self.check_expr(index)?;
                    if !matches!(index_ty, Ty::Int | Ty::Any) {
                        return Err(self.error(
                            DiagCode::TypeMismatch,
                            *span,
                            format!("índice deve ser int, recebeu {}", index_ty.name()),
                        ));
                    }
                    ty = match ty {
                        Ty::Str => Ty::Str,
                        Ty::List(inner) => (*inner).clone(),
                        Ty::Tuple(items) => {
                            // Seção 4.8: indexar uma tupla exige índice constante, verificado
                            // em tempo de compilação — cada posição pode ter um tipo diferente.
                            match index {
                                Expr::IntLit(digits, _) => {
                                    let idx: usize = digits.parse().unwrap_or(usize::MAX);
                                    match items.get(idx) {
                                        Some(item) => item.clone(),
                                        None => {
                                            return Err(self.error(
                                                DiagCode::IndexOutOfRange,
                                                *span,
                                                format!(
                                                    "índice constante {} em tupla de {} posições",
                                                    idx,
                                                    items.len()
                                                ),
                                            ))
                                        }
                                    }
                                }
                                _ => {
                                    return Err(self.error(
                                        DiagCode::TypeMismatch,
                                        *span,
                                        "índice de tupla precisa ser um literal int constante (Seção 4.8)",
                                    ))
                                }
                            }
                        }
                        Ty::Any => Ty::Any,
                        other => {
                            return Err(self.error(
                                DiagCode::TypeMismatch,
                                *span,
                                format!("{} não pode ser indexado", other.name()),
                            ))
                        }
                    };
                }
                Ok(ty)
            }
            Expr::FnCall { target, arg, span } => self.check_call(target, arg.as_deref(), *span),
            Expr::Block(block) => self.check_block(block),
            Expr::Iif {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_ty = self.check_expr(cond)?;
                self.require_bool(&cond_ty, cond.span(), "iif")?;
                let then_ty = self.check_expr(then_branch)?;
                let else_ty = self.check_expr(else_branch)?;
                match unify(&then_ty, &else_ty) {
                    Unify::Ok(unified) => Ok(unified),
                    Unify::Mismatch(detail) => Err(self.error(
                        DiagCode::TypeMismatch,
                        *span,
                        format!("as ramificações de iif diferem ({})", detail),
                    )),
                    Unify::Size(detail) => Err(self.error(DiagCode::SizeMismatch, *span, detail)),
                }
            }
            Expr::For {
                init,
                cond,
                step,
                body,
                span,
            } => {
                self.scopes.push(HashMap::new());
                let result = (|| {
                    self.check_expr(init)?;
                    let cond_ty = self.check_expr(cond)?;
                    self.require_bool(&cond_ty, cond.span(), "for")?;
                    self.check_expr(step)?;
                    self.check_expr(body)?;
                    Ok(())
                })();
                self.scopes.pop();
                result?;
                let _ = span;
                Ok(Ty::Int)
            }
            Expr::Try(inner, span) => {
                let ty = self.check_expr(inner)?;
                if ty.contains_fn() {
                    return Err(self.error(
                        DiagCode::TypeMismatch,
                        *span,
                        "try exige um tipo com valor neutro (função não tem)".to_string(),
                    ));
                }
                // `try` devolve a tupla `[bool, T]`.
                Ok(Ty::Tuple(vec![Ty::Bool, ty]))
            }
            Expr::BracketLiteral(elements, span) => self.check_bracket(elements, *span),
        }
    }

    fn require_bool(&self, ty: &Ty, span: Span, context: &str) -> Result<(), SemaError> {
        match ty {
            Ty::Bool | Ty::Any => Ok(()),
            other => Err(self.error(
                DiagCode::TypeMismatch,
                span,
                format!("{} exige condição bool, recebeu {}", context, other.name()),
            )),
        }
    }

    fn check_name(
        &mut self,
        module_alias: Option<&str>,
        name: &str,
        span: Span,
    ) -> Result<Ty, SemaError> {
        if let Some(alias) = module_alias {
            return match self.imported.get(alias).and_then(|members| members.get(name)) {
                Some(ty) => Ok(ty.clone()),
                None => Err(self.error(
                    DiagCode::UndefinedIdentifier,
                    span,
                    format!("membro {} do módulo {}", name, alias),
                )),
            };
        }
        if let Some(ty) = self.lookup(name) {
            return Ok(ty);
        }
        if self.pending.iter().any(|pending| pending == name) {
            return Ok(Ty::Any);
        }
        if name == self.current {
            return Ok(Ty::Any);
        }
        if self.globals.contains_key(name) {
            return Ok(Ty::Any);
        }
        match predefined_constant(name) {
            Some(ty) => Ok(ty),
            None if BUILTINS.contains(&name) => Ok(Ty::Any),
            None => Err(self.error(
                DiagCode::UndefinedIdentifier,
                span,
                format!("{}", name),
            )),
        }
    }

    fn check_assignment(
        &mut self,
        target: &BindingPattern,
        value: &Expr,
        span: Span,
    ) -> Result<Ty, SemaError> {
        // Num cadeia `a = b = fn(...){...}`, todos os alvos já são nomes pendentes quando o valor
        // é uma `fn_def` (Seção 4.5).
        let mut pending = Vec::new();
        if matches!(value, Expr::FnDef(_)) {
            collect_names(target, &mut pending);
            self.pending.extend(pending.clone());
        }
        let value_ty = self.check_expr(value);
        for name in &pending {
            self.pending.retain(|pending_name| pending_name != name);
        }
        let value_ty = value_ty?;
        self.bind_pattern(target, &value_ty, span)?;
        Ok(value_ty)
    }

    fn bind_pattern(
        &mut self,
        pattern: &BindingPattern,
        ty: &Ty,
        span: Span,
    ) -> Result<(), SemaError> {
        match pattern {
            BindingPattern::Name(name, name_span) => {
                self.assign(name, ty.clone(), *name_span)?;
                Ok(())
            }
            BindingPattern::List(items, list_span) => {
                let element_types = match ty {
                    Ty::Tuple(types) => {
                        if types.len() != items.len() {
                            return Err(self.error(
                                DiagCode::SizeMismatch,
                                *list_span,
                                format!(
                                    "o padrão tem {} elementos, a tupla tem {}",
                                    items.len(),
                                    types.len()
                                ),
                            ));
                        }
                        types.clone()
                    }
                    Ty::List(inner) => vec![(**inner).clone(); items.len()],
                    Ty::Any => vec![Ty::Any; items.len()],
                    other => {
                        return Err(self.error(
                            DiagCode::TypeMismatch,
                            *list_span,
                            format!(
                                "o padrão de {} elementos exige lista ou tupla, recebeu {}",
                                items.len(),
                                other.name()
                            ),
                        ))
                    }
                };
                for (item, item_ty) in items.iter().zip(element_types.iter()) {
                    self.bind_pattern(item, item_ty, span)?;
                }
                let _ = span;
                Ok(())
            }
        }
    }

    fn check_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
    ) -> Result<Ty, SemaError> {
        let left = self.check_expr(lhs)?;
        let right = self.check_expr(rhs)?;
        let symbol = op.symbol();
        match op {
            BinOp::EqEq | BinOp::NotEq => {
                // `==`/`!=`: qualquer par unificável, exceto quando um lado é função.
                if left.contains_fn() || right.contains_fn() {
                    return Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("{} não compara valores de função", symbol),
                    ));
                }
                match unify(&left, &right) {
                    Unify::Ok(_) => Ok(Ty::Bool),
                    Unify::Mismatch(detail) => Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("{} entre {}", symbol, detail),
                    )),
                    Unify::Size(detail) => Err(self.error(DiagCode::SizeMismatch, span, detail)),
                }
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                let comparable = |ty: &Ty| {
                    ty.is_numeric()
                        || matches!(ty, Ty::Str | Ty::Any)
                        || matches!(ty, Ty::List(_) | Ty::Tuple(_))
                };
                if !comparable(&left) || !comparable(&right) {
                    return Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("{} entre {} e {}", symbol, left.name(), right.name()),
                    ));
                }
                match unify(&left, &right) {
                    Unify::Ok(_) => Ok(Ty::Bool),
                    Unify::Mismatch(detail) => Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("{} entre {}", symbol, detail),
                    )),
                    Unify::Size(detail) => Err(self.error(DiagCode::SizeMismatch, span, detail)),
                }
            }
            BinOp::Add => {
                // `+` é soma numérica ou concatenação de strings (e elemento a elemento).
                if matches!(left, Ty::Str) || matches!(right, Ty::Str) {
                    match (&left, &right) {
                        (Ty::Str, Ty::Str) | (Ty::Any, _) | (_, Ty::Any) => Ok(Ty::Str),
                        (a, b) => Err(self.error(
                            DiagCode::TypeMismatch,
                            span,
                            format!("+ entre {} e {} (concatenação exige duas strings)", a.name(), b.name()),
                        )),
                    }
                } else {
                    self.numeric_binary(symbol, &left, &right, span)
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                self.numeric_binary(symbol, &left, &right, span)
            }
        }
    }

    fn numeric_binary(
        &self,
        symbol: &str,
        left: &Ty,
        right: &Ty,
        span: Span,
    ) -> Result<Ty, SemaError> {
        match (left, right) {
            (Ty::Any, other) | (other, Ty::Any) => Ok(other.clone()),
            (a, b) if a.is_numeric() && b.is_numeric() => {
                Ok(if matches!((a, b), (Ty::Real, _) | (_, Ty::Real)) {
                    Ty::Real
                } else {
                    Ty::Int
                })
            }
            (Ty::List(a), Ty::List(b)) => {
                let inner = self.numeric_binary(symbol, a, b, span)?;
                Ok(Ty::List(Box::new(inner)))
            }
            (Ty::Tuple(a), Ty::Tuple(b)) => {
                if a.len() != b.len() {
                    return Err(self.error(
                        DiagCode::SizeMismatch,
                        span,
                        format!("{} entre tuplas de tamanhos {} e {}", symbol, a.len(), b.len()),
                    ));
                }
                let mut items = Vec::with_capacity(a.len());
                for (left_item, right_item) in a.iter().zip(b.iter()) {
                    items.push(self.numeric_binary(symbol, left_item, right_item, span)?);
                }
                Ok(Ty::Tuple(items))
            }
            (Ty::List(_) | Ty::Tuple(_), _) | (_, Ty::List(_) | Ty::Tuple(_)) => Err(self.error(
                DiagCode::TypeMismatch,
                span,
                format!(
                    "{} entre {} e {} (sem broadcasting escalar)",
                    symbol,
                    left.name(),
                    right.name()
                ),
            )),
            (a, b) => Err(self.error(
                DiagCode::TypeMismatch,
                span,
                format!("{} entre {} e {}", symbol, a.name(), b.name()),
            )),
        }
    }

    fn check_call(&mut self, target: &Expr, arg: Option<&Expr>, span: Span) -> Result<Ty, SemaError> {
        // Builtins têm assinatura própria (Seção 5), com o caso escolhido pelo tipo do argumento.
        if let Expr::QualifiedName {
            module_alias: None,
            name,
            ..
        } = target
        {
            if BUILTINS.contains(&name.as_str()) {
                return self.check_builtin(name, arg, span);
            }
        }
        let target_ty = self.check_expr(target)?;
        let arg_ty = match arg {
            Some(arg) => self.check_expr(arg)?,
            None => Ty::List(Box::new(Ty::Any)),
        };
        match target_ty {
            Ty::Fn(param, result) => {
                match unify_argument(&param, &arg_ty) {
                    Unify::Ok(_) => Ok(*result),
                    Unify::Mismatch(detail) => Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("argumento incompatível com o parâmetro ({})", detail),
                    )),
                    Unify::Size(detail) => Err(self.error(DiagCode::SizeMismatch, span, detail)),
                }
            }
            Ty::Any => Ok(Ty::Any),
            other => Err(self.error(
                DiagCode::TypeMismatch,
                span,
                format!("{} não é uma função", other.name()),
            )),
        }
    }

    fn check_builtin(
        &mut self,
        name: &str,
        arg: Option<&Expr>,
        span: Span,
    ) -> Result<Ty, SemaError> {
        let arg_ty = match arg {
            Some(arg) => self.check_expr(arg)?,
            // `f()` equivale a `f([])` (Seção 4.5).
            None => Ty::List(Box::new(Ty::Any)),
        };
        let renderer = |checker: &Self, ty: &Ty| -> Result<(), SemaError> {
            if ty.contains_fn() {
                return Err(checker.error(
                    DiagCode::RenderFunction,
                    span,
                    format!("{} não pode renderizar um valor de função", name),
                ));
            }
            Ok(())
        };
        match name {
            "print" | "println" | "string" => {
                renderer(self, &arg_ty)?;
                Ok(if name == "string" { Ty::Str } else { Ty::Int })
            }
            "format" => {
                renderer(self, &arg_ty)?;
                Ok(Ty::Str)
            }
            "len" => match &arg_ty {
                Ty::Str | Ty::List(_) | Ty::Any => Ok(Ty::Int),
                other => Err(self.error(
                    DiagCode::TypeMismatch,
                    span,
                    format!("len espera lista ou string, recebeu {}", other.name()),
                )),
            },
            "input" => Ok(Ty::Str),
            "fail" => Ok(Ty::Any),
            "int" | "floor" | "ceil" | "round" | "frac" => Ok(Ty::Int),
            "real" => Ok(Ty::Real),
            "any" | "all" => Ok(Ty::Bool),
            "cat" => match &arg_ty {
                Ty::List(inner) => match &**inner {
                    Ty::List(item) => Ok(Ty::List(item.clone())),
                    Ty::Any => Ok(Ty::List(Box::new(Ty::Any))),
                    other => Err(self.error(
                        DiagCode::TypeMismatch,
                        span,
                        format!("cat espera uma lista de listas, recebeu [{}]", other.name()),
                    )),
                },
                Ty::Any => Ok(Ty::List(Box::new(Ty::Any))),
                other => Err(self.error(
                    DiagCode::TypeMismatch,
                    span,
                    format!("cat espera uma lista de listas, recebeu {}", other.name()),
                )),
            },
            "fill" | "range" | "set" | "slice" | "at" | "apply" => Ok(Ty::Any),
            other => Err(self.error(
                DiagCode::UndefinedIdentifier,
                span,
                format!("builtin desconhecido: {}", other),
            )),
        }
    }

    fn check_block(&mut self, block: &Block) -> Result<Ty, SemaError> {
        self.scopes.push(HashMap::new());
        let mut last = Ty::List(Box::new(Ty::Any)); // bloco vazio vale `[]` (Seção 4.2)
        let mut result = Ok(());
        for stmt in &block.statements {
            match stmt {
                Stmt::Expr(expr) => match self.check_expr(expr) {
                    Ok(ty) => last = ty,
                    Err(err) => {
                        result = Err(err);
                        break;
                    }
                },
                Stmt::Import(_) => {
                    result = Err(self.error(
                        DiagCode::SyntaxError,
                        block.span,
                        "import dentro de bloco",
                    ));
                    break;
                }
            }
        }
        self.scopes.pop();
        result.map(|_| last)
    }

    fn check_bracket(&mut self, elements: &[Expr], span: Span) -> Result<Ty, SemaError> {
        if elements.is_empty() {
            // `[]` é uma lista vazia de qualquer tipo de elemento (Seção 2).
            return Ok(Ty::List(Box::new(Ty::Any)));
        }
        let mut types = Vec::with_capacity(elements.len());
        for element in elements {
            types.push(self.check_expr(element)?);
        }
        // Se todos os elementos unificam num único tipo, é uma **lista**; senão, uma **tupla**.
        let mut unified = types[0].clone();
        let mut all_same = true;
        for ty in &types[1..] {
            match unify(&unified, ty) {
                Unify::Ok(next) => unified = next,
                Unify::Size(detail) => return Err(self.error(DiagCode::SizeMismatch, span, detail)),
                Unify::Mismatch(_) => {
                    all_same = false;
                    break;
                }
            }
        }
        if all_same {
            Ok(Ty::List(Box::new(unified)))
        } else {
            Ok(Ty::Tuple(types))
        }
    }

    fn check_fn(&mut self, fn_def: &FnDef, self_name: Option<&str>) -> Result<Ty, SemaError> {
        self.scopes.push(HashMap::new());
        // Os parâmetros são definidos ao entrar na definição (Seção 4.5).
        let param_ty = self.bind_param(&fn_def.param);
        // Os nomes pendentes (o próprio nome da função em `fat = fn(x){ … fat(…) … }`, ou os alvos
        // de uma cadeia `a = b = fn(…){ … a(…) … }`) valem **dentro** do corpo: é a recursão
        // direta da Seção 4.5.
        let result = self.check_block(&fn_def.body);
        self.scopes.pop();
        let result_ty = result?;
        // Seção 4.10: se toda ramificação do resultado chama a própria função (nome corrente ou
        // pendente — a recursão direta da Seção 4.5), o tipo do resultado não é determinável.
        let is_self = |name: &str| -> bool {
            name == self.current || self.pending.iter().any(|pending| pending == name)
        };
        if let Some(Stmt::Expr(last)) = fn_def.body.statements.last() {
            if result_calls_self(last, &is_self) {
                return Err(self.error(
                    DiagCode::CannotInfer,
                    last.span(),
                    format!(
                        "o resultado de {} não é determinado: toda ramificação do resultado chama a própria função (Seção 4.10)",
                        self.current
                    ),
                ));
            }
        }
        let _ = self_name;
        Ok(Ty::Fn(Box::new(param_ty), Box::new(result_ty)))
    }

    fn bind_param(&mut self, pattern: &ParamPattern) -> Ty {
        match pattern {
            ParamPattern::Name { name, ty, .. } => {
                let declared = ty
                    .as_ref()
                    .map(type_annotation_to_ty)
                    .unwrap_or(Ty::Any);
                self.declare(name, declared.clone());
                declared
            }
            ParamPattern::List(items, _) => {
                let item_types: Vec<Ty> = items.iter().map(|item| self.bind_param(item)).collect();
                Ty::Tuple(item_types)
            }
        }
    }
}

fn collect_names(pattern: &BindingPattern, out: &mut Vec<String>) {
    match pattern {
        BindingPattern::Name(name, _) => out.push(name.clone()),
        BindingPattern::List(items, _) => {
            for item in items {
                collect_names(item, out);
            }
        }
    }
}

/// Converte a anotação de tipo de superfície no tipo estático.
pub fn type_annotation_to_ty(ty: &TypeAnnotation) -> Ty {
    match ty {
        TypeAnnotation::Bool => Ty::Bool,
        TypeAnnotation::Int => Ty::Int,
        TypeAnnotation::Real => Ty::Real,
        TypeAnnotation::String => Ty::Str,
        TypeAnnotation::Fn => Ty::Fn(Box::new(Ty::Any), Box::new(Ty::Any)),
        TypeAnnotation::AnyList => Ty::List(Box::new(Ty::Any)),
        TypeAnnotation::ListOf(inner) => Ty::List(Box::new(type_annotation_to_ty(inner))),
        TypeAnnotation::Tuple(items) => {
            Ty::Tuple(items.iter().map(type_annotation_to_ty).collect())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Testes unitários da checagem
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(source: &str) -> Result<HashMap<String, Ty>, SemaError> {
        let tokens = Lexer::new(source).tokenize().expect("lexer");
        let module = Parser::new(&tokens).parse_module().expect("parser");
        TypeChecker::new("teste.x").check_module(&module)
    }

    fn check_err(source: &str) -> DiagCode {
        match check(source) {
            Ok(_) => panic!("esperava erro de tipo em {:?}", source),
            Err(err) => err.code,
        }
    }

    #[test]
    fn soma_de_int_e_int_da_int() {
        let types = check("r = 1 + 2;").expect("válido");
        assert_eq!(types["r"], Ty::Int);
    }

    #[test]
    fn int_com_real_promove_para_real() {
        let types = check("r = 1 + 2.5;").expect("válido");
        assert_eq!(types["r"], Ty::Real);
    }

    #[test]
    fn soma_de_int_com_string_e_type_mismatch() {
        // A spec exige erro **em tempo de compilação** (Seção 1), mesmo sem executar.
        assert_eq!(check_err("r = 1 + \"a\";"), DiagCode::TypeMismatch);
    }

    #[test]
    fn comparacao_entre_tipos_incompativeis_e_erro() {
        assert_eq!(check_err("r = 1 < \"a\";"), DiagCode::TypeMismatch);
    }

    #[test]
    fn tuplas_de_tamanhos_diferentes_sao_size_mismatch_em_compilacao() {
        // `[1, "a"]` é a tupla [int, string]; comparar com [int, string, bool] é erro estático.
        assert_eq!(
            check_err("r = [1, \"a\"] == [1, \"a\", true];"),
            DiagCode::SizeMismatch
        );
    }

    #[test]
    fn lista_com_elementos_do_mesmo_tipo_e_lista() {
        let types = check("r = [1, 2, 3];").expect("válido");
        assert_eq!(types["r"], Ty::List(Box::new(Ty::Int)));
    }

    #[test]
    fn lista_com_tipos_diferentes_e_tupla() {
        let types = check("r = [1, \"a\"];").expect("válido");
        assert_eq!(types["r"], Ty::Tuple(vec![Ty::Int, Ty::Str]));
    }

    #[test]
    fn condicao_de_iif_precisa_ser_bool() {
        assert_eq!(check_err("_main = fn(x){ iif(1, 2, 3); };"), DiagCode::TypeMismatch);
    }

    #[test]
    fn ramificacoes_de_iif_precisam_unificar() {
        assert_eq!(check_err("_main = fn(x){ iif(true, 1, \"a\"); };"), DiagCode::TypeMismatch);
    }

    #[test]
    fn ramificacoes_de_iif_com_int_e_real_unificam_para_real() {
        // O `iif` devolve `real` (int unificado com real); verificado pelo tipo da função.
        let types = check("_main = fn(x){ iif(true, 1, 2.5); };").expect("válido");
        assert_eq!(
            types["_main"],
            Ty::Fn(Box::new(Ty::Any), Box::new(Ty::Real))
        );
    }

    #[test]
    fn for_retorna_int_e_exige_condicao_bool() {
        // O tipo do `for` é int; a condição precisa ser bool. Ambos verificados no corpo de uma
        // função, porque no nível de módulo um `for` não é `const_expr`.
        let types = check("_main = fn(x){ for(i = 0; i < 3; i = i + 1) { i; }; };").expect("válido");
        assert_eq!(types["_main"], Ty::Fn(Box::new(Ty::Any), Box::new(Ty::Int)));
        assert_eq!(
            check_err("_main = fn(x){ for(i = 0; i; i = i + 1) { i; }; };"),
            DiagCode::TypeMismatch
        );
    }

    #[test]
    fn variavel_nao_pode_mudar_de_tipo() {
        // Seção 4.7: uma variável tem um único tipo no escopo.
        assert_eq!(
            check_err("_main = fn(x){ v = 1; v = \"a\"; };"),
            DiagCode::TypeMismatch
        );
    }

    #[test]
    fn variavel_int_pode_receber_real() {
        // `x = 2.5; x = 1;` deixa `x` como real (Seção 2).
        let types = check("_main = fn(x){ v = 2.5; v = 1; };").expect("válido");
        assert!(types.contains_key("_main"));
    }

    #[test]
    fn chamada_de_valor_nao_funcao_e_type_mismatch() {
        assert_eq!(
            check_err("_main = fn(x){ v = 1; v(1); };"),
            DiagCode::TypeMismatch
        );
    }

    #[test]
    fn renderizar_funcao_e_erro() {
        // Seção 5: renderizadores rejeitam valores de função em tempo de compilação.
        assert_eq!(
            check_err("_main = fn(x){ f = fn(y){ y; }; println(f); };"),
            DiagCode::RenderFunction
        );
    }

    #[test]
    fn try_sobre_funcao_e_erro() {
        // Seção 4.9: um tipo de função não tem valor neutro.
        assert_eq!(
            check_err("_main = fn(x){ f = fn(y){ y; }; try(f); };"),
            DiagCode::TypeMismatch
        );
    }

    #[test]
    fn desestruturacao_de_tupla_verifica_tamanho() {
        assert_eq!(
            check_err("_main = fn(x){ [a, b] = [1, \"s\", true]; };"),
            DiagCode::SizeMismatch
        );
    }

    #[test]
    fn indice_precisa_ser_int() {
        assert_eq!(
            check_err("_main = fn(x){ v = [1, 2]; v[\"a\"]; };"),
            DiagCode::TypeMismatch
        );
    }

    // -----------------------------------------------------------------------------------------
    // assign-to-constant (Seção 6): constantes predefinidas e nomes de nível superior
    // -----------------------------------------------------------------------------------------

    #[test]
    fn constantes_predefinidas_sao_somente_leitura_no_nivel_do_modulo() {
        // Seção 6: "Atribuir a uma constante predefinida é um erro de compilação". O exemplo
        // canônico é `pi = 3.14`; a regra vale para todas as constantes, em qualquer escopo.
        for (name, value) in [
            ("true", "1"),
            ("false", "0"),
            ("NaN", "0.0"),
            ("INF", "1.0"),
            ("pi", "3.14"),
            ("PI", "3.14"),
            ("e", "2.7"),
            ("E", "2.7"),
        ] {
            assert_eq!(
                check_err(&format!("{} = {};", name, value)),
                DiagCode::AssignToConstant,
                "atribuição a {} deveria ser assign-to-constant",
                name
            );
        }
    }

    #[test]
    fn constante_predefinida_nao_pode_ser_atribuida_dentro_de_funcao() {
        assert_eq!(
            check_err("_main = fn(x){ pi = 3.14; };"),
            DiagCode::AssignToConstant
        );
        assert_eq!(
            check_err("_main = fn(x){ true = false; };"),
            DiagCode::AssignToConstant
        );
    }

    #[test]
    fn nome_de_nivel_superior_e_somente_leitura_dentro_de_funcao() {
        // Seção 6: as definições de nível superior "são definidas uma vez, avaliadas pelo
        // compilador e somente leitura para todas as funções".
        assert_eq!(
            check_err("y = 1; _main = fn(a){ y = 2; };"),
            DiagCode::AssignToConstant
        );
    }

    #[test]
    fn nome_de_nivel_superior_continua_legivel_dentro_de_funcao() {
        // Ler um nome de nível superior dentro de função é normal (é assim que `c = 9;
        // f = fn(x){ x + c; };` funciona — Seção 4.5).
        let types = check("y = 1; _main = fn(a){ z = y + 1; z; };").expect("válido");
        assert!(types.contains_key("_main"));
    }

    #[test]
    fn redefinicao_no_nivel_do_modulo_continua_valida() {
        // Seção 4.5 (exemplo das "variáveis salvas"): `c = 19;` no nível de módulo **redefine**
        // a constante do módulo; isso não é atribuição dentro de função.
        let types =
            check("c = 9; f = fn(x){ x + c; }; c = 19; g = fn(x){ x + c; };").expect("válido");
        assert!(types.contains_key("g"));
    }

    #[test]
    fn parametros_e_locais_continuam_atribuiveis() {
        // Parâmetro e local da função são variáveis comuns (Seção 4.5): podem ser reatribuídos.
        let types = check("_main = fn(x){ x = x + 1; v = 2; v = 3; x; };").expect("válido");
        assert!(types.contains_key("_main"));
    }

    #[test]
    fn padrao_de_lista_com_constante_ou_global_e_assign_to_constant() {
        // A regra vale folha a folha em desestruturações, não só em alvos simples.
        assert_eq!(
            check_err("_main = fn(x){ [pi, v] = [1.0, 2]; };"),
            DiagCode::AssignToConstant
        );
        assert_eq!(
            check_err("y = 1; _main = fn(x){ [y, v] = [1, 2]; };"),
            DiagCode::AssignToConstant
        );
    }

    #[test]
    fn constantes_predefinidas_tem_tipo_correto() {
        // Seção 6: `pi`/`PI`/`e`/`E` são reais (os dois grafões de cada são aceitos);
        // `true`/`false` são bools.
        let types = check("r = pi + PI + e + E;").expect("válido");
        assert_eq!(types["r"], Ty::Real);
        let types = check("b = true == false;").expect("válido");
        assert_eq!(types["b"], Ty::Bool);
    }

    // -----------------------------------------------------------------------------------------
    // cannot-infer (Seção 4.10): recursão sem ramificação determinante
    // -----------------------------------------------------------------------------------------

    #[test]
    fn recursao_pura_e_cannot_infer() {
        // "O tipo de resultado de uma função recursiva é determinado pelas ramificações que não
        // chamam a própria função. Se nenhuma ramificação o determinar, a função é rejeitada
        // em tempo de compilação com cannot-infer." (Seção 4.10)
        assert_eq!(check_err("f = fn(x){ f(x); };"), DiagCode::CannotInfer);
    }

    #[test]
    fn recursao_nas_duas_ramificacoes_do_iif_e_cannot_infer() {
        assert_eq!(
            check_err("f = fn(x){ iif(x, f(x), f(x)); };"),
            DiagCode::CannotInfer
        );
        // `try` acompanha o tipo da expressão tentada: o resultado continua indeterminado.
        assert_eq!(
            check_err("f = fn(x){ try(f(x)); };"),
            DiagCode::CannotInfer
        );
    }

    #[test]
    fn uma_ramificacao_sem_chamada_resolve_o_tipo() {
        // O `then` chama a própria, mas o `else` (`0`) determina int.
        let types = check("f = fn(x){ iif(x, f(x - 1), 0); };").expect("válido");
        assert!(types.contains_key("f"));
        // Fatorial clássico: os operadores têm tipo próprio, `x * fat(x-1)` não é chamada pura.
        let types = check("fat = fn(x){ iif(x <= 1, 1, x * fat(x - 1)); };").expect("válido");
        assert!(types.contains_key("fat"));
    }

    #[test]
    fn resultado_generico_por_builtin_nao_e_cannot_infer() {
        // `fill` devolve tipo genérico (`Any`); isso é a especialização pendente (limitação
        // registrada), não `cannot-infer` de recursão — a ramificação não chama a própria.
        let types = check("f = fn(x){ fill([x]); };").expect("válido");
        assert!(types.contains_key("f"));
    }

    #[test]
    fn recursao_de_nome_pendente_local_e_cannot_infer() {
        // Nome pendente de atribuição local (`g = fn(x){ g(x); }` dentro de função) segue a
        // mesma regra da recursão direta (Seção 4.5: os pendentes valem dentro do corpo).
        assert_eq!(
            check_err("_main = fn(a){ g = fn(x){ g(x); }; 0; };"),
            DiagCode::CannotInfer
        );
    }

    #[test]
    fn chamada_a_outra_funcao_no_resultado_nao_e_cannot_infer() {
        // Só a chamada à **própria** função conta; chamar outra é o caso comum.
        let types = check("f = fn(x){ x; }; g = fn(y){ f(y); };").expect("válido");
        assert!(types.contains_key("g"));
    }

    // -----------------------------------------------------------------------------------------
    // Índice de tupla constante (Seção 4.8 / linha 642 da spec)
    // -----------------------------------------------------------------------------------------

    #[test]
    fn indice_de_tupla_constante_devolve_o_tipo_da_posicao() {
        // `t[1]` de `[1, "a"]` é string — o tipo é o da POSIÇÃO indexada, não o da primeira.
        let types = check("t = [1, \"a\"]; r = t[1];").expect("válido");
        assert_eq!(types["r"], Ty::Str);
        // Tupla aninhada e o açúcar de vírgula (`v[a, b]` ≡ `v[a][b]`).
        let types = check("t = [[1, \"a\"], [2, \"b\"]]; r = t[1][0];").expect("válido");
        assert_eq!(types["r"], Ty::Int);
        let types = check("t = [[1, \"a\"], [2, \"b\"]]; r = t[0, 1];").expect("válido");
        assert_eq!(types["r"], Ty::Str);
    }

    #[test]
    fn indice_de_tupla_calculado_e_type_mismatch_em_compilacao() {
        // "Um índice calculado em uma tupla é um type-mismatch em tempo de compilação."
        assert_eq!(
            check_err("_main = fn(x){ t = [1, \"a\"]; i = 1; t[i]; };"),
            DiagCode::TypeMismatch
        );
    }

    #[test]
    fn indice_de_tupla_constante_fora_do_limite_e_index_out_of_range() {
        // "Um índice constante fora do limite da tupla é um index-out-of-range em tempo de
        // compilação."
        assert_eq!(
            check_err("t = [1, \"a\"]; r = t[5];"),
            DiagCode::IndexOutOfRange
        );
        assert_eq!(
            check_err("t = [1, \"a\"]; r = t[2];"),
            DiagCode::IndexOutOfRange
        );
    }

    #[test]
    fn indice_de_lista_continua_dinamico() {
        // "Indexar uma lista aceita qualquer expressão int" — só a tupla exige constante.
        let types = check("_main = fn(x){ l = [1, 2]; i = 1; r = l[i]; r; };").expect("válido");
        assert!(types.contains_key("_main"));
    }

    #[test]
    fn indice_sobre_tipo_any_nao_verifica_constancia() {
        // Parâmetro genérico: a verificação acontece na especialização (limitação registrada).
        let types = check("f = fn(xs){ xs[0]; };").expect("válido");
        assert!(types.contains_key("f"));
    }
}
