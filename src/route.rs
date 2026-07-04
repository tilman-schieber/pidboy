use crate::layout::{infer_port_side, LayoutInfo, SvgPos, SvgRect};
use crate::model::*;
use crate::symbols;

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub segments: Vec<RouteSegment>,
}

#[derive(Debug, Clone)]
pub struct RouteSegment {
    pub connection_id: String,
    pub points: Vec<SvgPos>,
    pub is_signal: bool,
    /// CSS class override; used for instrument attach leader lines.
    pub class: Option<String>,
}

/// How far a route runs straight out of a port before it may turn.
const STUB: f64 = 16.0;
const OBSTACLE_PENALTY: f64 = 10_000.0;
const BEND_PENALTY: f64 = 40.0;
/// Doubling back over the segment just travelled looks broken; avoid it
/// unless every alternative crosses symbols.
const REVERSAL_PENALTY: f64 = 5_000.0;
/// Running collinearly on top of an already-routed line hides one of the
/// two (dashes disappear under solids); charge per overlapping pixel.
/// Kept mild: long coincident runs get rerouted, short shared runs (signal
/// buses) are cheaper than a big detour.
const OVERLAP_PENALTY_PER_PX: f64 = 1.5;

/// A resolved connection endpoint.
struct Endpoint {
    pos: SvgPos,
    /// Outward direction the route should leave/enter through, if known.
    dir: Option<Side>,
    /// True when anchored at the symbol center: the route must be trimmed
    /// back to the symbol boundary so arrowheads stay visible.
    trim: bool,
}

pub fn route(diagram: &Diagram, layout: &LayoutInfo) -> RouteResult {
    let mut segments: Vec<RouteSegment> = Vec::new();

    // Route lines
    for line in diagram.lines.values() {
        let seg = match &line.to {
            Some(to) => route_connection(&line.id, &line.from, to, false, diagram, layout, &segments),
            None => route_open_stub(line, diagram, layout),
        };
        if let Some(seg) = seg {
            segments.push(seg);
        }
    }

    // Route signals
    for sig in diagram.signals.values() {
        if let Some(seg) = route_connection(&sig.id, &sig.from, &sig.to, true, diagram, layout, &segments) {
            segments.push(seg);
        }
    }

    // Leader lines from attached instruments to their equipment/port.
    for instr in diagram.instruments.values() {
        if let Some(attach) = &instr.attach {
            let from = ObjRef::new(instr.id.clone(), None);
            let id = format!("{}__attach", instr.id);
            if let Some(mut seg) = route_connection(&id, &from, attach, false, diagram, layout, &segments) {
                seg.class = Some("line-attach".to_string());
                segments.push(seg);
            }
        }
    }

    RouteResult { segments }
}

fn get_endpoint(
    obj_ref: &ObjRef,
    diagram: &Diagram,
    layout: &LayoutInfo,
    is_signal: bool,
) -> Option<Endpoint> {
    let center = *layout.get_pos(&obj_ref.id)?;

    if let Some(port_name) = &obj_ref.port {
        if let Some(ports) = diagram.get_ports(&obj_ref.id) {
            if let Some(pos) = layout.port_pos(&obj_ref.id, port_name, ports) {
                let side = ports
                    .iter()
                    .find(|p| p.name == *port_name)
                    .and_then(|p| p.side)
                    .or_else(|| infer_port_side(port_name));
                // A port that resolved to the plain center (unknown name, no
                // side) is handled like a center anchor below.
                if side.is_some() {
                    return Some(Endpoint { pos, dir: side, trim: false });
                }
            }
        }
    }

    // Signals into an actuated valve terminate on the actuator head.
    if is_signal {
        if let Some(v) = diagram.valves.get(&obj_ref.id) {
            if let Some((dx, dy)) = symbols::valve_signal_anchor(&v.valve_type, v.actuator.as_deref()) {
                return Some(Endpoint {
                    pos: SvgPos { x: center.x + dx, y: center.y + dy },
                    dir: Some(Side::North),
                    trim: false,
                });
            }
        }
    }

    Some(Endpoint { pos: center, dir: None, trim: true })
}

fn unit(side: Side) -> (f64, f64) {
    match side {
        Side::East => (1.0, 0.0),
        Side::West => (-1.0, 0.0),
        Side::North => (0.0, -1.0),
        Side::South => (0.0, 1.0),
    }
}

fn stub_point(e: &Endpoint) -> SvgPos {
    match e.dir {
        Some(side) => {
            let (ux, uy) = unit(side);
            SvgPos {
                x: e.pos.x + ux * STUB,
                y: e.pos.y + uy * STUB,
            }
        }
        None => e.pos,
    }
}

