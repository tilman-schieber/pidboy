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
    // Compute canvas bounds; a legend goes below the drawing and grows the
    // canvas so it can never clash with the diagram itself.
    let (mut canvas_w, mut canvas_h) = compute_canvas(diagram, layout, opts);
    let legend_entries = if opts.legend {
        build_legend_entries(diagram)
    } else {
        Vec::new()
    };
    let (_, content_bottom) = content_extent(diagram, layout);
    let annex_y = content_bottom + 44.0;
    let legend_origin = if legend_entries.is_empty() {
        None
    } else {
        let (lw, lh) = legend_dims(legend_entries.len());
        if opts.width.is_none() && opts.height.is_none() {
            canvas_w = canvas_w.max((60.0 + lw + 60.0).ceil() as u32);
            canvas_h = canvas_h.max((annex_y + lh + 60.0).ceil() as u32);
        }
        Some((60.0, annex_y))
    };

    // Equipment data table sits right of the legend (or at the left margin).
    let table_items: Vec<(&str, &Vec<(String, String)>)> = if opts.table {
        let mut items = Vec::new();
        for (kind, id) in &diagram.order {
            use crate::ast::DeclKind;
            let (label, data) = match kind {
                DeclKind::Equipment => diagram
                    .equipment
                    .get(id)
                    .map(|e| (e.label.as_deref().unwrap_or(id), &e.data)),
                DeclKind::Valve => diagram
                    .valves
                    .get(id)
                    .map(|v| (v.label.as_deref().unwrap_or(id), &v.data)),
                _ => None,
            }
            .unwrap_or(("", &EMPTY_DATA));
            if !data.is_empty() {
                items.push((label.split('\n').next().unwrap_or(label), data));
            }
        }
        items
    } else {
        Vec::new()
    };
    let table_origin = if table_items.is_empty() {
        None
    } else {
        let ox = match &legend_origin {
            Some((lx, _)) => lx + legend_dims(legend_entries.len()).0 + 40.0,
            None => 60.0,
        };
        let keys = table_keys(&table_items);
        let tw = 120.0 + table_items.len() as f64 * 115.0;
        let th = (keys.len() + 1) as f64 * 20.0;
        if opts.width.is_none() && opts.height.is_none() {
            canvas_w = canvas_w.max((ox + tw + 60.0).ceil() as u32);
            canvas_h = canvas_h.max((annex_y + th + 60.0).ceil() as u32);
        }
        Some((ox, annex_y))
    };

    // Title block bottom-right.
    let title_lines: Vec<String> = opts
        .title
        .iter()
        .cloned()
        .chain(opts.footers.iter().cloned())
        .collect();
    if !title_lines.is_empty() && opts.width.is_none() && opts.height.is_none() {
        canvas_h = canvas_h.max((annex_y + title_lines.len() as f64 * 16.0 + 40.0).ceil() as u32);
    }

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
    out.push_str(&build_symbol_defs(diagram, indent, opts.pretty));
    out.push_str(&format!("{}</defs>{}", indent, nl));

    // Module frames (framed groups): dashed boxes behind the drawing.
    let framed: Vec<&Group> = diagram.groups.values().filter(|g| g.frame).collect();
    if !framed.is_empty() {
        out.push_str(&format!("{}<g id=\"frames\">{}", indent, nl));
        for g in &framed {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for m in &g.members {
                if let Some(b) = layout.get_bounds(m) {
                    min_x = min_x.min(b.x);
                    min_y = min_y.min(b.y);
                    max_x = max_x.max(b.x + b.w);
                    max_y = max_y.max(b.y + b.h);
                }
            }
            if !min_x.is_finite() {
                continue;
            }
            // Room for member labels and tags around the symbols.
            let (fx, fy) = (min_x - 30.0, min_y - 30.0);
            let (fw, fh) = (max_x - min_x + 60.0, max_y - min_y + 64.0);
            out.push_str(&format!(
                "{}{}<rect id=\"{}\" x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"black\" stroke-width=\"1\" stroke-dasharray=\"2,3\"/>{}",
                indent, indent, escape_xml(&g.id), fx, fy, fw, fh, nl
            ));
            if let Some(label) = &g.label {
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"12\" font-weight=\"bold\" text-anchor=\"end\" fill=\"black\" stroke=\"none\">{}</text>{}",
                    indent, indent, fx + fw - 8.0, fy + fh - 8.0, escape_xml(label), nl
                ));
            }
        }
        out.push_str(&format!("{}</g>{}", indent, nl));
    }

    // Lines group
    out.push_str(&format!("{}<g id=\"lines\">{}", indent, nl));
    for seg in routes.iter().filter(|s| !s.is_signal) {
        let class = seg
            .class
            .clone()
            .unwrap_or_else(|| line_class_for_id(&seg.connection_id, diagram));
        // Flow-direction arrow on piping, but not on instrument leaders.
        // Leaders also get no id: their connection_id is the instrument's
        // id, which the symbol <use> already carries.
        let arrow = seg.class.is_none();
        let id = if seg.class.is_none() { Some(seg.connection_id.as_str()) } else { None };
        out.push_str(&render_polyline_id(&seg.points, &class, indent, opts.pretty, arrow, id));
        if let Some(line) = diagram.lines.get(&seg.connection_id) {
            if line.flexible {
                out.push_str(&render_flex_hose(&seg.points, indent, opts.pretty));
            }
            if line.insulated {
                out.push_str(&render_insulation(&seg.points, indent, opts.pretty));
            }
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    // Signals group
    out.push_str(&format!("{}<g id=\"signals\">{}", indent, nl));
    for seg in routes.iter().filter(|s| s.is_signal) {
        let class = signal_class_for_id(&seg.connection_id, diagram);
        out.push_str(&render_polyline_id(
            &seg.points, &class, indent, opts.pretty, true,
            Some(seg.connection_id.as_str()),
        ));
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

    // Spare nozzles: equipment ports declared but not used by any line or
    // attachment render as short blind stubs with the port name.
    {
        let mut used: HashSet<(String, String)> = HashSet::new();
        let mut mark = |r: &ObjRef| {
            if let Some(p) = &r.port {
                used.insert((r.id.clone(), p.clone()));
            }
        };
        for l in diagram.lines.values() {
            mark(&l.from);
            if let Some(t) = &l.to {
                mark(t);
            }
        }
        for sg in diagram.signals.values() {
            mark(&sg.from);
            mark(&sg.to);
        }
        for i in diagram.instruments.values() {
            if let Some(a) = &i.attach {
                mark(a);
            }
        }
        for e in diagram.equipment.values() {
            if let Some(a) = &e.attach {
                mark(a);
            }
        }
        let mut spare = String::new();
        for eq in diagram.equipment.values() {
            for port in &eq.ports {
                if used.contains(&(eq.id.clone(), port.name.clone())) {
                    continue;
                }
                let Some(pos) = layout.port_pos(&eq.id, &port.name, &eq.ports) else {
                    continue;
                };
                let Some(side) = port
                    .side
                    .or_else(|| crate::layout::infer_port_side(&port.name))
                else {
                    continue;
                };
                let (ux, uy) = match side {
                    Side::East => (1.0, 0.0),
                    Side::West => (-1.0, 0.0),
                    Side::North => (0.0, -1.0),
                    Side::South => (0.0, 1.0),
                };
                let (px, py) = (-uy, ux);
                let (ex, ey) = (pos.x + ux * 12.0, pos.y + uy * 12.0);
                spare.push_str(&format!(
                    "{}{}<path d=\"M {:.1} {:.1} L {:.1} {:.1} M {:.1} {:.1} L {:.1} {:.1}\" fill=\"none\" stroke=\"black\" stroke-width=\"1\"/>{}",
                    indent, indent, pos.x, pos.y, ex, ey,
                    ex - px * 5.0, ey - py * 5.0, ex + px * 5.0, ey + py * 5.0, nl
                ));
                spare.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"line-label\">{}</text>{}",
                    indent, indent,
                    ex + ux * 12.0,
                    ey + uy * 12.0 + 3.0,
                    escape_xml(&port.name.to_uppercase()),
                    nl
                ));
            }
        }
        if !spare.is_empty() {
            out.push_str(&format!("{}<g id=\"spare-nozzles\">{}", indent, nl));
            out.push_str(&spare);
            out.push_str(&format!("{}</g>{}", indent, nl));
        }
    }

    // Labels group
    out.push_str(&format!("{}<g id=\"labels\">{}", indent, nl));
    let mut label_rects: Vec<crate::layout::SvgRect> = Vec::new();
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
                let (half_w, half_h) = bounds.map(|b| (b.w / 2.0, b.h / 2.0)).unwrap_or((30.0, 30.0));

                // Instruments carry their tag inside the bubble (ISA style)
                // whenever it fits as two short lines.
                if matches!(kind, DeclKind::Instrument) {
                    if let Some((top, bottom)) = bubble_lines(label) {
                        out.push_str(&format!(
                            "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"8\" text-anchor=\"middle\" fill=\"black\" stroke=\"none\">{}</text>{}",
                            indent, indent, pos.x, pos.y - 2.0, escape_xml(&top), nl
                        ));
                        out.push_str(&format!(
                            "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"8\" text-anchor=\"middle\" fill=\"black\" stroke=\"none\">{}</text>{}",
                            indent, indent, pos.x, pos.y + 7.0, escape_xml(&bottom), nl
                        ));
                        continue;
                    }
                }

                let lines = label_lines(label);
                let n_lines = lines.len();
                let longest = lines
                    .iter()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(0) as f64;
                let block_h = n_lines as f64 * 12.0;

                // Large vessels/tanks (and packaged units) carry their tag
                // inside the shell when the whole block fits.
                let inside_key = matches!(kind, DeclKind::Equipment)
                    && diagram
                        .equipment
                        .get(id)
                        .map(|e| {
                            matches!(
                                symbols::equipment_symbol_key(&e.equip_type),
                                "vessel" | "thermostat"
                            )
                        })
                        .unwrap_or(false);
                let inside = inside_key
                    && longest * 6.6 + 30.0 < half_w * 2.0
                    && block_h + 20.0 <= half_h * 2.0;

                let (x, y, anchor, rect) = if inside {
                    let y0 = pos.y + 4.0 - (n_lines as f64 - 1.0) * 6.0;
                    let rect = crate::layout::SvgRect {
                        x: pos.x - longest * 3.3,
                        y: y0 - 12.0,
                        w: longest * 6.6,
                        h: block_h,
                    };
                    (pos.x, y0, "", rect)
                } else {
                    let order = if matches!(kind, DeclKind::Equipment)
                        && diagram
                            .equipment
                            .get(id)
                            .and_then(|e| e.attach.as_ref())
                            .map(|a| {
                                let declared = a.port.as_deref().and_then(|pn| {
                                    diagram.get_ports(&a.id).and_then(|ports| {
                                        ports
                                            .iter()
                                            .find(|p| p.name == pn)
                                            .and_then(|p| p.side)
                                    })
                                });
                                declared
                                    .or_else(|| {
                                        a.port
                                            .as_deref()
                                            .and_then(crate::layout::infer_port_side)
                                    })
                                    .unwrap_or(Side::South)
                                    == Side::North
                            })
                            .unwrap_or(false)
                    {
                        LabelOrder::High
                    } else {
                        LabelOrder::Default
                    };
                    let (x, y, anchor, mut r) = place_label_ordered(
                        lines[0], pos, half_w, half_h, routes, layout, id, &label_rects, order,
                    );
                    r.h = block_h;
                    (x, y, anchor, r)
                };
                label_rects.push(rect);
                for (i, line) in lines.iter().enumerate() {
                    out.push_str(&format!(
                        "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"label\"{}>{}</text>{}",
                        indent, indent,
                        x, y + i as f64 * 12.0, anchor,
                        escape_xml(line),
                        nl
                    ));
                }

                // Valve state / fail-action tag ("N.C.", "FC", ...) under
                // the valve, one line below where its label defaults to.
                if let DeclKind::Valve = kind {
                    if let Some(v) = diagram.valves.get(id) {
                        let tag = match v.state.as_deref() {
                            Some("nc") => Some("N.C.".to_string()),
                            Some("no") => Some("N.O.".to_string()),
                            Some(other) => Some(other.to_uppercase()),
                            None => match v.fail.as_deref() {
                                Some("closed") => Some("FC".to_string()),
                                Some("open") => Some("FO".to_string()),
                                Some("last") => Some("FL".to_string()),
                                _ => None,
                            },
                        };
                        for extra in [tag, v.setpoint.clone()].into_iter().flatten() {
                            // Offset past the valve label (its rect plus the
                            // 2px label-collision margin), so "below" stays
                            // available directly beneath the tag number.
                            let (tx, ty, tanchor, trect) = place_label_ordered(
                                &extra, pos, half_w, half_h + 16.0, routes, layout, id,
                                &label_rects, LabelOrder::Low,
                            );
                            label_rects.push(trect);
                            out.push_str(&format!(
                                "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"line-label\"{}>{}</text>{}",
                                indent, indent, tx, ty, tanchor, escape_xml(&extra), nl
                            ));
                        }
                    }
                }
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
            // Open-ended stubs carry their label just past the arrow tip
            // instead of at the (very short) midpoint.
            let is_stub = !seg.is_signal
                && diagram
                    .lines
                    .get(&seg.connection_id)
                    .map(|l| l.to.is_none())
                    .unwrap_or(false);
            if is_stub && seg.points.len() >= 2 {
                let tip = seg.points[seg.points.len() - 1];
                let prev = seg.points[seg.points.len() - 2];
                let dx = tip.x - prev.x;
                let dy = tip.y - prev.y;
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                let (ux, uy) = (dx / len, dy / len);
                let (tx, ty, anchor) = if uy.abs() > ux.abs() {
                    // vertical stub: label below/above the tip
                    (tip.x, tip.y + uy * 14.0 + if uy > 0.0 { 6.0 } else { 0.0 }, "")
                } else if ux > 0.0 {
                    (tip.x + 8.0, tip.y + 3.0, " style=\"text-anchor:start\"")
                } else {
                    (tip.x - 8.0, tip.y + 3.0, " style=\"text-anchor:end\"")
                };
                out.push_str(&format!(
                    "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"line-label\"{}>{}</text>{}",
                    indent, indent, tx, ty, anchor, escape_xml(label), nl
                ));
                continue;
            }
            if seg.points.len() >= 2 {
                let mid_idx = seg.points.len() / 2;
                let p1 = &seg.points[mid_idx - 1];
                let p2 = &seg.points[mid_idx];
                let mx = (p1.x + p2.x) / 2.0;
                let my = (p1.y + p2.y) / 2.0;
                let dx = (p2.x - p1.x).abs();
                let dy = (p2.y - p1.y).abs();
                // Offset the label perpendicular to the line: above for
                // horizontal runs, left for vertical (rotated) runs.
                let (transform, tx, ty) = if dy > dx {
                    (
                        format!(" transform=\"rotate(-90,{:.1},{:.1})\"", mx - 6.0, my),
                        mx - 6.0,
                        my,
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
                    "{}{}<text id=\"{}\" x=\"{:.1}\" y=\"{:.1}\" class=\"note\">{}</text>{}",
                    indent, indent,
                    escape_xml(&note.id),
                    pos.x, pos.y,
                    escape_xml(text),
                    nl
                ));
            }
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));

    if let Some((ox, oy)) = legend_origin {
        out.push_str(&render_legend(&legend_entries, ox, oy, indent, opts.pretty));
    }
    if let Some((ox, oy)) = table_origin {
        out.push_str(&render_table(&table_items, ox, oy, indent, opts.pretty));
    }
    if !title_lines.is_empty() {
        out.push_str(&format!("{}<g id=\"titleblock\">{}", indent, nl));
        let base_y = canvas_h as f64 - 24.0 - (title_lines.len() as f64 - 1.0) * 16.0;
        for (i, line) in title_lines.iter().enumerate() {
            let weight = if i == 0 { " font-weight=\"bold\"" } else { "" };
            let size = if i == 0 { 13 } else { 11 };
            out.push_str(&format!(
                "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"{}\"{} text-anchor=\"end\" fill=\"black\" stroke=\"none\">{}</text>{}",
                indent, indent,
                canvas_w as f64 - 60.0,
                base_y + i as f64 * 16.0,
                size, weight,
                escape_xml(line),
                nl
            ));
        }
        out.push_str(&format!("{}</g>{}", indent, nl));
    }

    out.push_str("</svg>");
    out
}

