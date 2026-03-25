/// Abstract symbol geometry definition.
/// All coordinates are relative to the symbol center (0,0),
/// sized to fit within [-w/2, w/2] x [-h/2, h/2].

#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub width: f64,
    pub height: f64,
    pub elements: Vec<SymbolElement>,
}

#[derive(Debug, Clone)]
pub enum SymbolElement {
    Rect { x: f64, y: f64, w: f64, h: f64, rx: f64 },
    Circle { cx: f64, cy: f64, r: f64 },
    Path { d: String },
    Line { x1: f64, y1: f64, x2: f64, y2: f64 },
    Polyline { points: Vec<(f64, f64)> },
}

/// Get a symbol definition for an equipment type.
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

/// Get a symbol definition for a valve type.
pub fn valve_symbol(valve_type: &str) -> SymbolDef {
    match valve_type {
        "control_valve" => control_valve_symbol(),
        "check_valve" => check_valve_symbol(),
        "relief_valve" | "safety_valve" => relief_valve_symbol(),
        _ => manual_valve_symbol(),
    }
}

/// Get a symbol definition for an instrument type.
pub fn instrument_symbol(instr_type: &str) -> SymbolDef {
    instrument_bubble_symbol(instr_type)
}

// ---- Equipment symbol implementations ----

fn pump_symbol() -> SymbolDef {
    let r = 25.0;
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Circle { cx: 0.0, cy: 0.0, r },
            // Arrow pointing right (pump direction)
            SymbolElement::Path {
                d: format!(
                    "M {} {} L {} {} L {} {} Z",
                    -r * 0.3, -r * 0.4,
                    r * 0.6, 0.0,
                    -r * 0.3, r * 0.4,
                ),
            },
        ],
    }
}

fn pump_pd_symbol() -> SymbolDef {
    // Positive displacement: circle with two rectangles inside
    let r = 25.0;
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Circle { cx: 0.0, cy: 0.0, r },
            SymbolElement::Rect { x: -15.0, y: -10.0, w: 12.0, h: 20.0, rx: 2.0 },
            SymbolElement::Rect { x: 3.0, y: -10.0, w: 12.0, h: 20.0, rx: 2.0 },
        ],
    }
}

fn heat_exchanger_symbol() -> SymbolDef {
    SymbolDef {
        width: 90.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Rect { x: -45.0, y: -25.0, w: 90.0, h: 50.0, rx: 5.0 },
            // Two internal circles representing tube bundles
            SymbolElement::Circle { cx: -18.0, cy: 0.0, r: 14.0 },
            SymbolElement::Circle { cx: 18.0, cy: 0.0, r: 14.0 },
        ],
    }
}

fn vessel_symbol() -> SymbolDef {
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            // Rectangle body with rounded top
            SymbolElement::Rect { x: -25.0, y: -20.0, w: 50.0, h: 40.0, rx: 0.0 },
            // Elliptical top head
            SymbolElement::Path {
                d: "M -25 -20 Q 0 -35 25 -20".to_string(),
            },
            // Flat bottom
            SymbolElement::Line { x1: -25.0, y1: 20.0, x2: 25.0, y2: 20.0 },
        ],
    }
}

fn separator_symbol() -> SymbolDef {
    // Horizontal vessel
    SymbolDef {
        width: 80.0,
        height: 50.0,
        elements: vec![
            SymbolElement::Rect { x: -35.0, y: -20.0, w: 70.0, h: 40.0, rx: 18.0 },
            // Separation level line
            SymbolElement::Line { x1: -30.0, y1: 0.0, x2: 30.0, y2: 0.0 },
        ],
    }
}

fn reactor_cstr_symbol() -> SymbolDef {
    // CSTR: tank with stirrer
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Rect { x: -25.0, y: -25.0, w: 50.0, h: 50.0, rx: 4.0 },
            // Stirrer shaft
            SymbolElement::Line { x1: 0.0, y1: -25.0, x2: 0.0, y2: 10.0 },
            // Stirrer blades
            SymbolElement::Line { x1: -15.0, y1: 5.0, x2: 15.0, y2: 5.0 },
            SymbolElement::Line { x1: -15.0, y1: 10.0, x2: 15.0, y2: 10.0 },
        ],
    }
}

fn reactor_pfr_symbol() -> SymbolDef {
    // PFR: long horizontal cylinder
    SymbolDef {
        width: 90.0,
        height: 40.0,
        elements: vec![
            SymbolElement::Rect { x: -45.0, y: -18.0, w: 90.0, h: 36.0, rx: 18.0 },
            // Flow direction arrows
            SymbolElement::Line { x1: -20.0, y1: 0.0, x2: 20.0, y2: 0.0 },
            SymbolElement::Path {
                d: "M 14 -5 L 20 0 L 14 5".to_string(),
            },
        ],
    }
}

