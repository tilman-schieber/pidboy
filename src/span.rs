/// A source location span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(line: u32, col: u32, start: usize, end: usize) -> Self {
        Self { line, col, start, end }
    }

    pub fn point(line: u32, col: u32, offset: usize) -> Self {
        Self { line, col, start: offset, end: offset }
    }

    /// Merge two spans into one covering both.
    pub fn merge(self, other: Span) -> Span {
        Span {
            line: self.line.min(other.line),
            col: self.col.min(other.col),
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
