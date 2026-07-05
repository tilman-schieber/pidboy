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
    /// Small annotation text baked into the symbol (internals like "Weir"),
    /// centered horizontally on `x` with baseline at `y`.
    Text { x: f64, y: f64, text: String, size: f64 },
    /// Ink-filled circle (globe valve plug, junction dots).
    Dot { cx: f64, cy: f64, r: f64 },
    /// Ink-filled path (solid valve bodies).
    SolidPath { d: String },
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
        "separator_3phase" => "separator_3phase",
        "reactor_cstr" | "reactor_batch" => "reactor_cstr",
        "reactor_pfr" => "reactor_pfr",
        "compressor" => "compressor",
        "blower" => "blower",
        "mixer" => "mixer",
        "distillation_column" => "column",
        "connector" => "connector",
        "heat_pad" => "heat_pad",
        "vacuum_pump" => "vacuum_pump",
        "canister" => "canister",
        "motor" => "motor",
        "thermostat" => "thermostat",
        _ => "equipment_default",
    }
}

/// Returns the canonical symbol key for a valve type + actuator style.
pub fn valve_symbol_key(valve_type: &str, actuator: Option<&str>) -> &'static str {
    if actuator == Some("solenoid") {
        return "valve_solenoid";
    }
    match valve_type {
        "control_valve" => {
            if actuator == Some("diaphragm") {
                "control_valve_diaphragm"
            } else {
                "control_valve"
            }
        }
        "check_valve" => "check_valve",
        "relief_valve" | "safety_valve" => "relief_valve",
        "globe" => "valve_globe",
        "needle" => "valve_needle",
        "three_way" => "valve_three_way",
        "pressure_reducer" => "valve_pcv",
        "bursting_disc" => "bursting_disc",
        _ => "valve_manual",
    }
}

/// Returns the canonical symbol key for an instrument, determined by mounting location.
/// Per ISA/IEC 5.1, bubble style is a function of location, not instrument type.
pub fn instrument_symbol_key(location: Option<&str>) -> &'static str {
    match location.unwrap_or("field") {
        "panel" => "instr_panel",
        "control_room" => "instr_control_room",
        "shared" => "instr_shared",
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
        "separator_3phase" => separator_3phase_symbol(),
        "reactor_cstr" | "reactor_batch" => reactor_cstr_symbol(),
        "reactor_pfr" => reactor_pfr_symbol(),
        "compressor" => compressor_symbol(),
        "blower" => blower_symbol(),
        "mixer" => mixer_symbol(),
        "distillation_column" => column_symbol(),
        "connector" => connector_symbol(),
        "heat_pad" => heat_pad_symbol(),
        "vacuum_pump" => vacuum_pump_symbol(),
        "canister" => canister_symbol(),
        "motor" => motor_symbol(),
        "thermostat" => thermostat_symbol(),
        _ => default_equipment_symbol(),
    }
}

/// Get a symbol definition for a valve type (ISO 10628-2).
pub fn valve_symbol(valve_type: &str, actuator: Option<&str>) -> SymbolDef {
    match valve_symbol_key(valve_type, actuator) {
        "control_valve" => control_valve_symbol(),
        "control_valve_diaphragm" => control_valve_diaphragm_symbol(),
        "check_valve" => check_valve_symbol(),
        "relief_valve" => relief_valve_symbol(),
        "valve_globe" => globe_valve_symbol(),
        "valve_needle" => needle_valve_symbol(),
        "valve_three_way" => three_way_valve_symbol(),
        "valve_pcv" => pcv_valve_symbol(),
        "valve_solenoid" => solenoid_valve_symbol(),
        "bursting_disc" => bursting_disc_symbol(),
        _ => manual_valve_symbol(),
    }
}

/// Local-coordinate offset from the symbol center where an incoming signal
/// line should terminate, for valves whose natural signal target is the
/// actuator head rather than the body.
pub fn valve_signal_anchor(valve_type: &str, actuator: Option<&str>) -> Option<(f64, f64)> {
    match valve_symbol_key(valve_type, actuator) {
        "control_valve" => Some((0.0, -41.0)),           // top of actuator circle
        "control_valve_diaphragm" => Some((0.0, -28.0)), // top of diaphragm dome
        "relief_valve" => Some((0.0, -22.0)),            // top of spring
        _ => None,
    }
}

