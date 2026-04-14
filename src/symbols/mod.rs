/// Abstract symbol geometry definition.
/// All coordinates are relative to the symbol center (0,0),
/// sized to fit within [-w/2, w/2] x [-h/2, h/2].
///
/// This module is intended to be backend-agnostic. Renderers (SVG, TikZ, etc.)
/// consume `SymbolDef` and translate the elements into their own output format.
///
/// # Backend coupling caveat
///
/// `SymbolElement::Path` uses SVG path syntax (`d` strings: `M`, `L`, `C`, `Q`, `Z`, …).
/// This is a deliberate pragmatic choice — SVG path syntax is the most expressive
/// geometry notation available without a heavier dependency. A non-SVG backend must
/// either parse these strings or provide parallel path definitions.
///
/// **Do not add SVG-specific element variants here** (e.g. `<use>`, `<symbol>`,
/// raw SVG markup). Any SVG structural optimisations (deduplication via `<defs>`,
/// `<symbol>/<use>` instantiation, etc.) belong in `src/render/svg.rs`, not here.

#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub width: f64,
    pub height: f64,
    /// Logical bounding box in symbol-local coordinates: (x, y, w, h).
    /// By convention this is (-width/2, -height/2, width, height).
    /// Renderers may use this for viewBox calculations or bounding-box queries;
    /// the SVG renderer does not emit it on `<symbol>` elements (no viewport needed).
    pub view_box: (f64, f64, f64, f64),
    pub elements: Vec<SymbolElement>,
}

#[derive(Debug, Clone)]
pub enum SymbolElement {
    Rect { x: f64, y: f64, w: f64, h: f64, rx: f64 },
    Circle { cx: f64, cy: f64, r: f64 },
    /// SVG path data (`d` attribute syntax). See module-level note on backend coupling.
    Path { d: String },
    Line { x1: f64, y1: f64, x2: f64, y2: f64 },
    Polyline { points: Vec<(f64, f64)> },
}

// Internal helper: build a SymbolDef with the default centred view_box.
fn sym(width: f64, height: f64, elements: Vec<SymbolElement>) -> SymbolDef {
    SymbolDef {
        view_box: (-width / 2.0, -height / 2.0, width, height),
        width,
        height,
        elements,
    }
}

// ---- Canonical key functions (for renderer deduplication) ----

/// Returns the canonical symbol key for an equipment type.
/// Multiple type aliases that share a symbol return the same key.
pub fn equipment_symbol_key(equip_type: &str) -> &'static str {
    match equip_type {
        "pump" | "pump_centrifugal" => "pump",
        "pump_positive_displacement" => "pump_pd",
        "heat_exchanger" | "heat_exchanger_shell_tube" => "heat_exchanger",
        "tank" | "vessel" => "vessel",
        "separator" => "separator",
        "reactor_cstr" | "reactor_batch" => "reactor_cstr",
        "reactor_pfr" => "reactor_pfr",
        "compressor" => "compressor",
        "blower" => "blower",
        "mixer" => "mixer",
        "distillation_column" => "column",
        _ => "equipment_default",
    }
}

/// Returns the canonical symbol key for a valve type.
pub fn valve_symbol_key(valve_type: &str) -> &'static str {
    match valve_type {
        "control_valve" => "control_valve",
        "check_valve" => "check_valve",
        "relief_valve" | "safety_valve" => "relief_valve",
        _ => "valve_manual",
    }
}

/// Returns the canonical symbol key for an instrument, determined by mounting location.
/// Per ISA/IEC 5.1, bubble style is a function of location, not instrument type.
pub fn instrument_symbol_key(location: Option<&str>) -> &'static str {
    match location.unwrap_or("field") {
        "panel" => "instr_panel",
        "control_room" => "instr_control_room",
        _ => "instr_field",
    }
}

// ---- Public dispatch ----

/// Get a symbol definition for an equipment type (ISO 10628-2).
pub fn equipment_symbol(equip_type: &str) -> SymbolDef {
    match equip_type {
        "pump" | "pump_centrifugal" => pump_symbol(),
        "pump_positive_displacement" => pump_pd_symbol(),
        "heat_exchanger" | "heat_exchanger_shell_tube" => heat_exchanger_symbol(),
        "tank" | "vessel" => vessel_symbol(),
        "separator" => separator_symbol(),
        "reactor_cstr" | "reactor_batch" => reactor_cstr_symbol(),
        "reactor_pfr" => reactor_pfr_symbol(),
        "compressor" => compressor_symbol(),
        "blower" => blower_symbol(),
        "mixer" => mixer_symbol(),
        "distillation_column" => column_symbol(),
        _ => default_equipment_symbol(),
    }
}

