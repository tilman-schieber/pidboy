use std::collections::HashMap;
use crate::model::*;

pub const GRID_SCALE: f64 = 80.0;
pub const SYMBOL_W: f64 = 80.0;
pub const SYMBOL_H: f64 = 80.0;
pub const VALVE_W: f64 = 55.0;
pub const VALVE_H: f64 = 55.0;

#[derive(Debug, Clone)]
pub struct SvgPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct SvgRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl SvgRect {
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    pub fn intersects_segment(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
        // Expand rect slightly for tolerance
        let margin = 2.0;
        let rx = self.x - margin;
        let ry = self.y - margin;
        let rw = self.w + margin * 2.0;
        let rh = self.h + margin * 2.0;

        // Check if segment passes through rectangle
        // Simple bounding-box check: does the segment's bounding box overlap the rect,
        // and does the segment actually cross the rect edges?
        let seg_min_x = x1.min(x2);
        let seg_max_x = x1.max(x2);
        let seg_min_y = y1.min(y2);
        let seg_max_y = y1.max(y2);

        // Quick reject
        if seg_max_x < rx || seg_min_x > rx + rw || seg_max_y < ry || seg_min_y > ry + rh {
            return false;
        }

        // For orthogonal segments (Manhattan routing), this bounding box check is sufficient
        true
    }
}

#[derive(Debug)]
pub struct LayoutInfo {
    pub positions: HashMap<String, SvgPos>,
    pub bounds: HashMap<String, SvgRect>,
}

impl LayoutInfo {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            bounds: HashMap::new(),
        }
    }

    pub fn get_pos(&self, id: &str) -> Option<&SvgPos> {
        self.positions.get(id)
    }

    pub fn get_bounds(&self, id: &str) -> Option<&SvgRect> {
        self.bounds.get(id)
    }

    /// Get port position for an object.
    pub fn port_pos(&self, id: &str, port_name: &str, ports: &[Port]) -> Option<SvgPos> {
        let center = self.positions.get(id)?;
        let bounds = self.bounds.get(id)?;
        let w = bounds.w;
        let h = bounds.h;

        // Find port with this name
        if let Some(port) = ports.iter().find(|p| p.name == port_name) {
            if let Some(side) = port.side {
                let (dx, dy) = match side {
                    Side::West => (-w / 2.0, 0.0),
                    Side::East => (w / 2.0, 0.0),
                    Side::North => (0.0, -h / 2.0),
                    Side::South => (0.0, h / 2.0),
                };
                return Some(SvgPos { x: center.x + dx, y: center.y + dy });
            }
        }

        // No side defined - infer from port name
        let (dx, dy) = infer_port_offset(port_name, w, h);
        Some(SvgPos { x: center.x + dx, y: center.y + dy })
    }
}

fn infer_port_offset(name: &str, w: f64, h: f64) -> (f64, f64) {
    match name {
        "in" | "inlet" | "west" => (-w / 2.0, 0.0),
        "out" | "outlet" | "east" => (w / 2.0, 0.0),
        "top" | "north" | "vent" => (0.0, -h / 2.0),
        "bottom" | "south" | "drain" => (0.0, h / 2.0),
        _ => (0.0, 0.0),
    }
}

impl Default for LayoutInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_layout(diagram: &Diagram) -> LayoutInfo {
    let mut layout = LayoutInfo::new();
    let mut fallback_idx = 0usize;

    // Process objects in declaration order
    for (kind, id) in &diagram.order {
        let pos = get_explicit_pos(diagram, kind, id);

        let svg_pos = if let Some(gp) = pos {
            SvgPos {
                x: gp.x as f64 * GRID_SCALE,
                y: gp.y as f64 * GRID_SCALE,
            }
        } else {
            // Fallback: arrange in rows of 3, 120px apart
            let col = fallback_idx % 3;
            let row = fallback_idx / 3;
            fallback_idx += 1;
            SvgPos {
                x: 60.0 + col as f64 * 150.0,
                y: 60.0 + row as f64 * 150.0,
            }
        };

        let w = symbol_width(diagram, kind, id);
        let h = symbol_height(diagram, kind, id);

        let bounds = SvgRect {
            x: svg_pos.x - w / 2.0,
            y: svg_pos.y - h / 2.0,
            w,
            h,
        };

        layout.positions.insert(id.clone(), svg_pos);
        layout.bounds.insert(id.clone(), bounds);
    }

