use std::collections::HashMap;
use crate::model::*;
use crate::symbols;

pub const GRID_SCALE: f64 = 80.0;
pub const SYMBOL_W: f64 = 80.0;
pub const SYMBOL_H: f64 = 80.0;
pub const VALVE_W: f64 = 55.0;
pub const VALVE_H: f64 = 55.0;

/// Edge-to-edge spacing used by automatic placement.
const H_GAP: f64 = 120.0;
const V_GAP: f64 = 110.0;
/// Minimum distance from canvas origin after normalisation.
const MARGIN: f64 = 60.0;

#[derive(Debug, Clone, Copy)]
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

    pub fn intersects_rect(&self, other: &SvgRect, margin: f64) -> bool {
        self.x - margin < other.x + other.w
            && other.x < self.x + self.w + margin
            && self.y - margin < other.y + other.h
            && other.y < self.y + self.h + margin
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

/// Outline family of a symbol, for placing ports on the drawn surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolShape {
    /// Ports sit on the bounding box (default).
    Rect,
    /// Stadium/capsule outline with head radius h/2: side ports follow the
    /// head arc, top/bottom ports near the ends drop onto the shoulder.
    Capsule,
}

#[derive(Debug)]
pub struct LayoutInfo {
    pub positions: HashMap<String, SvgPos>,
    pub bounds: HashMap<String, SvgRect>,
    pub shapes: HashMap<String, SymbolShape>,
    /// Uniform shift applied by `normalize_origin`; lets callers map SVG
    /// coordinates back to grid coordinates (`(svg - shift) / GRID_SCALE`).
    pub origin_shift: (f64, f64),
}

impl LayoutInfo {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            bounds: HashMap::new(),
            shapes: HashMap::new(),
            origin_shift: (0.0, 0.0),
        }
    }

    pub fn get_pos(&self, id: &str) -> Option<&SvgPos> {
        self.positions.get(id)
    }

    pub fn get_bounds(&self, id: &str) -> Option<&SvgRect> {
        self.bounds.get(id)
    }

    /// Get port position for an object. Ports sharing a side are distributed
    /// evenly along it in declaration order, so e.g. a vessel can have both a
    /// gas outlet and a relief nozzle on top without them coinciding.
    pub fn port_pos(&self, id: &str, port_name: &str, ports: &[Port]) -> Option<SvgPos> {
        let center = self.positions.get(id)?;
        let bounds = self.bounds.get(id)?;
        let shape = self.shapes.get(id).copied().unwrap_or(SymbolShape::Rect);
        match port_offset(ports, port_name, bounds.w, bounds.h, shape) {
            Some((dx, dy)) => Some(SvgPos { x: center.x + dx, y: center.y + dy }),
            None => Some(SvgPos { x: center.x, y: center.y }),
        }
    }
}

/// Local offset of a named port from the symbol center, given the symbol's
/// dimensions. Ports sharing a side are distributed evenly along it in
/// declaration order (1/(n+1), 2/(n+1), …), so e.g. a vessel can have both a
/// gas outlet and a relief nozzle on top without them coinciding.
pub fn port_offset(
    ports: &[Port],
    port_name: &str,
    w: f64,
    h: f64,
    shape: SymbolShape,
) -> Option<(f64, f64)> {
    let resolved = |p: &Port| p.side.or_else(|| infer_port_side(&p.name));

    let side = ports
        .iter()
        .find(|p| p.name == port_name)
        .and_then(resolved)
        .or_else(|| infer_port_side(port_name))?;

    let same_side: Vec<&Port> = ports
        .iter()
        .filter(|p| resolved(p) == Some(side))
        .collect();
    let idx = same_side.iter().position(|p| p.name == port_name);
    let frac = match (idx, same_side.len()) {
        (Some(i), n) if n > 0 => (i as f64 + 1.0) / (n as f64 + 1.0),
        _ => 0.5,
    };

    let rect = match side {
        Side::West => (-w / 2.0, (frac - 0.5) * h),
        Side::East => (w / 2.0, (frac - 0.5) * h),
        Side::North => ((frac - 0.5) * w, -h / 2.0),
        Side::South => ((frac - 0.5) * w, h / 2.0),
    };

    Some(match shape {
        SymbolShape::Rect => rect,
        SymbolShape::Capsule => capsule_surface(side, rect, w, h),
    })
}

/// Pull a bounding-box port position onto a capsule outline (stadium with
/// head radius h/2, straight section between ±(w/2 − r)), so lines meet the
/// drawn shell instead of stopping at the bounding box.
fn capsule_surface(side: Side, (dx, dy): (f64, f64), w: f64, h: f64) -> (f64, f64) {
    let r = h / 2.0;
    let c = (w / 2.0 - r).max(0.0);
    match side {
        Side::East | Side::West => {
            let x = c + (r * r - dy * dy).max(0.0).sqrt();
            (x * dx.signum(), dy)
        }
        Side::North | Side::South => {
            if dx.abs() <= c {
                (dx, dy)
            } else {
                let e = dx.abs() - c;
                let y = (r * r - e * e).max(0.0).sqrt();
                (dx, y * dy.signum())
            }
        }
    }
}

