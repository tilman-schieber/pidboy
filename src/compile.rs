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
    fn framed_group_at_pins_module_center() {
        let src = "\
equipment T1:
  type: tank
  ports:
    out: east

equipment T2:
  type: tank
  ports:
    in: west

line L1:
  class: process
  from: T1.out
  to: T2.in

group M1:
  members: T1, T2
  frame: true
  at: (10, 5)
";
        let res = compile_to_parts(src, false, &SvgOptions::default());
        assert!(!res.diags.has_errors(), "{:?}", res.diags.diagnostics);
        let layout = res.layout.expect("layout");
        let (b1, b2) = (&layout.bounds["T1"], &layout.bounds["T2"]);
        let x1 = b1.x.min(b2.x);
        let y1 = b1.y.min(b2.y);
        let x2 = (b1.x + b1.w).max(b2.x + b2.w);
        let y2 = (b1.y + b1.h).max(b2.y + b2.h);
        let (sx, sy) = layout.origin_shift;
        assert!(((x1 + x2) / 2.0 - sx - 800.0).abs() < 1e-6, "center x: {}", (x1 + x2) / 2.0);
        assert!(((y1 + y2) / 2.0 - sy - 400.0).abs() < 1e-6, "center y: {}", (y1 + y2) / 2.0);
    }

    #[test]
    fn errors_yield_no_svg() {
        let res = compile_to_parts("line L1:\n  from: A.out\n  to: B.in\n", false, &SvgOptions::default());
        assert!(res.diags.has_errors());
        assert!(res.svg.is_none());
    }
}