fn route_connection(
    id: &str,
    from: &ObjRef,
    to: &ObjRef,
    is_signal: bool,
    diagram: &Diagram,
    layout: &LayoutInfo,
    existing: &[RouteSegment],
) -> Option<RouteSegment> {
    let e1 = get_endpoint(from, diagram, layout, is_signal)?;
    let e2 = get_endpoint(to, diagram, layout, is_signal)?;

    // Obstacles: every placed symbol except the two being connected.
    let obstacles: Vec<SvgRect> = layout
        .bounds
        .iter()
        .filter(|(bid, _)| bid.as_str() != from.id && bid.as_str() != to.id)
        .map(|(_, b)| b.clone())
        .collect();

    let s1 = stub_point(&e1);
    let s2 = stub_point(&e2);

    let mut best: Option<(f64, Vec<SvgPos>)> = None;
    for cand in candidate_paths(&s1, &s2, &obstacles) {
        let cost = path_cost(&cand, e1.dir, e2.dir, &obstacles)
            + OVERLAP_PENALTY_PER_PX * coincident_overlap(&cand, existing);
        if best.as_ref().map_or(true, |(c, _)| cost < *c) {
            best = Some((cost, cand));
        }
    }
    let mid = best.map(|(_, p)| p)?;

    let mut points = Vec::with_capacity(mid.len() + 2);
    points.push(e1.pos);
    points.extend(mid);
    points.push(e2.pos);

    // Trim center-anchored ends back to the symbol boundary so arrowheads
    // land on the outline instead of underneath the symbol fill.
    if e2.trim {
        if let Some(b) = layout.get_bounds(&to.id) {
            trim_tail(&mut points, b);
        }
    }
    if e1.trim {
        if let Some(b) = layout.get_bounds(&from.id) {
            points.reverse();
            trim_tail(&mut points, b);
            points.reverse();
        }
    }

    let points = simplify(points);
    if points.len() < 2 {
        return None;
    }

    Some(RouteSegment {
        connection_id: id.to_string(),
        points,
        is_signal,
        class: None,
    })
}

/// Length of an open-ended stub line beyond the symbol boundary.
const OPEN_STUB_LEN: f64 = 40.0;

/// A line with no `to`: a short open-ended run drawn outward from `from`
/// (drain, vent, sample point). Direction comes from the port side; without
/// one, vents point up, drains down, everything else east.
fn route_open_stub(
    line: &Line,
    diagram: &Diagram,
    layout: &LayoutInfo,
) -> Option<RouteSegment> {
    let e = get_endpoint(&line.from, diagram, layout, false)?;
    let dir = e.dir.unwrap_or(match line.class.as_str() {
        "vent" => Side::North,
        "drain" => Side::South,
        _ => Side::East,
    });
    let (ux, uy) = unit(dir);

    // Center-anchored starts reach the boundary first, then extend beyond it.
    let extra = if e.trim {
        layout
            .get_bounds(&line.from.id)
            .map(|b| if uy != 0.0 { b.h / 2.0 } else { b.w / 2.0 })
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let len = extra + OPEN_STUB_LEN;
    let mut points = vec![
        e.pos,
        SvgPos {
            x: e.pos.x + ux * len,
            y: e.pos.y + uy * len,
        },
    ];
    if e.trim {
        if let Some(b) = layout.get_bounds(&line.from.id) {
            points.reverse();
            trim_tail(&mut points, b);
            points.reverse();
        }
    }
    if points.len() < 2 {
        return None;
    }
    Some(RouteSegment {
        connection_id: line.id.clone(),
        points,
        is_signal: false,
        class: None,
    })
}

/// Orthogonal candidate paths from `s1` to `s2` (both included): the two
/// single-bend L routes, double-bend routes through the midpoint, and escape
/// routes around the far edges of the obstacle field.
fn candidate_paths(s1: &SvgPos, s2: &SvgPos, obstacles: &[SvgRect]) -> Vec<Vec<SvgPos>> {
    let p = |x: f64, y: f64| SvgPos { x, y };
    let hvh = |xm: f64| vec![*s1, p(xm, s1.y), p(xm, s2.y), *s2];
    let vhv = |ym: f64| vec![*s1, p(s1.x, ym), p(s2.x, ym), *s2];

    let mut cands = vec![
        vec![*s1, p(s2.x, s1.y), *s2], // horizontal, then vertical
        vec![*s1, p(s1.x, s2.y), *s2], // vertical, then horizontal
        hvh((s1.x + s2.x) / 2.0),
        vhv((s1.y + s2.y) / 2.0),
    ];

    // Local hops: step just outside the endpoints' own span, enough to skip
    // over a symbol sitting between two endpoints on the same row/column.
    // Two step sizes, since the blocking symbol may extend past the first.
    for step in [60.0, 100.0] {
        cands.push(vhv(s1.y.min(s2.y) - step));
        cands.push(vhv(s1.y.max(s2.y) + step));
        cands.push(hvh(s1.x.min(s2.x) - step));
        cands.push(hvh(s1.x.max(s2.x) + step));
    }

    if !obstacles.is_empty() {
        let min_x = obstacles.iter().map(|b| b.x).fold(f64::INFINITY, f64::min);
        let max_x = obstacles.iter().map(|b| b.x + b.w).fold(f64::NEG_INFINITY, f64::max);
        let min_y = obstacles.iter().map(|b| b.y).fold(f64::INFINITY, f64::min);
        let max_y = obstacles.iter().map(|b| b.y + b.h).fold(f64::NEG_INFINITY, f64::max);
        cands.push(hvh(min_x - 40.0));
        cands.push(hvh(max_x + 40.0));
        cands.push(vhv(min_y - 40.0));
        cands.push(vhv(max_y + 40.0));
    }

    cands
}

fn seg_dir(a: &SvgPos, b: &SvgPos) -> Option<(f64, f64)> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return None;
    }
    // Not f64::signum — it maps 0.0 to 1.0, turning axis-aligned segments diagonal.
    let sign = |v: f64| {
        if v > 1e-9 {
            1.0
        } else if v < -1e-9 {
            -1.0
        } else {
            0.0
        }
    };
    Some((sign(dx), sign(dy)))
}