static EMPTY_DATA: Vec<(String, String)> = Vec::new();

/// Attribute keys across all table items, in first-seen order.
fn table_keys<'a>(items: &[(&'a str, &'a Vec<(String, String)>)]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for (_, data) in items {
        for (k, _) in data.iter() {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
    }
    keys
}

/// Equipment data table: one column per item, one row per attribute
/// (reference drawing style).
fn render_table(
    items: &[(&str, &Vec<(String, String)>)],
    ox: f64,
    oy: f64,
    indent: &str,
    pretty: bool,
) -> String {
    let nl = if pretty { "\n" } else { "" };
    let keys = table_keys(items);
    let key_w = 120.0;
    let col_w = 115.0;
    let row_h = 20.0;
    let w = key_w + items.len() as f64 * col_w;
    let h = (keys.len() + 1) as f64 * row_h;

    let mut out = format!("{}<g id=\"equipment-table\">{}", indent, nl);
    out.push_str(&format!(
        "{}{}<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"white\" stroke=\"black\" stroke-width=\"1\"/>{}",
        indent, indent, ox, oy, w, h, nl
    ));
    let mut grid = String::new();
    for r in 1..=keys.len() {
        let y = oy + r as f64 * row_h;
        grid.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} ", ox, y, ox + w, y));
    }
    for c in 0..items.len() {
        let x = ox + key_w + c as f64 * col_w;
        grid.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} ", x, oy, x, oy + h));
    }
    out.push_str(&format!(
        "{}{}<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.6\"/>{}",
        indent, indent, grid.trim_end(), nl
    ));

    let cell = |x: f64, y: f64, text: &str, bold: bool| {
        format!(
            "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"10\"{} fill=\"black\" stroke=\"none\">{}</text>{}",
            indent, indent, x, y,
            if bold { " font-weight=\"bold\"" } else { "" },
            escape_xml(text), nl
        )
    };
    out.push_str(&cell(ox + 6.0, oy + 14.0, "Position", true));
    for (c, (label, _)) in items.iter().enumerate() {
        out.push_str(&cell(ox + key_w + c as f64 * col_w + 6.0, oy + 14.0, label, true));
    }
    for (r, key) in keys.iter().enumerate() {
        let y = oy + (r + 1) as f64 * row_h + 14.0;
        let disp = {
            let mut d = key.replace('_', " ");
            if let Some(c0) = d.get_mut(0..1) {
                let up = c0.to_uppercase();
                d.replace_range(0..1, &up);
            }
            d
        };
        out.push_str(&cell(ox + 6.0, y, &disp, false));
        for (c, (_, data)) in items.iter().enumerate() {
            if let Some((_, v)) = data.iter().find(|(k, _)| k == key) {
                out.push_str(&cell(ox + key_w + c as f64 * col_w + 6.0, y, v, false));
            }
        }
    }
    out.push_str(&format!("{}</g>{}", indent, nl));
    out
}

