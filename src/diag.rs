use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub note: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span: None,
            note: None,
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span: None,
            note: None,
            help: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn print(&self, source: &str, filename: &str) {
        let loc = if let Some(span) = self.span {
            format!("{}:{}:{}: ", filename, span.line, span.col)
        } else {
            format!("{}: ", filename)
        };

        eprintln!("{}{}: {}", loc, self.severity, self.message);

        // Print source snippet if we have a span
        if let Some(span) = self.span {
            let lines: Vec<&str> = source.lines().collect();
            let line_idx = (span.line as usize).saturating_sub(1);
            if line_idx < lines.len() {
                let line_text = lines[line_idx];
                let line_num_str = span.line.to_string();
                eprintln!(" {} | {}", line_num_str, line_text);

                // Caret indicator
                let col0 = (span.col as usize).saturating_sub(1);
                let spaces = " ".repeat(line_num_str.len() + 3 + col0);
                let width = if span.end > span.start {
                    (span.end - span.start).min(line_text.len().saturating_sub(col0)).max(1)
                } else {
                    1
                };
                eprintln!(" {} | {}{}",
                    " ".repeat(line_num_str.len()),
                    spaces.trim_start_matches(' ')
                        .chars()
                        .take(0)
                        .collect::<String>(),
                    "^".repeat(width)
                );
                // simpler caret line
                let pad = " ".repeat(line_num_str.len() + 3);
                let col_pad = " ".repeat(col0);
                eprintln!("{}{}{}",
                    pad,
                    col_pad,
                    "^".repeat(width)
                );
            }
        }

        if let Some(note) = &self.note {
            eprintln!("  note: {}", note);
        }
        if let Some(help) = &self.help {
            eprintln!("  help: {}", help);
        }
    }
}

/// Collects diagnostics during compilation.
#[derive(Debug, Default)]
pub struct DiagEngine {
    pub diagnostics: Vec<Diagnostic>,
    pub strict: bool,
}

impl DiagEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn emit(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.emit(Diagnostic::error(message));
    }

    pub fn error_at(&mut self, span: Span, message: impl Into<String>) {
        self.emit(Diagnostic::error(message).with_span(span));
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.emit(Diagnostic::warning(message));
    }

    pub fn warn_at(&mut self, span: Span, message: impl Into<String>) {
        self.emit(Diagnostic::warning(message).with_span(span));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| {
            d.severity == Severity::Error
                || (self.strict && d.severity == Severity::Warning)
        })
    }

    pub fn print_all(&self, source: &str, filename: &str) {
        for diag in &self.diagnostics {
            diag.print(source, filename);
        }
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count()
    }
}
