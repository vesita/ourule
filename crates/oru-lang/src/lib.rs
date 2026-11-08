//! Oru 语言前端：词法、语法、AST、语义检查、Manifest。
//!
//! M0 的目标是先把“写得出来 + 检查得出来”做扎实；运行时/包依赖/LSP 在后续里程碑。

pub mod ast;
pub mod check;
pub mod diagnostic;
pub mod lexer;
pub mod manifest;
pub mod parser;
pub mod span;

pub use check::check;
pub use diagnostic::{Diagnostic, Severity};
pub use lexer::lex;
pub use manifest::{Manifest, ManifestError, find_project};
pub use parser::parse;
pub use span::{Span, line_col, line_text};

/// 解析并检查一段 `.oru` 源码。
pub fn check_source(source: &str) -> Result<ast::Program, Vec<Diagnostic>> {
    let tokens = lex(source)?;
    let program = parse(tokens).map_err(|diagnostic| vec![diagnostic])?;
    let diagnostics = check(&program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        Err(diagnostics)
    } else {
        Ok(program)
    }
}

/// 检查一个 `.oru` 文件；返回源码 + 警告（错误已经由 `Result` 表达）。
pub struct CheckedFile {
    pub source: String,
    pub program: ast::Program,
    pub warnings: Vec<Diagnostic>,
}

pub fn check_file(path: &std::path::Path) -> Result<CheckedFile, (String, Vec<Diagnostic>)> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| (format!("读不到 {}：{error}", path.display()), Vec::new()))?;
    let tokens = lex(&source).map_err(|diagnostics| (source.clone(), diagnostics))?;
    let program = parse(tokens).map_err(|diagnostic| (source.clone(), vec![diagnostic]))?;
    let diagnostics = check(&program);
    let errors: Vec<Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .cloned()
        .collect();
    if errors.is_empty() {
        let warnings = diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .collect();
        Ok(CheckedFile {
            source,
            program,
            warnings,
        })
    } else {
        Err((source, diagnostics))
    }
}
