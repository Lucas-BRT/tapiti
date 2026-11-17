//! The map view: world-to-screen transform and egui painting of rooms, corridors and overlays.

use crate::{class_label, role_label, view_box, BuildingMap, Overlays};
use eframe::egui::{
    epaint::PathShape, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape as Draw, Stroke,
    Vec2,
};
use tapiti_core::generate::{Floor, Node, Role};
use tapiti_core::geom::{dir_of, Shape};
use tapiti_render::view::{self, Channel};

/// World (generator pixels) to screen.
pub struct Xf {
    rect: Rect,
    origin: Pos2,
    pub scale: f32,
    ox: f32,
    oy: f32,
    vx: f64,
    vy: f64,
}

impl Xf {
    pub fn new(rect: Rect, min: [f64; 2], max: [f64; 2], zoom: f64, pan: [f64; 2]) -> Self {
        let (vx, vy, width, height) = view_box(min, max, zoom, pan);
        let scale = (f64::from(rect.width()) / width).min(f64::from(rect.height()) / height);
        Self {
            rect,
            origin: rect.min,
            scale: scale as f32,
            ox: ((f64::from(rect.width()) - width * scale) / 2.0) as f32,
            oy: ((f64::from(rect.height()) - height * scale) / 2.0) as f32,
            vx,
            vy,
        }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn pt(&self, p: [f64; 2]) -> Pos2 {
        Pos2::new(
            self.origin.x + self.ox + ((p[0] - self.vx) * f64::from(self.scale)) as f32,
            self.origin.y + self.oy + ((p[1] - self.vy) * f64::from(self.scale)) as f32,
        )
    }

    pub fn to_world(&self, s: Pos2) -> [f64; 2] {
        let scale = f64::from(self.scale).max(1e-6);
        [
            self.vx + f64::from(s.x - self.origin.x - self.ox) / scale,
            self.vy + f64::from(s.y - self.origin.y - self.oy) / scale,
        ]
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub corridor: Color32,
    pub grid: Color32,
    pub ink: Color32,
    pub hatch: Color32,
    /// The outline of a room with no role; the entrance and the exit are outlined in green and violet.
    pub edge: Color32,
    pub accent: Color32,
    pub success: Color32,
    /// The exit's outline and badge: not amber, which is for what is changed or picked.
    pub exit: Color32,
    pub danger: Color32,
    pub dark: bool,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        let c = crate::theme::colors(dark);
        Self {
            background: c.editor,
            corridor: if dark {
                Color32::from_rgb(0x3d, 0x48, 0x52)
            } else {
                Color32::from_rgb(0xb4, 0xbe, 0xc7)
            },
            grid: if dark {
                Color32::from_rgb(0x1b, 0x21, 0x27)
            } else {
                Color32::from_rgb(0xe3, 0xe8, 0xec)
            },
            ink: c.text,
            hatch: if dark {
                Color32::from_rgba_unmultiplied(8, 10, 12, 150)
            } else {
                Color32::from_rgba_unmultiplied(27, 33, 39, 105)
            },
            edge: if dark {
                Color32::from_rgb(0x0e, 0x10, 0x12)
            } else {
                Color32::from_rgb(0x1b, 0x21, 0x27)
            },
            accent: c.accent,
            success: c.success,
            exit: if dark {
                Color32::from_rgb(0xc9, 0xa0, 0xff)
            } else {
                Color32::from_rgb(0x5a, 0x2d, 0x91)
            },
            danger: c.error,
            dark,
        }
    }

    pub fn shape(&self, class: &str) -> Color32 {
        let (r, g, b) = match class {
            "rect" => (0x71, 0x9a, 0xb0),
            "disc" => (0x94, 0xb4, 0x9d),
            "oct" => (0xcd, 0xbb, 0xd2),
            "L" => (0xbb, 0x8d, 0x58),
            "star" => (0xe8, 0xd0, 0x7d),
            _ => (0x59, 0x67, 0x7a),
        };
        Color32::from_rgb(r, g, b)
    }

    /// Graphite labels on the light room washes.
    fn on_room(&self) -> Color32 {
        Color32::from_rgb(0x0e, 0x10, 0x12)
    }

    fn role(&self, role: Role) -> Color32 {
        match role {
            Role::Entrance => self.success,
            Role::Exit => self.exit,
            Role::Plain => self.edge,
        }
    }
}

/// Dark ink on a light fill and white on a dark one, whichever reads at least 4.5:1.
fn ink_on(fill: Color32) -> Color32 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(fill.r()) + 0.7152 * lin(fill.g()) + 0.0722 * lin(fill.b());
    // Contrast with white is 1.05 / (l + 0.05); with the graphite ink (about 0.005) it is (l + 0.05) / 0.055.
    if 1.05 / (l + 0.05) >= (l + 0.05) / 0.055 {
        Color32::WHITE
    } else {
        Color32::from_rgb(0x0e, 0x10, 0x12)
    }
}

