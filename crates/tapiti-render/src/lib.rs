//! SVG rendering of a generated floor. The output is a plain `String`, so the same floor can be inlined in a page, saved as a file
//! (README, docs, social cards) or compared in a test: the same seed and parameters always give the same bytes.
//!
//! Shapes carry CSS classes (`room c-rect`, `corridor`, …) and a `--d` custom property with the room's distance in rooms from the entrance,
//! so a page can colour them with its own tokens and reveal the floor outwards from the entrance with a staggered animation.

use std::fmt::Write;
use tapiti_core::generate::{room_hops, Floor, Node, Role};
use tapiti_core::geom::{dir_of, Shape, V};

mod hatch;
pub use hatch::{hatch, HatchMark};

/// How colours reach the SVG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Classes only: the surrounding page styles the shapes (see [`CLASS_NAMES`]). Meant for inlining.
    Inherit,
    /// A `<style>` for a dark background, so the file renders on its own.
    Dark,
    /// A `<style>` for a light background, so the file renders on its own.
    Light,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub theme: Theme,
    /// Draw the grid lines of the scaffold.
    pub grid: bool,
    /// Write `#id` on each room.
    pub labels: bool,
    /// Margin around the floor, in world units, as a fraction of the larger side.
    pub margin: f64,
    /// Accessible name of the figure; defaults to an English description with the seed.
    pub label: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            theme: Theme::Inherit,
            grid: false,
            labels: false,
            margin: 0.03,
            label: None,
        }
    }
}

/// Every class the renderer can emit, for pages that style an inlined SVG.
pub const CLASS_NAMES: &[&str] = &[
    "tapiti-floor",
    "grid",
    "corridor",
    "room",
    "leaf",
    "c-rect",
    "c-disc",
    "c-oct",
    "c-L",
    "c-star",
    "r-entrance",
    "r-exit",
    "r-plain",
    "junction",
    "door",
    "label",
    "hatch",
];

fn n(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
}

fn role_class(role: Role) -> &'static str {
    match role {
        Role::Entrance => "r-entrance",
        Role::Exit => "r-exit",
        Role::Plain => "r-plain",
    }
}

fn class_class(name: &str) -> &'static str {
    match name {
        "rect" => "c-rect",
        "disc" => "c-disc",
        "oct" => "c-oct",
        "L" => "c-L",
        "star" => "c-star",
        _ => "c-rect",
    }
}

fn points(pts: &[V]) -> String {
    let mut out = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(
            out,
            "{}{} {}",
            if i == 0 { "M" } else { "L" },
            n(p[0]),
            n(p[1])
        );
    }
    out
}

fn theme_css(theme: Theme) -> &'static str {
    match theme {
        Theme::Inherit => "",
        Theme::Dark => concat!(
            ".tapiti-floor{background:#1c1916}.grid{stroke:#484138}.corridor{stroke:#857d70}",
            ".room{stroke-width:2}.c-rect{fill:#719ab0}.c-disc{fill:#94b49d}.c-oct{fill:#cdbbd2}.c-L{fill:#bb8d58}.c-star{fill:#e8d07d}",
            ".r-plain{stroke:#49463f}.r-entrance{stroke:#8eaf91}.r-exit{stroke:#ff8fab}",
            ".junction{fill:#f4f0e5;stroke:#1c1916}.door{stroke:#f4f0e5}.label{fill:#252421;stroke:#fbf8ee;stroke-width:3;paint-order:stroke;stroke-linejoin:round;font-family:monospace}.hatch{color:#49463f}"
        ),
        Theme::Light => concat!(
            ".tapiti-floor{background:#fbf8ee}.grid{stroke:#dce1df}.corridor{stroke:#857d70}",
            ".room{stroke-width:2}.c-rect{fill:#719ab0}.c-disc{fill:#94b49d}.c-oct{fill:#cdbbd2}.c-L{fill:#bb8d58}.c-star{fill:#e8d07d}",
            ".r-plain{stroke:#49463f}.r-entrance{stroke:#31533d}.r-exit{stroke:#a51f48}",
            ".junction{fill:#302e2b;stroke:#fbf8ee}.door{stroke:#302e2b}.label{fill:#252421;stroke:#fbf8ee;stroke-width:3;paint-order:stroke;stroke-linejoin:round;font-family:monospace}.hatch{color:#49463f}"
        ),
    }
}

