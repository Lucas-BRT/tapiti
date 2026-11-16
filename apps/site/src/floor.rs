//! Floors drawn by the generator itself and inlined in pages as SVG.

use std::sync::OnceLock;
use tapiti_core::{
    generate::Floor,
    params::{Size, rich_of_size},
    stage::stage,
};
use tapiti_render::{Style, floor_svg};
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

/// The floor the landing page shows: the medium size with every option on, first draw valid, a main path of ten rooms through all five shapes, one
/// junction and a leaf chamber.
pub const WALK_SEED: u32 = 279;
pub const WALK_SIZE: Size = Size::Medium;

fn svg(floor: &Floor, label: String) -> TrustedHtml {
    TrustedHtml(floor_svg(
        floor,
        &Style {
            label: Some(label),
            ..Style::default()
        },
    ))
}

/// The landing floor (a validated stage, as the browser's wasm module returns it for the same seed).
pub fn walk() -> &'static Floor {
    static FLOOR: OnceLock<Floor> = OnceLock::new();
    FLOOR.get_or_init(|| stage(WALK_SEED, &rich_of_size(WALK_SIZE), 8).d)
}

pub fn walk_svg(label: String) -> TrustedHtml {
    svg(walk(), label)
}