/// A chip at a room's corner that says IN or OUT, at the same size at every zoom, so the entrance and the exit are found by more than
/// the colour of an outline.
fn role_badge(painter: &Painter, xf: &Xf, room: &tapiti_core::generate::Room, pal: &Palette) {
    let (text, fill) = match room.role {
        Role::Entrance => ("IN", pal.success),
        Role::Exit => ("OUT", pal.exit),
        Role::Plain => return,
    };
    let corner = xf.pt([room.bounds.lo[0], room.bounds.lo[1]]);
    let size = Vec2::new(if text == "IN" { 18.0 } else { 26.0 }, 13.0);
    let chip = Rect::from_min_size(corner + Vec2::new(3.0, 3.0), size);
    painter.rect_filled(chip, 0.0, fill);
    painter.text(
        chip.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(10.0),
        ink_on(fill),
    );
}

pub const SHAPE_CLASSES: [&str; 5] = ["rect", "disc", "oct", "L", "star"];

/// Ear clipping, for the concave rooms (L, star): egui fills only convex paths.
pub fn triangulate(pts: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sign
    };
    let mut ring: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    'clip: while ring.len() > 3 {
        let m = ring.len();
        for i in 0..m {
            let (p, c, q) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            if cross(pts[p], pts[c], pts[q]) <= 1e-9 {
                continue;
            }
            let blocked = ring.iter().any(|&o| {
                if [p, c, q].iter().any(|&v| pts[v] == pts[o]) {
                    return false;
                }
                cross(pts[p], pts[c], pts[o]) >= 0.0
                    && cross(pts[c], pts[q], pts[o]) >= 0.0
                    && cross(pts[q], pts[p], pts[o]) >= 0.0
            });
            if !blocked {
                out.push([p, c, q]);
                ring.remove(i);
                continue 'clip;
            }
        }
        break; // degenerate ring: fan the rest below
    }
    for i in 1..ring.len().saturating_sub(1) {
        out.push([ring[0], ring[i], ring[i + 1]]);
    }
    out
}

fn polyline(
    painter: &Painter,
    xf: &Xf,
    points: &[[f64; 2]],
    color: Color32,
    width: f32,
    dashed: bool,
) {
    if points.len() < 2 {
        return;
    }
    let width = (width * xf.scale).max(1.0);
    let screen: Vec<Pos2> = points.iter().map(|p| xf.pt(*p)).collect();
    if dashed {
        painter.extend(Draw::dashed_line(
            &screen,
            Stroke::new(width, color),
            width * 3.0,
            width * 2.0,
        ));
        return;
    }
    // Round joins and caps: a disc at every vertex hides the gaps egui leaves at bends.
    painter.add(PathShape::line(screen.clone(), Stroke::new(width, color)));
    if width > 2.5 {
        for p in screen {
            painter.circle_filled(p, width / 2.0, color);
        }
    }
}

fn room_shape(
    painter: &Painter,
    xf: &Xf,
    shape: &Shape,
    fill: Option<Color32>,
    stroke: Color32,
    width: f32,
    dashed: bool,
) {
    let width = (width * xf.scale).max(1.0);
    match shape {
        Shape::Disc { c, r } => {
            let (center, radius) = (xf.pt(*c), *r as f32 * xf.scale);
            if let Some(fill) = fill {
                painter.circle_filled(center, radius, fill);
            }
            if dashed {
                let ring: Vec<Pos2> = (0..=48)
                    .map(|i| {
                        let t = i as f32 / 48.0 * std::f32::consts::TAU;
                        center + Vec2::angled(t) * radius
                    })
                    .collect();
                painter.extend(Draw::dashed_line(
                    &ring,
                    Stroke::new(width, stroke),
                    width * 3.0,
                    width * 2.0,
                ));
            } else {
                painter.circle_stroke(center, radius, Stroke::new(width, stroke));
            }
        }
        Shape::Poly { pts } => {
            let screen: Vec<Pos2> = pts.iter().map(|p| xf.pt(*p)).collect();
            if let Some(fill) = fill {
                let mut mesh = Mesh::default();
                for p in &screen {
                    mesh.colored_vertex(*p, fill);
                }
                for [a, b, c] in triangulate(pts) {
                    mesh.add_triangle(a as u32, b as u32, c as u32);
                }
                painter.add(Draw::mesh(mesh));
            }
            if dashed {
                let mut closed = screen.clone();
                closed.extend(screen.first().copied());
                painter.extend(Draw::dashed_line(
                    &closed,
                    Stroke::new(width, stroke),
                    width * 3.0,
                    width * 2.0,
                ));
            } else {
                painter.add(PathShape::closed_line(screen, Stroke::new(width, stroke)));
            }
        }
    }
}

