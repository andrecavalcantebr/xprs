//! Catálogo de diagnósticos — **fonte autoritativa única** (ver `xpr-specs.md`, Seção 7, e
//! `plano-codegen.md`, subetapa E).
//!
//! Este arquivo é compilado **duas vezes**, como o mesmo código-fonte Rust:
//! * no compilador (`xprc`), via `src/diag/mod.rs`;
//! * na crate de runtime (`xpr_rt`), via `#[path = "../../src/diag/catalog.rs"]`.
//!
//! Assim, os dois binários relatam exatamente os mesmos IDs, severidades, mensagens e dicas **por
//! construção**, sem geração nem duplicação manual.

use std::fmt;

/// Severidade de um diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Erro de compilação: impede a geração de código.
    Error,
    /// Falha em tempo de execução, relata pelo processo compilado.
    Runtime,
}

impl Severity {
    pub fn prefix(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Runtime => "failure",
        }
    }
}

/// Identificadores públicos dos diagnósticos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagCode {
    // compilação
    SyntaxError,
    UndefinedIdentifier,
    TypeMismatch,
    SizeMismatch,
    CannotInfer,
    AssignToCaptured,
    AssignToConstant,
    MissingMain,
    MalformedNumber,
    ImportNotFound,
    ImportCycle,
    DuplicateAlias,
    RenderFunction,
    // execução
    DivByZero,
    Overflow,
    IndexOutOfRange,
    InvalidNumber,
    InvalidSize,
    EndOfInput,
    CallDepthExceeded,
    ExplicitFail,
}

/// Especificação de um diagnóstico no catálogo.
#[derive(Debug, Clone, Copy)]
pub struct DiagSpec {
    pub code: DiagCode,
    /// Identificador público usado nos diagnósticos e na documentação.
    pub public_id: &'static str,
    pub severity: Severity,
    /// Modelo de mensagem; `{}` é substituído pelo detalhe da ocorrência.
    pub message_template: &'static str,
    pub hint: Option<&'static str>,
}

macro_rules! specs {
    ($( $variant:ident => $id:literal, $sev:ident, $msg:literal, $hint:expr );* $(;)?) => {
        /// A lista completa de diagnósticos conhecidos — a única fonte de verdade.
        pub const CATALOG: &[DiagSpec] = &[
            $(DiagSpec {
                code: DiagCode::$variant,
                public_id: $id,
                severity: Severity::$sev,
                message_template: $msg,
                hint: $hint,
            }),*
        ];
    };
}

specs! {
    SyntaxError          => "syntax-error",           Error,   "sintaxe inválida: {}", None;
    UndefinedIdentifier  => "undefined-identifier",   Error,   "identificador não definido: {}", Some("defina o nome antes de usá-lo, na ordem textual do código");
    TypeMismatch         => "type-mismatch",          Error,   "tipos incompatíveis: {}", None;
    SizeMismatch         => "size-mismatch",          Error,   "tamanhos incompatíveis: {}", None;
    CannotInfer          => "cannot-infer",           Error,   "não foi possível inferir o tipo: {}", Some("adicione uma anotação de tipo ao parâmetro");
    AssignToCaptured     => "assign-to-captured",     Error,   "atribuição a nome capturado: {}", Some("variáveis capturadas por uma função são somente leitura; retorne o novo valor e atribua no chamador");
    AssignToConstant     => "assign-to-constant",     Error,   "atribuição a constante: {}", None;
    MissingMain          => "missing-main",           Error,   "o módulo de entrada não define {}", Some("o módulo de entrada deve definir _main");
    MalformedNumber      => "malformed-number",       Error,   "número malformado: {}", Some("inteiros devem caber em int64");
    ImportNotFound       => "import-not-found",       Error,   "módulo não encontrado: {}", None;
    ImportCycle          => "import-cycle",           Error,   "ciclo de import: {}", None;
    DuplicateAlias       => "duplicate-alias",        Error,   "alias duplicado no mesmo arquivo: {}", None;
    RenderFunction       => "render-function",        Error,   "um valor de função não pode ser renderizado: {}", None;
    DivByZero            => "div-by-zero",            Runtime, "divisão por zero", None;
    Overflow             => "overflow",               Runtime, "estouro aritmético em int", None;
    IndexOutOfRange      => "index-out-of-range",     Runtime, "índice fora do intervalo: {}", None;
    InvalidNumber        => "invalid-number",         Runtime, "texto não é um número: {}", None;
    InvalidSize          => "invalid-size",           Runtime, "tamanho inválido: {}", None;
    EndOfInput           => "end-of-input",           Runtime, "fim da entrada", None;
    CallDepthExceeded    => "call-depth-exceeded",    Runtime, "limite de profundidade de chamada excedido ({})", None;
    ExplicitFail         => "fail",                   Runtime, "{}", None;
}