/// Get a symbol definition for an instrument (ISA/IEC 5.1).
/// Bubble style is determined by `location`; `instr_type` is accepted for API
/// consistency and may be used for type-specific geometry in the future.
pub fn instrument_symbol(_instr_type: &str, location: Option<&str>) -> SymbolDef {
    match location.unwrap_or("field") {
        "panel" => instrument_bubble_panel(),
        "control_room" => instrument_bubble_control_room(),
        "shared" => instrument_bubble_shared(),
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

/// Horizontal three-phase separator: large drum with weir, demister pad and
/// vortex breakers over the liquid outlets. Internals carry small text
/// annotations. Intended port sides: inlet west, psv+gas north (declared in
/// that order), water+oil south (in that order, water upstream of the weir).
fn separator_3phase_symbol() -> SymbolDef {
    // Sized to dominate the sheet like a real separator drawing: declared
    // width covers the full drawn extent including the heads (arcs centered
    // ±165, radius 75 → ±240). Intended ports: `inlet: north, psv: north,
    // vent: north, gas: north` (in that order, gas over the demister) and
    // `water: south, oil: south` (water upstream of the weir).
    let mut elements = vec![
        // Drum outline with elliptical heads
        SymbolElement::Path {
            d: "M -165 -75 A 75 75 0 0 0 -165 75 L 165 75 A 75 75 0 0 0 165 -75 Z".into(),
        },
        // Weir between the water and oil compartments (bottom half)
        SymbolElement::Line { x1: 0.0, y1: 75.0, x2: 0.0, y2: 12.0 },
        SymbolElement::Text { x: 0.0, y: 4.0, text: "Weir".into(), size: 10.0 },
        // Demister pad under the gas nozzle (crosshatched strip)
        SymbolElement::Rect { x: 119.0, y: -73.0, w: 50.0, h: 12.0, rx: 0.0 },
        SymbolElement::Line { x1: 131.5, y1: -73.0, x2: 131.5, y2: -61.0 },
        SymbolElement::Line { x1: 144.0, y1: -73.0, x2: 144.0, y2: -61.0 },
        SymbolElement::Line { x1: 156.5, y1: -73.0, x2: 156.5, y2: -61.0 },
        SymbolElement::Text { x: 144.0, y: -48.0, text: "Demister pad".into(), size: 9.0 },
        SymbolElement::Text { x: -80.0, y: 50.0, text: "Vortex breakers".into(), size: 9.0 },
    ];
    // Vortex breaker tents over the two liquid outlets
    for cx in [-80.0, 80.0] {
        elements.push(SymbolElement::Line { x1: cx - 11.0, y1: 75.0, x2: cx, y2: 62.0 });
        elements.push(SymbolElement::Line { x1: cx, y1: 62.0, x2: cx + 11.0, y2: 75.0 });
    }
    sym(480.0, 150.0, elements)
}

/// CSTR reactor: vessel with shaft and two-level Rushton impeller blades.
fn reactor_cstr_symbol() -> SymbolDef {
    sym(50.0, 70.0, vec![
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

/// Off-page / utility-header connector: pentagon flag pointing east.
/// Used for streams entering or leaving the sheet (CWS, CWR, flare, …).
fn connector_symbol() -> SymbolDef {
    sym(60.0, 30.0, vec![
        SymbolElement::Path {
            d: "M -30 -15 L 15 -15 L 30 0 L 15 15 L -30 15 Z".into(),
        },
    ])
}

/// Symbol for an equipment item with a `size:` override. Returns the def and
/// a uniform scale factor for the renderer to apply.
///
/// Vessels/tanks are drawn parametrically at the exact requested size (a
/// capsule whose heads have radius `h/2`), scale 1.0. Every other type keeps
/// its canonical geometry and is scaled uniformly to fit the requested box.
pub fn equipment_symbol_sized(equip_type: &str, w: f64, h: f64) -> (SymbolDef, f64) {
    match equipment_symbol_key(equip_type) {
        "vessel" => {
            let r = h / 2.0;
            let cap = (w / 2.0 - r).max(0.0);
            let d = format!(
                "M {x0} {yn} A {r} {r} 0 0 0 {x0} {yp} L {x1} {yp} A {r} {r} 0 0 0 {x1} {yn} Z",
                x0 = -cap,
                x1 = cap,
                yn = -r,
                yp = r,
                r = r
            );
            (sym(w, h, vec![SymbolElement::Path { d }]), 1.0)
        }
        _ => {
            let base = equipment_symbol(equip_type);
            let s = (w / base.width).min(h / base.height);
            (base, s)
        }
    }
}

/// Final drawn dimensions for an equipment item with a `size:` override.
pub fn sized_dims(equip_type: &str, w: f64, h: f64) -> (f64, f64) {
    let (def, s) = equipment_symbol_sized(equip_type, w, h);
    (def.width * s, def.height * s)
}

/// Numerically scale a symbol's geometry (coordinates, radii, text sizes)
/// so it can be rendered without an SVG `scale()` transform — keeping
/// stroke widths constant. Arc flags/rotations are preserved.
pub fn scale_symbol(def: &SymbolDef, s: f64) -> SymbolDef {
    let elements = def
        .elements
        .iter()
        .map(|e| match e {
            SymbolElement::Rect { x, y, w, h, rx } => SymbolElement::Rect {
                x: x * s,
                y: y * s,
                w: w * s,
                h: h * s,
                rx: rx * s,
            },
            SymbolElement::Circle { cx, cy, r } => SymbolElement::Circle {
                cx: cx * s,
                cy: cy * s,
                r: r * s,
            },
            SymbolElement::Dot { cx, cy, r } => SymbolElement::Dot {
                cx: cx * s,
                cy: cy * s,
                r: r * s,
            },
            SymbolElement::Line { x1, y1, x2, y2 } => SymbolElement::Line {
                x1: x1 * s,
                y1: y1 * s,
                x2: x2 * s,
                y2: y2 * s,
            },
            SymbolElement::Polyline { points } => SymbolElement::Polyline {
                points: points.iter().map(|(x, y)| (x * s, y * s)).collect(),
            },
            SymbolElement::Text { x, y, text, size } => SymbolElement::Text {
                x: x * s,
                y: y * s,
                text: text.clone(),
                size: size * s,
            },
            SymbolElement::Path { d } => SymbolElement::Path { d: scale_path_data(d, s) },
            SymbolElement::SolidPath { d } => {
                SymbolElement::SolidPath { d: scale_path_data(d, s) }
            }
        })
        .collect();
    SymbolDef {
        width: def.width * s,
        height: def.height * s,
        view_box: (
            def.view_box.0 * s,
            def.view_box.1 * s,
            def.view_box.2 * s,
            def.view_box.3 * s,
        ),
        elements,
    }
}

/// Scale the numbers in an SVG path `d` string. For arc (`A`) commands the
/// radii and endpoint scale but the x-rotation and the two flags do not.
fn scale_path_data(d: &str, s: f64) -> String {
    let mut out = String::new();
    let mut chars = d.chars().peekable();
    let mut cmd = ' ';
    let mut idx = 0usize;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            cmd = c;
            idx = 0;
            out.push(c);
            out.push(' ');
            chars.next();
        } else if c.is_ascii_digit() || c == '-' || c == '.' {
            let mut num = String::new();
            while let Some(&c2) = chars.peek() {
                let starts = num.is_empty();
                if c2.is_ascii_digit()
                    || c2 == '.'
                    || (c2 == '-' && starts)
                {
                    num.push(c2);
                    chars.next();
                } else {
                    break;
                }
            }
            let v: f64 = num.parse().unwrap_or(0.0);
            let keep = cmd.eq_ignore_ascii_case(&'a') && matches!(idx % 7, 2 | 3 | 4);
            let scaled = if keep { v } else { v * s };
            out.push_str(&format!("{:.2} ", scaled));
            idx += 1;
        } else {
            chars.next();
        }
    }
    out.trim_end().to_string()
}

/// Simplified stand-ins for the legend: composite symbols whose internals
/// (weir, demister, annotations) would be illegible at thumbnail scale show
/// just their outline; the internals get their own legend glyphs.
pub fn legend_symbol(key: &str) -> Option<SymbolDef> {
    match key {
        "separator_3phase" => Some(sym(480.0, 150.0, vec![SymbolElement::Path {
            d: "M -165 -75 A 75 75 0 0 0 -165 75 L 165 75 A 75 75 0 0 0 165 -75 Z".into(),
        }])),
        _ => None,
    }
}

/// Legend glyph for the demister pad internal (crosshatched strip).
pub fn demister_glyph() -> SymbolDef {
    sym(50.0, 12.0, vec![
        SymbolElement::Rect { x: -25.0, y: -6.0, w: 50.0, h: 12.0, rx: 0.0 },
        SymbolElement::Line { x1: -12.5, y1: -6.0, x2: -12.5, y2: 6.0 },
        SymbolElement::Line { x1: 0.0, y1: -6.0, x2: 0.0, y2: 6.0 },
        SymbolElement::Line { x1: 12.5, y1: -6.0, x2: 12.5, y2: 6.0 },
    ])
}

/// Electric heat pad / tracing panel: thin strip with a resistive
/// serpentine element. Mount flush against a vessel with `attach:`.
fn heat_pad_symbol() -> SymbolDef {
    let mut elements = vec![SymbolElement::Rect {
        x: -180.0,
        y: -14.0,
        w: 360.0,
        h: 28.0,
        rx: 3.0,
    }];
    // Serpentine heating element
    let mut points = Vec::new();
    let n = 12;
    for i in 0..=n {
        let x = -150.0 + 300.0 * (i as f64) / (n as f64);
        let y = if i % 2 == 0 { -6.0 } else { 6.0 };
        points.push((x, y));
    }
    elements.push(SymbolElement::Polyline { points });
    sym(360.0, 28.0, elements)
}

/// Vacuum pump: circle with two curved vanes.
fn vacuum_pump_symbol() -> SymbolDef {
    sym(60.0, 60.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 25.0 },
        SymbolElement::Path { d: "M -14 -10 Q 2 0 -14 10".into() },
        SymbolElement::Path { d: "M 14 -10 Q -2 0 14 10".into() },
    ])
}

/// Feed canister / bottle.
fn canister_symbol() -> SymbolDef {
    sym(40.0, 56.0, vec![
        SymbolElement::Rect { x: -18.0, y: -18.0, w: 36.0, h: 46.0, rx: 4.0 },
        SymbolElement::Rect { x: -7.0, y: -28.0, w: 14.0, h: 10.0, rx: 2.0 },
    ])
}

/// Motor / stirrer drive: circle with an M. Mount on a reactor with
/// `attach:`.
fn motor_symbol() -> SymbolDef {
    sym(28.0, 28.0, vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 14.0 },
        SymbolElement::Text { x: 0.0, y: 4.0, text: "M".into(), size: 11.0 },
    ])
}