fn grid(painter: &Painter, xf: &Xf, grid_x: &[f64], grid_y: &[f64], color: Color32) {
    let (Some(&y0), Some(&y1), Some(&x0), Some(&x1)) =
        (grid_y.first(), grid_y.last(), grid_x.first(), grid_x.last())
    else {
        return;
    };
    let stroke = Stroke::new(1.0, color);
    for &x in grid_x {
        painter.line_segment([xf.pt([x, y0]), xf.pt([x, y1])], stroke);
    }
    for &y in grid_y {
        painter.line_segment([xf.pt([x0, y]), xf.pt([x1, y])], stroke);
    }
}

fn label(painter: &Painter, xf: &Xf, text: &str, at: [f64; 2], color: Color32, world_size: f32) {
    let size = (world_size * xf.scale).round();
    if size < 6.0 {
        return;
    }
    // Small rooms get only their number: the full label would spill over the neighbours.
    let text = if size < 12.0 {
        text.split(' ').next().unwrap_or(text)
    } else {
        text
    };
    let galley = painter.layout_no_wrap(
        text.to_string(),
        FontId::monospace(size.clamp(9.0, 13.0)),
        color,
    );
    let rect = Rect::from_center_size(xf.pt(at), galley.size());
    painter.rect_filled(rect.expand(2.0), 0.0, Color32::from_rgb(0xff, 0xfc, 0xf3));
    painter.galley(rect.min, galley, color);
}

pub fn draw_building(
    painter: &Painter,
    xf: &Xf,
    building: &BuildingMap,
    overlays: Overlays,
    pal: &Palette,
) {
    if overlays.cells {
        grid(painter, xf, &building.grid_x, &building.grid_y, pal.grid);
    }
    for points in &building.edges {
        polyline(
            painter,
            xf,
            points,
            pal.corridor,
            building.corridor_width.max(2.0) as f32,
            false,
        );
    }
    for shape in &building.rooms {
        room_shape(
            painter,
            xf,
            shape,
            Some(pal.shape("rect").gamma_multiply(0.9)),
            pal.grid,
            4.0,
            false,
        );
    }
}

/// The colour of the `k`-th kind of event.
pub fn event_color(k: usize) -> Color32 {
    const COLORS: [(u8, u8, u8); 8] = [
        (0xb8, 0x3a, 0x3a),
        (0x2f, 0x7d, 0x5b),
        (0x8a, 0x4f, 0xb0),
        (0xc9, 0x7a, 0x1f),
        (0x2b, 0x6c, 0xb0),
        (0x9a, 0x6b, 0x3a),
        (0xb0, 0x4a, 0x86),
        (0x4a, 0x8a, 0x8a),
    ];
    let (r, g, b) = COLORS[k % COLORS.len()];
    Color32::from_rgb(r, g, b)
}

/// The tints of the channels on show, room by room: what the overlays do to the colour of a class.
pub struct Tints {
    heights: Vec<Option<f32>>,
    lights: Vec<Option<f32>>,
    temperatures: Vec<Option<f32>>,
    depths: Vec<Option<f32>>,
}

impl Tints {
    pub fn new(floor: &Floor, overlays: Overlays) -> Self {
        Self {
            heights: channel_t(floor, overlays.height, Channel::Height),
            lights: channel_t(floor, overlays.light, Channel::Light),
            temperatures: channel_t(floor, overlays.temperature, Channel::Temperature),
            depths: channel_t(floor, overlays.water, Channel::Water),
        }
    }

    /// The colour of room `room` whose class has colour `base`.
    pub fn fill(&self, base: Color32, room: usize) -> Color32 {
        let mut fill = base;
        if let Some(Some(t)) = self.heights.get(room) {
            fill = fill.lerp_to_gamma(channel_color(Channel::Height, *t), 0.55);
        }
        if let Some(Some(v)) = self.lights.get(room) {
            // Darker where less light arrives.
            fill = fill.lerp_to_gamma(Color32::from_rgb(0x14, 0x14, 0x1c), 0.75 * (1.0 - v));
        }
        if let Some(Some(t)) = self.temperatures.get(room) {
            fill = fill.lerp_to_gamma(channel_color(Channel::Temperature, *t), 0.6);
        }
        // Only a room with water over its floor is tinted: the scale starts at the dry floor.
        if let Some(Some(t)) = self.depths.get(room).filter(|t| t.is_some_and(|t| t > 0.0)) {
            fill = fill.lerp_to_gamma(channel_color(Channel::Water, *t), 0.35 + 0.5 * t);
        }
        fill
    }
}

/// The position of each room's value on the scale of a channel, 0 to 1; empty when the overlay is off or the floor has no such layer.
fn channel_t(floor: &Floor, on: bool, c: Channel) -> Vec<Option<f32>> {
    let Some(scale) = on.then(|| view::scale(floor, c)).flatten() else {
        return Vec::new();
    };
    view::room_values(floor, c)
        .into_iter()
        .map(|v| v.map(|v| view::position(scale, v) as f32))
        .collect()
}

