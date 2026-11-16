//! Floors drawn by the generator itself and inlined in pages as SVG: the specimens of the landing page. Every figure is a real stage of
//! `tapiti-core` (the first valid seed from 1 for its parameters), so a page never shows a floor the generator cannot make.

use std::sync::OnceLock;
use tapiti_core::{
    generate::Floor,
    metrics::{Metrics, metrics_of},
    params::{Layout, Params, Size, TopologyProfile, params_of_size, rich_of_size},
    stage::stage,
};
use tapiti_render::{
    Style, View, floor_svg_view,
    view::{self, Channel},
};
use topcoat::{
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

/// Markup produced by this repository's own code (generator output, repository docs), never from request data, so it is rendered unescaped.
pub struct TrustedHtml(pub String);

impl NodeViewParts for TrustedHtml {
    fn into_view_parts(self, _cx: &Cx, parts: &mut PartsWriter<'_>) {
        parts.push_string_unescaped(self.0);
    }
}

/// The floor of the page header: the medium size with every option on, first draw valid, a main path of ten rooms through all five shapes,
/// one junction and a leaf chamber.
pub const HERO_SEED: u32 = 279;
const HERO_SIZE: Size = Size::Medium;

/// How many seeds a figure may skip to find a floor that passes every check.
const SEARCH: u32 = 400;

/// One drawn floor and what a caption may say about it.
pub struct Figure {
    pub svg: TrustedHtml,
    pub seed: u32,
    pub rooms: usize,
    pub valid: bool,
    pub metrics: Metrics,
    /// For a layer: its name and the range of its values, as text.
    pub range: Option<Range>,
}

pub struct Range {
    pub label: &'static str,
    pub low: String,
    pub high: String,
    /// The colour ramp as a CSS gradient.
    pub gradient: String,
}

/// The first valid stage for `params` from `from` on (or the last one tried).
fn first_valid(params: &Params, from: u32) -> (u32, Floor, bool) {
    let mut last = None;
    for seed in from..from + SEARCH {
        let staged = stage(seed, params, 8);
        let ok = staged.violations.is_empty();
        last = Some((seed, staged.d, ok));
        if ok {
            break;
        }
    }
    last.expect("SEARCH is not zero")
}

fn figure(
    params: &Params,
    from: u32,
    label: String,
    channel: Option<Channel>,
    pin: bool,
) -> Figure {
    let (seed, floor) = if pin {
        (from, stage(from, params, 8).d)
    } else {
        let (seed, floor, _) = first_valid(params, from);
        (seed, floor)
    };
    let valid = tapiti_core::validate::validate(&floor).is_empty();
    let metrics = metrics_of(&floor);
    let svg = floor_svg_view(
        &floor,
        &Style {
            label: Some(label),
            ..Style::default()
        },
        &View {
            level: None,
            channel,
        },
    );
    let range = channel.and_then(|c| {
        view::scale(&floor, c).map(|(low, high)| Range {
            label: c.label(),
            low: view::format_value(c, low),
            high: view::format_value(c, high),
            gradient: {
                let stops: Vec<String> = [0.0, 0.5, 1.0]
                    .iter()
                    .map(|&t| {
                        let [r, g, b] = view::color(c, t);
                        format!("rgb({r} {g} {b})")
                    })
                    .collect();
                format!("linear-gradient(90deg, {})", stops.join(", "))
            },
        })
    });
    Figure {
        svg: TrustedHtml(svg),
        seed,
        rooms: metrics.rooms + metrics.leaf_chambers,
        valid,
        metrics,
        range,
    }
}

/// The three layouts, the eight intents and the five layers, drawn once.
pub struct Gallery {
    pub hero: Figure,
    pub layouts: Vec<(Layout, Figure)>,
    pub intents: Vec<(TopologyProfile, &'static str, Figure)>,
    pub layers: Vec<(Channel, Figure)>,
}

/// The names the apps give the intents (`tapiti-schema` holds the labels of the forms).
///
/// These four change the floor that comes out (they plan their own rooms or ask for a second way to the exit), so the page draws them.
pub const SHAPING: [(TopologyProfile, &str); 4] = [
    (TopologyProfile::RedundantExit, "Redundant exit"),
    (TopologyProfile::SpineWithDetours, "Spine with detours"),
    (TopologyProfile::Ring, "Ring"),
    (TopologyProfile::HubAndWings, "Hub and wings"),
];

/// These four keep the layout as it is and require a property of the result, which the validator checks: drawn, they would show the
/// same floor, so the page lists them.
pub const CHECKED: [(TopologyProfile, &str); 4] = [
    (TopologyProfile::Classic, "Classic"),
    (TopologyProfile::Backbone, "Backbone"),
    (TopologyProfile::Hub, "Hub"),
    (TopologyProfile::LongLoop, "Long loop"),
];

pub const LAYOUTS: [(Layout, &str); 3] = [
    (Layout::Grid, "Grid"),
    (Layout::Growth, "Growth"),
    (Layout::Bsp, "Binary partition"),
];

fn layer_params() -> Params {
    let mut p = params_of_size(Size::Medium);
    p.entrance = true;
    p.exit_room = true;
    let env = &mut p.environment;
    env.height.enabled = true;
    env.water.enabled = true;
    env.climate.enabled = true;
    env.climate.season = 1.0;
    env.light.enabled = true;
    env.light.torches = 3;
    env.light.time = 0.0;
    p
}

pub fn intent_params(profile: TopologyProfile) -> Params {
    let mut p = params_of_size(Size::Medium);
    // Growth places rooms freely, so what differs between the intents is how they connect, not where a grid put the rooms.
    p.layout = Layout::Growth;
    p.apply_intent(profile);
    p
}

pub fn layout_params(layout: Layout) -> Params {
    let mut p = params_of_size(Size::Medium);
    p.layout = layout;
    p
}

pub fn gallery() -> &'static Gallery {
    static GALLERY: OnceLock<Gallery> = OnceLock::new();
    GALLERY.get_or_init(|| {
        let label = |what: &str, seed: u32| format!("A generated floor: {what}, seed {seed}.");
        Gallery {
            hero: figure(
                &rich_of_size(HERO_SIZE),
                HERO_SEED,
                label("medium size, every option on", HERO_SEED),
                None,
                true,
            ),
            layouts: LAYOUTS
                .iter()
                .map(|&(layout, name)| {
                    // One seed for the three, so the page shows what the layout changes.
                    (
                        layout,
                        figure(
                            &layout_params(layout),
                            HERO_SEED,
                            label(&format!("{name} layout"), HERO_SEED),
                            None,
                            false,
                        ),
                    )
                })
                .collect(),
            intents: SHAPING
                .iter()
                .map(|&(profile, name)| {
                    (
                        profile,
                        name,
                        figure(
                            &intent_params(profile),
                            1,
                            label(&format!("{name} intent"), 1),
                            None,
                            false,
                        ),
                    )
                })
                .collect(),
            layers: [
                Channel::Height,
                Channel::Water,
                Channel::Temperature,
                Channel::Light,
            ]
            .into_iter()
            .map(|channel| {
                (
                    channel,
                    figure(
                        &layer_params(),
                        1,
                        label(channel.label(), 1),
                        Some(channel),
                        false,
                    ),
                )
            })
            .collect(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_specimen_passes_every_check() {
        let g = gallery();
        assert!(g.hero.valid);
        for (layout, f) in &g.layouts {
            assert!(f.valid, "{layout:?} seed {}", f.seed);
        }
        for (profile, _, f) in &g.intents {
            assert!(intent_params(*profile).intent_error().is_none());
            assert!(f.valid, "{profile:?} seed {}", f.seed);
        }
        for (channel, f) in &g.layers {
            assert!(f.valid, "{channel:?} seed {}", f.seed);
            assert!(f.range.is_some(), "{channel:?} has no layer");
        }
    }

    #[test]
    fn the_intent_lists_cover_every_intent() {
        let mut all: Vec<_> = SHAPING.iter().chain(&CHECKED).map(|(p, _)| *p).collect();
        all.sort_by_key(|p| format!("{p:?}"));
        all.dedup();
        assert_eq!(all.len(), 8);
        for (profile, _) in CHECKED {
            assert!(intent_params(profile).intent_error().is_none());
        }
    }
}
