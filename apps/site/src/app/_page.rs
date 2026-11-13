use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    router::{href, page},
    view::{View, view},
};

use super::walk::{ROOM_IDS, pause, room, step, walk_backdrop};
use crate::{
    app::{leaf, t},
    floor::{self, TrustedHtml},
};

/// Where the releases are: the page for Windows and macOS, and the command for Linux.
const RELEASES: &str = "https://github.com/Lucas-BRT/tapiti/releases/latest";
const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/Lucas-BRT/tapiti/main/install.sh | bash";

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let speed = floor::speed();
    // For whoever reads the source.
    let hint = TrustedHtml(format!(
        "<!-- a dead end, off the map: {} -->",
        href!(leaf::page).resolve(cx)
    ));
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/landing.css"))>
        <div class="walk">
            walk_backdrop()
            pause()
            <div class="rooms">
                room(
                    id: ROOM_IDS[0],
                    station: 0,
                    <h1>(t!(hero_title))</h1>
                    <p class="lede">(t!(hero_text))</p>
                    <div class="install" data-install="">
                        <div class="install-linux" data-os="linux">
                            <div class="command">
                                <code>(INSTALL_COMMAND)</code>
                                <button
                                    class="copy-command"
                                    type="button"
                                    hidden=(true)
                                    data-copy=(INSTALL_COMMAND)
                                    data-copied=(t!(install_copied).resolve(cx).into_owned())
                                >
                                    (t!(install_copy))
                                </button>
                            </div>
                            <p class="caption">(t!(install_linux))</p>
                        </div>
                        <a class="button primary" data-os="windows" href=(RELEASES)>
                            (t!(install_windows))
                        </a>
                        <a class="button primary" data-os="macos" href=(RELEASES)>
                            (t!(install_macos))
                        </a>
                    </div>
                    <div class="cta-row">
                        <a class="button" href="#shapes">(t!(cta_explore))</a>
                        <a class="button" href=(href!(crate::app::lab::page))>
                            (t!(cta_lab))
                        </a>
                        <a
                            class="link-quiet"
                            href=(href!(
                                crate::app::docs::slug::_page::page,
                                crate::app::docs::slug::slug: "getting-started",
                            ).fragment(
                                "install",
                            ))
                        >
                            (t!(install_other))
                        </a>
                    </div>
                )
                room(
                    id: ROOM_IDS[1],
                    station: 1,
                    <h2>(t!(shapes_title))</h2>
                    <p>(t!(shapes_text))</p>
                )
                room(
                    id: ROOM_IDS[2],
                    station: 2,
                    <div class="table-text">
                        <h2>(t!(seed_title))</h2>
                        <p>(t!(seed_text))</p>
                        <form
                            class="seed-form"
                            action=(href!(crate::app::lab::page))
                            method="get"
                        >
                            <label for="landing-seed">(t!(seed_label))</label>
                            <input
                                id="landing-seed"
                                name="seed"
                                type="number"
                                inputmode="numeric"
                                min="0"
                                max="4294967295"
                                required=(true)
                                value=(floor::WALK_SEED)
                            >
                            <button class="button primary" type="submit">
                                (t!(seed_submit))
                            </button>
                        </form>
                    </div>
                )
                room(
                    id: ROOM_IDS[3],
                    station: 3,
                    <h2>(t!(descent_title))</h2>
                    <ol class="steps">
                        step(
                            <h3>(t!(descent_seed_title))</h3>
                            <p>(t!(descent_seed_text))</p>
                        )
                        step(
                            <h3>(t!(descent_rooms_title))</h3>
                            <p>(t!(descent_rooms_text))</p>
                        )
                        step(
                            <h3>(t!(descent_tunnels_title))</h3>
                            <p>(t!(descent_tunnels_text))</p>
                        )
                        step(
                            <h3>(t!(descent_check_title))</h3>
                            <p>(t!(descent_check_text))</p>
                        )
                    </ol>
                )
                room(
                    id: ROOM_IDS[4],
                    station: 4,
                    <h2>(t!(proof_label))</h2>
                    <dl class="register">
                        <div>
                            <dt>(t!(proof_speed_label))</dt>
                            <dd>(t!(proof_speed_value, ms: speed.median_ms))</dd>
                        </div>
                        <div>
                            <dt>(t!(proof_valid_label, total: speed.total))</dt>
                            <dd>
                                (t!(proof_valid_value, valid: speed.valid, total: speed.total))
                            </dd>
                        </div>
                        <div>
                            <dt>(t!(proof_seeds_label))</dt>
                            <dd>(t!(proof_seeds_value))</dd>
                        </div>
                    </dl>
                )
                room(
                    id: ROOM_IDS[5],
                    station: 5,
                    <h2>(t!(run_title))</h2>
                    <dl class="run-panel">
                        <div>
                            <dt>(t!(run_browser))</dt>
                            <dd>
                                <a href=(href!(crate::app::lab::page))>
                                    (t!(run_browser_link))
                                </a>
                            </dd>
                        </div>
                        <div>
                            <dt>(t!(run_desktop))</dt>
                            <dd><code>"cargo run --release"</code></dd>
                        </div>
                        <div>
                            <dt>(t!(run_crate))</dt>
                            <dd>
                                <code>
                                    "tapiti-core = { git = \"https://github.com/Lucas-BRT/tapiti\" }"
                                </code>
                            </dd>
                        </div>
                        <div>
                            <dt>"Docker"</dt>
                            <dd>
                                <code>
                                    "docker compose -f compose.local.yml up --build"
                                </code>
                            </dd>
                        </div>
                    </dl>
                )
                room(
                    id: ROOM_IDS[6],
                    station: 6,
                    <h2>(t!(final_title))</h2>
                    <p class="lede">(t!(final_text))</p>
                    <div class="cta-row">
                        <a class="button primary" href=(href!(crate::app::lab::page))>
                            (t!(cta_lab))
                        </a>
                        <a class="button" href=(href!(crate::app::docs::_page::page))>
                            (t!(cta_docs))
                        </a>
                    </div>
                )
            </div>
            (hint)
        </div>
    })
}
