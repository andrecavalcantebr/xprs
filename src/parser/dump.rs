//! Dump textual da AST, no formato normativo de `plano-parser.md`
//! (seção "Formato do dump de AST").
//!
//! Regras:
//! * 2 espaços por nível de indentação, um nó por linha, sem parênteses de fechamento;
//! * spans **não** aparecem (já são cobertos pelos fixtures do lexer);
//! * filhos rotulados ficam na forma `<rótulo>: <tag do nó>` na mesma linha, com os filhos desse
//!   nó indentados apenas um nível além;
//! * filhos sem rótulo viram linhas próprias, um nível abaixo do pai.

use super::ast::*;

pub fn dump_module(module: &Module) -> String {
    let mut out = String::new();
    out.push_str("Module");
    for import in &module.imports {
        out.push('\n');
        dump_import(&mut out, 1, import);
    }
    for def in &module.definitions {
        out.push('\n');
        dump_definition(&mut out, 1, def);
    }
    out
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn dump_import(out: &mut String, level: usize, import: &ImportDecl) {
    indent(out, level);
    out.push_str("Import name=");
    out.push_str(&import.name);
    out.push_str(" alias=");
    out.push_str(import.effective_alias());
}

fn dump_definition(out: &mut String, level: usize, def: &Definition) {
    indent(out, level);
    out.push_str("Def ");
    out.push_str(&def.name);
    out.push('\n');
    match &def.initializer {
        Initializer::FnDef(fn_def) => dump_fn_def(out, level + 1, fn_def),
        Initializer::ConstExpr(expr) => dump_expr(out, level + 1, expr),
    }
}

fn dump_fn_def(out: &mut String, level: usize, fn_def: &FnDef) {
    indent(out, level);
    out.push_str("FnDef param=");
    out.push_str(&format_param_pattern(&fn_def.param));
    out.push('\n');
    dump_block(out, level + 1, &fn_def.body);
}

fn dump_block(out: &mut String, level: usize, block: &Block) {
    indent(out, level);
    out.push_str("Block");
    for stmt in &block.statements {
        out.push('\n');
        match stmt {
            Stmt::Import(import) => dump_import(out, level + 1, import),
            Stmt::Expr(expr) => dump_expr(out, level + 1, expr),
        }
    }
}

fn dump_expr(out: &mut String, level: usize, expr: &Expr) {
    match expr {
        Expr::IntLit(text, _) => {
            indent(out, level);
            out.push_str("IntLit ");
            out.push_str(text);
        }
        Expr::RealLit(text, _) => {
            indent(out, level);
            out.push_str("RealLit ");
            out.push_str(text);
        }
        Expr::StringLit(text, _, _) => {
            indent(out, level);
            out.push_str("StrLit ");
            out.push_str(&format!("{:?}", text));
        }
        Expr::QualifiedName {
            module_alias, name, ..
        } => {
            indent(out, level);
            out.push_str("Name ");
            if let Some(alias) = module_alias {
                out.push_str(alias);
                out.push('.');
            }
            out.push_str(name);
        }
        Expr::Assignment { target, value, .. } => {
            indent(out, level);
            out.push_str("Assign target=");
            out.push_str(&format_binding_pattern(target));
            out.push('\n');
            dump_expr(out, level + 1, value);
        }
        Expr::FnDef(fn_def) => dump_fn_def(out, level, fn_def),
        Expr::LogicalOr(operands) => {
            indent(out, level);
            out.push_str("Or");
            for operand in operands {
                out.push('\n');
                dump_expr(out, level + 1, operand);
            }
        }
        Expr::LogicalAnd(operands) => {
            indent(out, level);
            out.push_str("And");
            for operand in operands {
                out.push('\n');
                dump_expr(out, level + 1, operand);
            }
        }
        Expr::LogicalNot(operand, _) => {
            indent(out, level);
            out.push_str("Not\n");
            dump_expr(out, level + 1, operand);
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            indent(out, level);
            out.push_str("BinOp ");
            out.push_str(op.symbol());
            out.push('\n');
            labeled_child(out, level + 1, "lhs: ", lhs);
            out.push('\n');
            labeled_child(out, level + 1, "rhs: ", rhs);
        }
        Expr::Unary {
            op,
            operand,
            sign_adjacent,
            ..
        } => {
            indent(out, level);
            out.push_str("Unary ");
            out.push_str(op.symbol());
            out.push_str(" adj=");
            out.push_str(if *sign_adjacent { "true" } else { "false" });
            out.push('\n');
            dump_expr(out, level + 1, operand);
        }
        Expr::Indexed { base, indices, .. } => {
            indent(out, level);
            out.push_str("Indexed\n");
            labeled_child(out, level + 1, "base: ", base);
            for (i, index) in indices.iter().enumerate() {
                out.push('\n');
                labeled_child(out, level + 1, &format!("idx[{}]: ", i), index);
            }
        }
        Expr::FnCall { target, arg, .. } => {
            indent(out, level);
            out.push_str("Call\n");
            labeled_child(out, level + 1, "target: ", target);
            if let Some(arg) = arg {
                out.push('\n');
                labeled_child(out, level + 1, "arg: ", arg);
            }
        }
        Expr::Block(block) => dump_block(out, level, block),
        Expr::Iif {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            indent(out, level);
            out.push_str("Iif\n");
            labeled_child(out, level + 1, "cond: ", cond);
            out.push('\n');
            labeled_child(out, level + 1, "then: ", then_branch);
            out.push('\n');
            labeled_child(out, level + 1, "else: ", else_branch);
        }
        Expr::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            indent(out, level);
            out.push_str("For\n");
            labeled_child(out, level + 1, "init: ", init);
            out.push('\n');
            labeled_child(out, level + 1, "cond: ", cond);
            out.push('\n');
            labeled_child(out, level + 1, "step: ", step);
            out.push('\n');
            labeled_child(out, level + 1, "body: ", body);
        }
        Expr::Try(expr, _) => {
            indent(out, level);
            out.push_str("Try\n");
            dump_expr(out, level + 1, expr);
        }
        Expr::BracketLiteral(elements, _) => {
            indent(out, level);
            out.push_str("Bracket");
            for element in elements {
                out.push('\n');
                dump_expr(out, level + 1, element);
            }
        }
    }
}