/// Thermostat / packaged unit: plain box (label goes inside when it fits).
fn thermostat_symbol() -> SymbolDef {
    sym(150.0, 60.0, vec![
        SymbolElement::Rect { x: -75.0, y: -30.0, w: 150.0, h: 60.0, rx: 2.0 },
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
    sym(44.0, 36.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
    ])
}

/// Control valve: bowtie body + vertical actuator stem + circle actuator head.
fn control_valve_symbol() -> SymbolDef {
    sym(44.0, 60.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line   { x1: 0.0, y1: 0.0, x2: 0.0, y2: -26.0 },
        SymbolElement::Circle { cx: 0.0, cy: -33.0, r: 8.0 },
    ])
}

/// Control valve with diaphragm actuator: solid bowtie body + stem + dome
/// (the filled body matches common vendor P&ID style for automatic valves).
fn control_valve_diaphragm_symbol() -> SymbolDef {
    sym(44.0, 60.0, vec![
        SymbolElement::SolidPath {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -18.0 },
        // Diaphragm dome (half-ellipse closed by its chord)
        SymbolElement::Path { d: "M -16 -18 A 16 10 0 0 1 16 -18 Z".into() },
    ])
}

/// Globe valve: solid bowtie (bypass/throttling valve style).
fn globe_valve_symbol() -> SymbolDef {
    sym(44.0, 36.0, vec![
        SymbolElement::SolidPath {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
    ])
}

/// Needle valve: bowtie with a long thin needle stem and cap.
fn needle_valve_symbol() -> SymbolDef {
    sym(44.0, 36.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line { x1: 0.0, y1: -2.0, x2: 0.0, y2: -16.0 },
        SymbolElement::Line { x1: -6.0, y1: -16.0, x2: 6.0, y2: -16.0 },
    ])
}