impl DiagCode {
    /// Entrada correspondente no catálogo.
    pub fn spec(self) -> &'static DiagSpec {
        CATALOG
            .iter()
            .find(|spec| spec.code == self)
            .expect("todo DiagCode deve ter uma entrada em CATALOG")
    }

    /// Identificador público (`syntax-error`, `type-mismatch`, ...).
    pub fn public_id(self) -> &'static str {
        self.spec().public_id
    }

    pub fn severity(self) -> Severity {
        self.spec().severity
    }

    pub fn message_template(self) -> &'static str {
        self.spec().message_template
    }

    pub fn hint(self) -> Option<&'static str> {
        self.spec().hint
    }
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.public_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_codigo_tem_entrada_e_id_unico() {
        let mut seen = Vec::new();
        for spec in CATALOG {
            assert_eq!(spec.code.public_id(), spec.public_id);
            assert!(!spec.public_id.is_empty());
            assert!(
                !seen.contains(&spec.public_id),
                "id público repetido: {}",
                spec.public_id
            );
            seen.push(spec.public_id);
        }
    }

    #[test]
    fn nomes_de_erro_citados_na_spec_estao_no_catalogo() {
        // Nomes mencionados ao longo de `xpr-specs.md` (Seções 4 e 7).
        for id in [
            "syntax-error",
            "undefined-identifier",
            "type-mismatch",
            "size-mismatch",
            "cannot-infer",
            "assign-to-captured",
            "assign-to-constant",
            "missing-main",
            "malformed-number",
            "div-by-zero",
            "overflow",
            "index-out-of-range",
            "invalid-number",
            "invalid-size",
            "end-of-input",
        ] {
            assert!(
                CATALOG.iter().any(|spec| spec.public_id == id),
                "diagnóstico ausente do catálogo: {}",
                id
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Diagnóstico pronto para exibição (compartilhado por compilador e runtime)
// ---------------------------------------------------------------------------------------------

/// Diagnóstico concreto: código do catálogo + detalhe já formatado.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub detail: String,
}

impl Diagnostic {
    pub fn new(code: DiagCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    /// Mensagem completa, substituindo `{}` pelo detalhe (quando o modelo tiver placeholder).
    pub fn message(&self) -> String {
        let template = self.code.message_template();
        if template.contains("{}") {
            template.replacen("{}", &self.detail, 1)
        } else if self.detail.is_empty() {
            template.to_string()
        } else {
            format!("{}: {}", template, self.detail)
        }
    }

    pub fn hint(&self) -> Option<&'static str> {
        self.code.hint()
    }

    /// Linha no formato usado pelo compilador e pelo runtime:
    /// `{prefixo}[{id}]: mensagem`, seguida da dica quando houver.
    pub fn render(&self) -> String {
        let mut out = format!(
            "{}[{}]: {}",
            self.code.severity().prefix(),
            self.code.public_id(),
            self.message()
        );
        if let Some(hint) = self.hint() {
            out.push_str("\n  dica: ");
            out.push_str(hint);
        }
        out
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}
