//! SVG rendering of a generated floor. The output is a plain `String`, so the same floor can be inlined in a page, saved as a file
//! (README, docs, social cards) or compared in a test: the same seed and parameters always give the same bytes.
//!
//! Shapes carry CSS classes (`room c-rect`, `corridor`, …) and a `--d` custom property with the room's distance in rooms from the entrance,
//! so a page can colour them with its own tokens and reveal the floor outwards from the entrance with a staggered animation.

use std::fmt::Write;
use tapiti_core::generate::{room_hops, Floor, Node, Role};
use tapiti_core::geom::{dir_of, Shape, V};

mod hatch;
pub mod mesh;
pub mod raster;
pub mod view;
pub mod walk;
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

/// What a floor is drawn for besides the style: the level of a stacked floor (levels overlap in plan, so one is drawn) and a channel of
/// the environment layers to tint the rooms by, with a scale under the floor. The default draws everything, in the colours of the
/// classes.
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    /// The level to draw, counted from 0; `None` draws them all, and a floor with one level ignores it.
    pub level: Option<usize>,
    pub channel: Option<view::Channel>,
}

/// Renders `d` as a standalone `<svg>` element.
pub fn floor_svg(d: &Floor, style: &Style) -> String {
    floor_svg_view(d, style, &View::default())
}

/// Like [`floor_svg`], for one level and tinted by one channel (see [`View`]).
pub fn floor_svg_view(d: &Floor, style: &Style, shown: &View) -> String {
    let level_of = tapiti_core::environment::height::node_levels(d);
    let on_level = |node: usize| {
        shown
            .level
            .filter(|_| d.stack.is_some())
            .is_none_or(|l| level_of[node] == l)
    };
    let on_room = |r: &tapiti_core::generate::Room| {
        shown
            .level
            .filter(|_| d.stack.is_some())
            .is_none_or(|l| r.level == l)
    };
    let tint = shown
        .channel
        .and_then(|c| view::scale(d, c).map(|s| (c, s, view::room_values(d, c))));
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
    for e in d.edges.iter().filter(|e| !e.stair && on_level(e.u)) {
        let _ = write!(
            out,
            r#"<path class="corridor" style="--d:{}" fill="none" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" d="{}"/>"#,
            hop_of_node(e.u).max(hop_of_node(e.v)),
            n(width),
            points(&e.pts)
        );
    }
    for r in d.rooms.iter().filter(|r| r.alive && on_room(r)) {
        let fill = tint
            .as_ref()
            .and_then(|(c, s, values)| values[r.id].map(|v| view::color(*c, view::position(*s, v))))
            .map_or(String::new(), |[red, green, blue]| {
                format!(";fill:rgb({red},{green},{blue})")
            });
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
                    r#"<circle class="{class}" style="--d:{d_attr}{fill}" fill-opacity="{opacity}" cx="{}" cy="{}" r="{}"/>"#,
                    n(c[0]),
                    n(c[1]),
                    n(*radius)
                );
            }
            Shape::Poly { pts } => {
                let _ = write!(
                    out,
                    r#"<path class="{class}" style="--d:{d_attr}{fill}" fill-opacity="{opacity}" d="{}Z"/>"#,
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
    for r in d.rooms.iter().filter(|r| r.alive && on_room(r)) {
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
    for node in d
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| on_level(*i))
        .map(|(_, n)| n)
    {
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
        for r in d.rooms.iter().filter(|r| r.alive && on_room(r)) {
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
    if let Some((c, (lo, hi), _)) = tint {
        // A scale under the floor: the ramp of the channel from its low end to its high end, with both values.
        let [vx, vy, vw, vh] = view_box(d, style);
        let (bar_w, bar_h, y) = (vw * 0.3, vh * 0.012, vy + vh - vh * 0.035);
        let _ = write!(
            out,
            r#"<g class="channel-scale" aria-label="{}"><defs><linearGradient id="channel-ramp">"#,
            c.label()
        );
        for k in 0..=4 {
            let [red, green, blue] = view::color(c, f64::from(k) / 4.0);
            let _ = write!(
                out,
                r#"<stop offset="{}" stop-color="rgb({red},{green},{blue})"/>"#,
                f64::from(k) / 4.0
            );
        }
        let _ = write!(
            out,
            r#"</linearGradient></defs><rect x="{}" y="{}" width="{}" height="{}" fill="url(#channel-ramp)"/><text class="label" font-size="{}" x="{}" y="{}">{}</text><text class="label" font-size="{}" text-anchor="end" x="{}" y="{}">{}</text></g>"#,
            n(vx + vw * 0.02),
            n(y),
            n(bar_w),
            n(bar_h),
            n(bar_h * 1.6),
            n(vx + vw * 0.02),
            n(y - bar_h * 0.4),
            view::format_value(c, lo),
            n(bar_h * 1.6),
            n(vx + vw * 0.02 + bar_w),
            n(y - bar_h * 0.4),
            view::format_value(c, hi)
        );
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

    #[test]
    fn a_view_picks_a_level_and_tints_by_a_channel() {
        use tapiti_core::params::{rich_of_size, Layout, Size};
        let mut p = rich_of_size(Size::Medium);
        p.layout = Layout::Growth;
        p.levels.count = 2;
        p.environment.height.enabled = true;
        p.environment.light.enabled = true;
        let f = tapiti_core::generate::generate(3, &p);
        let style = Style::default();
        assert_eq!(
            floor_svg(&f, &style),
            floor_svg_view(&f, &style, &View::default())
        );
        let count = |svg: &str| svg.matches("class=\"room ").count();
        let (a, b, all) = (
            floor_svg_view(
                &f,
                &style,
                &View {
                    level: Some(0),
                    channel: None,
                },
            ),
            floor_svg_view(
                &f,
                &style,
                &View {
                    level: Some(1),
                    channel: None,
                },
            ),
            floor_svg(&f, &style),
        );
        let live = |l: usize| f.rooms.iter().filter(|r| r.alive && r.level == l).count();
        assert_eq!(
            (count(&a), count(&b), count(&all)),
            (live(0), live(1), live(0) + live(1))
        );
        let tinted = floor_svg_view(
            &f,
            &style,
            &View {
                level: None,
                channel: Some(view::Channel::Height),
            },
        );
        assert!(tinted.contains(";fill:rgb(") && tinted.contains("channel-scale"));
        // No layer for the channel: the floor is drawn as it is.
        let none = floor_svg_view(
            &f,
            &style,
            &View {
                level: None,
                channel: Some(view::Channel::Water),
            },
        );
        assert_eq!(none, all);
    }
}