/// Three-way valve: three open triangles meeting at the seat. Declare
/// ports on the three sides in play, e.g. `in: east, out: west,
/// branch: south`.
fn three_way_valve_symbol() -> SymbolDef {
    sym(44.0, 40.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z M -12 20 L 0 0 L 12 20 Z".into(),
        },
    ])
}

/// Self-actuated pressure reducer (PCV): bowtie with the downstream
/// triangle filled.
fn pcv_valve_symbol() -> SymbolDef {
    sym(44.0, 36.0, vec![
        SymbolElement::Path { d: "M -22 -18 L 0 0 L -22 18 Z".into() },
        SymbolElement::SolidPath { d: "M 22 -18 L 0 0 L 22 18 Z".into() },
    ])
}

/// Solenoid valve: bowtie with a boxed S on the stem.
fn solenoid_valve_symbol() -> SymbolDef {
    sym(44.0, 60.0, vec![
        SymbolElement::Path {
            d: "M -22 -18 L 0 0 L -22 18 Z M 22 -18 L 0 0 L 22 18 Z".into(),
        },
        SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -14.0 },
        SymbolElement::Rect { x: -8.0, y: -30.0, w: 16.0, h: 16.0, rx: 0.0 },
        SymbolElement::Text { x: 0.0, y: -18.5, text: "S".into(), size: 10.0 },
    ])
}

