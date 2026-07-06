//! Library entry points for the full compile pipeline.
//!
//! Pure functions: no I/O, no printing. The CLI (`main.rs`) and the WASM
//! bindings both build on these.

use crate::ast;
use crate::diag::DiagEngine;
use crate::layout::{self, LayoutInfo};
use crate::lexer;
use crate::model::Diagram;
use crate::normalize;
use crate::parser;
use crate::render::{self, SvgOptions};
use crate::route;
use crate::validate;

pub struct CompileResult {
    /// Rendered SVG; `None` when diagnostics contain errors.
    pub svg: Option<String>,
    pub diagram: Option<Diagram>,
    pub layout: Option<LayoutInfo>,
    pub diags: DiagEngine,
}

/// Front half of the pipeline: lex → parse → normalize → validate.
/// Returns the AST and semantic model unconditionally; check
/// `diags.has_errors()` before trusting them.
pub fn analyze(source: &str, strict: bool) -> (ast::Document, Diagram, DiagEngine) {
    let mut diags = DiagEngine::new().with_strict(strict);
    let lex_lines = lexer::lex(source, &mut diags);
    let doc = parser::parse(&lex_lines, &mut diags);
    let diagram = normalize::normalize(&doc, &mut diags);
    validate::validate(&diagram, &mut diags);
    (doc, diagram, diags)
}

/// Full pipeline. Layout/route/render only run when analysis is error-free.
pub fn compile_to_parts(source: &str, strict: bool, opts: &SvgOptions) -> CompileResult {
    let (_doc, diagram, diags) = analyze(source, strict);
    if diags.has_errors() {
        return CompileResult { svg: None, diagram: None, layout: None, diags };
    }
    let layout = layout::compute_layout(&diagram);
    let routes = route::route(&diagram, &layout);
    let svg = render::svg::render(&diagram, &layout, &routes.segments, opts);
    CompileResult {
        svg: Some(svg),
        diagram: Some(diagram),
        layout: Some(layout),
        diags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
equipment T1:
  type: tank
  ports:
    out: east
  at: (2, 3)

equipment P1:
  type: pump_centrifugal
  ports:
    in: west
  at: (5, 3)

line L1:
  class: process
  from: T1.out
  to: P1.in
";

    #[test]
    fn compile_to_parts_produces_svg() {
        let res = compile_to_parts(SAMPLE, false, &SvgOptions::default());
        assert!(!res.diags.has_errors());
        let svg = res.svg.expect("svg");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("id=\"T1\""));
        let layout = res.layout.expect("layout");
        assert!(layout.positions.contains_key("P1"));
    }

    #[test]
    fn errors_yield_no_svg() {
        let res = compile_to_parts("line L1:\n  from: A.out\n  to: B.in\n", false, &SvgOptions::default());
        assert!(res.diags.has_errors());
        assert!(res.svg.is_none());
    }
}