/// The `viewBox` of the SVG of `d`, as `[x, y, width, height]` in world units (the floor plus the margin). Pages that draw over the SVG
/// (a minimap, a position marker) use it to turn world coordinates into percentages.
pub fn view_box(d: &Floor, style: &Style) -> [f64; 4] {
    let (x0, x1) = (
        d.grid_x.first().copied().unwrap_or(0.0),
        d.grid_x.last().copied().unwrap_or(1.0),
    );
    let (y0, y1) = (
        d.grid_y.first().copied().unwrap_or(0.0),
        d.grid_y.last().copied().unwrap_or(1.0),
    );
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let m = w.max(h) * style.margin;
    [x0 - m, y0 - m, w + 2.0 * m, h + 2.0 * m]
}

/// Renders `d` as a standalone `<svg>` element.
pub fn floor_svg(d: &Floor, style: &Style) -> String {
    let (x0, x1) = (
        d.grid_x.first().copied().unwrap_or(0.0),
        d.grid_x.last().copied().unwrap_or(1.0),
    );
    let (y0, y1) = (
        d.grid_y.first().copied().unwrap_or(0.0),
        d.grid_y.last().copied().unwrap_or(1.0),
    );
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let hops = room_hops(&d.nodes, &d.edges, d.origin());
    let hop_of_node = |node: usize| -> usize {
        let v = hops.get(node).copied().unwrap_or(0.0);
        if v.is_finite() {
            v as usize
        } else {
            0
        }
    };

    let mut out = String::with_capacity(32 * 1024);
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="tapiti-floor" viewBox="{} {} {} {}" role="img" aria-label="{}">"#,
        n(view_box(d, style)[0]),
        n(view_box(d, style)[1]),
        n(view_box(d, style)[2]),
        n(view_box(d, style)[3]),
        style
            .label
            .clone()
            .unwrap_or_else(|| format!("Tapiti floor, seed {}", d.seed))
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
    );
    let css = theme_css(style.theme);
    if !css.is_empty() {
        let _ = write!(out, "<style>{css}</style>");
    }
    if style.grid {
        out.push_str(r#"<path class="grid" fill="none" stroke-width="2" d=""#);
        for &x in &d.grid_x {
            let _ = write!(out, "M{} {}L{} {}", n(x), n(y0), n(x), n(y1));
        }
        for &y in &d.grid_y {
            let _ = write!(out, "M{} {}L{} {}", n(x0), n(y), n(x1), n(y));
        }
        out.push_str(r#""/>"#);
    }
    let width = d.params.width.max(2.0);
    for e in &d.edges {
        let _ = write!(
            out,
            r#"<path class="corridor" style="--d:{}" fill="none" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" d="{}"/>"#,
            hop_of_node(e.u).max(hop_of_node(e.v)),
            n(width),
            points(&e.pts)
        );
    }
    for r in d.rooms.iter().filter(|r| r.alive) {
        let class = format!(
            "room {} {}{}",
            class_class(r.cls.name()),
            role_class(r.role),
            if r.leaf { " leaf" } else { "" }
        );
        let d_attr = hop_of_node(
            d.nodes
                .iter()
                .position(|nd| matches!(nd, Node::Room { room } if *room == r.id))
                .unwrap_or(0),
        );
        let opacity = if r.leaf { "0.65" } else { "0.92" };
        match &r.shape {
            Shape::Disc { c, r: radius } => {
                let _ = write!(
                    out,
                    r#"<circle class="{class}" style="--d:{d_attr}" fill-opacity="{opacity}" cx="{}" cy="{}" r="{}"/>"#,
                    n(c[0]),
                    n(c[1]),
                    n(*radius)
                );
            }
            Shape::Poly { pts } => {
                let _ = write!(
                    out,
                    r#"<path class="{class}" style="--d:{d_attr}" fill-opacity="{opacity}" d="{}Z"/>"#,
                    points(pts)
                );
            }
        }
        out.push_str(r#"<g class="hatch" aria-hidden="true">"#);
        for mark in hatch(&r.shape, r.cls.name(), 10.0) {
            match mark {
                HatchMark::Dot(p) => {
                    let _ = write!(
                        out,
                        r#"<circle fill="currentColor" cx="{}" cy="{}" r="1.2"/>"#,
                        n(p[0]),
                        n(p[1])
                    );
                }
                HatchMark::Line(a, b) => {
                    let _ = write!(
                        out,
                        r#"<path fill="none" stroke="currentColor" stroke-width="1" d="M{} {}L{} {}"/>"#,
                        n(a[0]),
                        n(a[1]),
                        n(b[0]),
                        n(b[1])
                    );
                }
            }
        }
        out.push_str("</g>");
    }
    for r in d.rooms.iter().filter(|r| r.alive) {
        for port in &r.used {
            let k = dir_of(port.k);
            let _ = write!(
                out,
                r#"<path class="door" fill="none" stroke-width="5" d="M{} {}L{} {}"/>"#,
                n(port.p[0]),
                n(port.p[1]),
                n(port.p[0] + k[0] * 14.0),
                n(port.p[1] + k[1] * 14.0)
            );
        }
    }
    for node in &d.nodes {
        if let Node::Junction { p } = node {
            let _ = write!(
                out,
                r#"<circle class="junction" stroke-width="2" cx="{}" cy="{}" r="7"/>"#,
                n(p[0]),
                n(p[1])
            );
        }
    }
    if style.labels {
        let size = (w.max(h) / 60.0).max(10.0);
        for r in d.rooms.iter().filter(|r| r.alive) {
            let _ = write!(
                out,
                r#"<text class="label" text-anchor="middle" dominant-baseline="central" font-size="{}" x="{}" y="{}">#{}</text>"#,
                n(size),
                n(r.center[0]),
                n(r.center[1]),
                r.id
            );
        }
    }
    out.push_str("</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{
        generate::generate,
        params::{rich_of_size, Size},
    };

    /// The floor of `seed` at the medium size with every option on.
    fn svg(seed: u32, style: &Style) -> String {
        floor_svg(&generate(seed, &rich_of_size(Size::Medium)), style)
    }

    #[test]
    fn the_view_box_is_the_one_the_svg_declares() {
        let d = generate(5, &rich_of_size(Size::Medium));
        let [x, y, w, h] = view_box(&d, &Style::default());
        let svg = floor_svg(&d, &Style::default());
        assert!(
            svg.contains(&format!("viewBox=\"{} {} {} {}\"", n(x), n(y), n(w), n(h))),
            "{svg:.200}"
        );
    }

    #[test]
    fn same_seed_gives_the_same_bytes() {
        let style = Style {
            theme: Theme::Dark,
            grid: true,
            labels: true,
            ..Style::default()
        };
        assert_eq!(svg(7, &style), svg(7, &style));
        assert_ne!(svg(7, &style), svg(8, &style));
        let named = Style {
            label: Some("A \"floor\"".into()),
            ..Style::default()
        };
        assert!(svg(7, &named).contains("aria-label=\"A &quot;floor&quot;\""));
    }

    #[test]
    fn draws_rooms_corridors_and_a_viewbox() {
        let s = svg(3, &Style::default());
        assert!(s.starts_with("<svg ") && s.ends_with("</svg>"));
        assert!(s.contains("viewBox=\""));
        assert!(s.matches("class=\"room ").count() >= 5, "rooms");
        assert!(s.contains("class=\"corridor"), "corridors");
        assert!(
            s.contains("r-entrance") && s.contains("r-exit"),
            "entrance and exit"
        );
        assert!(
            !s.contains("<style>"),
            "inherit theme leaves colours to the page"
        );
    }

    #[test]
    fn standalone_themes_embed_their_colours() {
        assert!(svg(
            3,
            &Style {
                theme: Theme::Dark,
                ..Style::default()
            }
        )
        .contains("<style>"));
        assert!(svg(
            3,
            &Style {
                theme: Theme::Light,
                ..Style::default()
            }
        )
        .contains("#fbf8ee"));
        let s = svg(3, &Style::default());
        assert!(s.contains("class=\"hatch\""));
        assert!(
            !s.contains("url(#"),
            "inline floors must not share pattern ids"
        );
    }

    #[test]
    fn every_emitted_class_is_documented() {
        let s = svg(
            11,
            &Style {
                grid: true,
                labels: true,
                ..Style::default()
            },
        );
        for part in s.split("class=\"").skip(1) {
            let classes = part.split('"').next().unwrap();
            for c in classes.split(' ') {
                assert!(CLASS_NAMES.contains(&c), "undocumented class {c}");
            }
        }
    }
}
