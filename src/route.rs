use crate::layout::{LayoutInfo, SvgPos};
use crate::model::*;

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub segments: Vec<RouteSegment>,
}

#[derive(Debug, Clone)]
pub struct RouteSegment {
    pub connection_id: String,
    pub points: Vec<SvgPos>,
    pub is_signal: bool,
}

pub fn route(diagram: &Diagram, layout: &LayoutInfo) -> RouteResult {
    let mut segments = Vec::new();

    // Route lines
    for line in diagram.lines.values() {
        if let Some(seg) = route_connection(
            &line.id,
            &line.from,
            &line.to,
            false,
            diagram,
            layout,
        ) {
            segments.push(seg);
        }
    }

    // Route signals
    for sig in diagram.signals.values() {
        if let Some(seg) = route_connection(
            &sig.id,
            &sig.from,
            &sig.to,
            true,
            diagram,
            layout,
        ) {
            segments.push(seg);
        }
    }

    RouteResult { segments }
}

fn get_endpoint(
    obj_ref: &ObjRef,
    diagram: &Diagram,
    layout: &LayoutInfo,
) -> Option<SvgPos> {
    let center = layout.get_pos(&obj_ref.id)?;

    if let Some(port_name) = &obj_ref.port {
        // Try to get port position
        if let Some(ports) = diagram.get_ports(&obj_ref.id) {
            if let Some(pos) = layout.port_pos(&obj_ref.id, port_name, ports) {
                return Some(pos);
            }
        }
        // Fallback: use center
    }

    Some(SvgPos { x: center.x, y: center.y })
}

fn route_connection(
    id: &str,
    from: &ObjRef,
    to: &ObjRef,
    is_signal: bool,
    diagram: &Diagram,
    layout: &LayoutInfo,
) -> Option<RouteSegment> {
    let p1 = get_endpoint(from, diagram, layout)?;
    let p2 = get_endpoint(to, diagram, layout)?;

    // Collect obstacle boxes (all symbol bounds except from/to objects)
    let obstacles: Vec<_> = layout.bounds.iter()
        .filter(|(bid, _)| bid.as_str() != from.id && bid.as_str() != to.id)
        .map(|(_, b)| b)
        .collect();

    // Try H-then-V route: horizontal first then vertical
    let htv = route_htv(&p1, &p2);
    let htv_hits = count_obstacle_hits(&htv, &obstacles);

    // Try V-then-H route: vertical first then horizontal
    let vth = route_vth(&p1, &p2);
    let vth_hits = count_obstacle_hits(&vth, &obstacles);

    let points = if vth_hits < htv_hits { vth } else { htv };

    Some(RouteSegment {
        connection_id: id.to_string(),
        points,
        is_signal,
    })
}

fn route_htv(p1: &SvgPos, p2: &SvgPos) -> Vec<SvgPos> {
    // Go horizontal to dest x, then vertical to dest y
    let mid = SvgPos { x: p2.x, y: p1.y };
    vec![
        SvgPos { x: p1.x, y: p1.y },
        mid,
        SvgPos { x: p2.x, y: p2.y },
    ]
}

fn route_vth(p1: &SvgPos, p2: &SvgPos) -> Vec<SvgPos> {
    // Go vertical to dest y, then horizontal to dest x
    let mid = SvgPos { x: p1.x, y: p2.y };
    vec![
        SvgPos { x: p1.x, y: p1.y },
        mid,
        SvgPos { x: p2.x, y: p2.y },
    ]
}

fn count_obstacle_hits(points: &[SvgPos], obstacles: &[&crate::layout::SvgRect]) -> usize {
    let mut count = 0;
    for i in 0..points.len().saturating_sub(1) {
        let p1 = &points[i];
        let p2 = &points[i + 1];
        for obs in obstacles.iter() {
            if obs.intersects_segment(p1.x, p1.y, p2.x, p2.y) {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{LayoutInfo, SvgPos, SvgRect};
    use std::collections::HashMap;

    fn make_layout(items: &[(&str, f64, f64, f64, f64)]) -> LayoutInfo {
        let mut layout = LayoutInfo::new();
        for &(id, x, y, w, h) in items {
            layout.positions.insert(id.to_string(), SvgPos { x, y });
            layout.bounds.insert(id.to_string(), SvgRect {
                x: x - w / 2.0,
                y: y - h / 2.0,
                w,
                h,
            });
        }
        layout
    }

    #[test]
    fn test_straight_route() {
        let layout = make_layout(&[("A", 0.0, 0.0, 60.0, 60.0), ("B", 200.0, 0.0, 60.0, 60.0)]);
        let htv = route_htv(
            &SvgPos { x: 30.0, y: 0.0 },
            &SvgPos { x: 170.0, y: 0.0 },
        );
        // Should be a straight line (3 collinear points)
        assert_eq!(htv.len(), 3);
        assert_eq!(htv[0].y, htv[1].y);
        assert_eq!(htv[1].y, htv[2].y);
        let _ = layout;
    }

    #[test]
    fn test_one_bend_route() {
        let p1 = SvgPos { x: 0.0, y: 0.0 };
        let p2 = SvgPos { x: 100.0, y: 100.0 };
        let htv = route_htv(&p1, &p2);
        assert_eq!(htv.len(), 3);
        assert_eq!(htv[1].x, 100.0);
        assert_eq!(htv[1].y, 0.0);
    }

    #[test]
    fn test_deterministic_route() {
        let p1 = SvgPos { x: 10.0, y: 20.0 };
        let p2 = SvgPos { x: 80.0, y: 60.0 };
        let r1 = route_htv(&p1, &p2);
        let r2 = route_htv(&p1, &p2);
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert!((a.x - b.x).abs() < 1e-9);
            assert!((a.y - b.y).abs() < 1e-9);
        }
    }
}
