use std::collections::HashSet;

use crate::layout::{LayoutInfo, SvgPos};
use crate::model::*;
use crate::render::SvgOptions;
use crate::route::RouteSegment;
use crate::symbols::{self, SymbolElement};

pub fn render(
    diagram: &Diagram,
    layout: &LayoutInfo,
    routes: &[RouteSegment],
    opts: &SvgOptions,
) -> String {
    // Compute canvas bounds
    let (canvas_w, canvas_h) = compute_canvas(diagram, layout, opts);

    let nl = if opts.pretty { "\n" } else { "" };
    let indent = if opts.pretty { "  " } else { "" };

    let mut out = String::new();

    // SVG header — include xlink namespace for SVG 1.1 viewer compatibility
    out.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}">{}"#,
        canvas_w, canvas_h, canvas_w, canvas_h, nl
    ));

    // <defs>: styles + signal markers + symbol definitions
    out.push_str(&format!("{}<defs>{}", indent, nl));
    out.push_str(&format!("{}{}<style>{}", indent, indent, nl));
    out.push_str(&build_styles(indent, opts.pretty));
    out.push_str(&format!("{}{}</style>{}", indent, indent, nl));
    out.push_str(&build_markers(indent, opts.pretty));
    out.push_str(&build_symbol_defs(diagram, indent, opts.pretty));
    out.push_str(&format!("{}</defs>{}", indent, nl));

    // Lines group
    out.push_str(&format!("{}<g id=\"lines\">{}", indent, nl));
    for seg in routes.iter().filter(|s| !s.is_signal) {
        let class = line_class_for_id(&seg.connection_id, diagram);
        out.push_str(&render_polyline(&seg.points, &class, indent, opts.pretty, None));
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    // Signals group
    out.push_str(&format!("{}<g id=\"signals\">{}", indent, nl));
    for seg in routes.iter().filter(|s| s.is_signal) {
        let (class, marker) = signal_style_for_id(&seg.connection_id, diagram);
        out.push_str(&render_polyline(&seg.points, &class, indent, opts.pretty, marker.as_deref()));
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    // Symbols group — each object is a <use> referencing a <symbol> in <defs>
    out.push_str(&format!("{}<g id=\"symbols\">{}", indent, nl));
    for (kind, id) in &diagram.order {
        use crate::ast::DeclKind;
        match kind {
            DeclKind::Equipment => {
                if let Some(eq) = diagram.equipment.get(id) {
                    if let Some(pos) = layout.get_pos(id) {
                        out.push_str(&render_equipment(eq, pos, indent, opts.pretty));
                    }
                }
            }
            DeclKind::Valve => {
                if let Some(v) = diagram.valves.get(id) {
                    if let Some(pos) = layout.get_pos(id) {
                        out.push_str(&render_valve(v, pos, indent, opts.pretty));
                    }
                }
            }
            DeclKind::Instrument => {
                if let Some(instr) = diagram.instruments.get(id) {
                    if let Some(pos) = layout.get_pos(id) {
                        out.push_str(&render_instrument(instr, pos, indent, opts.pretty));
                    }
                }
            }
            DeclKind::Junction => {
                if let Some(pos) = layout.get_pos(id) {
                    out.push_str(&render_junction(id, pos, indent, opts.pretty));
                }
            }
            _ => {}
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    // Labels group
    out.push_str(&format!("{}<g id=\"labels\">{}", indent, nl));
    for (kind, id) in &diagram.order {
        use crate::ast::DeclKind;
        let label_text = match kind {
            DeclKind::Equipment => diagram.equipment.get(id).and_then(|e| e.label.as_deref()),
            DeclKind::Valve => diagram.valves.get(id).and_then(|v| v.label.as_deref()),
            DeclKind::Instrument => diagram.instruments.get(id).and_then(|i| i.label.as_deref()),
            _ => None,
        };
        if let Some(label) = label_text {
            if let Some(pos) = layout.get_pos(id) {
                let bounds = layout.get_bounds(id);
                let y_offset = bounds.map(|b| b.h / 2.0 + 14.0).unwrap_or(40.0);
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"label\">{}</text>{}",
                    indent, indent,
                    pos.x, pos.y + y_offset,
                    escape_xml(label),
                    nl
                ));
            }
        }
    }
    // Line labels: render near midpoint of each routed line
    for seg in routes.iter() {
        let line_label = if seg.is_signal {
            diagram.signals.get(&seg.connection_id).and_then(|s| s.label.as_deref())
        } else {
            diagram.lines.get(&seg.connection_id).and_then(|l| l.label.as_deref())
        };
        if let Some(label) = line_label {
            if seg.points.len() >= 2 {
                let mid_idx = seg.points.len() / 2;
                let p1 = &seg.points[mid_idx - 1];
                let p2 = &seg.points[mid_idx];
                let mx = (p1.x + p2.x) / 2.0;
                let my = (p1.y + p2.y) / 2.0;
                let dx = (p2.x - p1.x).abs();
                let dy = (p2.y - p1.y).abs();
                let (transform, tx, ty) = if dy > dx {
                    (
                        format!(" transform=\"rotate(-90,{:.1},{:.1})\"", mx, my - 8.0),
                        mx,
                        my - 8.0,
                    )
                } else {
                    (String::new(), mx, my - 8.0)
                };
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"line-label\"{}>{}",
                    indent, indent, tx, ty, transform, escape_xml(label)
                ));
                out.push_str(&format!("</text>{}", nl));
            }
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    // Notes group
    out.push_str(&format!("{}<g id=\"notes\">{}", indent, nl));
    for note in diagram.notes.values() {
        if let Some(pos) = layout.get_pos(&note.id) {
            if let Some(text) = &note.text {
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"note\">{}</text>{}",
                    indent, indent,
                    pos.x, pos.y,
                    escape_xml(text),
                    nl
                ));
            }
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    out.push_str("</svg>");
    out
}

// ---- <defs> / symbol building ----

/// Collect all unique symbol types used in `diagram` and emit them as
/// `<symbol id="sym-{key}">…</symbol>` entries within `<defs>`.
///
/// This is purely a renderer-level optimisation (deduplication via SVG `<defs>`).
/// Symbol geometry lives in `src/symbols/mod.rs` and is backend-agnostic.
fn build_symbol_defs(diagram: &Diagram, indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    let i2 = if pretty { format!("{}{}", indent, indent) } else { String::new() };
    let i3 = if pretty { format!("{}{}{}", indent, indent, indent) } else { String::new() };

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = String::new();

    for eq in diagram.equipment.values() {
        let key = symbols::equipment_symbol_key(&eq.equip_type);
        if seen.insert(key.to_string()) {
            let sym = symbols::equipment_symbol(&eq.equip_type);
            out.push_str(&emit_symbol_def(&format!("sym-{}", key), &sym, &i2, &i3, nl));
        }
    }

    for v in diagram.valves.values() {
        let key = symbols::valve_symbol_key(&v.valve_type);
        if seen.insert(key.to_string()) {
            let sym = symbols::valve_symbol(&v.valve_type);
            out.push_str(&emit_symbol_def(&format!("sym-{}", key), &sym, &i2, &i3, nl));
        }
    }

    for instr in diagram.instruments.values() {
        let key = symbols::instrument_symbol_key(instr.location.as_deref());
        if seen.insert(key.to_string()) {
            let sym = symbols::instrument_symbol(&instr.instr_type, instr.location.as_deref());
            out.push_str(&emit_symbol_def(&format!("sym-{}", key), &sym, &i2, &i3, nl));
        }
    }

    out
}

/// Serialise one `SymbolDef` as a `<g id="…">` block inside `<defs>`.
///
/// We use `<g>` rather than `<symbol>` deliberately: `<symbol>` creates an SVG
/// viewport whose default `overflow` is `hidden`, and many viewers (Illustrator,
/// Affinity, older Inkscape) clip symbol content to a 0×0 box when no explicit
/// `width`/`height` is set — producing the invisible/partial-arc artefacts.
/// A `<g>` inside `<defs>` has no viewport and no clipping; `<use>` can reference
/// it identically and the transform applies cleanly.
fn emit_symbol_def(id: &str, sym: &symbols::SymbolDef, i2: &str, i3: &str, nl: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}<g id=\"{}\">{}",  i2, id, nl));
    for elem in &sym.elements {
        out.push_str(&render_element(elem, i3, nl));
    }
    out.push_str(&format!("{}</g>{}", i2, nl));
    out
}