/// The colour of a channel at `t` on its scale.
pub fn channel_color(c: Channel, t: f32) -> Color32 {
    let [r, g, b] = view::color(c, f64::from(t));
    Color32::from_rgb(r, g, b)
}

/// How much of a node at `hop` rooms from the entrance is shown when the reveal front is at `front` rooms: nothing before the front
/// reaches it, everything when it has passed by one room.
fn reveal_alpha(front: f64, hop: f64) -> f32 {
    (front - hop).clamp(0.0, 1.0) as f32
}

/// Draws the plan with the floor revealed outwards from the entrance: `reveal` runs from 0 (nothing) to 1 (all of it), and each room and
/// corridor fades in when the front reaches its distance in rooms from the entrance.
pub fn draw_floor(
    painter: &Painter,
    xf: &Xf,
    floor: &Floor,
    overlays: Overlays,
    pal: &Palette,
    reveal: f32,
) {
    let hops = (reveal < 1.0)
        .then(|| tapiti_core::generate::room_hops(&floor.nodes, &floor.edges, floor.origin()));
    let front = hops.as_ref().map_or(0.0, |h| {
        let far = h
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0, f64::max);
        f64::from(reveal) * (far + 1.5)
    });
    // How much of a node is shown: 1 when the front has passed it by a whole room.
    let shown = |node: usize| -> f32 {
        match &hops {
            None => 1.0,
            Some(h) => {
                let d = h
                    .get(node)
                    .copied()
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0);
                reveal_alpha(front, d)
            }
        }
    };
    let mut room_node = vec![0usize; floor.rooms.len()];
    for (i, node) in floor.nodes.iter().enumerate() {
        if let Node::Room { room } = node {
            room_node[*room] = i;
        }
    }
    // A stacked floor is drawn one level at a time: levels overlap in plan, and drawn together they cannot be read.
    let view = overlays.level.filter(|_| floor.stack.is_some());
    let on = |room: &tapiti_core::generate::Room| view.is_none_or(|v| room.level == v);
    let node_level = tapiti_core::environment::height::node_levels(floor);
    if overlays.cells {
        grid(painter, xf, &floor.grid_x, &floor.grid_y, pal.grid);
    }
    if overlays.pairs && view.is_none_or(|v| v == 0) {
        for [a, b] in &floor.pairs {
            if let (Some(first), Some(second)) = (floor.rooms.get(*a), floor.rooms.get(*b)) {
                painter.add(Draw::dashed_line(
                    &[xf.pt(first.center), xf.pt(second.center)],
                    Stroke::new(2.0, pal.accent.gamma_multiply(0.6)),
                    8.0,
                    7.0,
                ));
            }
        }
    }
    if overlays.candidates && view.is_none_or(|v| v == 0) {
        for candidate in &floor.candidates {
            polyline(
                painter,
                xf,
                &candidate.pts,
                pal.accent.gamma_multiply(0.45),
                5.0,
                false,
            );
        }
    }
    for edge in &floor.edges {
        if edge.stair {
            continue;
        }
        if view.is_some_and(|v| node_level[edge.u] != v) {
            continue;
        }
        let a = shown(edge.u).max(shown(edge.v));
        if a <= 0.0 {
            continue;
        }
        polyline(
            painter,
            xf,
            &edge.pts,
            pal.corridor.gamma_multiply(a),
            floor.params.width.max(2.0) as f32,
            false,
        );
    }
    let tints = Tints::new(floor, overlays);
    for room in floor.rooms.iter().filter(|r| on(r)) {
        if room.alive {
            let a = shown(room_node[room.id]);
            if a <= 0.0 {
                continue;
            }
            let alpha = if room.leaf { 0.65 } else { 0.92 };
            let fill = tints.fill(pal.shape(room.cls.name()), room.id);
            let outline = if room.role == Role::Plain { 2.5 } else { 5.0 };
            room_shape(
                painter,
                xf,
                &room.shape,
                Some(fill.gamma_multiply(alpha * a)),
                pal.role(room.role).gamma_multiply(a),
                outline,
                false,
            );
            if room.role != Role::Plain && a >= 0.6 {
                role_badge(painter, xf, room, pal);
            }
            let spacing = 10.0 / f64::from(xf.scale.max(0.01));
            let hatches = if a < 0.6 {
                Vec::new()
            } else {
                tapiti_render::hatch(&room.shape, room.cls.name(), spacing)
            };
            for mark in hatches {
                match mark {
                    tapiti_render::HatchMark::Dot(p) => {
                        painter.circle_filled(xf.pt(p), 1.2, pal.hatch);
                    }
                    tapiti_render::HatchMark::Line(a, b) => {
                        painter.line_segment([xf.pt(a), xf.pt(b)], Stroke::new(1.0, pal.hatch));
                    }
                }
            }
        } else {
            room_shape(painter, xf, &room.shape, None, pal.danger, 3.0, true);
        }
    }
    if overlays.ports {
        for room in floor.rooms.iter().filter(|r| on(r)) {
            for port in &room.ports {
                let half = (4.0 * xf.scale).max(1.5);
                painter.rect_filled(
                    Rect::from_center_size(xf.pt(port.p), Vec2::splat(half * 2.0)),
                    0.0,
                    pal.ink.gamma_multiply(0.5),
                );
            }
        }
    }
    for room in floor.rooms.iter().filter(|r| on(r)) {
        let a = shown(room_node[room.id]);
        for port in &room.used {
            let d = dir_of(port.k);
            polyline(
                painter,
                xf,
                &[port.p, [port.p[0] + d[0] * 14.0, port.p[1] + d[1] * 14.0]],
                pal.ink.gamma_multiply(a),
                5.0,
                false,
            );
        }
    }
    for (i, node) in floor.nodes.iter().enumerate() {
        if let Node::Junction { p } = node {
            let a = shown(i);
            painter.circle(
                xf.pt(**p),
                (7.0 * xf.scale).max(2.5),
                pal.ink.gamma_multiply(a),
                Stroke::new(1.5, pal.background.gamma_multiply(a)),
            );
        }
    }
    if overlays.events {
        if let Some(events) = floor.environment.as_ref().and_then(|e| e.events.as_ref()) {
            // A coloured badge with the first letter of the kind at the top right of each room, side by side when several share one.
            let mut slot = vec![0usize; floor.rooms.len()];
            for (k, kind) in events.kinds.iter().enumerate() {
                let color = event_color(k);
                let letter = kind
                    .name
                    .chars()
                    .next()
                    .map_or("?".to_string(), |c| c.to_uppercase().to_string());
                for &i in &kind.nodes {
                    if let Node::Room { room } = &floor.nodes[i] {
                        if !on(&floor.rooms[*room]) {
                            continue;
                        }
                        let r = &floor.rooms[*room];
                        let at = xf.pt([r.bounds.hi[0], r.bounds.lo[1]])
                            + eframe::egui::vec2(-9.0 - 15.0 * slot[*room] as f32, 9.0);
                        slot[*room] += 1;
                        painter.circle_filled(at, 7.0, color);
                        painter.circle_stroke(at, 7.0, eframe::egui::Stroke::new(1.0, pal.ink));
                        painter.text(
                            at,
                            eframe::egui::Align2::CENTER_CENTER,
                            letter.clone(),
                            eframe::egui::FontId::monospace(10.0),
                            ink_on(color),
                        );
                    }
                }
            }
        }
    }
    if overlays.light {
        if let Some(light) = floor.environment.as_ref().and_then(|e| e.light.as_ref()) {
            // A warm dot at the corner of each room with a direct source, a pale one at each opening to the surface (the middle holds the label).
            for (nodes, color) in [
                (&light.torches, Color32::from_rgb(0xf2, 0xb1, 0x2c)),
                (&light.openings, Color32::from_rgb(0xe8, 0xf1, 0xff)),
            ] {
                for &i in nodes {
                    if let Node::Room { room } = &floor.nodes[i] {
                        if !on(&floor.rooms[*room]) {
                            continue;
                        }
                        let at = xf.pt(floor.rooms[*room].bounds.lo) + eframe::egui::vec2(9.0, 9.0);
                        painter.circle_filled(at, 6.0, color);
                        painter.circle_stroke(at, 6.0, eframe::egui::Stroke::new(1.0, pal.ink));
                    }
                }
            }
        }
    }
    if overlays.labels {
        for room in floor.rooms.iter().filter(|room| room.alive && on(room)) {
            let role = if room.role == Role::Plain {
                String::new()
            } else {
                format!(" · {}", role_label(room.role))
            };
            let text = format!("#{} {}{}", room.id, class_label(room.cls.name()), role);
            label(painter, xf, &text, room.center, pal.on_room(), 18.0);
        }
    }
    // The stairs, over the rooms: marks where the level goes up or down.
    for edge in floor.edges.iter().filter(|e| e.stair) {
        if view.is_none_or(|v| node_level[edge.u] == v || node_level[edge.v] == v) {
            let at = xf.pt(edge.pts[0]);
            painter.circle_filled(at, 9.0, pal.accent);
            painter.circle_stroke(at, 9.0, eframe::egui::Stroke::new(1.0, pal.ink));
            painter.text(
                at,
                eframe::egui::Align2::CENTER_CENTER,
                "S",
                eframe::egui::FontId::monospace(11.0),
                pal.on_room(),
            );
        }
    }
}

