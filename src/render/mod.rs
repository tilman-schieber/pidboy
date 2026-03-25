pub mod svg;

#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pretty: bool,
    pub no_route: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            pretty: false,
            no_route: false,
        }
    }
}