/// Map object id → index of the framed group it belongs to (first framed
/// group listing it wins). Framed groups act as layout clusters.
pub fn framed_group_of(diagram: &Diagram) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (gi, g) in diagram.groups.values().filter(|g| g.frame).enumerate() {
        for m in &g.members {
            map.entry(m.clone()).or_insert(gi);
        }
    }
    map
}

/// A framed group laid out as an independent sub-diagram: local member
/// positions/bounds plus the overall extent, ready to be dropped into the
/// global layout as one super-node.
struct ModuleLayout {
    local: HashMap<String, (SvgPos, SvgRect)>,
    extent: SvgRect,
}

/// Sub-diagram of a framed module: its members plus the connections fully
/// inside it. Members whose placement depends on the outside world
/// (instruments attached elsewhere, equipment mounted elsewhere) are left
/// to the global passes. Explicit `at:` on members is ignored — the module
/// is positioned as a whole.
fn make_subdiagram(diagram: &Diagram, members: &[String]) -> Diagram {
    use crate::ast::DeclKind;
    let mset: std::collections::HashSet<&str> = members.iter().map(|s| s.as_str()).collect();
    let mut included: std::collections::HashSet<String> = std::collections::HashSet::new();
    for m in members {
        let ok = if let Some(e) = diagram.equipment.get(m) {
            e.attach
                .as_ref()
                .map(|a| mset.contains(a.id.as_str()))
                .unwrap_or(true)
        } else if let Some(i) = diagram.instruments.get(m) {
            match &i.attach {
                Some(a) => mset.contains(a.id.as_str()),
                None => diagram.signals.values().any(|sg| {
                    (sg.from.id == *m && mset.contains(sg.to.id.as_str()))
                        || (sg.to.id == *m && mset.contains(sg.from.id.as_str()))
                }),
            }
        } else {
            diagram.valves.contains_key(m) || diagram.junctions.contains_key(m)
        };
        if ok {
            included.insert(m.clone());
        }
    }

    let mut sub = Diagram::new();
    for (kind, id) in &diagram.order {
        match kind {
            DeclKind::Equipment => {
                if included.contains(id) {
                    let mut e = diagram.equipment[id.as_str()].clone();
                    e.pos = None;
                    sub.equipment.insert(id.clone(), e);
                    sub.order.push((*kind, id.clone()));
                }
            }
            DeclKind::Valve => {
                if included.contains(id) {
                    let mut v = diagram.valves[id.as_str()].clone();
                    v.pos = None;
                    sub.valves.insert(id.clone(), v);
                    sub.order.push((*kind, id.clone()));
                }
            }
            DeclKind::Junction => {
                if included.contains(id) {
                    let mut j = diagram.junctions[id.as_str()].clone();
                    j.pos = None;
                    sub.junctions.insert(id.clone(), j);
                    sub.order.push((*kind, id.clone()));
                }
            }
            DeclKind::Instrument => {
                if included.contains(id) {
                    let mut i = diagram.instruments[id.as_str()].clone();
                    i.pos = None;
                    sub.instruments.insert(id.clone(), i);
                    sub.order.push((*kind, id.clone()));
                }
            }
            DeclKind::Line => {
                let l = &diagram.lines[id.as_str()];
                let from_in = included.contains(&l.from.id);
                let to_in = l
                    .to
                    .as_ref()
                    .map(|t| included.contains(&t.id))
                    .unwrap_or(true);
                if from_in && to_in {
                    sub.lines.insert(id.clone(), l.clone());
                    sub.order.push((*kind, id.clone()));
                }
            }
            DeclKind::Signal => {
                let sg = &diagram.signals[id.as_str()];
                if included.contains(&sg.from.id) && included.contains(&sg.to.id) {
                    sub.signals.insert(id.clone(), sg.clone());
                    sub.order.push((*kind, id.clone()));
                }
            }
            _ => {}
        }
    }
    sub
}

fn build_modules(diagram: &Diagram) -> Vec<ModuleLayout> {
    diagram
        .groups
        .values()
        .filter(|g| g.frame)
        .map(|g| {
            let sub = make_subdiagram(diagram, &g.members);
            let sub_layout = compute_layout(&sub);
            let mut local = HashMap::new();
            let mut ext: Option<SvgRect> = None;
            for m in &g.members {
                if let (Some(p), Some(b)) =
                    (sub_layout.positions.get(m), sub_layout.bounds.get(m))
                {
                    ext = Some(match ext {
                        None => b.clone(),
                        Some(e) => {
                            let x = e.x.min(b.x);
                            let y = e.y.min(b.y);
                            let x2 = (e.x + e.w).max(b.x + b.w);
                            let y2 = (e.y + e.h).max(b.y + b.h);
                            SvgRect { x, y, w: x2 - x, h: y2 - y }
                        }
                    });
                    local.insert(m.clone(), (*p, b.clone()));
                }
            }
            ModuleLayout {
                local,
                extent: ext.unwrap_or(SvgRect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 }),
            }
        })
        .collect()
}