/// Choose a spot for an object label that doesn't sit on a routed line,
/// another symbol, or an already-placed label: below the symbol by default,
/// then right, above, and the lower/upper right corners.
#[allow(clippy::too_many_arguments)]
fn place_label(
    label: &str,
    pos: &SvgPos,
    half_w: f64,
    half_h: f64,
    routes: &[RouteSegment],
    layout: &LayoutInfo,
    own_id: &str,
    placed_labels: &[crate::layout::SvgRect],
) -> (f64, f64, &'static str, crate::layout::SvgRect) {
    place_label_ordered(
        label, pos, half_w, half_h, routes, layout, own_id, placed_labels, LabelOrder::Default,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum LabelOrder {
    Default,
    /// Keep the text at or below the symbol if at all possible (valve state
    /// tags read wrong when they float above, near other rows).
    Low,
    /// Prefer above (equipment mounted on top of something, e.g. motors).
    High,
}

#[allow(clippy::too_many_arguments)]
fn place_label_ordered(
    label: &str,
    pos: &SvgPos,
    half_w: f64,
    half_h: f64,
    routes: &[RouteSegment],
    layout: &LayoutInfo,
    own_id: &str,
    placed_labels: &[crate::layout::SvgRect],
    order: LabelOrder,
) -> (f64, f64, &'static str, crate::layout::SvgRect) {
    let text_w = label.chars().count() as f64 * 6.6;
    let text_h = 12.0;
    let start = r#" style="text-anchor:start""#;
    let end = r#" style="text-anchor:end""#;

    let below = (pos.x, pos.y + half_h + 14.0, "");
    let above = (pos.x, pos.y - half_h - 8.0, "");
    let right = (pos.x + half_w + 8.0, pos.y + 4.0, start);
    let left = (pos.x - half_w - 8.0, pos.y + 4.0, end);
    let below_right = (pos.x + half_w + 6.0, pos.y + half_h + 14.0, start);
    let above_right = (pos.x + half_w + 6.0, pos.y - half_h - 8.0, start);

    let rect_for = |x: f64, y: f64, anchor: &str| crate::layout::SvgRect {
        x: if anchor.is_empty() {
            x - text_w / 2.0
        } else if anchor == end {
            x - text_w
        } else {
            x
        },
        y: y - text_h + 2.0,
        w: text_w,
        h: text_h,
    };

    let candidates = match order {
        LabelOrder::Low => [below, below_right, right, left, above, above_right],
        LabelOrder::High => [above, above_right, right, left, below, below_right],
        LabelOrder::Default => [below, above, right, left, below_right, above_right],
    };
    for (x, y, anchor) in candidates {
        let rect = rect_for(x, y, anchor);
        let hits_route = routes.iter().any(|seg| {
            seg.points
                .windows(2)
                .any(|w| rect.intersects_segment(w[0].x, w[0].y, w[1].x, w[1].y))
        });
        let hits_symbol = layout
            .bounds
            .iter()
            .any(|(bid, b)| bid != own_id && b.intersects_rect(&rect, 2.0));
        let hits_label = placed_labels.iter().any(|r| r.intersects_rect(&rect, 2.0));
        if !hits_route && !hits_symbol && !hits_label {
            return (x, y, anchor, rect);
        }
    }
    let rect = rect_for(below.0, below.1, below.2);
    (below.0, below.1, below.2, rect)
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
        let key = symbols::valve_symbol_key(&v.valve_type, v.actuator.as_deref());
        if seen.insert(key.to_string()) {
            let sym = symbols::valve_symbol(&v.valve_type, v.actuator.as_deref());
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
        SymbolElement::Dot { cx, cy, r } => format!(
            "{}<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"black\" stroke=\"none\"/>{}",
            indent, cx, cy, r, nl
        ),
        SymbolElement::SolidPath { d } => format!(
            "{}<path d=\"{}\" fill=\"black\" stroke=\"black\" stroke-width=\"{}\"/>{}",
            indent, d, SYM_SW, nl
        ),
        SymbolElement::Text { x, y, text, size } => format!(
            "{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"{:.1}\" text-anchor=\"middle\" fill=\"#555\" stroke=\"none\">{}</text>{}",
            indent, x, y, size, escape_xml(text), nl
        ),
    }
}

// ---- Canvas / styles / markers ----

/// Rightmost/bottommost drawing extent including a 60px margin.
fn content_extent(diagram: &Diagram, layout: &LayoutInfo) -> (f64, f64) {
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

    (max_x, max_y)
}

fn compute_canvas(diagram: &Diagram, layout: &LayoutInfo, opts: &SvgOptions) -> (u32, u32) {
    if opts.width.is_some() || opts.height.is_some() {
        return (opts.width.unwrap_or(800), opts.height.unwrap_or(600));
    }
    let (max_x, max_y) = content_extent(diagram, layout);
    (max_x.ceil() as u32, max_y.ceil() as u32)
}

/// Longest segment of a polyline with its unit vector, if any.
fn longest_segment(points: &[SvgPos]) -> Option<(SvgPos, SvgPos, f64, f64, f64)> {
    let mut best: Option<(SvgPos, SvgPos, f64)> = None;
    for w in points.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if best.as_ref().map_or(true, |(_, _, l)| len > *l) {
            best = Some((w[0], w[1], len));
        }
    }
    best.map(|(a, b, len)| {
        let ux = (b.x - a.x) / len;
        let uy = (b.y - a.y) / len;
        (a, b, len, ux, uy)
    })
}

/// Flexible-hose squiggle: whites out a window at the middle of the run's
/// longest segment and draws a sine wave across it.
fn render_flex_hose(points: &[SvgPos], indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    let Some((a, _b, len, ux, uy)) = longest_segment(points) else {
        return String::new();
    };
    let window = 56.0f64.min(len - 8.0);
    if window < 24.0 {
        return String::new();
    }
    let start = (len - window) / 2.0;
    let (px, py) = (-uy, ux); // perpendicular
    // White-out the straight line under the squiggle (explicit shape, no
    // masks, for strict SVG importers).
    let wx = a.x + ux * start;
    let wy = a.y + uy * start;
    let mut out = format!(
        "{}{}<path d=\"M {:.1} {:.1} L {:.1} {:.1}\" fill=\"none\" stroke=\"white\" stroke-width=\"6\"/>{}",
        indent, indent, wx, wy, wx + ux * window, wy + uy * window, nl
    );
    // Sine squiggle: 4 full waves, amplitude 5.
    let mut d = format!("M {:.1} {:.1} ", wx, wy);
    let waves = 4;
    let half = window / (waves as f64 * 2.0);
    for i in 0..(waves * 2) {
        let s0 = start + i as f64 * half;
        let amp = if i % 2 == 0 { 5.0 } else { -5.0 };
        let cx = a.x + ux * (s0 + half / 2.0) + px * amp * 2.0;
        let cy = a.y + uy * (s0 + half / 2.0) + py * amp * 2.0;
        let ex = a.x + ux * (s0 + half);
        let ey = a.y + uy * (s0 + half);
        d.push_str(&format!("Q {:.1} {:.1} {:.1} {:.1} ", cx, cy, ex, ey));
    }
    out.push_str(&format!(
        "{}{}<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"1.2\"/>{}",
        indent, indent, d.trim_end(), nl
    ));
    out
}

/// Insulation: hatched band over the middle of the run's longest segment.
fn render_insulation(points: &[SvgPos], indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    let Some((a, _b, len, ux, uy)) = longest_segment(points) else {
        return String::new();
    };
    let window = 44.0f64.min(len - 8.0);
    if window < 20.0 {
        return String::new();
    }
    let start = (len - window) / 2.0;
    let (px, py) = (-uy, ux);
    let hw = 7.0; // band half-width
    let corner = |s: f64, side: f64| -> (f64, f64) {
        (a.x + ux * s + px * hw * side, a.y + uy * s + py * hw * side)
    };
    let (x0, y0) = corner(start, 1.0);
    let (x1, y1) = corner(start + window, 1.0);
    let (x2, y2) = corner(start + window, -1.0);
    let (x3, y3) = corner(start, -1.0);
    let mut out = format!(
        "{}{}<path d=\"M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z\" fill=\"white\" stroke=\"black\" stroke-width=\"1\"/>{}",
        indent, indent, x0, y0, x1, y1, x2, y2, x3, y3, nl
    );
    // Diagonal hatch lines
    let mut d = String::new();
    let n = (window / 8.0) as usize;
    for i in 1..n {
        let s0 = start + i as f64 * 8.0;
        let (hx0, hy0) = corner(s0 - 5.0, -1.0);
        let (hx1, hy1) = corner(s0, 1.0);
        d.push_str(&format!("M {:.1} {:.1} L {:.1} {:.1} ", hx0, hy0, hx1, hy1));
    }
    if !d.is_empty() {
        out.push_str(&format!(
            "{}{}<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"0.8\"/>{}",
            indent, indent, d.trim_end(), nl
        ));
    }
    out
}

// ---- Legend ----

enum LegendSample {
    /// A `<use>` of an already-emitted symbol def, scaled to fit the cell.
    Symbol { key: String, w: f64, h: f64 },
    /// A legend-only symbol drawn inline (simplified outlines, internals).
    Inline(symbols::SymbolDef),
    /// A short sample line with the class's stroke/dash and a flow arrow.
    Stroke { class: String, arrow: bool },
    /// Flexible-hose squiggle sample.
    Flex,
    /// Insulation hatch-band sample.
    Insulation,
    /// Junction/tee dot.
    Junction,
}

struct LegendEntry {
    sample: LegendSample,
    label: &'static str,
}

const LEGEND_ROWS_PER_COL: usize = 5;
const LEGEND_CELL_H: f64 = 50.0;
const LEGEND_COL_W: f64 = 250.0;
const LEGEND_PAD: f64 = 14.0;
const LEGEND_TITLE_H: f64 = 26.0;

fn legend_dims(n: usize) -> (f64, f64) {
    let cols = n.div_ceil(LEGEND_ROWS_PER_COL).max(1);
    let rows = n.min(LEGEND_ROWS_PER_COL).max(1);
    (
        LEGEND_PAD * 2.0 + cols as f64 * LEGEND_COL_W,
        LEGEND_PAD + LEGEND_TITLE_H + rows as f64 * LEGEND_CELL_H + LEGEND_PAD,
    )
}

fn equipment_key_label(key: &str) -> &'static str {
    match key {
        "pump" => "Centrifugal pump",
        "pump_pd" => "Positive-displacement pump",
        "heat_exchanger" => "Heat exchanger",
        "vessel" => "Vessel / tank",
        "separator" => "Separator",
        "separator_3phase" => "Three-phase separator",
        "reactor_cstr" => "Stirred reactor (CSTR)",
        "reactor_pfr" => "Plug-flow reactor",
        "compressor" => "Compressor",
        "blower" => "Blower / fan",
        "mixer" => "Mixer",
        "column" => "Distillation column",
        "connector" => "Off-page connector",
        "heat_pad" => "Heat pad (electric)",
        "vacuum_pump" => "Vacuum pump",
        "canister" => "Feed canister",
        "motor" => "Motor / stirrer drive",
        "thermostat" => "Thermostat / packaged unit",
        _ => "Equipment",
    }
}