/// Get a symbol definition for a valve type (ISO 10628-2).
pub fn valve_symbol(valve_type: &str) -> SymbolDef {
    match valve_type {
        "control_valve" => control_valve_symbol(),
        "check_valve" => check_valve_symbol(),
        "relief_valve" | "safety_valve" => relief_valve_symbol(),
        _ => manual_valve_symbol(),
    }
}

/// Get a symbol definition for an instrument (ISA/IEC 5.1).
/// Bubble style is determined by `location`; `instr_type` is accepted for API
/// consistency and may be used for type-specific geometry in the future.
pub fn instrument_symbol(_instr_type: &str, location: Option<&str>) -> SymbolDef {
    match location.unwrap_or("field") {
        "panel" => instrument_bubble_panel(),
        "control_room" => instrument_bubble_control_room(),
        _ => instrument_bubble_field(),
    }
}

// ---- Equipment symbols (ISO 10628-2) ----

/// Centrifugal pump: circle with inscribed right-pointing triangle (impeller/volute).
fn pump_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 25.0 },
        // Triangle with tip at discharge (right), base toward suction (left)
        SymbolElement::Path { d: "M -8 -18 L 22 0 L -8 18 Z".into() },
    ])
}

/// Positive-displacement pump: circle with two piston rectangles.
fn pump_pd_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 25.0 },
        SymbolElement::Rect { x: -16.0, y: -10.0, w: 13.0, h: 20.0, rx: 1.0 },
        SymbolElement::Rect { x: 3.0,   y: -10.0, w: 13.0, h: 20.0, rx: 1.0 },
    ])
}

/// Shell-and-tube heat exchanger (ISO 10628-2): shell body with elliptical heads
/// and a horizontal pass-partition line.
fn heat_exchanger_symbol() -> SymbolDef {
    sym(120.0, 60.0, vec![
        // Closed outline: left head + shell body + right head
        SymbolElement::Path {
            d: "M -55 -22 A 22 22 0 0 0 -55 22 L 55 22 A 22 22 0 0 0 55 -22 Z".into(),
        },
        // Pass-partition line
        SymbolElement::Line { x1: -55.0, y1: 0.0, x2: 55.0, y2: 0.0 },
    ])
}

/// Horizontal pressure vessel / tank (ISO 10628-2): cylinder with elliptical heads.
fn vessel_symbol() -> SymbolDef {
    sym(100.0, 50.0, vec![
        SymbolElement::Path {
            d: "M -45 -20 A 20 20 0 0 0 -45 20 L 45 20 A 20 20 0 0 0 45 -20 Z".into(),
        },
    ])
}

/// Horizontal separator with liquid boot nozzle and internal level line.
fn separator_symbol() -> SymbolDef {
    sym(80.0, 60.0, vec![
        // Vessel outline
        SymbolElement::Path {
            d: "M -36 -18 A 18 18 0 0 0 -36 18 L 36 18 A 18 18 0 0 0 36 -18 Z".into(),
        },
        // Boot nozzle at bottom
        SymbolElement::Rect { x: -7.0, y: 18.0, w: 14.0, h: 16.0, rx: 0.0 },
        SymbolElement::Line { x1: -9.0, y1: 34.0, x2: 9.0, y2: 34.0 },
        // Internal level indicator line
        SymbolElement::Line { x1: -36.0, y1: 4.0, x2: 36.0, y2: 4.0 },
    ])
}

/// CSTR reactor: vessel with shaft and two-level Rushton impeller blades.
fn reactor_cstr_symbol() -> SymbolDef {
    sym(60.0, 80.0, vec![
        SymbolElement::Rect { x: -25.0, y: -35.0, w: 50.0, h: 70.0, rx: 5.0 },
        // Agitator shaft
        SymbolElement::Line { x1: 0.0, y1: -35.0, x2: 0.0, y2: 15.0 },
        // Upper impeller blades
        SymbolElement::Line { x1: -18.0, y1: 2.0,  x2: -3.0, y2: 2.0  },
        SymbolElement::Line { x1:  3.0,  y1: 2.0,  x2: 18.0, y2: 2.0  },
        // Lower impeller blades
        SymbolElement::Line { x1: -18.0, y1: 10.0, x2: -3.0, y2: 10.0 },
        SymbolElement::Line { x1:  3.0,  y1: 10.0, x2: 18.0, y2: 10.0 },
    ])
}

/// Plug-flow reactor (PFR): elongated rounded cylinder with flow arrow.
fn reactor_pfr_symbol() -> SymbolDef {
    sym(90.0, 40.0, vec![
        SymbolElement::Rect { x: -42.0, y: -16.0, w: 84.0, h: 32.0, rx: 16.0 },
        SymbolElement::Line { x1: -22.0, y1: 0.0, x2: 14.0, y2: 0.0 },
        SymbolElement::Path { d: "M 11 -6 L 20 0 L 11 6".into() },
    ])
}