/// Drop a module into the global layout at the given offset.
fn place_module(layout: &mut LayoutInfo, module: &ModuleLayout, dx: f64, dy: f64) {
    for (id, (p, b)) in &module.local {
        layout.positions.insert(id.clone(), SvgPos { x: p.x + dx, y: p.y + dy });
        layout.bounds.insert(
            id.clone(),
            SvgRect { x: b.x + dx, y: b.y + dy, w: b.w, h: b.h },
        );
    }
}

/// Whether the module's extent (shifted by dx,dy) collides with anything
/// already placed.
fn module_collides(layout: &LayoutInfo, module: &ModuleLayout, dx: f64, dy: f64) -> bool {
    let rect = SvgRect {
        x: module.extent.x + dx,
        y: module.extent.y + dy,
        w: module.extent.w,
        h: module.extent.h,
    };
    collides(layout, &rect, 30.0)
}

/// Shape family of an object's drawn outline.
pub fn symbol_shape(diagram: &Diagram, id: &str) -> SymbolShape {
    if let Some(e) = diagram.equipment.get(id) {
        let key = symbols::equipment_symbol_key(&e.equip_type);
        // The parametric sized vessel and the big separator drum draw exact
        // capsules; the small fixed symbols overdraw their bounding box, so
        // rectangle ports already land under the fill.
        if (key == "vessel" && e.size.is_some()) || key == "separator_3phase" {
            return SymbolShape::Capsule;
        }
    }
    SymbolShape::Rect
}

/// True when every sided port of the valve lies on north/south — it sits in
/// a vertical run and its symbol is drawn rotated 90°.
pub fn valve_is_vertical(v: &Valve) -> bool {
    let sides: Vec<Side> = v
        .ports
        .iter()
        .filter_map(|p| p.side.or_else(|| infer_port_side(&p.name)))
        .collect();
    !sides.is_empty() && sides.iter().all(|s| matches!(s, Side::North | Side::South))
}

/// Infer which side of a symbol a port sits on from its conventional name.
pub fn infer_port_side(name: &str) -> Option<Side> {
    match name {
        "in" | "inlet" | "west" => Some(Side::West),
        "out" | "outlet" | "east" => Some(Side::East),
        "top" | "north" | "vent" => Some(Side::North),
        "bottom" | "south" | "drain" => Some(Side::South),
        _ => None,
    }
}

impl Default for LayoutInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_layout(diagram: &Diagram) -> LayoutInfo {
    let mut layout = LayoutInfo::new();

    // Real symbol dimensions per object, so ports and line endpoints land on
    // the drawn geometry instead of a nominal bounding box.
    let dims: HashMap<String, (f64, f64)> = diagram
        .order
        .iter()
        .map(|(kind, id)| (id.clone(), symbol_dims(diagram, kind, id)))
        .collect();
    for (_, id) in &diagram.order {
        layout.shapes.insert(id.clone(), symbol_shape(diagram, id));
    }

    // Framed groups are laid out first as independent sub-diagrams; the
    // global pass then treats each as a super-node, positioning the whole
    // module so a member port lines up with whatever it connects to.
    let cluster = framed_group_of(diagram);
    let modules = build_modules(diagram);

    // Pass 1: explicit grid positions (module members are positioned by
    // their module, so `at:` on them is ignored).
    for (kind, id) in &diagram.order {
        if cluster.contains_key(id) {
            continue;
        }
        if let Some(gp) = get_explicit_pos(diagram, kind, id) {
            let pos = SvgPos {
                x: gp.x as f64 * GRID_SCALE,
                y: gp.y as f64 * GRID_SCALE,
            };
            place(&mut layout, id, pos, dim_of(&dims, id));
        }
    }

    // Pass 2: propagate placement along process lines.
    place_line_endpoints(diagram, &mut layout, &dims, &modules, &cluster);

    // Pass 2.5: equipment mounted flush on other equipment (heat pads,
    // jackets), before instruments so bubbles avoid their bounds.
    place_attached_equipment(diagram, &mut layout, &dims);

    // Pass 3: instruments attached to equipment.
    place_attached_instruments(diagram, &mut layout, &dims);

    // Pass 4: instruments placed relative to their signal peers.
    place_signal_instruments(diagram, &mut layout, &dims);

    // Pass 5: anything still unplaced goes in a row below the diagram.
    place_leftovers(diagram, &mut layout, &dims);

    // Pass 6: shift everything so the drawing starts inside the canvas margin.
    normalize_origin(&mut layout);

    layout
}

fn place(layout: &mut LayoutInfo, id: &str, pos: SvgPos, (w, h): (f64, f64)) {
    layout.bounds.insert(
        id.to_string(),
        SvgRect {
            x: pos.x - w / 2.0,
            y: pos.y - h / 2.0,
            w,
            h,
        },
    );
    layout.positions.insert(id.to_string(), pos);
}

fn dim_of(dims: &HashMap<String, (f64, f64)>, id: &str) -> (f64, f64) {
    dims.get(id).copied().unwrap_or((SYMBOL_W, SYMBOL_H))
}