/// A scale for every channel on show (with its layer on the floor): the ramp from the low end to the high end, with both values, stacked
/// down from the top right of the map.
pub fn scales(
    painter: &Painter,
    rect: Rect,
    text_color: Color32,
    floor: &Floor,
    overlays: Overlays,
) {
    let shown = [
        (overlays.height, Channel::Height),
        (overlays.water, Channel::Water),
        (overlays.temperature, Channel::Temperature),
        (overlays.light, Channel::Light),
    ];
    let font = FontId::proportional(11.0);
    let (w, h) = (150.0, 8.0);
    let mut y = rect.top() + 44.0;
    for (_, c) in shown.into_iter().filter(|(on, _)| *on) {
        let Some((lo, hi)) = view::scale(floor, c) else {
            continue;
        };
        let x0 = rect.right() - w - 14.0;
        painter.text(
            Pos2::new(x0, y),
            Align2::LEFT_BOTTOM,
            c.label(),
            font.clone(),
            text_color,
        );
        let steps = 30;
        for k in 0..steps {
            let t = k as f32 / (steps - 1) as f32;
            let at = x0 + w * k as f32 / steps as f32;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(at, y + 2.0), Vec2::new(w / steps as f32 + 0.5, h)),
                0.0,
                channel_color(c, t),
            );
        }
        painter.text(
            Pos2::new(x0, y + h + 4.0),
            Align2::LEFT_TOP,
            view::format_value(c, lo),
            font.clone(),
            text_color,
        );
        painter.text(
            Pos2::new(x0 + w, y + h + 4.0),
            Align2::RIGHT_TOP,
            view::format_value(c, hi),
            font.clone(),
            text_color,
        );
        y += 46.0;
    }
}

