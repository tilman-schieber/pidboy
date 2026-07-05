pub mod svg;

#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pretty: bool,
    pub no_route: bool,
    /// Append a legend box below the drawing explaining used symbols.
    pub legend: bool,
    /// Append the equipment data table.
    pub table: bool,
    /// Title block text, drawn bottom-right.
    pub title: Option<String>,
    /// Footer lines under the title.
    pub footers: Vec<String>,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            pretty: false,
            no_route: false,
            legend: false,
            table: false,
            title: None,
            footers: Vec::new(),
        }
    }
}