fn unit(side: Side) -> (f64, f64) {
    match side {
        Side::East => (1.0, 0.0),
        Side::West => (-1.0, 0.0),
        Side::North => (0.0, -1.0),
        Side::South => (0.0, 1.0),
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::East => Side::West,
        Side::West => Side::East,
        Side::North => Side::South,
        Side::South => Side::North,
    }
}

fn is_vertical_side(side: Side) -> bool {
    matches!(side, Side::North | Side::South)
}

/// The side of `ep`'s object that the connection leaves/enters through.
fn endpoint_side(diagram: &Diagram, ep: &ObjRef, is_from: bool) -> Side {
    if let Some(pn) = &ep.port {
        if let Some(ports) = diagram.get_ports(&ep.id) {
            if let Some(p) = ports.iter().find(|p| p.name == *pn) {
                if let Some(s) = p.side {
                    return s;
                }
            }
        }
        if let Some(s) = infer_port_side(pn) {
            return s;
        }
    }
    // Default flow direction: out of the east side, into the west side.
    if is_from {
        Side::East
    } else {
        Side::West
    }
}

fn collides(layout: &LayoutInfo, rect: &SvgRect, margin: f64) -> bool {
    layout.bounds.values().any(|b| b.intersects_rect(rect, margin))
}

/// Place `new_id` adjacent to the already-placed `anchor_id`, offset toward
/// `side`, nudging further along that direction until it doesn't overlap
/// anything already placed.
fn place_adjacent(
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
    anchor_id: &str,
    new_id: &str,
    side: Side,
) {
    try_place_adjacent(layout, dims, anchor_id, new_id, side, 100, true, &[]);
}

/// Like [`place_adjacent`], but refuses (returns false, placing nothing) if
/// no free spot exists within `max_nudges` steps — unless `force` is set,
/// in which case the last attempted spot is used.
fn try_place_adjacent(
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
    anchor_id: &str,
    new_id: &str,
    side: Side,
    max_nudges: usize,
    force: bool,
    avoid: &[SvgRect],
) -> bool {
    let anchor_pos = match layout.positions.get(anchor_id) {
        Some(p) => *p,
        None => return false,
    };
    let (aw, ah) = layout
        .bounds
        .get(anchor_id)
        .map(|b| (b.w, b.h))
        .unwrap_or((SYMBOL_W, SYMBOL_H));
    // Start from the anchor's boundary toward `side`.
    let (ux, uy) = unit(side);
    let half = match side {
        Side::East | Side::West => aw / 2.0,
        Side::North | Side::South => ah / 2.0,
    };
    let edge = SvgPos {
        x: anchor_pos.x + ux * half,
        y: anchor_pos.y + uy * half,
    };
    place_from_point(layout, dims, edge, new_id, side, max_nudges, force, avoid, None)
}

/// Place `new_id` one gap away from `pt` (a symbol boundary or port point)
/// toward `side`, nudging along that direction until free.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn place_from_point(
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
    pt: SvgPos,
    new_id: &str,
    side: Side,
    max_nudges: usize,
    force: bool,
    avoid: &[SvgRect],
    // Local offset of the target's connecting port from its center; when
    // known, the target is shifted so that port (not its centerline) lines
    // up with `pt`. Defaults to a centered port on the facing side.
    port_local: Option<(f64, f64)>,
) -> bool {
    let (nw, nh) = dim_of(dims, new_id);
    let (ux, uy) = unit(side);
    let (plx, ply) = port_local.unwrap_or((-ux * nw / 2.0, -uy * nh / 2.0));
    let gap = match side {
        Side::East | Side::West => H_GAP,
        Side::North | Side::South => V_GAP,
    };
    let mut pos = SvgPos {
        x: pt.x + ux * gap - plx,
        y: pt.y + uy * gap - ply,
    };
    for _ in 0..=max_nudges {
        let rect = SvgRect {
            x: pos.x - nw / 2.0,
            y: pos.y - nh / 2.0,
            w: nw,
            h: nh,
        };
        if !collides(layout, &rect, 30.0)
            && !avoid.iter().any(|c| c.intersects_rect(&rect, 0.0))
        {
            place(layout, new_id, pos, (nw, nh));
            return true;
        }
        pos.x += ux * 40.0;
        pos.y += uy * 40.0;
    }
    if force {
        place(layout, new_id, pos, (nw, nh));
        return true;
    }
    false
}

/// Place `new_id` around a pipe corner: the run leaves `pt` toward `along`,
/// turns, and enters the target through its `entry` side. The target sits
/// one gap along `along` and one approach-run away on the perpendicular.
fn place_corner(
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
    pt: SvgPos,
    new_id: &str,
    along: Side,
    entry: Side,
) {
    let (nw, nh) = dim_of(dims, new_id);
    let (ax, ay) = unit(along);
    let approach = opposite(entry);
    let (tx, ty) = unit(approach);
    let rise = if is_vertical_side(along) { V_GAP } else { H_GAP };
    let run = if is_vertical_side(approach) {
        V_GAP + nh / 2.0
    } else {
        H_GAP + nw / 2.0
    };
    let mut pos = SvgPos {
        x: pt.x + ax * rise + tx * run,
        y: pt.y + ay * rise + ty * run,
    };
    for _ in 0..100 {
        let rect = SvgRect {
            x: pos.x - nw / 2.0,
            y: pos.y - nh / 2.0,
            w: nw,
            h: nh,
        };
        if !collides(layout, &rect, 30.0) {
            break;
        }
        pos.x += ax * 40.0;
        pos.y += ay * 40.0;
    }
    place(layout, new_id, pos, (nw, nh));
}

