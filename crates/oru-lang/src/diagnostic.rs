//! 诊断：所有检查错误/警告的统一形状与 rustc 风格渲染。

use serde::Serialize;

use crate::span::{Span, line_col, line_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            span,
            label: None,
            help: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Warning,
            message: message.into(),
            span,
            label: None,
            help: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// rustc 风格的诊断文本。
    pub fn render(&self, source: &str) -> String {
        self.render_with_path("<source>", source)
    }

    /// 带文件名的 rustc 风格诊断文本。
    pub fn render_with_path(&self, path: &str, source: &str) -> String {
        let start = line_col(source, self.span.start);
        let end = line_col(source, self.span.end);
        let kind = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let mut out = String::new();
        out.push_str(&format!("{}[{}]: {}\n", kind, self.code, self.message));
        out.push_str(&format!(" --> {}:{}:{}\n", path, start.line, start.column));
        out.push_str("  |\n");
        let text = line_text(source, start.line);
        out.push_str(&format!("{} | {}\n", start.line, text));
        let pad = " ".repeat(start.column.saturating_sub(1));
        let underline_len = if end.line == start.line {
            end.column.saturating_sub(start.column).max(1)
        } else {
            text.chars()
                .count()
                .saturating_sub(start.column.saturating_sub(1))
                .max(1)
        };
        out.push_str(&format!("  | {}{}", pad, "^".repeat(underline_len)));
        if let Some(label) = &self.label {
            out.push(' ');
            out.push_str(label);
        }
        out.push('\n');
        if let Some(help) = &self.help {
            out.push_str(&format!("  = help: {help}\n"));
        }
        out
    }
}