    // Second pass: instruments with attach but no pos
    // Track how many instruments have been placed at each (equip_id, port_name) to avoid overlap
    let mut port_placement_count: HashMap<(String, String), usize> = HashMap::new();
    let mut attach_updates: Vec<(String, SvgPos, SvgRect)> = Vec::new();
    for instr in diagram.instruments.values() {
        if instr.pos.is_none() {
            if let Some(attach) = &instr.attach {
                if let Some(attach_pos) = layout.positions.get(&attach.id) {
                    // Place instrument near attached object - above it
                    let attach_bounds = layout.bounds.get(&attach.id);
                    let offset_y = attach_bounds.map(|b| b.h / 2.0 + 40.0).unwrap_or(80.0);

                    // For port-attached instruments, use port position if available
                    let base_pos = if let Some(port_name) = &attach.port {
                        if let Some(ports) = diagram.get_ports(&attach.id) {
                            layout.port_pos(&attach.id, port_name, ports)
                                .map(|p| SvgPos { x: p.x, y: p.y - offset_y })
                                .unwrap_or(SvgPos {
                                    x: attach_pos.x,
                                    y: attach_pos.y - offset_y,
                                })
                        } else {
                            SvgPos {
                                x: attach_pos.x,
                                y: attach_pos.y - offset_y,
                            }
                        }
                    } else {
                        SvgPos {
                            x: attach_pos.x,
                            y: attach_pos.y - offset_y,
                        }
                    };

                    // Determine perpendicular offset for instruments sharing the same port
                    let port_key = (
                        attach.id.clone(),
                        attach.port.clone().unwrap_or_default(),
                    );
                    let count = port_placement_count.entry(port_key).or_insert(0);
                    // Determine port side to choose perpendicular direction
                    let port_side = attach.port.as_deref().and_then(|pn| {
                        diagram.get_ports(&attach.id).and_then(|ports| {
                            ports.iter().find(|p| p.name == pn).and_then(|p| p.side)
                        })
                    }).unwrap_or_else(|| {
                        // Infer from port name
                        match attach.port.as_deref().unwrap_or("") {
                            "top" | "north" | "vent" => Side::North,
                            "bottom" | "south" | "drain" => Side::South,
                            "in" | "inlet" | "west" => Side::West,
                            "out" | "outlet" | "east" => Side::East,
                            _ => Side::North,
                        }
                    });

                    // Perpendicular offset: for north/south ports offset in x; for east/west in y
                    let perp_offset = (*count as f64) * 40.0;
                    let svg_pos = if *count == 0 {
                        base_pos
                    } else {
                        match port_side {
                            Side::North | Side::South => SvgPos {
                                x: base_pos.x + perp_offset,
                                y: base_pos.y,
                            },
                            Side::East | Side::West => SvgPos {
                                x: base_pos.x,
                                y: base_pos.y + perp_offset,
                            },
                        }
                    };
                    *count += 1;

                    let w = SYMBOL_W * 0.75;
                    let h = SYMBOL_H * 0.75;
                    let bounds = SvgRect {
                        x: svg_pos.x - w / 2.0,
                        y: svg_pos.y - h / 2.0,
                        w,
                        h,
                    };
                    attach_updates.push((instr.id.clone(), svg_pos, bounds));
                }
            }
        }
    }

    for (id, pos, bounds) in attach_updates {
        layout.positions.insert(id.clone(), pos);
        layout.bounds.insert(id, bounds);
    }

    layout
}

fn get_explicit_pos<'a>(diagram: &'a Diagram, kind: &crate::ast::DeclKind, id: &str) -> Option<&'a GridPos> {
    use crate::ast::DeclKind;
    match kind {
        DeclKind::Equipment => diagram.equipment.get(id).and_then(|e| e.pos.as_ref()),
        DeclKind::Valve => diagram.valves.get(id).and_then(|v| v.pos.as_ref()),
        DeclKind::Instrument => diagram.instruments.get(id).and_then(|i| i.pos.as_ref()),
        DeclKind::Junction => diagram.junctions.get(id).and_then(|j| j.pos.as_ref()),
        DeclKind::Note => diagram.notes.get(id).and_then(|n| n.pos.as_ref()),
        _ => None,
    }
}

fn symbol_width(diagram: &Diagram, kind: &crate::ast::DeclKind, id: &str) -> f64 {
    use crate::ast::DeclKind;
    match kind {
        DeclKind::Equipment => {
            if let Some(e) = diagram.equipment.get(id) {
                match e.equip_type.as_str() {
                    "heat_exchanger" | "heat_exchanger_shell_tube" => 120.0,
                    "distillation_column" => SYMBOL_W,
                    "tank" | "vessel" => 100.0,
                    "separator" => 80.0,
                    "compressor" => 80.0,
                    _ => SYMBOL_W,
                }
            } else {
                SYMBOL_W
            }
        }
        DeclKind::Valve => VALVE_W,
        DeclKind::Instrument => SYMBOL_W * 0.75,
        DeclKind::Junction => 10.0,
        _ => SYMBOL_W,
    }
}

fn symbol_height(diagram: &Diagram, kind: &crate::ast::DeclKind, id: &str) -> f64 {
    use crate::ast::DeclKind;
    match kind {
        DeclKind::Equipment => {
            if let Some(e) = diagram.equipment.get(id) {
                match e.equip_type.as_str() {
                    "distillation_column" => SYMBOL_H * 2.0,
                    "heat_exchanger" | "heat_exchanger_shell_tube" => 60.0,
                    "tank" | "vessel" => 50.0,
                    "separator" => 50.0,
                    "compressor" => 80.0,
                    _ => SYMBOL_H,
                }
            } else {
                SYMBOL_H
            }
        }
        DeclKind::Valve => VALVE_H,
        DeclKind::Instrument => SYMBOL_H * 0.75,
        DeclKind::Junction => 10.0,
        _ => SYMBOL_H,
    }
}