fn content_bottom(layout: &LayoutInfo) -> f64 {
    layout
        .bounds
        .values()
        .map(|b| b.y + b.h)
        .fold(0.0, f64::max)
}

/// BFS-style placement along declared lines: any line with exactly one placed
/// endpoint pulls its other endpoint next to it, on the side implied by the
/// placed endpoint's port. Fully unplaced components get seeded below the
/// existing content and grow from there.
fn place_line_endpoints(
    diagram: &Diagram,
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
    modules: &[ModuleLayout],
    cluster: &HashMap<String, usize>,
) {
    // Lines fully inside one module were consumed by its sub-layout; the
    // global sweep only walks cross-module and free connections.
    let ordered: Vec<&Line> = diagram
        .lines
        .values()
        .filter(|l| {
            let a = cluster.get(l.from.id.as_str());
            let b = l.to.as_ref().and_then(|t| cluster.get(t.id.as_str()));
            !matches!((a, b), (Some(x), Some(y)) if x == y)
        })
        .collect();
    loop {
        let mut progress = false;
        for line in &ordered {
            // Open-ended stubs have no second object to place.
            let Some(line_to) = &line.to else { continue };
            let from_placed = layout.positions.contains_key(&line.from.id);
            let to_placed = layout.positions.contains_key(&line_to.id);
            if from_placed == to_placed {
                continue;
            }
            let (anchor, new) = if from_placed {
                (&line.from, line_to)
            } else {
                (line_to, &line.from)
            };
            let side = endpoint_side(diagram, anchor, from_placed);
            // Anchor on the port the line connects to, so the new object
            // lines up with its nozzle instead of the anchor's centerline.
            let port_pt = anchor.port.as_ref().and_then(|pn| {
                diagram
                    .get_ports(&anchor.id)
                    .and_then(|ports| layout.port_pos(&anchor.id, pn, ports))
            });

            // The unplaced side belongs to a module: position the whole
            // module so that this member's port meets the anchor.
            if let Some(&mi) = cluster.get(new.id.as_str()) {
                let module = &modules[mi];
                if let Some((lp, lb)) = module.local.get(&new.id) {
                    let anchor_pt = port_pt.unwrap_or_else(|| {
                        let apos = layout.positions[&anchor.id];
                        let ab = &layout.bounds[&anchor.id];
                        let (ux, uy) = unit(side);
                        SvgPos {
                            x: apos.x + ux * ab.w / 2.0,
                            y: apos.y + uy * ab.h / 2.0,
                        }
                    });
                    let (ux, uy) = unit(side);
                    let gap = if is_vertical_side(side) { V_GAP } else { H_GAP };
                    // Member port in module-local coordinates
                    let (plx, ply) = new
                        .port
                        .as_ref()
                        .and_then(|pn| {
                            diagram.get_ports(&new.id).and_then(|ports| {
                                port_offset(
                                    ports,
                                    pn,
                                    lb.w,
                                    lb.h,
                                    symbol_shape(diagram, &new.id),
                                )
                            })
                        })
                        .unwrap_or((-ux * lb.w / 2.0, -uy * lb.h / 2.0));
                    let target_x = anchor_pt.x + ux * gap;
                    let target_y = anchor_pt.y + uy * gap;
                    let mut dx = target_x - (lp.x + plx);
                    let mut dy = target_y - (lp.y + ply);
                    // Keep the module off other objects AND off the pipe
                    // corridors of already-placed ports, so it doesn't wall
                    // in a nozzle that still needs its riser.
                    let corridors: Vec<SvgRect> = diagram
                        .order
                        .iter()
                        .filter(|(_, oid)| !cluster.get(oid.as_str()).map_or(false, |&c| c == mi))
                        .flat_map(|(_, oid)| used_port_corridors(diagram, layout, oid))
                        .collect();
                    for _ in 0..100 {
                        let rect = SvgRect {
                            x: module.extent.x + dx,
                            y: module.extent.y + dy,
                            w: module.extent.w,
                            h: module.extent.h,
                        };
                        if !module_collides(layout, module, dx, dy)
                            && !corridors.iter().any(|c| c.intersects_rect(&rect, 0.0))
                        {
                            break;
                        }
                        dx += ux * 40.0;
                        dy += uy * 40.0;
                    }
                    place_module(layout, module, dx, dy);
                    progress = true;
                    continue;
                }
            }
            // The target's own entry side, when explicitly resolvable.
            let target_side = new.port.as_ref().and_then(|pn| {
                diagram
                    .get_ports(&new.id)
                    .and_then(|ports| {
                        ports.iter().find(|p| p.name == *pn).and_then(|p| p.side)
                    })
                    .or_else(|| infer_port_side(pn))
            });
            match (port_pt, target_side) {
                // Ports on orthogonal axes: the pipe turns a corner. Place
                // the target diagonally so the elbow lands in the run, not
                // inside a symbol.
                (Some(pt), Some(tside))
                    if is_vertical_side(tside) != is_vertical_side(side) =>
                {
                    place_corner(layout, dims, pt, &new.id, side, tside);
                }
                (Some(pt), _) => {
                    let port_local = new.port.as_ref().and_then(|pn| {
                        let (w, h) = dim_of(dims, &new.id);
                        let shape = symbol_shape(diagram, &new.id);
                        diagram
                            .get_ports(&new.id)
                            .and_then(|ports| port_offset(ports, pn, w, h, shape))
                    });
                    place_from_point(layout, dims, pt, &new.id, side, 100, true, &[], port_local);
                }
                (None, _) => place_adjacent(layout, dims, &anchor.id, &new.id, side),
            }
            progress = true;
        }
        if progress {
            continue;
        }
        // No half-placed line left; seed the next unplaced component (if any).
        let seed = ordered
            .iter()
            .find(|l| !layout.positions.contains_key(&l.from.id))
            .map(|l| l.from.id.clone());
        match seed {
            Some(id) => {
                let y = content_bottom(layout) + 200.0;
                if let Some(&mi) = cluster.get(id.as_str()) {
                    let module = &modules[mi];
                    let dx = MARGIN - module.extent.x;
                    let dy = y - module.extent.y;
                    place_module(layout, module, dx, dy);
                } else {
                    let (w, h) = dim_of(dims, &id);
                    place(layout, &id, SvgPos { x: MARGIN + w / 2.0, y }, (w, h));
                }
            }
            None => break,
        }
    }
    // Modules with no cross connections at all: park below the content.
    for module in modules {
        if module
            .local
            .keys()
            .next()
            .map(|k| layout.positions.contains_key(k))
            .unwrap_or(true)
        {
            continue;
        }
        let y = content_bottom(layout) + 200.0;
        let dx = MARGIN - module.extent.x;
        let dy = y - module.extent.y;
        place_module(layout, module, dx, dy);
    }
}