fn valve_key_label(key: &str) -> &'static str {
    match key {
        "control_valve" => "Control valve",
        "control_valve_diaphragm" => "Control valve (diaphragm)",
        "check_valve" => "Check valve",
        "relief_valve" => "Relief valve (PSV)",
        "valve_globe" => "Globe valve",
        "valve_needle" => "Needle valve",
        "valve_three_way" => "3-way valve",
        "valve_pcv" => "Pressure reducer (PCV)",
        "valve_solenoid" => "Solenoid valve",
        "bursting_disc" => "Bursting disc",
        _ => "Manual valve",
    }
}

fn instrument_key_label(key: &str) -> &'static str {
    match key {
        "instr_panel" => "Panel instrument",
        "instr_control_room" => "Control-room instrument",
        "instr_shared" => "Shared display (DCS)",
        _ => "Field instrument",
    }
}

fn line_class_label(class: &str) -> &'static str {
    match class {
        "process" => "Process line",
        "utility" => "Utility line",
        "drain" => "Drain line",
        "vent" => "Vent line",
        _ => "Line",
    }
}

fn signal_type_label(sig: &str) -> &'static str {
    match sig {
        "electrical" => "Electrical signal",
        "pneumatic" => "Pneumatic signal",
        "hydraulic" => "Hydraulic signal",
        "digital" => "Digital signal",
        _ => "Signal",
    }
}