fn path_cost(
    path: &[SvgPos],
    d1: Option<Side>,
    d2: Option<Side>,
    obstacles: &[SvgRect],
) -> f64 {
    let mut cost = 0.0;
    let mut prev_dir = d1.map(unit);

    for pair in path.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        for obs in obstacles {
            if obs.intersects_segment(a.x, a.y, b.x, b.y) {
                cost += OBSTACLE_PENALTY;
            }
        }
        cost += (b.x - a.x).abs() + (b.y - a.y).abs();
        let Some(dir) = seg_dir(a, b) else { continue };
        if let Some(pd) = prev_dir {
            if dir != pd {
                cost += BEND_PENALTY;
                if dir == (-pd.0, -pd.1) {
                    cost += REVERSAL_PENALTY;
                }
            }
        }
        prev_dir = Some(dir);
    }

    // The final leg after the path is stub → port, moving opposite the port
    // direction. Arriving at the stub while moving *in* the port direction
    // would double back over it.
    if let (Some(d2), Some(pd)) = (d2, prev_dir) {
        if pd == unit(d2) {
            cost += REVERSAL_PENALTY;
        }
    }

    cost
}

/// Total length (px) of this path's segments that run collinearly on top of
/// already-routed segments. Perpendicular crossings don't count.
fn coincident_overlap(path: &[SvgPos], existing: &[RouteSegment]) -> f64 {
    fn span_overlap(a1: f64, a2: f64, b1: f64, b2: f64) -> f64 {
        let lo = a1.min(a2).max(b1.min(b2));
        let hi = a1.max(a2).min(b1.max(b2));
        (hi - lo).max(0.0)
    }
    let mut total = 0.0;
    for w in path.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let vert = (a.x - b.x).abs() < 1e-9;
        let horiz = (a.y - b.y).abs() < 1e-9;
        for seg in existing {
            for e in seg.points.windows(2) {
                let (c, d) = (&e[0], &e[1]);
                if vert && (c.x - d.x).abs() < 1e-9 && (a.x - c.x).abs() < 4.0 {
                    total += span_overlap(a.y, b.y, c.y, d.y);
                } else if horiz && (c.y - d.y).abs() < 1e-9 && (a.y - c.y).abs() < 4.0 {
                    total += span_overlap(a.x, b.x, c.x, d.x);
                }
            }
        }
    }
    total
}

/// While the last point lies inside `rect`, pull it back to the boundary
/// along the final segment (dropping fully-interior points).
fn trim_tail(points: &mut Vec<SvgPos>, rect: &SvgRect) {
    while points.len() >= 2 {
        let n = points.len();
        let b = points[n - 1];
        if !rect.contains_point(b.x, b.y) {
            return;
        }
        let a = points[n - 2];
        if rect.contains_point(a.x, a.y) {
            points.pop();
            continue;
        }
        if let Some(t) = segment_rect_entry(&a, &b, rect) {
            points[n - 1] = SvgPos {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            };
        }
        return;
    }
}