fn compressor_symbol() -> SymbolDef {
    // Triangle pointing right
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 28.0 },
            SymbolElement::Path {
                d: "M -20 -20 L 20 0 L -20 20 Z".to_string(),
            },
        ],
    }
}

fn blower_symbol() -> SymbolDef {
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Circle { cx: 0.0, cy: 0.0, r: 25.0 },
            SymbolElement::Path {
                d: "M -15 -15 Q 0 0 -15 15 M 0 -20 Q 10 0 0 20".to_string(),
            },
        ],
    }
}

fn mixer_symbol() -> SymbolDef {
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Rect { x: -25.0, y: -25.0, w: 50.0, h: 50.0, rx: 4.0 },
            SymbolElement::Line { x1: 0.0, y1: -20.0, x2: 0.0, y2: 15.0 },
            SymbolElement::Line { x1: -15.0, y1: 10.0, x2: 15.0, y2: 10.0 },
            SymbolElement::Line { x1: -15.0, y1: 15.0, x2: 0.0, y2: 10.0 },
            SymbolElement::Line { x1: 15.0, y1: 15.0, x2: 0.0, y2: 10.0 },
        ],
    }
}

fn column_symbol() -> SymbolDef {
    // Tall vertical cylinder
    SymbolDef {
        width: 60.0,
        height: 120.0,
        elements: vec![
            SymbolElement::Rect { x: -25.0, y: -55.0, w: 50.0, h: 110.0, rx: 5.0 },
            // Trays
            SymbolElement::Line { x1: -25.0, y1: -20.0, x2: 25.0, y2: -20.0 },
            SymbolElement::Line { x1: -25.0, y1: 0.0, x2: 25.0, y2: 0.0 },
            SymbolElement::Line { x1: -25.0, y1: 20.0, x2: 25.0, y2: 20.0 },
        ],
    }
}

fn default_equipment_symbol() -> SymbolDef {
    SymbolDef {
        width: 60.0,
        height: 60.0,
        elements: vec![
            SymbolElement::Rect { x: -28.0, y: -28.0, w: 56.0, h: 56.0, rx: 4.0 },
        ],
    }
}

// ---- Valve symbol implementations ----

fn manual_valve_symbol() -> SymbolDef {
    // Two triangles facing each other (bowtie)
    SymbolDef {
        width: 45.0,
        height: 45.0,
        elements: vec![
            SymbolElement::Path {
                d: "M -20 -15 L 0 0 L -20 15 Z M 20 -15 L 0 0 L 20 15 Z".to_string(),
            },
        ],
    }
}

fn control_valve_symbol() -> SymbolDef {
    // Bowtie + actuator stem + circle on top
    SymbolDef {
        width: 45.0,
        height: 55.0,
        elements: vec![
            // Bowtie body
            SymbolElement::Path {
                d: "M -18 -12 L 0 0 L -18 12 Z M 18 -12 L 0 0 L 18 12 Z".to_string(),
            },
            // Actuator stem
            SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -22.0 },
            // Actuator circle
            SymbolElement::Circle { cx: 0.0, cy: -27.0, r: 6.0 },
        ],
    }
}

fn check_valve_symbol() -> SymbolDef {
    SymbolDef {
        width: 45.0,
        height: 45.0,
        elements: vec![
            // Triangle pointing right + vertical bar
            SymbolElement::Path {
                d: "M -15 -15 L 15 0 L -15 15 Z".to_string(),
            },
            SymbolElement::Line { x1: 15.0, y1: -15.0, x2: 15.0, y2: 15.0 },
        ],
    }
}

fn relief_valve_symbol() -> SymbolDef {
    SymbolDef {
        width: 45.0,
        height: 55.0,
        elements: vec![
            // Bowtie
            SymbolElement::Path {
                d: "M -18 -12 L 0 0 L -18 12 Z M 18 -12 L 0 0 L 18 12 Z".to_string(),
            },
            // Spring symbol above
            SymbolElement::Line { x1: 0.0, y1: 0.0, x2: 0.0, y2: -20.0 },
            SymbolElement::Path {
                d: "M -8 -20 Q 0 -28 8 -20".to_string(),
            },
        ],
    }
}

// ---- Instrument bubble ----

fn instrument_bubble_symbol(instr_type: &str) -> SymbolDef {
    let r = 18.0;
    let line_style = instr_line_style(instr_type);

    let mut elements = vec![
        SymbolElement::Circle { cx: 0.0, cy: 0.0, r },
    ];

    // Add dashes for panel/DCS mounted (line through circle)
    if line_style == "dashed" {
        elements.push(SymbolElement::Line {
            x1: -r, y1: 0.0, x2: r, y2: 0.0,
        });
    }

    SymbolDef {
        width: r * 2.0,
        height: r * 2.0,
        elements,
    }
}

fn instr_line_style(instr_type: &str) -> &'static str {
    match instr_type {
        t if t.ends_with("_controller") => "dashed",
        t if t.ends_with("_transmitter") => "solid",
        _ => "solid",
    }
}