// Explicit colours used inside <symbol> elements.
// Using presentation attributes directly (not CSS inheritance) ensures correct
// rendering in all SVG renderers including ImageMagick, Inkscape, and SVG 1.1
// viewers that do not propagate CSS through the <use> shadow tree.
const SYM_FILL: &str = "white";
const SYM_STROKE: &str = "black";
const SYM_SW: &str = "1.5";

/// Serialise a single `SymbolElement` to an SVG string with explicit
/// fill/stroke presentation attributes.
fn render_element(elem: &SymbolElement, indent: &str, nl: &str) -> String {
    match elem {
        SymbolElement::Rect { x, y, w, h, rx } => format!(
            "{}<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>{}",
            indent, x, y, w, h, rx, SYM_FILL, SYM_STROKE, SYM_SW, nl
        ),
        SymbolElement::Circle { cx, cy, r } => format!(
            "{}<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>{}",
            indent, cx, cy, r, SYM_FILL, SYM_STROKE, SYM_SW, nl
        ),
        SymbolElement::Path { d } => format!(
            "{}<path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>{}",
            indent, d, SYM_FILL, SYM_STROKE, SYM_SW, nl
        ),
        SymbolElement::Line { x1, y1, x2, y2 } => format!(
            "{}<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>{}",
            indent, x1, y1, x2, y2, SYM_STROKE, SYM_SW, nl
        ),
        SymbolElement::Polyline { points } => {
            let pts: String = points.iter()
                .map(|(x, y)| format!("{:.1},{:.1}", x, y))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>{}", indent, pts, SYM_STROKE, SYM_SW, nl)
        }
    }
}