/// Liang-Barsky: parameter t in [0,1] where segment a→b first enters `rect`,
/// assuming `a` is outside and `b` inside.
fn segment_rect_entry(a: &SvgPos, b: &SvgPos, r: &SvgRect) -> Option<f64> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let mut t0 = 0.0f64;
    let mut t1 = 1.0f64;
    let checks = [
        (-dx, a.x - r.x),
        (dx, r.x + r.w - a.x),
        (-dy, a.y - r.y),
        (dy, r.y + r.h - a.y),
    ];
    for (p, q) in checks {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t1 {
                return None;
            }
            if t > t0 {
                t0 = t;
            }
        } else {
            if t < t0 {
                return None;
            }
            if t < t1 {
                t1 = t;
            }
        }
    }
    Some(t0)
}

/// Remove consecutive duplicates and merge collinear same-direction segments.
fn simplify(points: Vec<SvgPos>) -> Vec<SvgPos> {
    let mut out: Vec<SvgPos> = Vec::with_capacity(points.len());
    for p in points {
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() < 1e-9 && (last.y - p.y).abs() < 1e-9 {
                continue;
            }
        }
        out.push(p);
    }
    let mut i = 1;
    while i + 1 < out.len() {
        let same_dir = match (seg_dir(&out[i - 1], &out[i]), seg_dir(&out[i], &out[i + 1])) {
            (Some(d1), Some(d2)) => d1 == d2,
            _ => false,
        };
        if same_dir {
            out.remove(i);
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> SvgPos {
        SvgPos { x, y }
    }

    #[test]
    fn test_straight_route_has_no_bend_cost() {
        let path = vec![p(0.0, 0.0), p(100.0, 0.0)];
        let cost = path_cost(&path, Some(Side::East), Some(Side::West), &[]);
        assert!((cost - 100.0).abs() < 1e-9, "cost = {}", cost);
    }

    #[test]
    fn test_obstacle_forces_detour() {
        // Obstacle sits directly between the endpoints on the straight line.
        let obstacles = vec![SvgRect { x: 40.0, y: -20.0, w: 20.0, h: 40.0 }];
        let s1 = p(0.0, 0.0);
        let s2 = p(100.0, 0.0);
        let best = candidate_paths(&s1, &s2, &obstacles)
            .into_iter()
            .min_by(|a, b| {
                path_cost(a, None, None, &obstacles)
                    .partial_cmp(&path_cost(b, None, None, &obstacles))
                    .unwrap()
            })
            .unwrap();
        let hits: usize = best
            .windows(2)
            .map(|w| {
                obstacles
                    .iter()
                    .filter(|o| o.intersects_segment(w[0].x, w[0].y, w[1].x, w[1].y))
                    .count()
            })
            .sum();
        assert_eq!(hits, 0, "best path should avoid the obstacle: {:?}", best);
    }

    #[test]
    fn test_trim_tail_stops_at_boundary() {
        // Line ends at the center of a 40x40 rect at (100,0).
        let rect = SvgRect { x: 80.0, y: -20.0, w: 40.0, h: 40.0 };
        let mut points = vec![p(0.0, 0.0), p(100.0, 0.0)];
        trim_tail(&mut points, &rect);
        assert!((points[1].x - 80.0).abs() < 1e-9, "trimmed to {:?}", points);
        assert!((points[1].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_simplify_merges_collinear_but_keeps_reversals() {
        let merged = simplify(vec![p(0.0, 0.0), p(50.0, 0.0), p(100.0, 0.0)]);
        assert_eq!(merged.len(), 2);

        // A reversal is geometry, not redundancy — must survive.
        let reversal = simplify(vec![p(0.0, 0.0), p(100.0, 0.0), p(50.0, 0.0)]);
        assert_eq!(reversal.len(), 3);
    }

    #[test]
    fn test_deterministic_route() {
        let s1 = p(10.0, 20.0);
        let s2 = p(80.0, 60.0);
        let c1 = candidate_paths(&s1, &s2, &[]);
        let c2 = candidate_paths(&s1, &s2, &[]);
        assert_eq!(c1.len(), c2.len());
        for (a, b) in c1.iter().flatten().zip(c2.iter().flatten()) {
            assert!((a.x - b.x).abs() < 1e-9);
            assert!((a.y - b.y).abs() < 1e-9);
        }
    }
}
