//! Suíte golden-file do Parser (ver `MEMORY.md`, seção "Contratos de testes e fixtures").
//!
//! Itera `tests/parser/cases/*.x` (sucesso, par com `.ast`) e
//! `tests/parser/cases/errors/*.x` (erro, par com `.error`), tokeniza com o lexer real, parseia e
//! compara o dump da AST com o arquivo-irmão esperado. Imprime `[OK]`/`[FAIL]` por fixture e entra
//! em panic com um resumo no fim se qualquer fixture falhar.
//!
//! Formatos normativos (MEMORY.md):
//!   `.ast`   — árvore em texto, 2 espaços por nível, um nó por linha, sem spans
//!   `.error` — `{posição} {VarianteDeSyntaxErrorKind}`

use std::fs;
use std::path::{Path, PathBuf};

use xprc::lexer::{Lexer, Span};
use xprc::parser::dump::dump_module;
use xprc::parser::{Parser, SyntaxErrorKind};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("parser")
        .join("cases")
}

fn fmt_span(span: &Span) -> String {
    if span.line_start == span.line_end {
        format!("{}:{}-{}", span.line_start, span.col_start, span.col_end)
    } else {
        format!(
            "{}:{}-{}:{}",
            span.line_start, span.col_start, span.line_end, span.col_end
        )
    }
}

fn fmt_error_kind(kind: &SyntaxErrorKind) -> &'static str {
    match kind {
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

fn read_normalized(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("não foi possível ler {}: {}", path.display(), e));
    text.replace("\r\n", "\n").trim_end_matches('\n').to_string()
}

fn sorted_x_files(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("não foi possível listar {}: {}", dir.display(), e))
        .map(|entry| entry.expect("entrada de diretório inválida").path())
        .filter(|p| p.extension().map(|e| e == "x").unwrap_or(false))
        .collect();
    paths.sort();
    paths
}

struct Report {
    ok: usize,
    failures: Vec<String>,
}

impl Report {
    fn new() -> Self {
        Report {
            ok: 0,
            failures: Vec::new(),
        }
    }

    fn pass(&mut self, name: &str) {
        self.ok += 1;
        println!("[OK] {}", name);
    }

    fn fail(&mut self, name: &str, motivo: String) {
        println!("[FAIL] {}: {}", name, motivo);
        self.failures.push(format!("{}: {}", name, motivo));
    }
}

fn run_success_case(report: &mut Report, path: &Path) {
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let expected_path = path.with_extension("ast");
    let expected = read_normalized(&expected_path);
    let source = fs::read_to_string(path).expect("fonte .x ilegível");

    let tokens = match Lexer::new(&source).tokenize() {
        Ok(tokens) => tokens,
        Err(err) => {
            report.fail(
                &name,
                format!("erro léxico inesperado: {:?} em {}", err.kind, fmt_span(&err.span)),
            );
            return;
        }
    };

    match Parser::new(&tokens).parse_module() {
        Err(err) => report.fail(
            &name,
            format!(
                "erro sintático inesperado: {} em {}",
                fmt_error_kind(&err.kind),
                fmt_span(&err.span)
            ),
        ),
        Ok(module) => {
            let actual = dump_module(&module);
            if actual == expected {
                report.pass(&name);
            } else {
                report.fail(
                    &name,
                    format!(
                        "dump divergente de {}\n--- esperado ---\n{}\n--- obtido ---\n{}",
                        expected_path.display(),
                        expected,
                        actual
                    ),
                );
            }
        }
    }
}

fn run_error_case(report: &mut Report, path: &Path) {
    let name = format!("errors/{}", path.file_name().unwrap().to_string_lossy());
    let expected_path = path.with_extension("error");
    let expected = read_normalized(&expected_path);
    let source = fs::read_to_string(path).expect("fonte .x ilegível");

    let tokens = match Lexer::new(&source).tokenize() {
        Ok(tokens) => tokens,
        Err(err) => {
            report.fail(
                &name,
                format!("erro léxico inesperado: {:?} em {}", err.kind, fmt_span(&err.span)),
            );
            return;
        }
    };

    match Parser::new(&tokens).parse_module() {
        Ok(module) => report.fail(
            &name,
            format!(
                "esperava erro sintático, mas o parse teve sucesso ({} definições):\n{}",
                module.definitions.len(),
                dump_module(&module)
            ),
        ),
        Err(err) => {
            let actual = format!("{} {}", fmt_span(&err.span), fmt_error_kind(&err.kind));
            if actual == expected {
                report.pass(&name);
            } else {
                report.fail(
                    &name,
                    format!(
                        "erro divergente de {}\n--- esperado ---\n{}\n--- obtido ---\n{}",
                        expected_path.display(),
                        expected,
                        actual
                    ),
                );
            }
        }
    }
}

#[test]
fn parser_golden_file_fixtures() {
    let cases = cases_dir();
    let errors_dir = cases.join("errors");

    let mut report = Report::new();

    let success_files = sorted_x_files(&cases);
    assert!(
        !success_files.is_empty(),
        "nenhuma fixture de sucesso encontrada em {}",
        cases.display()
    );
    for path in &success_files {
        run_success_case(&mut report, path);
    }

    let error_files = sorted_x_files(&errors_dir);
    assert!(
        !error_files.is_empty(),
        "nenhuma fixture de erro encontrada em {}",
        errors_dir.display()
    );
    for path in &error_files {
        run_error_case(&mut report, path);
    }

    let total = report.ok + report.failures.len();
    println!("== fixtures do parser: {}/{} OK ==", report.ok, total);

    if !report.failures.is_empty() {
        panic!(
            "{} de {} fixtures do parser falharam:\n  {}",
            report.failures.len(),
            total,
            report.failures.join("\n  ")
        );
    }
}