/// The thickness of the rulers on the top and left edges of the plan, and the height of the strip under it, in points.
pub const RULER: f32 = 18.0;
pub const HUD: f32 = 24.0;

/// Where `v` falls on a grid, in cells and fractions of a cell (3.5 is the middle of the fourth cell), or `None` outside the grid.
pub fn cell_coord(grid: &[f64], v: f64) -> Option<f64> {
    let (first, last) = (*grid.first()?, *grid.last()?);
    if v < first || v > last {
        return None;
    }
    let i = grid.windows(2).position(|w| v >= w[0] && v <= w[1])?;
    Some(i as f64 + (v - grid[i]) / (grid[i + 1] - grid[i]).max(1e-9))
}

/// Rulers along the top and left edges of `rect`, numbered in grid cells like a drafting sheet. The ticks follow the pan and the zoom;
/// `xf` is the transform of the plan, whose rect leaves room for the rulers.
pub fn rulers(painter: &Painter, rect: Rect, xf: &Xf, floor: &Floor, pal: &Palette) {
    let c = crate::theme::colors(pal.dark);
    let inner = xf.rect();
    let top = Rect::from_min_max(
        Pos2::new(inner.left(), rect.top()),
        Pos2::new(rect.right(), inner.top()),
    );
    let left = Rect::from_min_max(
        Pos2::new(rect.left(), inner.top()),
        Pos2::new(inner.left(), rect.bottom()),
    );
    let corner = Rect::from_min_max(rect.min, inner.min);
    for band in [top, left, corner] {
        painter.rect_filled(band, 0.0, c.bar);
    }
    let hair = Stroke::new(1.0, c.border_variant);
    painter.hline(rect.x_range(), inner.top(), hair);
    painter.vline(inner.left(), rect.y_range(), hair);
    let font = FontId::new(10.0, crate::theme::medium());
    let tick = Stroke::new(1.0, c.text_muted);
    let (Some(&y0), Some(&x0)) = (floor.grid_y.first(), floor.grid_x.first()) else {
        return;
    };
    let cells = |grid: &[f64]| grid.len().saturating_sub(1);
    for (i, &x) in floor.grid_x.iter().enumerate() {
        let sx = xf.pt([x, y0]).x;
        if sx < inner.left() || sx > rect.right() {
            continue;
        }
        painter.vline(sx, (top.bottom() - 7.0)..=top.bottom(), tick);
        let wide = floor
            .grid_x
            .get(i + 1)
            .is_some_and(|n| (n - x) as f32 * xf.scale >= 20.0);
        if i < cells(&floor.grid_x) && wide {
            painter.text(
                Pos2::new(sx + 3.0, top.center().y - 1.0),
                Align2::LEFT_CENTER,
                i.to_string(),
                font.clone(),
                c.text_muted,
            );
        }
    }
    for (j, &y) in floor.grid_y.iter().enumerate() {
        let sy = xf.pt([x0, y]).y;
        if sy < inner.top() || sy > rect.bottom() {
            continue;
        }
        painter.hline((left.right() - 7.0)..=left.right(), sy, tick);
        let tall = floor
            .grid_y
            .get(j + 1)
            .is_some_and(|n| (n - y) as f32 * xf.scale >= 16.0);
        if j < cells(&floor.grid_y) && tall {
            painter.text(
                Pos2::new(left.center().x, sy + 9.0),
                Align2::CENTER_CENTER,
                j.to_string(),
                font.clone(),
                c.text_muted,
            );
        }
    }
}