/// One legend entry per distinct symbol / line style actually present in
/// the diagram, in a stable order: equipment, valves, instruments,
/// junctions, line classes, signal types.
fn build_legend_entries(diagram: &Diagram) -> Vec<LegendEntry> {
    let mut entries = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for eq in diagram.equipment.values() {
        let key = symbols::equipment_symbol_key(&eq.equip_type);
        if seen.insert(format!("e:{}", key)) {
            // Composite symbols use a simplified outline in the legend;
            // their internals get standalone entries below.
            let sample = match symbols::legend_symbol(key) {
                Some(def) => LegendSample::Inline(def),
                None => {
                    let s = symbols::equipment_symbol(&eq.equip_type);
                    LegendSample::Symbol {
                        key: key.to_string(),
                        w: s.width,
                        h: s.height,
                    }
                }
            };
            entries.push(LegendEntry {
                sample,
                label: equipment_key_label(key),
            });
            if key == "separator_3phase" {
                entries.push(LegendEntry {
                    sample: LegendSample::Inline(symbols::demister_glyph()),
                    label: "Demister pad",
                });
            }
        }
    }
    for v in diagram.valves.values() {
        let key = symbols::valve_symbol_key(&v.valve_type, v.actuator.as_deref());
        if seen.insert(format!("v:{}", key)) {
            let s = symbols::valve_symbol(&v.valve_type, v.actuator.as_deref());
            entries.push(LegendEntry {
                sample: LegendSample::Symbol {
                    key: key.to_string(),
                    w: s.width,
                    h: s.height,
                },
                label: valve_key_label(key),
            });
        }
    }
    for instr in diagram.instruments.values() {
        let key = symbols::instrument_symbol_key(instr.location.as_deref());
        if seen.insert(format!("i:{}", key)) {
            let s = symbols::instrument_symbol(&instr.instr_type, instr.location.as_deref());
            entries.push(LegendEntry {
                sample: LegendSample::Symbol {
                    key: key.to_string(),
                    w: s.width,
                    h: s.height,
                },
                label: instrument_key_label(key),
            });
        }
    }
    if !diagram.junctions.is_empty() {
        entries.push(LegendEntry {
            sample: LegendSample::Junction,
            label: "Junction / tee",
        });
    }
    for line in diagram.lines.values() {
        if seen.insert(format!("l:{}", line.class)) {
            entries.push(LegendEntry {
                sample: LegendSample::Stroke {
                    class: format!("line-{}", line.class),
                    arrow: true,
                },
                label: line_class_label(&line.class),
            });
        }
    }
    if diagram.lines.values().any(|l| l.flexible) {
        entries.push(LegendEntry {
            sample: LegendSample::Flex,
            label: "Flexible tube",
        });
    }
    if diagram.lines.values().any(|l| l.insulated) {
        entries.push(LegendEntry {
            sample: LegendSample::Insulation,
            label: "Insulation",
        });
    }
    for sig in diagram.signals.values() {
        if seen.insert(format!("s:{}", sig.sig_type)) {
            entries.push(LegendEntry {
                sample: LegendSample::Stroke {
                    class: format!("signal-{}", sig.sig_type),
                    arrow: true,
                },
                label: signal_type_label(&sig.sig_type),
            });
        }
    }

    entries
}

