use pidc::diag::DiagEngine;
use pidc::layout::compute_layout;
use pidc::lexer::lex;
use pidc::normalize::normalize;
use pidc::parser::parse;
use pidc::render::svg::render;
use pidc::render::SvgOptions;
use pidc::route::route;
use pidc::validate::validate;

const SAMPLE: &str = include_str!("cases/sample.pid");

fn compile_sample() -> (pidc::model::Diagram, DiagEngine) {
    let mut diags = DiagEngine::new();
    let lines = lex(SAMPLE, &mut diags);
    let doc = parse(&lines, &mut diags);
    let diagram = normalize(&doc, &mut diags);
    validate(&diagram, &mut diags);
    (diagram, diags)
}

#[test]
fn test_sample_parses_ok() {
    let (_, diags) = compile_sample();
    assert!(!diags.has_errors(), "Sample should parse without errors: {:?}", diags.diagnostics);
}

#[test]
fn test_sample_validates_ok() {
    let (_, diags) = compile_sample();
    let errors: Vec<_> = diags.diagnostics.iter()
        .filter(|d| d.severity == pidc::diag::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Sample should validate without errors: {:?}", errors);
}

#[test]
fn test_sample_svg_generated() {
    let (diagram, diags) = compile_sample();
    assert!(!diags.has_errors());

    let layout = compute_layout(&diagram);
    let routes = route(&diagram, &layout);
    let opts = SvgOptions::default();
    let svg = render(&diagram, &layout, &routes.segments, &opts);

    assert!(svg.starts_with("<svg"), "Output should be SVG");
    assert!(svg.contains("</svg>"), "SVG should be closed");
}

#[test]
fn test_sample_svg_contains_elements() {
    let (diagram, diags) = compile_sample();
    assert!(!diags.has_errors());

    let layout = compute_layout(&diagram);
    let routes = route(&diagram, &layout);
    let opts = SvgOptions::default();
    let svg = render(&diagram, &layout, &routes.segments, &opts);

    // Check all declared items are in the SVG
    assert!(svg.contains("id=\"P101\""), "P101 equipment should be rendered");
    assert!(svg.contains("id=\"CV101\""), "CV101 valve should be rendered");
    assert!(svg.contains("id=\"E101\""), "E101 equipment should be rendered");
    assert!(svg.contains("id=\"TI101\""), "TI101 instrument should be rendered");
    assert!(svg.contains("id=\"TIC101\""), "TIC101 instrument should be rendered");
}

#[test]
fn test_sample_svg_has_lines_and_signals() {
    let (diagram, diags) = compile_sample();
    assert!(!diags.has_errors());

    let layout = compute_layout(&diagram);
    let routes = route(&diagram, &layout);
    let opts = SvgOptions::default();
    let svg = render(&diagram, &layout, &routes.segments, &opts);

    assert!(svg.contains("id=\"lines\""), "SVG should have lines group");
    assert!(svg.contains("id=\"signals\""), "SVG should have signals group");
    assert!(svg.contains("id=\"symbols\""), "SVG should have symbols group");
    assert!(svg.contains("id=\"labels\""), "SVG should have labels group");
}

#[test]
fn test_svg_output_is_deterministic() {
    let (diagram1, _) = compile_sample();
    let layout1 = compute_layout(&diagram1);
    let routes1 = route(&diagram1, &layout1);
    let svg1 = render(&diagram1, &layout1, &routes1.segments, &SvgOptions::default());

    let (diagram2, _) = compile_sample();
    let layout2 = compute_layout(&diagram2);
    let routes2 = route(&diagram2, &layout2);
    let svg2 = render(&diagram2, &layout2, &routes2.segments, &SvgOptions::default());

    assert_eq!(svg1, svg2, "SVG output should be deterministic");
}

#[test]
fn test_inline_form_parses() {
    let src = "line L100 class=process from=P101.out to=CV101.in\n\
               equipment P101:\n  type: pump\n\
               equipment CV101:\n  type: pump\n";
    let mut diags = DiagEngine::new();
    let lines = lex(src, &mut diags);
    let doc = parse(&lines, &mut diags);
    let diagram = normalize(&doc, &mut diags);
    assert!(!diags.has_errors(), "{:?}", diags.diagnostics);
    assert_eq!(diagram.lines.len(), 1);
    assert_eq!(diagram.equipment.len(), 2);
}

#[test]
fn test_check_rejects_invalid() {
    // Missing required `type` property for equipment
    let src = "equipment P101:\n  at: (1,1)\n";
    let mut diags = DiagEngine::new();
    let lines = lex(src, &mut diags);
    let doc = parse(&lines, &mut diags);
    let diagram = normalize(&doc, &mut diags);
    validate(&diagram, &mut diags);
    assert!(diags.has_errors(), "Should have errors for missing `type`");
}

#[test]
fn test_unresolved_references_detected() {
    let src = "line L1:\n  class: process\n  from: GHOST\n  to: PHANTOM\n";
    let mut diags = DiagEngine::new();
    let lines = lex(src, &mut diags);
    let doc = parse(&lines, &mut diags);
    let diagram = normalize(&doc, &mut diags);
    validate(&diagram, &mut diags);
    assert!(diags.has_errors());
}
