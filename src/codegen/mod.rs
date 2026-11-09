//! Emissão de código Rust a partir da AST (subetapas A, C, D e G de `plano-codegen.md`).
//!
//! ## Modelo de execução
//!
//! Todo valor XPR é um [`xpr_rt::Value`]. Toda expressão é emitida como um bloco Rust cujo valor é
//! `XprResult` (`Result<Value, Fail>`), de modo que `?` propaga falhas exatamente como a Seção 4.9
//! exige — sem `setjmp`/`longjmp`.
//!
//! ## Escopos e captura
//!
//! Cada variável XPR vira uma variável Rust `Value`. Uma função literal vira uma closure Rust
//! `move`, com **cópias explícitas** das variáveis livres do escopo envolvente
//! (`let __cap_x = x.clone();`). Isso dá, de graça, a semântica da Seção 4.5: captura **por cópia,
//! congelada no momento da definição**. Atribuir a um nome capturado é erro
//! `assign-to-captured`.
//!
//! Recursão direta (`nome = fn(x){ … nome(…) … }`) usa
//! [`xpr_rt::recursive_closure`], que constrói a auto-referência.
//!
//! ## Módulos (subetapa B)
//!
//! Cada módulo XPR vira um `mod` Rust com uma `pub fn` por definição, retornando o `Value` daquela
//! definição. `alias.membro` vira `modulo::membro()`. Nada de *mangling* manual: o sistema de
//! módulos do Rust cuida das colisões, e o prefixo `xpr_mod_` isola os nomes dos módulos XPR do
//! resto do arquivo gerado.

pub mod types;

pub use types::{Ty, TypeChecker};

use std::collections::{HashMap, HashSet};

use crate::diag::DiagCode;
use crate::lexer::Span;
use crate::parser::ast::*;

/// Erro de compilação produzido pela análise/emissão.
#[derive(Debug, Clone)]
pub struct SemaError {
    pub code: DiagCode,
    pub span: Option<Span>,
    pub detail: String,
}

impl SemaError {
    fn new(code: DiagCode, span: Span, detail: impl Into<String>) -> Self {
        Self {
            code,
            span: Some(span),
            detail: detail.into(),
        }
    }
}

/// Nomes dos builtins da Seção 5.
pub const BUILTINS: &[&str] = &[
    "print", "println", "len", "input", "fail", "string", "format", "int", "real", "floor",
    "ceil", "round", "frac", "any", "all", "cat", "at", "fill", "range", "set", "slice", "apply",
];

/// Constantes predefinidas da Seção 6.
const CONSTANTS: &[(&str, &str)] = &[
    ("true", "Ok(xpr_rt::Value::Bool(true))"),
    ("false", "Ok(xpr_rt::Value::Bool(false))"),
    ("NaN", "Ok(xpr_rt::Value::Real(f64::NAN))"),
    ("INF", "Ok(xpr_rt::Value::Real(f64::INFINITY))"),
    ("pi", "Ok(xpr_rt::Value::Real(std::f64::consts::PI))"),
    ("E", "Ok(xpr_rt::Value::Real(std::f64::consts::E))"),
];

/// Como um nome visível se resolve no código gerado.
#[derive(Debug, Clone)]
enum Binding {
    /// Variável local: identificador Rust direto.
    Local(String),
    /// Cópia capturada por uma closure: identificador Rust do `__cap_*`.
    Captured(String),
    /// Nome pendente da própria função (recursão direta): recebido como `__self`.
    Pending(String),
}

/// Um módulo já parseado, pronto para emissão.
pub struct ModuleUnit {
    /// Nome lógico do módulo (nome do arquivo sem `.x`).
    pub name: String,
    /// Caminho do arquivo `.x`, usado nos diagnósticos.
    pub path: String,
    pub ast: Module,
}

/// Gera o código Rust completo de um programa (módulo de entrada + imports).
pub fn emit_program(entry: &str, modules: &[ModuleUnit]) -> Result<String, SemaError> {
    check_types(entry, modules)?;

    let mut out = String::new();
    out.push_str("// Gerado por xprc — não edite.\n");
    out.push_str("#![allow(clippy::all, unused_variables, unused_mut, dead_code)]\n");
    out.push_str("use xpr_rt::{self, Value, XprResult};\n\n");

    for unit in modules {
        out.push_str(&emit_module(unit)?);
        out.push('\n');
    }

    let entry_unit = modules
        .iter()
        .find(|unit| unit.name == entry)
        .expect("módulo de entrada ausente");
    let entry_mod = rust_module_name(&entry_unit.name);
    if !entry_unit.ast.definitions.iter().any(|def| def.name == "_main") {
        return Err(SemaError {
            code: DiagCode::MissingMain,
            span: None,
            detail: format!("{} (módulo de entrada {})", "_main", entry_unit.path),
        });
    }

    out.push_str("fn main() {\n");
    out.push_str("    xpr_rt::run_main_with(|| {\n");
    out.push_str(&format!(
        "        let __entry = {}::_main();\n",
        entry_mod
    ));
    out.push_str(
        "        xpr_rt::finish(xpr_rt::call(__entry, xpr_rt::program_args(), None));\n",
    );
    out.push_str("    });\n");
    out.push_str("}\n");
    Ok(out)
}