/// Escreve um filho rotulado: o rótulo e a **tag** do nó na mesma linha, com os filhos desse nó
/// um nível abaixo. A tag é obtida desenhando o nó em um buffer próprio e reindentando.
fn labeled_child(out: &mut String, level: usize, label: &str, child: &Expr) {
    let mut buffer = String::new();
    dump_expr(&mut buffer, 0, child);

    let mut lines = buffer.split('\n');
    let first = lines.next().unwrap_or("");
    indent(out, level);
    out.push_str(label);
    out.push_str(first);
    for line in lines {
        out.push('\n');
        if !line.is_empty() {
            // `line` já traz a indentação relativa ao nó-filho (1 nível = 2 espaços); basta somar
            // o nível do rótulo para que os descendentes fiquem exatamente um nível abaixo dele.
            indent(out, level);
            out.push_str(line);
        }
    }
}

/// Forma de superfície de um padrão (usada em `param=`/`target=`), recursiva:
/// `x`, `x:int`, `[a, b]`, `[[a,b],[c,d]]`, `[a:int, b:int]`.
fn format_binding_pattern(pattern: &BindingPattern) -> String {
    match pattern {
        BindingPattern::Name(name, _) => name.clone(),
        BindingPattern::List(items, _) => {
            let inner = items
                .iter()
                .map(format_binding_pattern)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
    }
}

fn format_param_pattern(pattern: &ParamPattern) -> String {
    match pattern {
        ParamPattern::Name { name, ty, .. } => match ty {
            Some(ty) => format!("{}:{}", name, format_type(ty)),
            None => name.clone(),
        },
        ParamPattern::List(items, _) => {
            let inner = items
                .iter()
                .map(format_param_pattern)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
    }
}

fn format_type(ty: &TypeAnnotation) -> String {
    match ty {
        TypeAnnotation::Bool => "bool".to_string(),
        TypeAnnotation::Int => "int".to_string(),
        TypeAnnotation::Real => "real".to_string(),
        TypeAnnotation::String => "string".to_string(),
        TypeAnnotation::Fn => "fn".to_string(),
        TypeAnnotation::AnyList => "[]".to_string(),
        TypeAnnotation::ListOf(inner) => format!("[{}]", format_type(inner)),
        TypeAnnotation::Tuple(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Forma de superfície de um padrão, exposta para uso em mensagens de diagnóstico.
pub fn binding_pattern_text(pattern: &BindingPattern) -> String {
    format_binding_pattern(pattern)
}

/// Forma de superfície de um padrão de parâmetro, exposta para diagnóstico.
pub fn param_pattern_text(pattern: &ParamPattern) -> String {
    format_param_pattern(pattern)
}