/// Equipment with `attach:` mounts flush against its host — a heat pad
/// under a vessel, a jacket on its side. The attach port picks the side
/// (plain `attach: X` means below); the piece sits a hair off the shell,
/// centered on the port.
fn place_attached_equipment(
    diagram: &Diagram,
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
) {
    const FLUSH_GAP: f64 = 6.0;
    for eq in diagram.equipment.values() {
        if eq.pos.is_some() || layout.positions.contains_key(&eq.id) {
            continue;
        }
        let Some(attach) = &eq.attach else { continue };
        let Some(host_pos) = layout.positions.get(&attach.id).copied() else {
            continue;
        };

        let side = attach
            .port
            .as_deref()
            .and_then(|pn| {
                diagram.get_ports(&attach.id).and_then(|ports| {
                    ports.iter().find(|p| p.name == pn).and_then(|p| p.side)
                })
            })
            .or_else(|| attach.port.as_deref().and_then(infer_port_side))
            .unwrap_or(Side::South);

        let anchor_pt = attach
            .port
            .as_ref()
            .and_then(|pn| {
                diagram
                    .get_ports(&attach.id)
                    .and_then(|ports| layout.port_pos(&attach.id, pn, ports))
            })
            .unwrap_or_else(|| {
                // Plain attach: middle of the host's boundary on `side`.
                let (hw, hh) = layout
                    .bounds
                    .get(&attach.id)
                    .map(|b| (b.w, b.h))
                    .unwrap_or((SYMBOL_W, SYMBOL_H));
                let (ux, uy) = unit(side);
                SvgPos {
                    x: host_pos.x + ux * hw / 2.0,
                    y: host_pos.y + uy * hh / 2.0,
                }
            });

        let (w, h) = dim_of(dims, &eq.id);
        let (ux, uy) = unit(side);
        let half = if is_vertical_side(side) { h / 2.0 } else { w / 2.0 };
        let pos = SvgPos {
            x: anchor_pt.x + ux * (FLUSH_GAP + half),
            y: anchor_pt.y + uy * (FLUSH_GAP + half),
        };
        place(layout, &eq.id, pos, (w, h));
    }
}