/// Verifica tipos de todos os módulos antes de qualquer emissão (Seção 1: "todo o programa é
/// verificado em termos de tipo antes que qualquer código seja gerado").
fn check_types(entry: &str, modules: &[ModuleUnit]) -> Result<(), SemaError> {
    // Os módulos vêm em ordem de carregamento (importados antes de quem importa), então cada um
    // pode registrar seus tipos para os seguintes.
    let mut exported: HashMap<String, HashMap<String, Ty>> = HashMap::new();

    // Fase 1 — descobre os tipos exportados por cada módulo. Repetimos até `modules.len()`
    // passadas para que a ordem da lista não importe (um módulo pode importar outro que aparece
    // depois dele); erros desta fase são ignorados, pois podem ser apenas dependências ainda não
    // resolvidas. Grafo acíclico é garantido pelo carregador de módulos.
    for _ in 0..modules.len() {
        for unit in modules {
            let mut checker = TypeChecker::new(&unit.path);
            for import in &unit.ast.imports {
                if let Some(members) = exported.get(&import.name) {
                    checker.register_import(import.effective_alias(), members.clone());
                }
            }
            if let Ok(members) = checker.check_module(&unit.ast) {
                exported.insert(unit.name.clone(), members);
            }
        }
    }

    // Fase 2 — checagem definitiva: agora qualquer erro é real e é relatado.
    for unit in modules {
        let mut checker = TypeChecker::new(&unit.path);
        for import in &unit.ast.imports {
            if let Some(members) = exported.get(&import.name) {
                checker.register_import(import.effective_alias(), members.clone());
            }
        }
        checker.check_module(&unit.ast)?;
    }
    let _ = entry;
    Ok(())
}

