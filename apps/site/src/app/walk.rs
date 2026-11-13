//! The landing page as a walk down one floor: seven rooms, a route (minimap and room list) and the floor itself as the backdrop.
//! Everything here is plain HTML and CSS; with scripts off, the page reads the same and its links still move you between rooms.

use std::sync::OnceLock;

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    view::{Child, View, component, view},
};

use crate::{
    app::t,
    floor::{self, STATIONS},
};

/// Whether the page loads its 3D scene. `TAPITI_SCENE=off` turns it off for everyone without a deploy: the dimmed map of the same floor
/// stays behind the page. Read once, at the first request.
fn scene_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("TAPITI_SCENE").map_or(true, |value| value != "off"))
}

/// The id of each room's section, in walking order. The route links to these.
pub const ROOM_IDS: [&str; STATIONS] = [
    "entrance", "shapes", "gate", "dig", "numbers", "run", "stairs",
];

fn floor_label(cx: &Cx) -> String {
    t!(walk_floor_label, seed: floor::WALK_SEED, size: "medium")
        .resolve(cx)
        .into_owned()
}

/// One room of the walk: a section at least one screen tall on wide screens. Each room lays its text out its own way (see landing.css);
/// there is no box around it, the scene shows through.
#[component]
pub async fn room(
    cx: &Cx,
    id: &'static str,
    station: u8,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <section class="room" id=(id) data-station=(station.to_string())>
            <div class="room-body">(child)</div>
        </section>
    })
}

/// The one control the scene needs: it stops the flames and makes the walk jump from room to room. The script shows it.
#[component]
pub async fn pause(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <button
            class="route-pause"
            type="button"
            hidden=(true)
            aria-pressed="false"
            data-pause=(t!(walk_pause).resolve(cx).into_owned())
            data-resume=(t!(walk_resume).resolve(cx).into_owned())
        ></button>
    })
}

/// The slot the 3D scene draws into: a fixed backdrop with a canvas and the data the script needs. `mode` is `landing` or `leaf`; what
/// the backdrop holds without the scene (the dimmed map, or nothing) is its child. Off with `TAPITI_SCENE=off`: then only the child shows.
#[component]
pub async fn scene_backdrop(
    cx: &Cx,
    mode: &'static str,
    class: &'static str,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let scene = scene_enabled();
    Ok(view! {
        <div
            class=(class)
            aria-hidden="true"
            if scene {
                data-mode=(mode)
                data-three=(asset!("assets/vendor/three-subset.min.js"))
                data-build=(asset!("assets/walk/build.js"))
                data-seed=(floor::WALK_SEED.to_string())
                data-size=(floor::WALK_SIZE_INDEX.to_string())
            }
        >
            (child)
            if scene {
                <canvas></canvas>
            }
        </div>
        if scene {
            if mode == "landing" {
                <script type="module" src=(asset!("assets/walk/dungeon.js"))></script>
            } else {
                <script type="module" src=(asset!("assets/walk/scene.js"))></script>
            }
        }
    })
}

/// The floor behind the page. The scene draws over it; without that scene, it stays as a dimmed map.
#[component]
pub async fn walk_backdrop(cx: &Cx) -> Result<impl View> {
    let svg = floor::walk_svg(floor_label(cx));
    Ok(view! { scene_backdrop(mode: "landing", class: "walk-backdrop", (svg)) })
}

/// One step of "how a floor is dug".
#[component]
pub async fn step(cx: &Cx, #[default] child: Child<'_>) -> Result<impl View> {
    Ok(view! { <li class="step"><div>(child)</div></li> })
}