fn place_attached_instruments(
    diagram: &Diagram,
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
) {
    // Track how many instruments have been placed at each (equip_id, port_name) to avoid overlap
    let mut port_placement_count: HashMap<(String, String), usize> = HashMap::new();
    for instr in diagram.instruments.values() {
        if instr.pos.is_some() || layout.positions.contains_key(&instr.id) {
            continue;
        }
        let Some(attach) = &instr.attach else { continue };
        let Some(attach_pos) = layout.positions.get(&attach.id).copied() else { continue };

        // Bubble sits outward from the attach point on the side implied by
        // the attach port (default: above), far enough that the bubble and
        // its label clear the equipment outline, with room for a leader.
        let attach_bounds = layout.bounds.get(&attach.id);

        // Determine port side to choose offset direction
        let port_side = attach
            .port
            .as_deref()
            .and_then(|pn| {
                diagram.get_ports(&attach.id).and_then(|ports| {
                    ports.iter().find(|p| p.name == pn).and_then(|p| p.side)
                })
            })
            .or_else(|| attach.port.as_deref().and_then(infer_port_side))
            .unwrap_or(Side::North);
        let (ox, oy) = unit(port_side);

        // Port-attached: offset from the port point (already on the
        // boundary); center-attached: offset from the center by the
        // half-extent plus the same clearance.
        let port_pt = attach.port.as_ref().and_then(|pn| {
            diagram
                .get_ports(&attach.id)
                .and_then(|ports| layout.port_pos(&attach.id, pn, ports))
        });
        let (anchor_pt, clearance) = match port_pt {
            Some(p) => (p, 70.0),
            None => {
                let half = attach_bounds
                    .map(|b| if is_vertical_side(port_side) { b.h / 2.0 } else { b.w / 2.0 })
                    .unwrap_or(40.0);
                (attach_pos, half + 70.0)
            }
        };
        let base_pos = SvgPos {
            x: anchor_pt.x + ox * clearance,
            y: anchor_pt.y + oy * clearance,
        };

        // Determine perpendicular offset for instruments sharing the same port
        let port_key = (
            attach.id.clone(),
            attach.port.clone().unwrap_or_default(),
        );
        let count = port_placement_count.entry(port_key).or_insert(0);

        // Perpendicular offset: for north/south ports offset in x; for east/west in y
        let perp_offset = (*count as f64) * 44.0;
        let (px, py) = match port_side {
            Side::North | Side::South => (1.0, 0.0),
            Side::East | Side::West => (0.0, 1.0),
        };
        let mut svg_pos = SvgPos {
            x: base_pos.x + px * perp_offset,
            y: base_pos.y + py * perp_offset,
        };
        *count += 1;

        // Nudge perpendicular until clear of other placed symbols, previously
        // placed attached instruments, and the corridors of ports that lines
        // connect to (so the bubble doesn't sit on a pipe run).
        let corridors = used_port_corridors(diagram, layout, &attach.id);
        let (w, h) = dim_of(dims, &instr.id);
        for _ in 0..100 {
            let rect = SvgRect {
                x: svg_pos.x - w / 2.0,
                y: svg_pos.y - h / 2.0,
                w,
                h,
            };
            if !collides(layout, &rect, 16.0)
                && !corridors.iter().any(|c| c.intersects_rect(&rect, 0.0))
            {
                break;
            }
            svg_pos.x += px * 40.0;
            svg_pos.y += py * 40.0;
        }

        // Place immediately so later attached instruments see this one.
        place(layout, &instr.id, svg_pos, (w, h));
    }
}

/// Straight strips extending outward from every port of `id` that some line
/// connects to — the space a pipe run will occupy once routed.
fn used_port_corridors(diagram: &Diagram, layout: &LayoutInfo, id: &str) -> Vec<SvgRect> {
    let mut out = Vec::new();
    let Some(ports) = diagram.get_ports(id) else {
        return out;
    };
    let used = |port_name: &str| {
        diagram.lines.values().any(|l| {
            (l.from.id == id && l.from.port.as_deref() == Some(port_name))
                || l.to.as_ref().is_some_and(|t| {
                    t.id == id && t.port.as_deref() == Some(port_name)
                })
        })
    };
    const LEN: f64 = 160.0;
    const HALF_W: f64 = 10.0;
    for p in ports {
        if !used(&p.name) {
            continue;
        }
        let Some(pos) = layout.port_pos(id, &p.name, ports) else {
            continue;
        };
        let Some(side) = p.side.or_else(|| infer_port_side(&p.name)) else {
            continue;
        };
        let (ux, uy) = unit(side);
        let (x2, y2) = (pos.x + ux * LEN, pos.y + uy * LEN);
        out.push(SvgRect {
            x: pos.x.min(x2) - HALF_W,
            y: pos.y.min(y2) - HALF_W,
            w: (x2 - pos.x).abs() + HALF_W * 2.0,
            h: (y2 - pos.y).abs() + HALF_W * 2.0,
        });
    }
    out
}