// ---- Canvas / styles / markers ----

fn compute_canvas(diagram: &Diagram, layout: &LayoutInfo, opts: &SvgOptions) -> (u32, u32) {
    if opts.width.is_some() || opts.height.is_some() {
        return (opts.width.unwrap_or(800), opts.height.unwrap_or(600));
    }

    let mut max_x = 400.0f64;
    let mut max_y = 300.0f64;

    for (_, id) in &diagram.order {
        if let Some(bounds) = layout.get_bounds(id) {
            let right  = bounds.x + bounds.w + 60.0;
            let bottom = bounds.y + bounds.h + 60.0;
            if right  > max_x { max_x = right;  }
            if bottom > max_y { max_y = bottom; }
        }
    }

    (max_x.ceil() as u32, max_y.ceil() as u32)
}

fn build_styles(_indent: &str, _pretty: bool) -> String {
    // Line weights and dash patterns follow ISO 10628-2 / ISA-5.1.
    // All signal lines use black with type-specific dash patterns (ISA-5.1 Table 1).
    r#"
    .equipment { fill: white; stroke: black; stroke-width: 1.5; }
    .valve { fill: white; stroke: black; stroke-width: 1.5; }
    .instrument { fill: white; stroke: black; stroke-width: 1.5; }
    .junction { fill: black; stroke: none; }
    .label { font-family: sans-serif; font-size: 11px; text-anchor: middle; fill: #333; }
    .line-label { font-family: sans-serif; font-size: 9px; text-anchor: middle; fill: #666; }
    .note { font-family: sans-serif; font-size: 11px; fill: #555; font-style: italic; }
    .line-process { fill: none; stroke: black; stroke-width: 2; }
    .line-utility { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 8,4; }
    .line-drain { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 4,2; }
    .line-vent { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 2,3; }
    .signal-electrical { fill: none; stroke: black; stroke-width: 1; }
    .signal-pneumatic { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 8,4; }
    .signal-hydraulic { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 10,2,1,2; }
    .signal-digital { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 6,2,1,2; }
"#.to_string()
}

fn build_markers(_indent: &str, _pretty: bool) -> String {
    let mut s = String::new();
    // Filled arrowhead for electrical signals
    s.push_str("    <marker id=\"arrow-end\" markerWidth=\"8\" markerHeight=\"8\" refX=\"6\" refY=\"3\" orient=\"auto\">\n");
    s.push_str("      <path d=\"M 0 0 L 6 3 L 0 6 Z\" fill=\"black\"/>\n");
    s.push_str("    </marker>\n");
    // Open (double-chevron) arrowhead for pneumatic signals
    s.push_str("    <marker id=\"arrow-end-pneumatic\" markerWidth=\"10\" markerHeight=\"8\" refX=\"8\" refY=\"4\" orient=\"auto\">\n");
    s.push_str("      <path d=\"M 0 0 L 4 4 L 0 8\" fill=\"none\" stroke=\"black\" stroke-width=\"1\"/>\n");
    s.push_str("      <path d=\"M 4 0 L 8 4 L 4 8\" fill=\"none\" stroke=\"black\" stroke-width=\"1\"/>\n");
    s.push_str("    </marker>\n");
    s
}

// ---- Polyline / use rendering ----

fn render_polyline(
    points: &[SvgPos],
    css_class: &str,
    indent: &str,
    pretty: bool,
    marker_end: Option<&str>,
) -> String {
    let nl = if pretty { "\n" } else { "" };
    if points.is_empty() {
        return String::new();
    }

    let pts_str: String = points
        .iter()
        .map(|p| format!("{:.1},{:.1}", p.x, p.y))
        .collect::<Vec<_>>()
        .join(" ");

    let marker_attr = marker_end
        .map(|m| format!(" marker-end=\"url(#{})\"", m))
        .unwrap_or_default();

    // Explicit presentation attributes alongside the CSS class so that the
    // polyline renders correctly in viewers that do not apply internal CSS
    // stylesheets (Illustrator, Affinity, many SVG 1.1 renderers).
    let attrs = line_presentation_attrs(css_class);

    format!(
        "{}{}<polyline points=\"{}\" class=\"{}\"{}{}/>{}",
        indent, indent, pts_str, css_class, attrs, marker_attr, nl
    )
}

/// Return explicit SVG presentation attributes matching the CSS rule for a
/// given line/signal class. These act as a fallback for viewers without CSS.
fn line_presentation_attrs(css_class: &str) -> &'static str {
    match css_class {
        "line-process"       => r#" fill="none" stroke="black" stroke-width="2""#,
        "line-utility"       => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="8,4""#,
        "line-drain"         => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="4,2""#,
        "line-vent"          => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="2,3""#,
        "signal-electrical"  => r#" fill="none" stroke="black" stroke-width="1""#,
        "signal-pneumatic"   => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="8,4""#,
        "signal-hydraulic"   => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="10,2,1,2""#,
        "signal-digital"     => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="6,2,1,2""#,
        _                    => r#" fill="none" stroke="black" stroke-width="1""#,
    }
}

/// Emit a `<use>` element that instantiates a `<symbol>` defined in `<defs>`.
/// Both `href` (SVG 2) and `xlink:href` (SVG 1.1) are emitted for maximum
/// viewer compatibility.
fn render_use(id: &str, sym_id: &str, css_class: &str, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    format!(
        "{}{}<use id=\"{}\" href=\"#{}\" xlink:href=\"#{}\" class=\"{}\" transform=\"translate({:.1},{:.1})\"/>{}",
        indent, indent, id, sym_id, sym_id, css_class, pos.x, pos.y, nl
    )
}

fn render_equipment(eq: &Equipment, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let key = symbols::equipment_symbol_key(&eq.equip_type);
    render_use(&eq.id, &format!("sym-{}", key), "equipment", pos, indent, pretty)
}

fn render_valve(v: &Valve, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let key = symbols::valve_symbol_key(&v.valve_type);
    render_use(&v.id, &format!("sym-{}", key), "valve", pos, indent, pretty)
}

fn render_instrument(instr: &Instrument, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let key = symbols::instrument_symbol_key(instr.location.as_deref());
    render_use(&instr.id, &format!("sym-{}", key), "instrument", pos, indent, pretty)
}

fn render_junction(id: &str, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    format!(
        "{}{}<circle id=\"{}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" class=\"junction\"/>{}",
        indent, indent, id, pos.x, pos.y, nl
    )
}

// ---- Helpers ----

fn line_class_for_id(id: &str, diagram: &Diagram) -> String {
    if let Some(line) = diagram.lines.get(id) {
        format!("line-{}", line.class)
    } else {
        "line-process".to_string()
    }
}

fn signal_style_for_id(id: &str, diagram: &Diagram) -> (String, Option<String>) {
    if let Some(sig) = diagram.signals.get(id) {
        let class = format!("signal-{}", sig.sig_type);
        let marker = match sig.sig_type.as_str() {
            "electrical" => Some("arrow-end".to_string()),
            "pneumatic"  => Some("arrow-end-pneumatic".to_string()),
            _ => None,
        };
        (class, marker)
    } else {
        ("signal-electrical".to_string(), None)
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::DiagEngine;
    use crate::layout::compute_layout;
    use crate::lexer::lex;
    use crate::normalize::normalize;
    use crate::parser::parse;
    use crate::route::route;
    use crate::validate::validate;

    const SAMPLE: &str = include_str!("../../tests/cases/sample.pid");

    #[test]
    fn test_render_produces_svg() {
        let mut diags = DiagEngine::new();
        let lines = lex(SAMPLE, &mut diags);
        let doc = parse(&lines, &mut diags);
        let diagram = normalize(&doc, &mut diags);
        validate(&diagram, &mut diags);
        assert!(!diags.has_errors(), "Errors: {:?}", diags.diagnostics);

        let layout = compute_layout(&diagram);
        let routes = route(&diagram, &layout);
        let opts = SvgOptions::default();
        let svg = render(&diagram, &layout, &routes.segments, &opts);

        assert!(svg.starts_with("<svg"), "SVG should start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
        assert!(svg.contains("P101"), "SVG should contain P101 element");
        assert!(svg.contains("CV101"), "SVG should contain CV101 element");
        assert!(svg.contains("<defs>"), "SVG should contain <defs>");
        assert!(svg.contains("sym-pump"), "SVG should contain pump symbol def");
        assert!(svg.contains("href=\"#sym-"), "SVG should use <use> references");
    }
}