/// Nome Rust de um módulo XPR. O prefixo evita colisão com identificadores do arquivo gerado.
pub fn rust_module_name(xpr_name: &str) -> String {
    let mut name = String::from("xpr_mod_");
    for (i, c) in xpr_name.chars().enumerate() {
        let valid = c.is_ascii_alphanumeric() || c == '_';
        if valid && !(i == 0 && c.is_ascii_digit()) {
            name.push(c.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }
    name
}

/// Emite um módulo XPR como `mod` Rust com uma função por definição.
fn emit_module(unit: &ModuleUnit) -> Result<String, SemaError> {
    let mut out = String::new();
    out.push_str(&format!("pub mod {} {{\n", rust_module_name(&unit.name)));
    out.push_str("    use super::*;\n");

    let aliases: HashMap<String, String> = unit
        .ast
        .imports
        .iter()
        .map(|import| {
            (
                import.effective_alias().to_string(),
                rust_module_name(&import.name),
            )
        })
        .collect();

    // Definições visíveis até o ponto corrente: a spec exige uso após definição (Seção 4.6).
    let mut defined: HashSet<String> = HashSet::new();
    for def in &unit.ast.definitions {
        let mut emitter = Emitter::new(&unit.path, &aliases, def.name.clone());
        emitter.defined = defined.clone();
        let body = emitter.emit_definition(def)?;
        out.push_str(&body);
        defined.insert(def.name.clone());
    }
    out.push_str("}\n");
    Ok(out)
}

/// Emissor de uma definição dentro de um módulo.
struct Emitter<'a> {
    /// Arquivo `.x` de origem, para os diagnósticos de runtime.
    file: &'a str,
    /// Alias de import → nome do módulo Rust.
    aliases: &'a HashMap<String, String>,
    /// Nome da definição sendo emitida (nome pendente para recursão direta).
    current_def: String,
    /// Definições do módulo já disponíveis (uso após definição).
    defined: HashSet<String>,
    /// Pilha de escopos locais.
    scopes: Vec<HashMap<String, Binding>>,
    /// Nomes-alvo de uma atribuição cujo valor é uma `fn_def`: são "pendentes" (Seção 4.5) e
    /// referências a eles dentro do corpo resolvem para a própria função (`__self`).
    pending_self: Vec<String>,
    temp: usize,
}

impl<'a> Emitter<'a> {
    fn new(file: &'a str, aliases: &'a HashMap<String, String>, current_def: String) -> Self {
        Self {
            file,
            aliases,
            current_def,
            defined: HashSet::new(),
            scopes: vec![HashMap::new()],
            pending_self: Vec::new(),
            temp: 0,
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.temp += 1;
        format!("__{}{}", prefix, self.temp)
    }

    fn position(&self, span: Span) -> String {
        format!("Some(({}, {}))", span.line_start, span.col_start)
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding.clone());
            }
        }
        None
    }

    fn declare(&mut self, name: &str) -> String {
        let rust_name = sanitize_ident(name);
        self.scopes
            .last_mut()
            .expect("pilha de escopos vazia")
            .insert(name.to_string(), Binding::Local(rust_name.clone()));
        rust_name
    }

    // -----------------------------------------------------------------------------------------
    // Definições
    // -----------------------------------------------------------------------------------------

    /// Emite `pub fn <nome>() -> Value { … }`.
    fn emit_definition(&mut self, def: &Definition) -> Result<String, SemaError> {
        let mut out = String::new();
        out.push_str(&format!("    pub fn {}() -> Value {{\n", sanitize_ident(&def.name)));
        let body = match &def.initializer {
            Initializer::FnDef(fn_def) => {
                self.emit_fn_def(fn_def, Some(&def.name), def.span)?
            }
            Initializer::ConstExpr(expr) => self.emit_expr(expr)?,
        };
        // O corpo é um bloco cujo valor é `XprResult`; definições de módulo expõem o `Value`.
        out.push_str(&format!("        xpr_rt::force({})\n", indent(&body, 2).trim_end()));
        out.push_str("    }\n");
        Ok(out)
    }

    // -----------------------------------------------------------------------------------------
    // Expressões
    // -----------------------------------------------------------------------------------------

    /// Emite uma expressão como um **bloco Rust** cujo valor é `XprResult`.
    fn emit_expr(&mut self, expr: &Expr) -> Result<String, SemaError> {
        match expr {
            Expr::IntLit(text, _) => {
                let value: i64 = text.parse().unwrap_or(0);
                Ok(format!("Ok(xpr_rt::Value::Int({}))", value))
            }
            Expr::RealLit(text, _) => {
                let value: f64 = text.parse().unwrap_or(0.0);
                Ok(format!("Ok(xpr_rt::Value::Real({:?}))", value))
            }
            Expr::StringLit(decoded, _, _) => {
                Ok(format!("Ok(xpr_rt::Value::string({}))", rust_string(decoded)))
            }
            Expr::QualifiedName {
                module_alias,
                name,
                span,
            } => self.emit_name(module_alias.as_deref(), name, *span),
            Expr::Assignment { target, value, span } => {
                self.emit_assignment(target, value, *span)
            }
            Expr::FnDef(fn_def) => self.emit_fn_def(fn_def, None, fn_def.span),
            Expr::LogicalOr(operands) => self.emit_chain(operands, true),
            Expr::LogicalAnd(operands) => self.emit_chain(operands, false),
            Expr::LogicalNot(operand, span) => {
                let inner = self.emit_expr(operand)?;
                Ok(format!(
                    "xpr_rt::ops::not({{ {} }}?, {})",
                    inner,
                    self.position(*span)
                ))
            }
            Expr::Binary {
                op,
                lhs,
                rhs,
                span,
            } => {
                let left = self.emit_expr(lhs)?;
                let right = self.emit_expr(rhs)?;
                let func = match op {
                    BinOp::Add => "add",
                    BinOp::Sub => "sub",
                    BinOp::Mul => "mul",
                    BinOp::Div => "div",
                    BinOp::Mod => "rem",
                    BinOp::Lt => "lt",
                    BinOp::Gt => "gt",
                    BinOp::Le => "le",
                    BinOp::Ge => "ge",
                    BinOp::EqEq => "eq",
                    BinOp::NotEq => "ne",
                };
                Ok(format!(
                    "xpr_rt::ops::{}({{ {} }}?, {{ {} }}?, {})",
                    func,
                    left,
                    right,
                    self.position(*span)
                ))
            }
            Expr::Unary {
                op,
                operand,
                span,
                ..
            } => {
                let inner = self.emit_expr(operand)?;
                let func = match op {
                    UnOp::Plus => "plus",
                    UnOp::Minus => "neg",
                };
                Ok(format!(
                    "xpr_rt::ops::{}({{ {} }}?, {})",
                    func,
                    inner,
                    self.position(*span)
                ))
            }
            Expr::Indexed {
                base,
                indices,
                span,
            } => {
                // `v[a, b, c]` é açúcar para `v[a][b][c]` (Seção 4.8): cada índice consome um
                // nível. Emitimos um bloco com `let`s encadeados para que a expressão final seja
                // exatamente um `XprResult` (e não um `Value` com `?` sobrando).
                let mut out = String::from("{\n");
                let current = self.fresh("base");
                out.push_str(&format!("  let {} = {{ {} }}?;\n", current, self.emit_expr(base)?));
                let mut current = current;
                for index in indices {
                    let index_ident = self.fresh("i");
                    out.push_str(&format!(
                        "  let {} = {{ {} }}?;\n",
                        index_ident,
                        self.emit_expr(index)?
                    ));
                    let next = self.fresh("v");
                    out.push_str(&format!(
                        "  let {} = xpr_rt::ops::index({}, {}, {})?;\n",
                        next,
                        current,
                        index_ident,
                        self.position(*span)
                    ));
                    current = next;
                }
                out.push_str(&format!("  Ok({})\n}}", current));
                Ok(out)
            }
            Expr::FnCall { target, arg, span } => self.emit_call(target, arg.as_deref(), *span),
            Expr::Block(block) => self.emit_block(block),
            Expr::Iif {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_code = self.emit_expr(cond)?;
                let then_code = self.emit_expr(then_branch)?;
                let else_code = self.emit_expr(else_branch)?;
                let cond_var = self.fresh("c");
                Ok(format!(
                    "{{\n  let {} = {{ {} }}?;\n  if xpr_rt::as_bool(&{}, {})? {{\n    {}\n  }} else {{\n    {}\n  }}\n}}",
                    cond_var,
                    cond_code,
                    cond_var,
                    self.position(*span),
                    indent(&then_code, 2),
                    indent(&else_code, 2),
                ))
            }
            Expr::For {
                init,
                cond,
                step,
                body,
                span,
            } => self.emit_for(init, cond, step, body, *span),
            Expr::Try(inner, _) => {
                // `try(e)` devolve sempre `[bool, T]` (Seção 4.9); no caminho de falha o segundo
                // elemento é o valor neutro do tipo de `e`. A anotação de tipo no `match` deixa o
                // `Ok(...)` do caminho de sucesso com tipo conhecido.
                let code = self.emit_expr(inner)?;
                let neutral = self.neutral_expr(inner);
                // O corpo precisa rodar **dentro de uma closure**: os `?` que ele contém têm de
                // retornar dela (virando o `Err` capturado), e não propagar pela função inteira.
                Ok(format!(
                    "{{\n  let __attempt: XprResult = (|| -> XprResult {{ {} }})();\n  match __attempt {{\n    Ok(__v) => Ok(xpr_rt::Value::seq(vec![xpr_rt::Value::Bool(true), __v])),\n    Err(_) => Ok(xpr_rt::Value::seq(vec![xpr_rt::Value::Bool(false), {}])),\n  }}\n}}",
                    code, neutral
                ))
            }
            Expr::BracketLiteral(elements, _) => {
                let mut parts = Vec::with_capacity(elements.len());
                for element in elements {
                    parts.push(format!("{{ {} }}?", self.emit_expr(element)?));
                }
                Ok(format!(
                    "Ok(xpr_rt::Value::seq(vec![{}]))",
                    parts.join(", ")
                ))
            }
        }
    }

    /// Expressão Rust que produz o **valor neutro** do tipo de `expr` (Seção 4.9).
    ///
    /// A escolha é feita pela forma sintática da expressão, que é o que o emissor tem em mãos: um
    /// builtin de conversão determina o tipo de destino (`int(...)` → `0`, `real(...)` → `0.0`,
    /// `string(...)` → `""`); um literal de string → `""`; uma comparação ou `and`/`or`/`not` →
    /// `false`; um literal real → `0.0`; um literal entre colchetes → `[]`; e todo o resto → `0`.
    fn neutral_expr(&self, expr: &Expr) -> String {
        let value = match expr {
            Expr::StringLit(_, _, _) => "xpr_rt::Value::string(\"\")".to_string(),
            Expr::RealLit(_, _) => "xpr_rt::Value::Real(0.0)".to_string(),
            Expr::IntLit(_, _) => "xpr_rt::Value::Int(0)".to_string(),
            Expr::BracketLiteral(_, _) => "xpr_rt::Value::seq(vec![])".to_string(),
            Expr::Binary { op, .. } => match op {
                BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::EqEq | BinOp::NotEq => {
                    "xpr_rt::Value::Bool(false)".to_string()
                }
                _ => "xpr_rt::Value::Int(0)".to_string(),
            },
            Expr::LogicalOr(_) | Expr::LogicalAnd(_) | Expr::LogicalNot(_, _) => {
                "xpr_rt::Value::Bool(false)".to_string()
            }
            Expr::FnCall { target, .. } => match target.as_ref() {
                Expr::QualifiedName {
                    module_alias: None,
                    name,
                    ..
                } => match name.as_str() {
                    "string" | "format" => "xpr_rt::Value::string(\"\")".to_string(),
                    "real" | "floor" | "ceil" | "round" | "frac" => {
                        "xpr_rt::Value::Real(0.0)".to_string()
                    }
                    "any" | "all" => "xpr_rt::Value::Bool(false)".to_string(),
                    "print" | "println" => "xpr_rt::Value::Int(0)".to_string(),
                    _ => "xpr_rt::Value::Int(0)".to_string(),
                },
                _ => "xpr_rt::Value::Int(0)".to_string(),
            },
            _ => "xpr_rt::Value::Int(0)".to_string(),
        };
        value
    }

    fn emit_name(
        &mut self,
        module_alias: Option<&str>,
        name: &str,
        span: Span,
    ) -> Result<String, SemaError> {
        if let Some(alias) = module_alias {
            let module = match self.aliases.get(alias) {
                Some(module) => module.clone(),
                None => {
                    return Err(SemaError::new(
                        DiagCode::UndefinedIdentifier,
                        span,
                        format!("o alias de módulo {}", alias),
                    ))
                }
            };
            return Ok(format!("Ok({}::{}())", module, sanitize_ident(name)));
        }

        if let Some(binding) = self.lookup(name) {
            let ident = match binding {
                Binding::Local(rust) | Binding::Captured(rust) | Binding::Pending(rust) => rust,
            };
            return Ok(format!("Ok({}.clone())", ident));
        }
        if name == self.current_def {
            // Recursão direta: `__self` é injetado pela closure recursiva.
            return Ok("Ok(__self.clone())".to_string());
        }
        if self.defined.contains(name) {
            return Ok(format!("Ok({}())", sanitize_ident(name)));
        }
        if let Some((_, code)) = CONSTANTS.iter().find(|(constant, _)| *constant == name) {
            return Ok((*code).to_string());
        }
        if BUILTINS.contains(&name) {
            // Builtin usado como valor de primeira classe (`use_func(print)` é válido).
            if let Some(code) = builtin_value(name) {
                return Ok(code);
            }
        }
        Err(SemaError::new(
            DiagCode::UndefinedIdentifier,
            span,
            format!("{} (em {})", name, self.file),
        ))
    }

    fn emit_assignment(
        &mut self,
        target: &BindingPattern,
        value: &Expr,
        span: Span,
    ) -> Result<String, SemaError> {
        let value_code = self.emit_expr(value)?;
        let mut out = String::new();
        let temp = self.fresh("v");
        out.push_str(&format!("let {} = {{ {} }}?;\n", temp, value_code));
        self.emit_bind_pattern(&mut out, target, &temp, span)?;
        out.push_str(&format!("Ok({})", temp));
        Ok(out)
    }

    /// Liga um valor já avaliado a um padrão, declarando nomes novos e atribuindo aos existentes.
    fn emit_bind_pattern(
        &mut self,
        out: &mut String,
        pattern: &BindingPattern,
        value_ident: &str,
        span: Span,
    ) -> Result<(), SemaError> {
        match pattern {
            BindingPattern::Name(name, name_span) => {
                match self.lookup(name) {
                    Some(Binding::Local(rust)) => {
                        out.push_str(&format!("{} = {}.clone();\n", rust, value_ident));
                    }
                    Some(Binding::Captured(_)) => {
                        return Err(SemaError::new(
                            DiagCode::AssignToCaptured,
                            *name_span,
                            format!("{} (em {})", name, self.file),
                        ))
                    }
                    Some(Binding::Pending(_)) => {
                        // Fim do "nome pendente": agora o valor existe e vira uma variável local
                        // comum (o `__self` da closure já está resolvido lá dentro).
                        let rust = sanitize_ident(name);
                        self.scopes
                            .last_mut()
                            .unwrap()
                            .insert(name.clone(), Binding::Local(rust.clone()));
                        out.push_str(&format!("let mut {} = {}.clone();\n", rust, value_ident));
                    }
                    None => {
                        let rust = self.declare(name);
                        // `mut`: uma variável XPR pode ser reatribuída em qualquer ponto do seu
                        // escopo (Seção 4.6); o *tipo* é que fica fixo, e isso é checado à parte.
                        out.push_str(&format!("let mut {} = {}.clone();\n", rust, value_ident));
                    }
                }
                Ok(())
            }
            BindingPattern::List(items, list_span) => {
                let items_var = self.fresh("items");
                out.push_str(&format!(
                    "let {} = xpr_rt::expect_seq(&{}, {}, {})?;\n",
                    items_var,
                    value_ident,
                    items.len(),
                    self.position(*list_span)
                ));
                for (index, item) in items.iter().enumerate() {
                    let element = format!("{}[{}].clone()", items_var, index);
                    let item_var = self.fresh("e");
                    out.push_str(&format!("let {} = {};\n", item_var, element));
                    self.emit_bind_pattern(out, item, &item_var, span)?;
                }
                Ok(())
            }
        }
    }

    fn emit_chain(&mut self, operands: &[Expr], is_or: bool) -> Result<String, SemaError> {
        // `a or b or c` vira um encadeamento de curto-circuito, avaliando da esquerda para a
        // direita e parando no primeiro verdadeiro (`or`) ou no primeiro falso (`and`).
        if operands.is_empty() {
            return Ok("Ok(xpr_rt::Value::Bool(false))".to_string());
        }
        let first = self.emit_expr(&operands[0])?;
        if operands.len() == 1 {
            return Ok(first);
        }
        let rest = self.emit_chain(&operands[1..], is_or)?;
        let temp = self.fresh("l");
        let (short_circuit, continuation) = if is_or { ("true", "false") } else { ("false", "true") };
        Ok(format!(
            "{{\n  let {} = {{ {} }}?;\n  if xpr_rt::as_bool(&{}, None)? == {} {{\n    Ok(xpr_rt::Value::Bool({}))\n  }} else {{\n    {}\n  }}\n}}",
            temp,
            first,
            temp,
            short_circuit,
            short_circuit,
            rest
        ))
        .map(|code| {
            let _ = continuation;
            code
        })
    }

    fn emit_block(&mut self, block: &Block) -> Result<String, SemaError> {
        let mut out = String::new();
        out.push_str("{\n");
        self.scopes.push(HashMap::new());
        if block.statements.is_empty() {
            out.push_str("  Ok(xpr_rt::Value::seq(vec![]))\n");
        } else {
            let last_index = block.statements.len() - 1;
            for (index, stmt) in block.statements.iter().enumerate() {
                let is_last = index == last_index;
                let code = match stmt {
                    Stmt::Expr(expr) => self.emit_statement(expr, is_last)?,
                    Stmt::Import(_) => {
                        return Err(SemaError {
                            code: DiagCode::SyntaxError,
                            span: Some(block.span),
                            detail: "import dentro de bloco".to_string(),
                        })
                    }
                };
                out.push_str(&indent(&code, 1));
            }
        }
        self.scopes.pop();
        out.push_str("}");
        Ok(out)
    }

    /// Emite uma expressão **em posição de statement** de um bloco.
    ///
    /// A diferença em relação a [`Self::emit_expr`] é o escopo das declarações: em XPR, uma
    /// atribuição a um nome novo cria a variável no **escopo corrente** (Seção 4.6), de modo que
    /// ela continua visível nos statements seguintes do mesmo bloco. Por isso as atribuições
    /// (inclusive cadeias `a = b = expr`) são emitidas **inline**, sem envolver o código em um
    /// bloco `{}` próprio.
    fn emit_statement(&mut self, expr: &Expr, is_last: bool) -> Result<String, SemaError> {
        match expr {
            Expr::Assignment { .. } => {
                let mut out = String::new();
                let value_ident = self.emit_assignment_spine(&mut out, expr)?;
                if is_last {
                    out.push_str(&format!("Ok({})\n", value_ident));
                }
                Ok(out)
            }
            other => {
                let code = self.emit_expr(other)?;
                if is_last {
                    Ok(format!("{}\n", code))
                } else {
                    Ok(format!("let _ = {{ {} }}?;\n", code))
                }
            }
        }
    }

    /// Descarrega uma cadeia de atribuições (`a = b = expr`) em um bloco de statements, devolvendo
    /// o identificador Rust que guarda o valor atribuído.
    fn emit_assignment_spine(
        &mut self,
        out: &mut String,
        expr: &Expr,
    ) -> Result<String, SemaError> {
        match expr {
            Expr::Assignment { target, value, span } => {
                // A cadeia é associativa à direita: primeiro resolve o valor mais à direita...
                let inner = match value.as_ref() {
                    Expr::Assignment { .. } => {
                        Some(self.emit_assignment_spine(out, value)?)
                    }
                    other => {
                        // Se o valor é uma `fn_def`, os alvos da cadeia já são "nomes pendentes"
                        // antes de o valor existir (Seção 4.5): a função pode se auto-referenciar.
                        let mut pending: Vec<String> = Vec::new();
                        if matches!(other, Expr::FnDef(_)) {
                            collect_target_names(target, &mut pending);
                        }
                        for name in &pending {
                            self.scopes.last_mut().unwrap().insert(
                                name.clone(),
                                Binding::Pending("__self".to_string()),
                            );
                        }
                        let saved = std::mem::replace(&mut self.pending_self, pending.clone());
                        let emitted = self.emit_expr(other);
                        self.pending_self = saved;
                        let code = emitted?;
                        let ident = self.fresh("v");
                        out.push_str(&format!("let {} = {{ {} }}?;\n", ident, code));
                        Some(ident)
                    }
                };
                let value_ident = inner.expect("cadeia de atribuição sem valor");
                // ...e então liga os alvos, do mais interno para o mais externo.
                self.emit_bind_pattern(out, target, &value_ident, *span)?;
                Ok(value_ident)
            }
            other => {
                let code = self.emit_expr(other)?;
                let ident = self.fresh("v");
                out.push_str(&format!("let {} = {{ {} }}?;\n", ident, code));
                Ok(ident)
            }
        }
    }

    fn emit_for(
        &mut self,
        init: &Expr,
        cond: &Expr,
        step: &Expr,
        body: &Expr,
        span: Span,
    ) -> Result<String, SemaError> {
        let mut out = String::new();
        out.push_str("{\n");
        // O escopo do `for` abriga as variáveis criadas no `init` (visíveis em cond/step/body).
        self.scopes.push(HashMap::new());

        // `init` inline, para que suas declarações fiquem visíveis em cond/step/body.
        let init_code = self.emit_statement(init, false)?;
        out.push_str(&indent(&init_code, 1));

        let count = self.fresh("n");
        out.push_str(&format!("  let mut {} = 0i64;\n", count));
        out.push_str("  loop {\n");
        let cond_code = self.emit_expr(cond)?;
        let cond_var = self.fresh("c");
        out.push_str(&format!("    let {} = {{ {} }}?;\n", cond_var, cond_code));
        out.push_str(&format!(
            "    if !xpr_rt::as_bool(&{}, {})? {{ break; }}\n",
            cond_var,
            self.position(span)
        ));
        let body_code = self.emit_statement(body, false)?;
        out.push_str(&indent(&body_code, 2));
        out.push_str(&format!("    {} += 1;\n", count));
        let step_code = self.emit_statement(step, false)?;
        out.push_str(&indent(&step_code, 2));
        out.push_str("  }\n");
        out.push_str(&format!("  Ok(xpr_rt::Value::Int({}))\n", count));
        self.scopes.pop();
        out.push_str("}");
        Ok(out)
    }

    fn emit_call(
        &mut self,
        target: &Expr,
        arg: Option<&Expr>,
        span: Span,
    ) -> Result<String, SemaError> {
        // Builtins são resolvidos em tempo de compilação (Seção 5: sem custo em tempo de execução).
        if let Expr::QualifiedName {
            module_alias: None,
            name,
            ..
        } = target
        {
            if BUILTINS.contains(&name.as_str()) {
                let arg_code = match arg {
                    Some(arg) => format!("{{ {} }}?", self.emit_expr(arg)?),
                    // `f()` equivale a `f([])` (Seção 4.5).
                    None => "xpr_rt::Value::seq(vec![])".to_string(),
                };
                let call = match name.as_str() {
                    "print" => format!("xpr_rt::bi::print({})", arg_code),
                    "println" => format!("xpr_rt::bi::println({})", arg_code),
                    "len" => format!("xpr_rt::bi::len({}, {})", arg_code, self.position(span)),
                    "input" => format!("xpr_rt::bi::input({}, {})", arg_code, self.position(span)),
                    "fail" => format!("xpr_rt::bi::fail({}, {})", arg_code, self.position(span)),
                    "string" => format!("xpr_rt::bi::to_string({})", arg_code),
                    "format" => format!("xpr_rt::bi::format({}, {})", arg_code, self.position(span)),
                    "int" => format!("xpr_rt::bi::to_int({}, {})", arg_code, self.position(span)),
                    "real" => format!("xpr_rt::bi::to_real({}, {})", arg_code, self.position(span)),
                    "floor" => format!("xpr_rt::bi::floor({}, {})", arg_code, self.position(span)),
                    "ceil" => format!("xpr_rt::bi::ceil({}, {})", arg_code, self.position(span)),
                    "round" => format!("xpr_rt::bi::round({}, {})", arg_code, self.position(span)),
                    "frac" => format!("xpr_rt::bi::frac({}, {})", arg_code, self.position(span)),
                    "any" => format!("xpr_rt::bi::any({}, {})", arg_code, self.position(span)),
                    "all" => format!("xpr_rt::bi::all({}, {})", arg_code, self.position(span)),
                    "cat" => format!("xpr_rt::bi::cat({}, {})", arg_code, self.position(span)),
                    "at" => format!("xpr_rt::bi::at({}, {})", arg_code, self.position(span)),
                    "fill" => format!("xpr_rt::bi::fill({}, {})", arg_code, self.position(span)),
                    "range" => format!("xpr_rt::bi::range({}, {})", arg_code, self.position(span)),
                    "set" => format!("xpr_rt::bi::set({}, {})", arg_code, self.position(span)),
                    "slice" => format!("xpr_rt::bi::slice({}, {})", arg_code, self.position(span)),
                    "apply" => format!("xpr_rt::bi::apply({}, {})", arg_code, self.position(span)),
                    _ => unreachable!("builtin não mapeado"),
                };
                return Ok(call);
            }
        }

        let target_code = self.emit_expr(target)?;
        let arg_code = match arg {
            Some(arg) => format!("{{ {} }}?", self.emit_expr(arg)?),
            None => "xpr_rt::Value::seq(vec![])".to_string(),
        };
        let target_var = self.fresh("f");
        Ok(format!(
            "{{\n  let {} = {{ {} }}?;\n  xpr_rt::call({}, {}, {})\n}}",
            target_var,
            target_code,
            target_var,
            arg_code,
            self.position(span)
        ))
    }

    /// Emite uma `fn_def`. Quando `self_name` existe e o corpo referencia o próprio nome, usa a
    /// construção recursiva (auto-referência) de `xpr_rt`.
    fn emit_fn_def(
        &mut self,
        fn_def: &FnDef,
        self_name: Option<&str>,
        span: Span,
    ) -> Result<String, SemaError> {
        // Nomes que, dentro do corpo, se referem à própria função: o nome da definição de módulo
        // (`f = fn(x){ f(x - 1); }`) ou os alvos pendentes de uma atribuição local
        // (`fat = fn(x){ … fat(…) … }`, Seção 4.5).
        let recursive_names: Vec<String> = match self_name {
            Some(name) => vec![name.to_string()],
            None => self.pending_self.clone(),
        };
        let recursive = recursive_names
            .iter()
            .any(|name| self_name_referenced(&fn_def.body, name));

        // Capturas: nomes livres do corpo que existem como variáveis locais no escopo atual.
        // Nomes pendentes (a própria função) não são capturados por cópia: eles são `__self`.
        let free = free_names(&fn_def.body, &param_names(&fn_def.param));
        let mut captures: Vec<(String, String)> = Vec::new();
        for name in &free {
            match self.lookup(name) {
                Some(Binding::Pending(_)) => {}
                Some(Binding::Local(rust)) | Some(Binding::Captured(rust)) => {
                    captures.push((name.clone(), rust));
                }
                None => {}
            }
        }

        let mut out = String::new();
        out.push_str("{\n");
        for (name, rust) in &captures {
            out.push_str(&format!("  let __cap_{} = {}.clone();\n", sanitize_ident(name), rust));
        }

        // O corpo é emitido em um escopo próprio, com os parâmetros e as capturas.
        self.scopes.push(HashMap::new());
        for (name, _) in &captures {
            self.scopes.last_mut().unwrap().insert(
                name.clone(),
                Binding::Captured(format!("__cap_{}", sanitize_ident(name))),
            );
        }
        let params = param_names(&fn_def.param);
        for name in &params {
            self.scopes
                .last_mut()
                .unwrap()
                .insert(name.clone(), Binding::Local(sanitize_ident(name)));
        }
        if recursive {
            for name in &recursive_names {
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), Binding::Pending("__self".to_string()));
            }
        }

        let mut body = String::new();
        body.push_str("{\n");
        self.emit_param_pattern(&mut body, &fn_def.param, "__arg", span)?;
        let body_code = self.emit_block(&fn_def.body)?;
        body.push_str(&format!("  {}\n", body_code.trim_start_matches("{\n").trim_end_matches('}')));
        body.push_str("}");
        self.scopes.pop();

        let indent_captures = 2;
        if recursive {
            out.push_str(&format!(
                "  let __this = xpr_rt::recursive_closure(move |__self: Value, __arg: Value| -> XprResult {{\n{}\n  }});\n",
                indent(&body, indent_captures)
            ));
            out.push_str("  Ok(__this)\n");
        } else {
            out.push_str(&format!(
                "  Ok(xpr_rt::Value::function(move |__arg: Value| -> XprResult {{\n{}\n  }}))\n",
                indent(&body, indent_captures)
            ));
        }
        out.push_str("}");
        Ok(out)
    }

    /// Desestrutura o parâmetro (`__arg`) nos nomes do padrão.
    fn emit_param_pattern(
        &mut self,
        out: &mut String,
        pattern: &ParamPattern,
        value_ident: &str,
        span: Span,
    ) -> Result<(), SemaError> {
        match pattern {
            ParamPattern::Name { name, .. } => {
                out.push_str(&format!(
                    "let mut {} = {}.clone();\n",
                    sanitize_ident(name),
                    value_ident
                ));
                Ok(())
            }
            ParamPattern::List(items, list_span) => {
                let items_var = self.fresh("p");
                out.push_str(&format!(
                    "let {} = xpr_rt::expect_seq(&{}, {}, {})?;\n",
                    items_var,
                    value_ident,
                    items.len(),
                    self.position(*list_span)
                ));
                for (index, item) in items.iter().enumerate() {
                    let element = format!("{}[{}].clone()", items_var, index);
                    self.emit_param_pattern(out, item, &element, span)?;
                }
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Análise auxiliar: nomes livres e nomes de parâmetro
// ---------------------------------------------------------------------------------------------

fn param_names(pattern: &ParamPattern) -> Vec<String> {
    match pattern {
        ParamPattern::Name { name, .. } => vec![name.clone()],
        ParamPattern::List(items, _) => items.iter().flat_map(param_names).collect(),
    }
}

/// `true` se o corpo referencia o próprio nome da função sendo definida (recursão direta).
fn self_name_referenced(block: &Block, name: &str) -> bool {
    let mut found = false;
    for stmt in &block.statements {
        if let Stmt::Expr(expr) = stmt {
            visit_names(expr, &mut |used| {
                if used == name {
                    found = true;
                }
            });
        }
    }
    found
}

fn visit_names(expr: &Expr, f: &mut impl FnMut(&str)) {
    match expr {
        Expr::QualifiedName {
            module_alias: None,
            name,
            ..
        } => f(name),
        Expr::QualifiedName { .. } => {}
        Expr::IntLit(..) | Expr::RealLit(..) | Expr::StringLit(..) => {}
        Expr::Assignment { target, value, .. } => {
            visit_binding_pattern(target, f);
            visit_names(value, f);
        }
        Expr::FnDef(fn_def) => visit_block_names(&fn_def.body, f),
        Expr::LogicalOr(operands) | Expr::LogicalAnd(operands) | Expr::BracketLiteral(operands, _) => {
            for operand in operands {
                visit_names(operand, f);
            }
        }
        Expr::LogicalNot(operand, _) | Expr::Unary { operand, .. } | Expr::Try(operand, _) => {
            visit_names(operand, f)
        }
        Expr::Binary { lhs, rhs, .. } => {
            visit_names(lhs, f);
            visit_names(rhs, f);
        }
        Expr::Indexed { base, indices, .. } => {
            visit_names(base, f);
            for index in indices {
                visit_names(index, f);
            }
        }
        Expr::FnCall { target, arg, .. } => {
            visit_names(target, f);
            if let Some(arg) = arg {
                visit_names(arg, f);
            }
        }
        Expr::Block(block) => visit_block_names(block, f),
        Expr::Iif {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            visit_names(cond, f);
            visit_names(then_branch, f);
            visit_names(else_branch, f);
        }
        Expr::For {
            init, cond, step, body, ..
        } => {
            visit_names(init, f);
            visit_names(cond, f);
            visit_names(step, f);
            visit_names(body, f);
        }
    }
}

fn visit_binding_pattern(pattern: &BindingPattern, f: &mut impl FnMut(&str)) {
    match pattern {
        BindingPattern::Name(_, _) => {}
        BindingPattern::List(items, _) => {
            for item in items {
                visit_binding_pattern(item, f);
            }
        }
    }
}

fn visit_block_names(block: &Block, f: &mut impl FnMut(&str)) {
    for stmt in &block.statements {
        if let Stmt::Expr(expr) = stmt {
            visit_names(expr, f);
        }
    }
}

/// Nomes usados no corpo que **não** são ligados dentro dele (candidatos a captura).
fn free_names(block: &Block, params: &[String]) -> Vec<String> {
    let bound: HashSet<String> = params.iter().cloned().collect();
    let mut free: Vec<String> = Vec::new();
    collect_free_block(block, &mut vec![bound.clone()], &mut free);
    free
}

fn collect_free_block(
    block: &Block,
    scopes: &mut Vec<HashSet<String>>,
    free: &mut Vec<String>,
) {
    scopes.push(HashSet::new());
    for stmt in &block.statements {
        if let Stmt::Expr(expr) = stmt {
            collect_free_expr(expr, scopes, free);
        }
    }
    scopes.pop();
}

fn is_bound(scopes: &[HashSet<String>], name: &str) -> bool {
    scopes.iter().any(|scope| scope.contains(name))
}

fn collect_free_expr(expr: &Expr, scopes: &mut Vec<HashSet<String>>, free: &mut Vec<String>) {
    match expr {
        Expr::QualifiedName {
            module_alias: None,
            name,
            ..
        } => {
            if !is_bound(scopes, name) && !free.iter().any(|existing| existing == name) {
                free.push(name.clone());
            }
        }
        Expr::QualifiedName { .. }
        | Expr::IntLit(..)
        | Expr::RealLit(..)
        | Expr::StringLit(..) => {}
        Expr::Assignment { target, value, .. } => {
            collect_free_expr(value, scopes, free);
            collect_bind_pattern_free(target, scopes, free);
        }
        Expr::FnDef(fn_def) => {
            let params = param_names(&fn_def.param);
            scopes.push(params.into_iter().collect());
            collect_free_block(&fn_def.body, scopes, free);
            scopes.pop();
        }
        Expr::LogicalOr(operands) | Expr::LogicalAnd(operands) | Expr::BracketLiteral(operands, _) => {
            for operand in operands {
                collect_free_expr(operand, scopes, free);
            }
        }
        Expr::LogicalNot(operand, _) | Expr::Unary { operand, .. } | Expr::Try(operand, _) => {
            collect_free_expr(operand, scopes, free)
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_free_expr(lhs, scopes, free);
            collect_free_expr(rhs, scopes, free);
        }
        Expr::Indexed { base, indices, .. } => {
            collect_free_expr(base, scopes, free);
            for index in indices {
                collect_free_expr(index, scopes, free);
            }
        }
        Expr::FnCall { target, arg, .. } => {
            collect_free_expr(target, scopes, free);
            if let Some(arg) = arg {
                collect_free_expr(arg, scopes, free);
            }
        }
        Expr::Block(block) => collect_free_block(block, scopes, free),
        Expr::Iif {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_free_expr(cond, scopes, free);
            collect_free_expr(then_branch, scopes, free);
            collect_free_expr(else_branch, scopes, free);
        }
        Expr::For {
            init, cond, step, body, ..
        } => {
            scopes.push(HashSet::new());
            collect_free_expr(init, scopes, free);
            collect_free_expr(cond, scopes, free);
            collect_free_expr(step, scopes, free);
            collect_free_expr(body, scopes, free);
            scopes.pop();
        }
    }
}

fn collect_bind_pattern_free(
    pattern: &BindingPattern,
    scopes: &mut Vec<HashSet<String>>,
    free: &mut Vec<String>,
) {
    match pattern {
        BindingPattern::Name(name, _) => {
            // Uma atribuição a um nome não ligado localmente é um uso (captura) — o emissor
            // reporta `assign-to-captured` se ele existir no escopo envolvente.
            if !is_bound(scopes, name) && !free.iter().any(|existing| existing == name) {
                free.push(name.clone());
            }
            scopes.last_mut().unwrap().insert(name.clone());
        }
        BindingPattern::List(items, _) => {
            for item in items {
                collect_bind_pattern_free(item, scopes, free);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Utilidades de texto
// ---------------------------------------------------------------------------------------------

/// Identificador Rust seguro para um nome XPR. Nomes XPR são ASCII alfanuméricos + `_`, mas podem
/// coincidir com palavras-chave do Rust (`fn`, `for`, `loop`, ...), então prefixamos quando
/// necessário.
fn sanitize_ident(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
    ];
    if RESERVED.contains(&name) || name == "_" {
        format!("x_{}", name)
    } else {
        name.to_string()
    }
}

/// Literal de string Rust válido para um texto arbitrário.
fn rust_string(text: &str) -> String {
    format!("{:?}", text)
}

/// Indenta cada linha de um trecho.
fn indent(text: &str, levels: usize) -> String {
    let pad = "  ".repeat(levels);
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", pad, line)
            }
        })
        .map(|line| format!("{}\n", line))
        .collect()
}

fn builtin_value(name: &str) -> Option<String> {
    let value = match name {
        "print" => "xpr_rt::Value::function(xpr_rt::bi::print)",
        "println" => "xpr_rt::Value::function(xpr_rt::bi::println)",
        "string" => "xpr_rt::Value::function(xpr_rt::bi::to_string)",
        "len" => "xpr_rt::Value::function(|v| xpr_rt::bi::len(v, None))",
        "input" => "xpr_rt::Value::function(|v| xpr_rt::bi::input(v, None))",
        "fail" => "xpr_rt::Value::function(|v| xpr_rt::bi::fail(v, None))",
        "format" => "xpr_rt::Value::function(|v| xpr_rt::bi::format(v, None))",
        "int" => "xpr_rt::Value::function(|v| xpr_rt::bi::to_int(v, None))",
        "real" => "xpr_rt::Value::function(|v| xpr_rt::bi::to_real(v, None))",
        "floor" => "xpr_rt::Value::function(|v| xpr_rt::bi::floor(v, None))",
        "ceil" => "xpr_rt::Value::function(|v| xpr_rt::bi::ceil(v, None))",
        "round" => "xpr_rt::Value::function(|v| xpr_rt::bi::round(v, None))",
        "frac" => "xpr_rt::Value::function(|v| xpr_rt::bi::frac(v, None))",
        "any" => "xpr_rt::Value::function(|v| xpr_rt::bi::any(v, None))",
        "all" => "xpr_rt::Value::function(|v| xpr_rt::bi::all(v, None))",
        "cat" => "xpr_rt::Value::function(|v| xpr_rt::bi::cat(v, None))",
        "at" => "xpr_rt::Value::function(|v| xpr_rt::bi::at(v, None))",
        "fill" => "xpr_rt::Value::function(|v| xpr_rt::bi::fill(v, None))",
        "range" => "xpr_rt::Value::function(|v| xpr_rt::bi::range(v, None))",
        "set" => "xpr_rt::Value::function(|v| xpr_rt::bi::set(v, None))",
        "slice" => "xpr_rt::Value::function(|v| xpr_rt::bi::slice(v, None))",
        "apply" => "xpr_rt::Value::function(|v| xpr_rt::bi::apply(v, None))",
        _ => return None,
    };
    Some(format!("Ok({})", value))
}

/// Nomes simples de um padrão de ligação, na ordem.
fn collect_target_names(pattern: &BindingPattern, out: &mut Vec<String>) {
    match pattern {
        BindingPattern::Name(name, _) => out.push(name.clone()),
        BindingPattern::List(items, _) => {
            for item in items {
                collect_target_names(item, out);
            }
        }
    }
}