/// The room picked on the plan: a thick amber outline round it, with a thin dark one inside so it reads on any fill.
pub fn selection(painter: &Painter, xf: &Xf, shape: &Shape, pal: &Palette) {
    room_shape(painter, xf, shape, None, pal.accent, 7.0, false);
    room_shape(painter, xf, shape, None, pal.edge, 1.5, false);
}

/// The strip under the plan: the pointer in cells on the left, what is under it, and on the right what is picked.
pub fn hud(painter: &Painter, rect: Rect, pal: &Palette, pointer: &str, under: &str, picked: &str) {
    let c = crate::theme::colors(pal.dark);
    let strip = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - HUD), rect.max);
    painter.rect_filled(strip, 0.0, c.bar);
    painter.hline(
        strip.x_range(),
        strip.top(),
        Stroke::new(1.0, c.border_variant),
    );
    let font = FontId::new(11.0, crate::theme::medium());
    let mid = strip.center().y;
    painter.text(
        Pos2::new(strip.left() + 12.0, mid),
        Align2::LEFT_CENTER,
        pointer,
        font.clone(),
        c.text,
    );
    // The room that is picked has the right of the strip; what is under the pointer takes what is left of it, cut off, and is not
    // said twice when it is the picked room.
    let picked_w = painter
        .layout_no_wrap(picked.to_string(), font.clone(), c.accent)
        .size()
        .x;
    let room = Rect::from_min_max(
        Pos2::new(strip.left() + 150.0, strip.top()),
        Pos2::new(
            (strip.right() - picked_w - 28.0).max(strip.left() + 150.0),
            strip.bottom(),
        ),
    );
    if under != picked {
        painter.with_clip_rect(room).text(
            Pos2::new(room.left(), mid),
            Align2::LEFT_CENTER,
            under,
            font.clone(),
            c.text_muted,
        );
    }
    painter.text(
        Pos2::new(strip.right() - 12.0, mid),
        Align2::RIGHT_CENTER,
        picked,
        font,
        c.accent,
    );
}

/// What a room is, in a line: its role, its shape, its size in cells and its distance from the entrance in rooms.
pub fn describe(floor: &Floor, room: &tapiti_core::generate::Room) -> String {
    let role = match room.role {
        Role::Entrance => "entrance",
        Role::Exit => "exit",
        Role::Plain if room.leaf => "leaf chamber",
        Role::Plain => "room",
    };
    let (w, h) = match &room.shape {
        Shape::Disc { r, .. } => (2.0 * r, 2.0 * r),
        Shape::Poly { .. } => (
            room.bounds.hi[0] - room.bounds.lo[0],
            room.bounds.hi[1] - room.bounds.lo[1],
        ),
    };
    let size = format!(
        "{:.1} × {:.1} cells",
        w / floor.params.cell_w.max(1.0),
        h / floor.params.cell_h.max(1.0)
    );
    let hops = tapiti_core::generate::room_hops(&floor.nodes, &floor.edges, floor.origin());
    let at = floor
        .nodes
        .iter()
        .position(|n| matches!(n, Node::Room { room: r } if *r == room.id))
        .and_then(|i| hops.get(i).copied())
        .filter(|h| h.is_finite())
        .map_or(String::new(), |h| format!(" · {h:.0} steps"));
    format!(
        "#{} {role} · {} · {size}{at}",
        room.id,
        class_label(room.cls.name())
    )
}

/// Small key over the map: what the fills and outlines mean, for the shapes the floor has.
pub fn legend(painter: &Painter, rect: Rect, pal: &Palette, text_color: Color32, present: &[&str]) {
    let c = crate::theme::colors(pal.dark);
    let font = FontId::new(11.0, crate::theme::medium());
    let row = 17.0;
    let items: Vec<(Color32, String)> = SHAPE_CLASSES
        .iter()
        .filter(|class| present.contains(class))
        .map(|class| (pal.shape(class), class_label(class).to_string()))
        .collect();
    let height = row * (items.len() as f32 + 3.0) + 12.0;
    let panel = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.bottom() - height - 10.0),
        Vec2::new(176.0, height),
    );
    painter.rect_filled(panel, 0.0, c.panel.gamma_multiply(0.94));
    painter.rect_stroke(
        panel,
        0.0,
        Stroke::new(1.0, c.border_variant),
        eframe::egui::StrokeKind::Inside,
    );
    let mut y = panel.top() + 14.0;
    let entry = |y: f32, draw: &dyn Fn(Rect), name: &str| {
        draw(Rect::from_min_size(
            Pos2::new(panel.left() + 10.0, y - 6.0),
            Vec2::splat(12.0),
        ));
        painter.text(
            Pos2::new(panel.left() + 30.0, y),
            Align2::LEFT_CENTER,
            name,
            font.clone(),
            text_color,
        );
    };
    for (color, name) in items {
        entry(
            y,
            &|r| {
                painter.rect_filled(r, 0.0, color);
            },
            &name,
        );
        y += row;
    }
    entry(
        y,
        &|r| {
            painter.rect_stroke(
                r.shrink(1.0),
                0.0,
                Stroke::new(2.5, pal.success),
                eframe::egui::StrokeKind::Inside,
            );
        },
        "entrance",
    );
    y += row;
    entry(
        y,
        &|r| {
            painter.rect_stroke(
                r.shrink(1.0),
                0.0,
                Stroke::new(2.5, pal.exit),
                eframe::egui::StrokeKind::Inside,
            );
        },
        "exit",
    );
    y += row;
    entry(
        y,
        &|r| {
            painter.circle_filled(r.center(), 4.0, pal.ink);
        },
        "junction",
    );
}