fn render_legend(
    entries: &[LegendEntry],
    origin_x: f64,
    origin_y: f64,
    indent: &str,
    pretty: bool,
) -> String {
    let nl = if pretty { "\n" } else { "" };
    let (box_w, box_h) = legend_dims(entries.len());
    let mut out = String::new();

    out.push_str(&format!("{}<g id=\"legend\">{}", indent, nl));
    out.push_str(&format!(
        "{}{}<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"white\" stroke=\"black\" stroke-width=\"1\"/>{}",
        indent, indent, origin_x, origin_y, box_w, box_h, nl
    ));
    out.push_str(&format!(
        "{}{}<text x=\"{:.1}\" y=\"{:.1}\" font-family=\"sans-serif\" font-size=\"12\" font-weight=\"bold\" fill=\"black\" stroke=\"none\">LEGEND</text>{}",
        indent, indent,
        origin_x + LEGEND_PAD,
        origin_y + LEGEND_PAD + 6.0,
        nl
    ));

    for (i, entry) in entries.iter().enumerate() {
        let col = i / LEGEND_ROWS_PER_COL;
        let row = i % LEGEND_ROWS_PER_COL;
        let cell_x = origin_x + LEGEND_PAD + col as f64 * LEGEND_COL_W;
        let cell_y = origin_y + LEGEND_PAD + LEGEND_TITLE_H + row as f64 * LEGEND_CELL_H;
        // Sample centered in an 84px-wide slot; label text to its right.
        let cx = cell_x + 42.0;
        let cy = cell_y + LEGEND_CELL_H / 2.0;

        match &entry.sample {
            LegendSample::Symbol { key, w, h } => {
                let scale = (72.0 / w).min(34.0 / h).min(0.75);
                out.push_str(&format!(
                    "{}{}<use href=\"#sym-{}\" xlink:href=\"#sym-{}\" transform=\"translate({:.1},{:.1}) scale({:.3})\"/>{}",
                    indent, indent, key, key, cx, cy, scale, nl
                ));
            }
            LegendSample::Inline(def) => {
                let scale = (72.0 / def.width).min(34.0 / def.height).min(0.75);
                out.push_str(&format!(
                    "{}{}<g transform=\"translate({:.1},{:.1}) scale({:.3})\">{}",
                    indent, indent, cx, cy, scale, nl
                ));
                for elem in &def.elements {
                    out.push_str(&render_element(elem, indent, nl));
                }
                out.push_str(&format!("{}{}</g>{}", indent, indent, nl));
            }
            LegendSample::Stroke { class, arrow } => {
                let pts = [
                    SvgPos { x: cx - 36.0, y: cy },
                    SvgPos { x: cx + 36.0, y: cy },
                ];
                out.push_str(&render_polyline(&pts, class, indent, pretty, *arrow));
            }
            LegendSample::Flex => {
                let pts = [
                    SvgPos { x: cx - 36.0, y: cy },
                    SvgPos { x: cx + 36.0, y: cy },
                ];
                out.push_str(&render_polyline(&pts, "line-attach", indent, pretty, false));
                out.push_str(&render_flex_hose(&pts, indent, pretty));
            }
            LegendSample::Insulation => {
                let pts = [
                    SvgPos { x: cx - 36.0, y: cy },
                    SvgPos { x: cx + 36.0, y: cy },
                ];
                out.push_str(&render_polyline(&pts, "line-attach", indent, pretty, false));
                out.push_str(&render_insulation(&pts, indent, pretty));
            }
            LegendSample::Junction => {
                out.push_str(&format!(
                    "{}{}<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"black\" stroke=\"none\"/>{}",
                    indent, indent, cx, cy, nl
                ));
            }
        }

        out.push_str(&format!(
            "{}{}<text x=\"{:.1}\" y=\"{:.1}\" class=\"label\" style=\"text-anchor:start\">{}</text>{}",
            indent, indent,
            cell_x + 92.0,
            cy + 4.0,
            escape_xml(entry.label),
            nl
        ));
    }

    out.push_str(&format!("{}</g>{}", indent, nl));
    out
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
    .line-utility { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 14,5; }
    .line-drain { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 9,3,1.5,3; }
    .line-vent { fill: none; stroke: black; stroke-width: 1; stroke-dasharray: 1.5,4; stroke-linecap: round; }
    .line-attach { fill: none; stroke: black; stroke-width: 1; }
    .signal-electrical { fill: none; stroke: black; stroke-width: 1.5; stroke-dasharray: 6,3; }
    .signal-pneumatic { fill: none; stroke: black; stroke-width: 1.5; }
    .signal-hydraulic { fill: none; stroke: black; stroke-width: 1.5; stroke-dasharray: 10,2,1,2; }
    .signal-digital { fill: none; stroke: black; stroke-width: 1.5; stroke-dasharray: 6,2,1,2; }
"#.to_string()
}

