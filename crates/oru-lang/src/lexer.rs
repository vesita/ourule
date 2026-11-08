//! `.oru` 的词法器：保留 byte span，换行也作为 token（用于分隔字段/效果）。

use crate::diagnostic::Diagnostic;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Str(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Dot,
    Comma,
    Semicolon,
    Newline,
    Op(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut errors = Vec::new();

    while index < bytes.len() {
        let ch = bytes[index] as char;
        match ch {
            ' ' | '\t' | '\r' => index += 1,
            '\n' => {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    span: Span::new(index, index + 1),
                });
                index += 1;
            }
            '#' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            '{' => {
                tokens.push(punct(TokenKind::LBrace, index));
                index += 1;
            }
            '}' => {
                tokens.push(punct(TokenKind::RBrace, index));
                index += 1;
            }
            '(' => {
                tokens.push(punct(TokenKind::LParen, index));
                index += 1;
            }
            ')' => {
                tokens.push(punct(TokenKind::RParen, index));
                index += 1;
            }
            '.' => {
                tokens.push(punct(TokenKind::Dot, index));
                index += 1;
            }
            ',' => {
                tokens.push(punct(TokenKind::Comma, index));
                index += 1;
            }
            ';' => {
                tokens.push(punct(TokenKind::Semicolon, index));
                index += 1;
            }
            '"' => {
                let start = index;
                index += 1;
                let mut value = String::new();
                let mut closed = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    if byte == b'"' {
                        closed = true;
                        index += 1;
                        break;
                    }
                    if byte == b'\\' {
                        index += 1;
                        if index >= bytes.len() {
                            break;
                        }
                        let escaped = bytes[index] as char;
                        value.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        });
                        index += 1;
                        continue;
                    }
                    let rest = &source[index..];
                    let Some(next) = rest.chars().next() else {
                        break;
                    };
                    value.push(next);
                    index += next.len_utf8();
                }
                if !closed {
                    errors.push(
                        Diagnostic::error("ORU0101", "字符串没有闭合", Span::new(start, index))
                            .with_help("补上结尾的 `\"`"),
                    );
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Str(value),
                        span: Span::new(start, index),
                    });
                }
            }
            '0'..='9' => {
                let start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                let text = &source[start..index];
                match text.parse::<i64>() {
                    Ok(value) => tokens.push(Token {
                        kind: TokenKind::Int(value),
                        span: Span::new(start, index),
                    }),
                    Err(_) => errors.push(Diagnostic::error(
                        "ORU0102",
                        format!("整数超出范围：{text}"),
                        Span::new(start, index),
                    )),
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Ident(source[start..index].to_string()),
                    span: Span::new(start, index),
                });
            }
            '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' => {
                let start = index;
                index += 1;
                let mut op = ch.to_string();
                if index < bytes.len() && bytes[index] == b'=' {
                    op.push('=');
                    index += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Op(op),
                    span: Span::new(start, index),
                });
            }
            other => {
                errors.push(Diagnostic::error(
                    "ORU0103",
                    format!("不认识的字符 `{other}`"),
                    Span::new(index, index + other.len_utf8()),
                ));
                index += other.len_utf8();
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::empty(source.len()),
    });
    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

fn punct(kind: TokenKind, at: usize) -> Token {
    Token {
        kind,
        span: Span::new(at, at + 1),
    }
}
