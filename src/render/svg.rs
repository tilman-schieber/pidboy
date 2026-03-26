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

    // SVG header
    out.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">{}"#,
        canvas_w, canvas_h, canvas_w, canvas_h, nl
    ));

    // Defs / styles
    out.push_str(&format!("{}<defs>{}", indent, nl));
    out.push_str(&format!("{}{}<style>{}", indent, indent, nl));
    out.push_str(&build_styles(indent, opts.pretty));
    out.push_str(&format!("{}{}</style>{}", indent, indent, nl));

    // Arrow markers for signals
    out.push_str(&build_markers(indent, opts.pretty));

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

    // Symbols group
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
                // Determine if this segment is mostly vertical
                let dx = (p2.x - p1.x).abs();
                let dy = (p2.y - p1.y).abs();
                let (transform, tx, ty) = if dy > dx {
                    // Vertical segment — rotate label
                    (
                        format!(" transform=\"rotate(-90,{:.1},{:.1})\"", mx, my - 8.0),
                        mx,
                        my - 8.0,
                    )
                } else {
                    // Horizontal segment — offset 8px above
                    (String::new(), mx, my - 8.0)
                };
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"line-label\"{}>{}{}",
                    indent, indent,
                    tx, ty,
                    transform,
                    escape_xml(label),
                    if opts.pretty { "\n" } else { "" }
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

fn compute_canvas(diagram: &Diagram, layout: &LayoutInfo, opts: &SvgOptions) -> (u32, u32) {
    if opts.width.is_some() || opts.height.is_some() {
        return (
            opts.width.unwrap_or(800),
            opts.height.unwrap_or(600),
        );
    }

    // Compute from bounds
    let mut max_x = 400.0f64;
    let mut max_y = 300.0f64;

    for (_, id) in &diagram.order {
        if let Some(bounds) = layout.get_bounds(id) {
            let right = bounds.x + bounds.w + 60.0;
            let bottom = bounds.y + bounds.h + 60.0;
            if right > max_x { max_x = right; }
            if bottom > max_y { max_y = bottom; }
        }
    }

    (max_x.ceil() as u32, max_y.ceil() as u32)
}

fn build_styles(_indent: &str, _pretty: bool) -> String {
    r#"
    .equipment { fill: white; stroke: black; stroke-width: 1.5; }
    .valve { fill: white; stroke: black; stroke-width: 1.5; }
    .instrument { fill: white; stroke: black; stroke-width: 1.5; }
    .junction { fill: black; stroke: none; }
    .label { font-family: sans-serif; font-size: 11px; text-anchor: middle; fill: #333; }
    .line-label { font-family: sans-serif; font-size: 9px; text-anchor: middle; fill: #666; }
    .note { font-family: sans-serif; font-size: 11px; fill: #555; font-style: italic; }
    .line-process { fill: none; stroke: black; stroke-width: 2; }
    .line-utility { fill: none; stroke: black; stroke-width: 1.5; stroke-dasharray: 8,4; }
    .line-drain { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 2,3; }
    .line-vent { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 2,3; }
    .signal-electrical { fill: none; stroke: #1a1aff; stroke-width: 1.5; }
    .signal-pneumatic { fill: none; stroke: #1a1aff; stroke-width: 1.5; stroke-dasharray: 8,4; }
    .signal-hydraulic { fill: none; stroke: #007700; stroke-width: 1.5; stroke-dasharray: 4,4; }
    .signal-digital { fill: none; stroke: #770077; stroke-width: 1.5; stroke-dasharray: 1,3; }
"#.to_string()
}

fn build_markers(_indent: &str, _pretty: bool) -> String {
    let mut s = String::new();
    s.push_str("    <marker id=\"arrow-end\" markerWidth=\"8\" markerHeight=\"8\" refX=\"6\" refY=\"3\" orient=\"auto\">\n");
    s.push_str("      <path d=\"M 0 0 L 6 3 L 0 6 Z\" fill=\"#1a1aff\"/>\n");
    s.push_str("    </marker>\n");
    s.push_str("    <marker id=\"arrow-end-pneumatic\" markerWidth=\"10\" markerHeight=\"8\" refX=\"8\" refY=\"4\" orient=\"auto\">\n");
    s.push_str("      <path d=\"M 0 0 L 4 4 L 0 8\" fill=\"none\" stroke=\"#1a1aff\" stroke-width=\"1\"/>\n");
    s.push_str("      <path d=\"M 4 0 L 8 4 L 4 8\" fill=\"none\" stroke=\"#1a1aff\" stroke-width=\"1\"/>\n");
    s.push_str("    </marker>\n");
    s
}

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

    format!(
        "{}{}<polyline points=\"{}\" class=\"{}\"{}/>{}",
        indent, indent, pts_str, css_class, marker_attr, nl
    )
}

fn render_equipment(eq: &Equipment, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let sym = symbols::equipment_symbol(&eq.equip_type);
    render_symbol_elements(&sym.elements, pos, "equipment", &eq.id, indent, pretty)
}

fn render_valve(v: &Valve, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let sym = symbols::valve_symbol(&v.valve_type);
    render_symbol_elements(&sym.elements, pos, "valve", &v.id, indent, pretty)
}

fn render_instrument(instr: &Instrument, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let sym = symbols::instrument_symbol(&instr.instr_type);
    render_symbol_elements(&sym.elements, pos, "instrument", &instr.id, indent, pretty)
}

fn render_junction(id: &str, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    format!(
        "{}{}<circle id=\"{}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" class=\"junction\"/>{}",
        indent, indent, id, pos.x, pos.y, nl
    )
}

fn render_symbol_elements(
    elements: &[SymbolElement],
    pos: &SvgPos,
    css_class: &str,
    id: &str,
    indent: &str,
    pretty: bool,
) -> String {
    let nl = if pretty { "\n" } else { "" };
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}<g id=\"{}\" class=\"{}\" transform=\"translate({:.1},{:.1})\">{}",
        indent, indent, id, css_class, pos.x, pos.y, nl
    ));

    for elem in elements {
        let elem_str = match elem {
            SymbolElement::Rect { x, y, w, h, rx } => {
                format!(
                    "{}{}{}<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\"/>{}",
                    indent, indent, indent, x, y, w, h, rx, nl
                )
            }
            SymbolElement::Circle { cx, cy, r } => {
                format!(
                    "{}{}{}<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\"/>{}",
                    indent, indent, indent, cx, cy, r, nl
                )
            }
            SymbolElement::Path { d } => {
                format!(
                    "{}{}{}<path d=\"{}\"/>{}",
                    indent, indent, indent, d, nl
                )
            }
            SymbolElement::Line { x1, y1, x2, y2 } => {
                format!(
                    "{}{}{}<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>{}",
                    indent, indent, indent, x1, y1, x2, y2, nl
                )
            }
            SymbolElement::Polyline { points } => {
                let pts: String = points.iter()
                    .map(|(x, y)| format!("{:.1},{:.1}", x, y))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(
                    "{}{}{}<polyline points=\"{}\"/>{}",
                    indent, indent, indent, pts, nl
                )
            }
        };
        out.push_str(&elem_str);
    }

    out.push_str(&format!("{}{}</g>{}", indent, indent, nl));
    out
}

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
            "pneumatic" => Some("arrow-end-pneumatic".to_string()),
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
    }
}