// ---- Polyline / use rendering ----

fn render_polyline(
    points: &[SvgPos],
    css_class: &str,
    indent: &str,
    pretty: bool,
    arrow: bool,
) -> String {
    render_polyline_id(points, css_class, indent, pretty, arrow, None)
}

fn render_polyline_id(
    points: &[SvgPos],
    css_class: &str,
    indent: &str,
    pretty: bool,
    arrow: bool,
    id: Option<&str>,
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

    // Explicit presentation attributes alongside the CSS class so that the
    // polyline renders correctly in viewers that do not apply internal CSS
    // stylesheets (Illustrator, Affinity, many SVG 1.1 renderers).
    let attrs = line_presentation_attrs(css_class);

    let id_attr = match id {
        Some(id) => format!(" id=\"{}\"", escape_xml(id)),
        None => String::new(),
    };
    let mut out = format!(
        "{}{}<polyline{} points=\"{}\" class=\"{}\"{}/>{}",
        indent, indent, id_attr, pts_str, css_class, attrs, nl
    );
    if css_class == "signal-pneumatic" {
        out.push_str(&render_pneumatic_marks(points, indent, pretty));
    }
    if arrow {
        out.push_str(&render_arrowhead(points, indent, pretty));
    }
    out
}

/// ISA-5.1 pneumatic signal marking: pairs of short slashes drawn across
/// the (solid) line at regular intervals. Drawn as one explicit `<path>`
/// so it survives viewers without marker/CSS support.
fn render_pneumatic_marks(points: &[SvgPos], indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    const HALF_LEN: f64 = 6.0; // half-length of one slash
    const PAIR_GAP: f64 = 5.0; // spacing between the two slashes of a pair
    const SPACING: f64 = 60.0; // nominal distance between pairs

    let mut d = String::new();
    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        // Too short for a legible pair (e.g. actuator stubs).
        if len < 26.0 {
            continue;
        }
        let (ux, uy) = (dx / len, dy / len);
        // Slash direction: segment direction rotated by ~65 degrees, so a
        // horizontal run gets "//" leaning forward.
        let (sin_t, cos_t) = 65f64.to_radians().sin_cos();
        let (vx, vy) = (ux * cos_t + uy * sin_t, -ux * sin_t + uy * cos_t);
        let n = ((len / SPACING).floor() as usize).max(1);
        for i in 0..n {
            let t = (i as f64 + 0.5) / n as f64 * len;
            for off in [-PAIR_GAP / 2.0, PAIR_GAP / 2.0] {
                let px = a.x + ux * (t + off);
                let py = a.y + uy * (t + off);
                d.push_str(&format!(
                    "M {:.1} {:.1} L {:.1} {:.1} ",
                    px - vx * HALF_LEN,
                    py - vy * HALF_LEN,
                    px + vx * HALF_LEN,
                    py + vy * HALF_LEN
                ));
            }
        }
    }
    if d.is_empty() {
        return String::new();
    }
    format!(
        "{}{}<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"1.2\"/>{}",
        indent,
        indent,
        d.trim_end(),
        nl
    )
}

