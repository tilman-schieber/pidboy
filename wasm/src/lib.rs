//! Browser bindings for the pidc compiler.
//!
//! Thin shim: the pipeline and source editing live in the `pidc` crate;
//! this crate only serializes results across the JS boundary as JSON
//! strings. The DTOs here are editor-facing API, deliberately kept out of
//! the core crate so it stays serde-free.

use pidc::ast::DeclKind;
use pidc::layout::{framed_group_of, GRID_SCALE};
use pidc::render::SvgOptions;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticOut {
    severity: String,
    message: String,
    line: Option<u32>,
    col: Option<u32>,
    note: Option<String>,
    help: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeOut {
    id: String,
    kind: String,
    /// Symbol center in SVG user units.
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Center in grid units (fractional for auto-placed nodes).
    grid_x: f64,
    grid_y: f64,
    /// Whether the source pins this node with an explicit `at:`.
    pinned: bool,
    /// False for framed-module members, whose `at:` layout ignores.
    draggable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileOut {
    ok: bool,
    svg: Option<String>,
    diagnostics: Vec<DiagnosticOut>,
    nodes: Vec<NodeOut>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveOut {
    ok: bool,
    source: Option<String>,
    error: Option<String>,
}

fn diagnostics_out(diags: &pidc::diag::DiagEngine) -> Vec<DiagnosticOut> {
    diags
        .diagnostics
        .iter()
        .map(|d| DiagnosticOut {
            severity: d.severity.to_string(),
            message: d.message.clone(),
            line: d.span.map(|s| s.line),
            col: d.span.map(|s| s.col),
            note: d.note.clone(),
            help: d.help.clone(),
        })
        .collect()
}

/// Compile `.pid` source. Returns JSON:
/// `{ok, svg, diagnostics: [...], nodes: [...]}`.
#[wasm_bindgen]
pub fn compile(source: &str) -> String {
    let res = pidc::compile::compile_to_parts(source, false, &SvgOptions::default());

    let mut nodes = Vec::new();
    if let (Some(diagram), Some(layout)) = (&res.diagram, &res.layout) {
        let framed = framed_group_of(diagram);
        let (shift_x, shift_y) = layout.origin_shift;
        for (kind, id) in &diagram.order {
            let positionable = matches!(
                kind,
                DeclKind::Equipment
                    | DeclKind::Valve
                    | DeclKind::Instrument
                    | DeclKind::Junction
                    | DeclKind::Note
            );
            if !positionable {
                continue;
            }
            let (Some(pos), Some(bounds)) = (layout.positions.get(id), layout.bounds.get(id))
            else {
                continue;
            };
            let pinned = match kind {
                DeclKind::Equipment => diagram.equipment[id].pos.is_some(),
                DeclKind::Valve => diagram.valves[id].pos.is_some(),
                DeclKind::Instrument => diagram.instruments[id].pos.is_some(),
                DeclKind::Junction => diagram.junctions[id].pos.is_some(),
                DeclKind::Note => diagram.notes[id].pos.is_some(),
                _ => false,
            };
            nodes.push(NodeOut {
                id: id.clone(),
                kind: kind.to_string(),
                x: pos.x,
                y: pos.y,
                w: bounds.w,
                h: bounds.h,
                grid_x: (pos.x - shift_x) / GRID_SCALE,
                grid_y: (pos.y - shift_y) / GRID_SCALE,
                pinned,
                draggable: !framed.contains_key(id),
            });
        }
    }

    let out = CompileOut {
        ok: res.svg.is_some(),
        svg: res.svg,
        diagnostics: diagnostics_out(&res.diags),
        nodes,
    };
    serde_json::to_string(&out).unwrap_or_else(|e| {
        format!(
            "{{\"ok\":false,\"svg\":null,\"nodes\":[],\"diagnostics\":[{{\"severity\":\"error\",\"message\":\"serialization failed: {}\"}}]}}",
            e
        )
    })
}

/// Rewrite `at: (x,y)` for node `id` in `source`. Returns JSON:
/// `{ok, source, error}` — the caller recompiles the returned source.
#[wasm_bindgen]
pub fn move_node(source: &str, id: &str, x: i32, y: i32) -> String {
    let out = match pidc::edit::set_node_position(source, id, x, y) {
        Ok(new_source) => MoveOut { ok: true, source: Some(new_source), error: None },
        Err(e) => MoveOut { ok: false, source: None, error: Some(e.to_string()) },
    };
    serde_json::to_string(&out)
        .unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":\"serialization failed: {}\"}}", e))
}