/// Centrifugal compressor: large circle with inscribed triangle (same convention as pump).
fn compressor_symbol() -> SymbolDef {
    sym(80.0, 80.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 36.0 },
        SymbolElement::Path { d: "M -20 -26 L 28 0 L -20 26 Z".into() },
    ])
}

/// Blower/fan: circle with impeller arc indicating rotation.
fn blower_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 25.0 },
        SymbolElement::Path { d: "M -8 -20 Q 18 0 -8 20".into() },
    ])
}

/// Mixer/agitated vessel: tank rectangle with shaft and angled mixing arm.
fn mixer_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Rect { x: -25.0, y: -25.0, w: 50.0, h: 50.0, rx: 4.0 },
        SymbolElement::Line { x1:  0.0, y1: -20.0, x2:  0.0, y2: 12.0 },
        SymbolElement::Line { x1: -16.0, y1: 8.0,  x2: 16.0, y2: 8.0  },
        SymbolElement::Line { x1: -16.0, y1: 12.0, x2:  0.0, y2: 8.0  },
        SymbolElement::Line { x1:  16.0, y1: 12.0, x2:  0.0, y2: 8.0  },
    ])
}

/// Distillation column: tall cylinder with alternating left/right tray lines.
fn column_symbol() -> SymbolDef {
    sym(60.0, 160.0, vec![
        SymbolElement::Rect { x: -25.0, y: -75.0, w: 50.0, h: 150.0, rx: 5.0 },
        // Alternating trays (left tray has gap on right, right tray has gap on left)
        SymbolElement::Line { x1: -25.0, y1: -45.0, x2:  8.0, y2: -45.0 },
        SymbolElement::Line { x1:  -8.0, y1: -20.0, x2: 25.0, y2: -20.0 },
        SymbolElement::Line { x1: -25.0, y1:   5.0, x2:  8.0, y2:   5.0 },
        SymbolElement::Line { x1:  -8.0, y1:  30.0, x2: 25.0, y2:  30.0 },
        SymbolElement::Line { x1: -25.0, y1:  55.0, x2:  8.0, y2:  55.0 },
    ])
}

/// Fallback for unrecognised equipment types.
fn default_equipment_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Rect { x: -28.0, y: -28.0, w: 56.0, h: 56.0, rx: 4.0 },
    ])
}

// ---- Valve symbols (ISO 10628-2) ----

/// Manual valve (gate, globe, ball, butterfly, plug): bowtie — two filled triangles
/// meeting tip-to-tip, with the flow axis left-to-right.
fn manual_valve_symbol() -> SymbolDef {
    sym(50.0, 50.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
    ])
}

/// Control valve: bowtie body + vertical actuator stem + circle actuator head.
fn control_valve_symbol() -> SymbolDef {
    sym(50.0, 60.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line   { x1: 0.0, y1: 0.0, x2: 0.0, y2: -26.0 },
        SymbolElement::Circle { cx: 0.0, cy: -33.0, r: 8.0 },
    ])
}

/// Check valve: single right-pointing triangle + vertical stop bar.
fn check_valve_symbol() -> SymbolDef {
    sym(50.0, 50.0, vec![
        SymbolElement::Path {
            d: "M -18 -18 L 18 0 L -18 18 Z".into(),
        },
        SymbolElement::Line { x1: 18.0, y1: -18.0, x2: 18.0, y2: 18.0 },
    ])
}

/// Relief / safety valve: bowtie + stem + spring arch above.
fn relief_valve_symbol() -> SymbolDef {
    sym(50.0, 60.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -22.0 },
        // Spring arch (single quadratic arc)
        SymbolElement::Path { d: "M -10 -22 Q 0 -34 10 -22".into() },
    ])
}

// ---- Instrument bubbles (ISA/IEC 5.1) ----
//
// Bubble style is determined by mounting location:
//   field        — solid circle only (no line)
//   panel        — circle + solid horizontal line (panel front-accessible)
//   control_room — two concentric circles (DCS / behind-panel)

fn instrument_bubble_field() -> SymbolDef {
    sym(36.0, 36.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 18.0 },
    ])
}

fn instrument_bubble_panel() -> SymbolDef {
    sym(36.0, 36.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 18.0 },
        SymbolElement::Line { x1: -18.0, y1: 0.0, x2: 18.0, y2: 0.0 },
    ])
}

fn instrument_bubble_control_room() -> SymbolDef {
    sym(36.0, 36.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 18.0 },
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 13.0 },
    ])
}
