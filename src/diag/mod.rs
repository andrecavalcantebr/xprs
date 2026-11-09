//! Catálogo de diagnósticos compartilhado por compilador e runtime.
//!
//! `catalog.rs` é a **única** fonte de códigos, IDs públicos, severidades, modelos de mensagem e
//! dicas (ver `xpr-specs.md`, Seção 7). É incluído aqui pelo compilador e também pela crate de
//! runtime `xpr_rt` (via `#[path = ...]`), de modo que os dois lados compilam literalmente o mesmo
//! código — nada é gerado de um lado para o outro nem duplicado manualmente.

pub mod catalog;

pub use catalog::{DiagCode, DiagSpec, Diagnostic, Severity, CATALOG};
