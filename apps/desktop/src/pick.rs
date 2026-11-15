//! Picking on the map: which room, door or corridor lies under the pointer.

use crate::{class_label, role_label};
use tapiti_core::generate::{Floor, Node};
use tapiti_core::geom::{dist, Shape};

fn point_in_shape(shape: &Shape, point: [f64; 2]) -> bool {
    match shape {
        Shape::Disc { c, r } => dist(*c, point) <= *r,
        Shape::Poly { pts } => {
            let mut inside = false;
            let mut j = pts.len().saturating_sub(1);
            for i in 0..pts.len() {
                let [xi, yi] = pts[i];
                let [xj, yj] = pts[j];
                let crosses = (yi > point[1]) != (yj > point[1])
                    && point[0] < (xj - xi) * (point[1] - yi) / (yj - yi) + xi;
                if crosses {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }
    }
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 {
        0.0
    } else {
        (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2).clamp(0.0, 1.0)
    };
    dist(point, [a[0] + dx * t, a[1] + dy * t])
}

pub(crate) fn node_label(floor: &Floor, node: usize) -> String {
    match floor.nodes.get(node) {
        Some(Node::Junction { .. }) => "junction".into(),
        Some(Node::Room { room }) => format!("room #{id}", id = floor.rooms[*room].id),
        None => "point".into(),
    }
}

pub(crate) fn map_object_at(
    floor: &Floor,
    point: [f64; 2],
    zoom: f64,
    level: Option<usize>,
) -> String {
    // A stacked floor shows one level at a time: only what is on it can be under the pointer.
    let view = level.filter(|_| floor.stack.is_some());
    let node_level = tapiti_core::environment::height::node_levels(floor);
    for room in floor.rooms.iter().rev() {
        if room.alive && view.is_none_or(|v| room.level == v) && point_in_shape(&room.shape, point)
        {
            let kind = if room.leaf { "leaf chamber" } else { "room" };
            return format!(
                "{kind} · {role}\nRoom #{id} · {class}\nArea: {area:.0} px²\nDoors used: {used} · possible: {possible}",
                kind = kind,
                role = role_label(room.role),
                id = room.id,
                class = class_label(room.cls.name()),
                area = room.area,
                used = room.used.len(),
                possible = room.ports.len(),
            );
        }
    }
    for (id, node) in floor.nodes.iter().enumerate() {
        if let Node::Junction { p } = node {
            if view.is_none_or(|v| node_level[id] == v)
                && dist(**p, point) <= (8.0 / zoom.max(0.1)).max(6.0)
            {
                let degree = floor
                    .edges
                    .iter()
                    .filter(|edge| edge.u == id || edge.v == id)
                    .count();
                return format!(
                    "Corridor junction\n{degree} arms connected\nPosition: {x:.0}, {y:.0} px",
                    degree = degree,
                    x = p[0],
                    y = p[1]
                );
            }
        }
    }
    for edge in floor
        .edges
        .iter()
        .filter(|e| view.is_none_or(|v| node_level[e.u] == v || (e.stair && node_level[e.v] == v)))
    {
        if edge.stair && dist(edge.pts[0], point) <= 9.0 / zoom.max(0.1) {
            return format!(
                "Stair #{id}\n{from} – {to}",
                id = edge.id,
                from = node_label(floor, edge.u),
                to = node_label(floor, edge.v)
            );
        }
        if !edge.stair
            && edge.pts.windows(2).any(|segment| {
                distance_to_segment(point, segment[0], segment[1])
                    <= floor.params.width / 2.0 + 5.0 / zoom.max(0.1)
            })
        {
            let length = edge
                .pts
                .windows(2)
                .map(|segment| dist(segment[0], segment[1]))
                .sum::<f64>();
            return format!(
                "Corridor segment #{id}\n{from} – {to}\n{n} segment(s) · {length:.0} px",
                id = edge.id,
                from = node_label(floor, edge.u),
                to = node_label(floor, edge.v),
                n = edge.pts.len().saturating_sub(1),
                length = length
            );
        }
    }
    String::new()
}