/// Instruments with no position and no attach are placed relative to their
/// signal partners. A controller goes above the valve it actuates (short,
/// legible signal drop); otherwise fall back to any placed peer — above
/// equipment/valves, beside other instruments (so transmitter → controller
/// chains form a signal row).
fn place_signal_instruments(
    diagram: &Diagram,
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
) {
    // Pipe runs occupy the corridors outside every line-connected port;
    // instrument bubbles must stay off them. Equipment/valve positions are
    // final by this pass, so compute once.
    let corridors: Vec<SvgRect> = diagram
        .order
        .iter()
        .flat_map(|(_, id)| used_port_corridors(diagram, layout, id))
        .collect();
    loop {
        let mut progress = false;
        for instr in diagram.instruments.values() {
            if layout.positions.contains_key(&instr.id) {
                continue;
            }
            // Anchor preference: the valve this instrument actuates, then any
            // placed signal peer. A candidate is skipped when its spot is so
            // congested that placement would drift far away.
            let mut anchors: Vec<(String, Side)> = Vec::new();
            for sig in diagram.signals.values() {
                if sig.from.id == instr.id
                    && diagram.valves.contains_key(&sig.to.id)
                    && layout.positions.contains_key(&sig.to.id)
                {
                    anchors.push((sig.to.id.clone(), Side::North));
                }
            }
            for sig in diagram.signals.values() {
                let other = if sig.from.id == instr.id {
                    Some(&sig.to.id)
                } else if sig.to.id == instr.id {
                    Some(&sig.from.id)
                } else {
                    None
                };
                if let Some(o) = other {
                    if layout.positions.contains_key(o.as_str()) {
                        let side = if diagram.instruments.contains_key(o.as_str()) {
                            Side::East
                        } else {
                            Side::North
                        };
                        anchors.push((o.clone(), side));
                    }
                }
            }
            let cluster = framed_group_of(diagram);
            let my_cluster = cluster.get(instr.id.as_str()).copied();
            if my_cluster.is_some() {
                anchors.sort_by_key(|(pid, _)| {
                    if cluster.get(pid.as_str()).copied() == my_cluster {
                        0usize
                    } else {
                        1
                    }
                });
            }
            let mut done = false;
            for (pid, side) in &anchors {
                if try_place_adjacent(layout, dims, pid, &instr.id, *side, 3, false, &corridors) {
                    done = true;
                    break;
                }
            }
            if !done {
                if let Some((pid, side)) = anchors.first() {
                    try_place_adjacent(layout, dims, pid, &instr.id, *side, 100, true, &corridors);
                    done = true;
                }
            }
            if done {
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
}

fn place_leftovers(
    diagram: &Diagram,
    layout: &mut LayoutInfo,
    dims: &HashMap<String, (f64, f64)>,
) {
    use crate::ast::DeclKind;
    let row_y = content_bottom(layout) + 160.0;
    let mut x = MARGIN;
    for (kind, id) in &diagram.order {
        // Only symbols get placed; lines/signals are routed and groups/areas
        // are annotations.
        if !matches!(
            kind,
            DeclKind::Equipment | DeclKind::Valve | DeclKind::Instrument | DeclKind::Junction | DeclKind::Note
        ) {
            continue;
        }
        if layout.positions.contains_key(id) {
            continue;
        }
        let (w, h) = dim_of(dims, id);
        x += w / 2.0;
        place(layout, id, SvgPos { x, y: row_y }, (w, h));
        x += w / 2.0 + H_GAP;
    }
}

fn normalize_origin(layout: &mut LayoutInfo) {
    if layout.bounds.is_empty() {
        return;
    }
    let min_x = layout.bounds.values().map(|b| b.x).fold(f64::INFINITY, f64::min);
    let min_y = layout.bounds.values().map(|b| b.y).fold(f64::INFINITY, f64::min);
    let dx = if min_x < MARGIN { MARGIN - min_x } else { 0.0 };
    let dy = if min_y < MARGIN { MARGIN - min_y } else { 0.0 };
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    layout.origin_shift = (dx, dy);
    for p in layout.positions.values_mut() {
        p.x += dx;
        p.y += dy;
    }
    for b in layout.bounds.values_mut() {
        b.x += dx;
        b.y += dy;
    }
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

/// Width/height of the drawn symbol for an object, from the symbol library.
fn symbol_dims(diagram: &Diagram, kind: &crate::ast::DeclKind, id: &str) -> (f64, f64) {
    use crate::ast::DeclKind;
    match kind {
        DeclKind::Equipment => diagram
            .equipment
            .get(id)
            .map(|e| match &e.size {
                Some(sz) => symbols::sized_dims(
                    &e.equip_type,
                    sz.x as f64 * GRID_SCALE,
                    sz.y as f64 * GRID_SCALE,
                ),
                None => {
                    let s = symbols::equipment_symbol(&e.equip_type);
                    (s.width, s.height)
                }
            })
            .unwrap_or((SYMBOL_W, SYMBOL_H)),
        DeclKind::Valve => diagram
            .valves
            .get(id)
            .map(|v| {
                let s = symbols::valve_symbol(&v.valve_type, v.actuator.as_deref());
                if valve_is_vertical(v) {
                    (s.height, s.width)
                } else {
                    (s.width, s.height)
                }
            })
            .unwrap_or((VALVE_W, VALVE_H)),
        DeclKind::Instrument => diagram
            .instruments
            .get(id)
            .map(|i| {
                let s = symbols::instrument_symbol(&i.instr_type, i.location.as_deref());
                (s.width, s.height)
            })
            .unwrap_or((36.0, 36.0)),
        DeclKind::Junction => (10.0, 10.0),
        _ => (SYMBOL_W, SYMBOL_H),
    }
}