/// Filled arrowhead drawn as an explicit `<path>` at the endpoint of a
/// polyline. SVG `<marker>` is deliberately avoided: Illustrator and several
/// other importers drop polylines carrying `marker-end`, which made every
/// signal line disappear.
fn render_arrowhead(points: &[SvgPos], indent: &str, pretty: bool) -> String {
    let nl = if pretty { "\n" } else { "" };
    let n = points.len();
    if n < 2 {
        return String::new();
    }
    let tip = &points[n - 1];
    // Last point that isn't coincident with the tip, for direction.
    let tail = points[..n - 1]
        .iter()
        .rev()
        .find(|p| (p.x - tip.x).abs() > 1e-6 || (p.y - tip.y).abs() > 1e-6);
    let Some(tail) = tail else {
        return String::new();
    };
    let angle = (tip.y - tail.y).atan2(tip.x - tail.x).to_degrees();
    format!(
        "{}{}<path d=\"M 0 0 L -9 -3.5 L -9 3.5 Z\" fill=\"black\" stroke=\"none\" transform=\"translate({:.1},{:.1}) rotate({:.1})\"/>{}",
        indent, indent, tip.x, tip.y, angle, nl
    )
}

/// Return explicit SVG presentation attributes matching the CSS rule for a
/// given line/signal class. These act as a fallback for viewers without CSS.
fn line_presentation_attrs(css_class: &str) -> &'static str {
    match css_class {
        "line-process"       => r#" fill="none" stroke="black" stroke-width="2""#,
        "line-utility"       => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="14,5""#,
        "line-drain"         => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="9,3,1.5,3""#,
        "line-vent"          => r#" fill="none" stroke="black" stroke-width="1" stroke-dasharray="1.5,4" stroke-linecap="round""#,
        "line-attach"        => r#" fill="none" stroke="black" stroke-width="1""#,
        "signal-electrical"  => r#" fill="none" stroke="black" stroke-width="1.5" stroke-dasharray="6,3""#,
        "signal-pneumatic"   => r#" fill="none" stroke="black" stroke-width="1.5""#,
        "signal-hydraulic"   => r#" fill="none" stroke="black" stroke-width="1.5" stroke-dasharray="10,2,1,2""#,
        "signal-digital"     => r#" fill="none" stroke="black" stroke-width="1.5" stroke-dasharray="6,2,1,2""#,
        _                    => r#" fill="none" stroke="black" stroke-width="1.5""#,
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
    // Size-overridden equipment is drawn inline (parametric geometry for
    // vessels, uniform scale otherwise) instead of via the shared def.
    if let Some(sz) = &eq.size {
        let nl = if pretty { "\n" } else { "" };
        let (def, scale) = symbols::equipment_symbol_sized(
            &eq.equip_type,
            sz.x as f64 * crate::layout::GRID_SCALE,
            sz.y as f64 * crate::layout::GRID_SCALE,
        );
        // Scale the geometry numerically instead of using an SVG scale()
        // transform, so stroke widths stay at their nominal weight.
        let def = if (scale - 1.0).abs() < 1e-9 {
            def
        } else {
            symbols::scale_symbol(&def, scale)
        };
        let mut out = format!(
            "{}{}<g id=\"{}\" class=\"equipment\" transform=\"translate({:.1},{:.1})\">{}",
            indent, indent, eq.id, pos.x, pos.y, nl
        );
        for elem in &def.elements {
            out.push_str(&render_element(elem, indent, nl));
        }
        out.push_str(&format!("{}{}</g>{}", indent, indent, nl));
        return out;
    }
    let key = symbols::equipment_symbol_key(&eq.equip_type);
    render_use(&eq.id, &format!("sym-{}", key), "equipment", pos, indent, pretty)
}

fn render_valve(v: &Valve, pos: &SvgPos, indent: &str, pretty: bool) -> String {
    let key = symbols::valve_symbol_key(&v.valve_type, v.actuator.as_deref());
    if crate::layout::valve_is_vertical(v) {
        let nl = if pretty { "\n" } else { "" };
        return format!(
            "{}{}<use id=\"{}\" href=\"#sym-{}\" xlink:href=\"#sym-{}\" class=\"valve\" transform=\"translate({:.1},{:.1}) rotate(90)\"/>{}",
            indent, indent, v.id, key, key, pos.x, pos.y, nl
        );
    }
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

fn signal_class_for_id(id: &str, diagram: &Diagram) -> String {
    if let Some(sig) = diagram.signals.get(id) {
        format!("signal-{}", sig.sig_type)
    } else {
        "signal-electrical".to_string()
    }
}

/// Split a label into display lines (the lexer turns "\\n" escapes in
/// quoted strings into real newlines).
fn label_lines(label: &str) -> Vec<&str> {
    label.split('\n').collect()
}

/// Two short lines to draw inside an instrument bubble: explicit "\\n"
/// split, else split a "TT-101"-style tag at the dash. None when the text
/// wouldn't fit legibly in a 36px bubble.
fn bubble_lines(label: &str) -> Option<(String, String)> {
    let parts = label_lines(label);
    let (top, bottom) = match parts.as_slice() {
        [one] => match one.split_once('-') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => return None,
        },
        [a, b] => (a.to_string(), b.to_string()),
        _ => return None,
    };
    if top.chars().count() <= 6 && bottom.chars().count() <= 6 {
        Some((top, bottom))
    } else {
        None
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
