use crate::diag::{DiagEngine, Diagnostic};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Quoted(String),
    Equals,
    Colon,
    LParen,
    RParen,
    Comma,
    Dot,
    Hash, // comment marker (rest of line consumed separately)
}

impl Token {
    pub fn as_ident(&self) -> Option<&str> {
        match self {
            Token::Ident(s) => Some(s),
            _ => None,
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(s) => write!(f, "{}", s),
            Token::Quoted(s) => write!(f, "\"{}\"", s),
            Token::Equals => write!(f, "="),
            Token::Colon => write!(f, ":"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::Hash => write!(f, "#"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// One lexed line with indentation info.
#[derive(Debug, Clone)]
pub struct LexLine {
    pub indent: usize,
    pub tokens: Vec<SpannedToken>,
    pub line_num: u32,
    pub is_blank: bool,
    pub is_comment: bool,
}

impl LexLine {
    pub fn first_ident(&self) -> Option<&str> {
        self.tokens.first().and_then(|t| t.token.as_ident())
    }
}

pub fn lex(source: &str, diags: &mut DiagEngine) -> Vec<LexLine> {
    let mut result = Vec::new();

    for (line_idx, raw_line) in source.lines().enumerate() {
        let line_num = (line_idx + 1) as u32;
        let lex_line = lex_line(raw_line, line_num, diags);
        result.push(lex_line);
    }

    result
}

fn lex_line(raw: &str, line_num: u32, diags: &mut DiagEngine) -> LexLine {
    // Count leading whitespace and validate indentation
    let mut indent = 0usize;
    let mut has_tab_error = false;

    for (i, ch) in raw.char_indices() {
        if ch == ' ' {
            indent += 1;
        } else if ch == '\t' {
            if !has_tab_error {
                let span = Span::new(line_num, (i + 1) as u32, i, i + 1);
                diags.emit(
                    Diagnostic::error("tabs are not allowed for indentation; use 2 spaces per level")
                        .with_span(span)
                        .with_help("replace the tab character with spaces"),
                );
                has_tab_error = true;
            }
            indent += 1; // treat as 1 space for recovery
        } else {
            break;
        }
    }

    // Validate indent is multiple of 2
    if indent % 2 != 0 && !has_tab_error {
        let span = Span::new(line_num, 1, 0, indent);
        diags.emit(
            Diagnostic::error(format!(
                "indentation must be a multiple of 2 spaces, found {} space(s)",
                indent
            ))
            .with_span(span)
            .with_help("use 2-space indentation only"),
        );
    }

    let content = &raw[indent..];

    // Check for blank/comment
    let trimmed = content.trim_start();
    if trimmed.is_empty() {
        return LexLine {
            indent,
            tokens: vec![],
            line_num,
            is_blank: true,
            is_comment: false,
        };
    }
    if trimmed.starts_with('#') {
        return LexLine {
            indent,
            tokens: vec![],
            line_num,
            is_blank: false,
            is_comment: true,
        };
    }

    // Tokenize content
    let tokens = tokenize_content(content, line_num, indent, diags);

    let is_blank = tokens.is_empty();
    LexLine {
        indent,
        tokens,
        line_num,
        is_blank,
        is_comment: false,
    }
}

fn tokenize_content(
    content: &str,
    line_num: u32,
    base_col_offset: usize,
    diags: &mut DiagEngine,
) -> Vec<SpannedToken> {
    let mut tokens = Vec::new();
    let bytes = content.as_bytes();
    let mut pos = 0;

    macro_rules! col {
        ($p:expr) => {
            (base_col_offset + $p + 1) as u32
        };
    }

    while pos < bytes.len() {
        let ch = bytes[pos] as char;

        // Skip spaces
        if ch == ' ' || ch == '\t' {
            pos += 1;
            continue;
        }

        // Comment
        if ch == '#' {
            break;
        }

        // Single-char punctuation
        let single = match ch {
            '=' => Some(Token::Equals),
            ':' => Some(Token::Colon),
            '(' => Some(Token::LParen),
            ')' => Some(Token::RParen),
            ',' => Some(Token::Comma),
            '.' => Some(Token::Dot),
            _ => None,
        };
        if let Some(tok) = single {
            let span = Span::new(line_num, col!(pos), base_col_offset + pos, base_col_offset + pos + 1);
            tokens.push(SpannedToken { token: tok, span });
            pos += 1;
            continue;
        }

        // Quoted string
        if ch == '"' {
            let start = pos;
            pos += 1;
            let mut s = String::new();
            let mut closed = false;
            while pos < bytes.len() {
                let c = bytes[pos] as char;
                if c == '"' {
                    pos += 1;
                    closed = true;
                    break;
                } else if c == '\\' && pos + 1 < bytes.len() {
                    pos += 1;
                    let escaped = bytes[pos] as char;
                    match escaped {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        other => {
                            s.push('\\');
                            s.push(other);
                        }
                    }
                    pos += 1;
                } else {
                    s.push(c);
                    pos += 1;
                }
            }
            if !closed {
                let span = Span::new(line_num, col!(start), base_col_offset + start, base_col_offset + pos);
                diags.emit(Diagnostic::error("unterminated string literal").with_span(span));
            }
            let span = Span::new(line_num, col!(start), base_col_offset + start, base_col_offset + pos);
            tokens.push(SpannedToken { token: Token::Quoted(s), span });
            continue;
        }

        // Identifier / bare word: letters, digits, underscore, hyphen
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            let start = pos;
            while pos < bytes.len() {
                let c = bytes[pos] as char;
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    pos += 1;
                } else {
                    break;
                }
            }
            let s = &content[start..pos];
            let span = Span::new(line_num, col!(start), base_col_offset + start, base_col_offset + pos);
            tokens.push(SpannedToken { token: Token::Ident(s.to_string()), span });
            continue;
        }

        // Unknown character - emit diagnostic and skip
        let span = Span::new(line_num, col!(pos), base_col_offset + pos, base_col_offset + pos + 1);
        diags.emit(Diagnostic::error(format!("unexpected character '{}'", ch)).with_span(span));
        pos += 1;
    }

    tokens
}