/// Bursting (rupture) disc in its holder: two flange bars with the disc
/// bulging toward the upstream side.
fn bursting_disc_symbol() -> SymbolDef {
    sym(20.0, 28.0, vec![
        SymbolElement::Line { x1: -10.0, y1: -14.0, x2: -10.0, y2: 14.0 },
        SymbolElement::Line { x1: 10.0, y1: -14.0, x2: 10.0, y2: 14.0 },
        SymbolElement::Path { d: "M -10 10 Q 2 0 -10 -10".into() },
    ])
}

/// Check valve: single right-pointing triangle + vertical stop bar.
fn check_valve_symbol() -> SymbolDef {
    sym(36.0, 36.0, vec![
        SymbolElement::Path {
            d: "M -18 -18 L 18 0 L -18 18 Z".into(),
        },
        SymbolElement::Line { x1: 18.0, y1: -18.0, x2: 18.0, y2: 18.0 },
    ])
}

/// Relief / safety valve, angle pattern: inlet from below, outlet to the
/// side, spring on top. Declare `ports: in: south, out: east`.
fn relief_valve_symbol() -> SymbolDef {
    sym(44.0, 44.0, vec![
        // Inlet triangle (base at the bottom nozzle, apex at the seat)
        SymbolElement::Path { d: "M -12 22 L 0 0 L 12 22 Z".into() },
        // Outlet triangle (base at the side nozzle)
        SymbolElement::Path { d: "M 22 -12 L 0 0 L 22 12 Z".into() },
        // Spring above the seat
        SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -10.0 },
        SymbolElement::Path { d: "M -8 -10 Q 0 -22 8 -10".into() },
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

/// Shared display / shared control (DCS function): circle inscribed in a square.
fn instrument_bubble_shared() -> SymbolDef {
    sym(36.0, 36.0, vec![
        SymbolElement::Rect { x: -18.0, y: -18.0, w: 36.0, h: 36.0, rx: 0.0 },
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 18.0 },
    ])
}
