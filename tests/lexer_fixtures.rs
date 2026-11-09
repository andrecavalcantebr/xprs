//! Suíte golden-file do Lexer (ver `plano-lexer.md`, seção "Fixtures golden-file").
//!
//! Itera `tests/lexer/cases/*.x` (sucesso, par com `.tokens`) e
//! `tests/lexer/cases/errors/*.x` (erro, par com `.error`), tokeniza cada fonte e compara o
//! dump formatado com o arquivo-irmão esperado. Imprime `[OK]`/`[FAIL]` por fixture (visível com
//! `--nocapture`) e entra em panic com um resumo no fim se qualquer fixture falhar.
//!
//! O formato é normativo (plano-lexer.md):
//!   `{posição} {representação-do-TokenKind}[ adj]`
//! e o formato de erro:
//!   `{posição} {VarianteDeLexErrorKind}`
//! onde `{posição}` é `L:C-C` quando `line_start == line_end`, senão `L:C-L:C`.

use std::fs;
use std::path::{Path, PathBuf};

use xprc::lexer::{LexError, LexErrorKind, Lexer, Span, Token, TokenKind};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("lexer")
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

fn kind_name(kind: &TokenKind) -> String {
    match kind {
        TokenKind::IntLit(text) => format!("IntLit({})", text),
        TokenKind::RealLit(text) => format!("RealLit({})", text),
        TokenKind::StringLit { raw, quote } => {
            let q = if *quote == '"' { "dq" } else { "sq" };
            format!("StringLit({}, {:?})", q, raw)
        }
        TokenKind::Ident(name) => format!("Ident({})", name),
        TokenKind::KwFn => "KwFn".to_string(),
        TokenKind::KwIif => "KwIif".to_string(),
        TokenKind::KwFor => "KwFor".to_string(),
        TokenKind::KwTry => "KwTry".to_string(),
        TokenKind::KwImport => "KwImport".to_string(),
        TokenKind::KwAnd => "KwAnd".to_string(),
        TokenKind::KwOr => "KwOr".to_string(),
        TokenKind::KwNot => "KwNot".to_string(),
        TokenKind::Plus => "Plus".to_string(),
        TokenKind::Minus => "Minus".to_string(),
        TokenKind::Star => "Star".to_string(),
        TokenKind::Slash => "Slash".to_string(),
        TokenKind::Percent => "Percent".to_string(),
        TokenKind::Lt => "Lt".to_string(),
        TokenKind::Gt => "Gt".to_string(),
        TokenKind::Le => "Le".to_string(),
        TokenKind::Ge => "Ge".to_string(),
        TokenKind::EqEq => "EqEq".to_string(),
        TokenKind::NotEq => "NotEq".to_string(),
        TokenKind::Eq => "Eq".to_string(),
        TokenKind::LParen => "LParen".to_string(),
        TokenKind::RParen => "RParen".to_string(),
        TokenKind::LBrace => "LBrace".to_string(),
        TokenKind::RBrace => "RBrace".to_string(),
        TokenKind::LBracket => "LBracket".to_string(),
        TokenKind::RBracket => "RBracket".to_string(),
        TokenKind::Comma => "Comma".to_string(),
        TokenKind::Semi => "Semi".to_string(),
        TokenKind::Colon => "Colon".to_string(),
        TokenKind::Dot => "Dot".to_string(),
        TokenKind::Eof => "Eof".to_string(),
    }
}

fn fmt_token(token: &Token) -> String {
    let mut line = format!("{} {}", fmt_span(&token.span), kind_name(&token.kind));
    // O sufixo ` adj` só aparece em sinais (`Plus`/`Minus`) adjacentes ao próximo token.
    if token.adjacent_to_next && matches!(token.kind, TokenKind::Plus | TokenKind::Minus) {
        line.push_str(" adj");
    }
    line
}

fn fmt_error_kind(kind: &LexErrorKind) -> &'static str {
    match kind {
        LexErrorKind::MalformedNumber => "MalformedNumber",
        LexErrorKind::UnterminatedString => "UnterminatedString",
        LexErrorKind::InvalidCharacter => "InvalidCharacter",
        LexErrorKind::UnterminatedComment => "UnterminatedComment",
    }
}

fn fmt_error(err: &LexError) -> String {
    format!("{} {}", fmt_span(&err.span), fmt_error_kind(&err.kind))
}

fn read_normalized(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("não foi possível ler {}: {}", path.display(), e));
    // Comparação insensível a CRLF e a espaços em branco no fim do arquivo.
    let text = text.replace("\r\n", "\n");
    text.trim_end_matches('\n').to_string()
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
    let expected_path = path.with_extension("tokens");
    let expected = read_normalized(&expected_path);
    let source = fs::read_to_string(path).expect("fonte .x ilegível");

    match Lexer::new(&source).tokenize() {
        Err(err) => report.fail(&name, format!("erro léxico inesperado: {}", fmt_error(&err))),
        Ok(tokens) => {
            let actual = tokens
                .iter()
                .map(fmt_token)
                .collect::<Vec<_>>()
                .join("\n");
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
    let name = format!(
        "errors/{}",
        path.file_name().unwrap().to_string_lossy()
    );
    let expected_path = path.with_extension("error");
    let expected = read_normalized(&expected_path);
    let source = fs::read_to_string(path).expect("fonte .x ilegível");

    match Lexer::new(&source).tokenize() {
        Ok(tokens) => report.fail(
            &name,
            format!(
                "esperava erro, mas a tokenização teve sucesso ({} tokens)",
                tokens.len()
            ),
        ),
        Err(err) => {
            let actual = fmt_error(&err);
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
fn lexer_golden_file_fixtures() {
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
    println!(
        "== fixtures do lexer: {}/{} OK ==",
        report.ok, total
    );

    if !report.failures.is_empty() {
        panic!(
            "{} de {} fixtures do lexer falharam:\n  {}",
            report.failures.len(),
            total,
            report.failures.join("\n  ")
        );
    }
}