#[cfg(test)]
mod tests {
    use super::{cell_coord, describe, ink_on, reveal_alpha, triangulate, Palette};
    use eframe::egui::Color32;
    use tapiti_core::generate::Role;
    use tapiti_core::params::{rich_of_size, Size};
    use tapiti_core::stage::stage;

    #[test]
    fn cells_are_counted_from_the_grid() {
        let grid = [0.0, 10.0, 30.0];
        assert_eq!(cell_coord(&grid, 5.0), Some(0.5));
        assert_eq!(cell_coord(&grid, 20.0), Some(1.5));
        assert_eq!(cell_coord(&grid, 30.0), Some(2.0));
        assert_eq!(cell_coord(&grid, -0.1), None);
        assert_eq!(cell_coord(&grid, 30.1), None);
        assert_eq!(cell_coord(&[], 1.0), None);
    }

    #[test]
    fn the_exit_is_not_amber_and_marks_read_on_their_fill() {
        for dark in [true, false] {
            let pal = Palette::new(dark);
            assert_ne!(pal.exit, pal.accent, "the exit and what is picked differ");
            assert_ne!(pal.exit, pal.success);
        }
        // Dark ink on amber and on the light room washes, white on the dark colours.
        assert_eq!(
            ink_on(Color32::from_rgb(0xff, 0xb4, 0x54)),
            Color32::from_rgb(0x0e, 0x10, 0x12)
        );
        assert_eq!(ink_on(Color32::from_rgb(0x20, 0x30, 0x80)), Color32::WHITE);
    }

    #[test]
    fn the_reveal_runs_outwards_from_the_entrance() {
        // Nothing is shown at the start, everything at the end, and a nearer room is never less shown than a farther one.
        assert_eq!(reveal_alpha(0.0, 0.0), 0.0);
        assert_eq!(reveal_alpha(10.0, 8.0), 1.0);
        for front in [0.5, 1.5, 3.0, 6.5] {
            let shown: Vec<f32> = (0..9)
                .map(|hop| reveal_alpha(front, f64::from(hop)))
                .collect();
            assert!(shown.windows(2).all(|w| w[0] >= w[1]), "{shown:?}");
            assert!(shown.iter().all(|a| (0.0..=1.0).contains(a)));
        }
    }

    #[test]
    fn a_room_is_described_by_role_shape_size_and_distance() {
        let floor = stage(279, &rich_of_size(Size::Medium), 8).d;
        let room_of = |role: Role| {
            floor
                .rooms
                .iter()
                .find(|r| r.alive && r.role == role)
                .unwrap()
        };
        let entrance = describe(&floor, room_of(Role::Entrance));
        assert!(
            entrance.contains("entrance") && entrance.contains(" 0 steps"),
            "{entrance}"
        );
        let exit = describe(&floor, room_of(Role::Exit));
        assert!(exit.contains("exit") && exit.contains("cells"), "{exit}");
        assert!(!exit.contains(" 0 steps"), "{exit}");
    }

    fn area(pts: &[[f64; 2]]) -> f64 {
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            .abs()
            / 2.0
    }

    fn tri_area(pts: &[[f64; 2]], t: [usize; 3]) -> f64 {
        area(&[pts[t[0]], pts[t[1]], pts[t[2]]])
    }

    #[test]
    fn concave_l_and_star_are_covered_exactly() {
        let l = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 2.0],
            [2.0, 2.0],
            [2.0, 4.0],
            [0.0, 4.0],
        ];
        let star = [
            [0.0, -4.0],
            [1.0, -1.0],
            [4.0, 0.0],
            [1.0, 1.0],
            [0.0, 4.0],
            [-1.0, 1.0],
            [-4.0, 0.0],
            [-1.0, -1.0],
        ];
        for pts in [&l[..], &star[..], &[l[5], l[4], l[3], l[2], l[1], l[0]][..]] {
            let tris = triangulate(pts);
            assert_eq!(tris.len(), pts.len() - 2);
            let sum: f64 = tris.iter().map(|t| tri_area(pts, *t)).sum();
            assert!((sum - area(pts)).abs() < 1e-9, "{sum} vs {}", area(pts));
        }
    }
}
