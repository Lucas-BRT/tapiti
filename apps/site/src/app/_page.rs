use topcoat::{
    Result,
    context::Cx,
    router::{href, page},
    view::{View, view},
};

use crate::{
    app::{GITHUB_ISSUES, t},
    floor,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/Lucas-BRT/tapiti/main/install.sh | bash";

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let label = t!(home_floor_label, seed: floor::WALK_SEED)
        .resolve(cx)
        .into_owned();
    let map = floor::walk_svg(label);
    Ok(view! {
        <div class="page">
            <h1>(t!(home_title))</h1>
            <p class="lead">(t!(home_lead))</p>
            <p class="meta">(t!(home_meta, version: VERSION))</p>

            <figure>
                (map)
                <figcaption>(t!(home_caption, seed: floor::WALK_SEED))</figcaption>
            </figure>
            <form class="seed-form" action=(href!(crate::app::lab::page)) method="get">
                <label for="home-seed">(t!(home_seed_label))</label>
                <input
                    id="home-seed"
                    name="seed"
                    type="number"
                    inputmode="numeric"
                    min="0"
                    max="4294967295"
                    required=(true)
                    value=(floor::WALK_SEED)
                >
                <button type="submit">(t!(home_seed_submit))</button>
            </form>

            <h2>(t!(home_does_title))</h2>
            <h3>(t!(home_det_title))</h3>
            <p>(t!(home_det_text))</p>
            <h3>(t!(home_val_title))</h3>
            <p>
                (t!(home_val_text))
                " "
                <a
                    href=(href!(
                        crate::app::docs::slug::_page::page,
                        crate::app::docs::slug::slug: "model",
                    ).fragment(
                        "65-when-a-floor-is-valid",
                    ))
                >
                    (t!(home_val_link))
                </a>
                "."
            </p>
            <h3>(t!(home_params_title))</h3>
            <p>(t!(home_params_text))</p>
            <h3>(t!(home_layouts_title))</h3>
            <p>(t!(home_layouts_text))</p>

            <h2>(t!(home_run_title))</h2>
            <dl class="run">
                <dt><a href=(href!(crate::app::lab::page))>(t!(home_run_lab))</a></dt>
                <dd>(t!(home_run_lab_text))</dd>
                <dt>(t!(home_run_desktop))</dt>
                <dd>
                    (t!(home_run_desktop_text))
                    <pre><code>(INSTALL_COMMAND)</code></pre>
                    <a
                        href=(href!(
                            crate::app::docs::slug::_page::page,
                            crate::app::docs::slug::slug: "getting-started",
                        ).fragment(
                            "install",
                        ))
                    >
                        (t!(home_run_desktop_more))
                    </a>
                </dd>
                <dt>(t!(home_run_crate))</dt>
                <dd>
                    (t!(home_run_crate_text))
                    <pre>
                        <code>
                            "tapiti-core = { git = \"https://github.com/Lucas-BRT/tapiti\" }"
                        </code>
                    </pre>
                </dd>
                <dt>(t!(home_run_docker))</dt>
                <dd>
                    (t!(home_run_docker_text))
                    <pre>
                        <code>"docker compose -f compose.local.yml up --build"</code>
                    </pre>
                </dd>
            </dl>

            <h2>(t!(home_status_title))</h2>
            <p>(t!(home_status_text, version: VERSION))</p>
            <p>(t!(home_status_open))</p>
            <p>
                <a
                    href=(href!(
                        crate::app::docs::slug::_page::page,
                        crate::app::docs::slug::slug: "roadmap",
                    ))
                >
                    (t!(home_status_roadmap))
                </a>
                " · "
                <a href=(GITHUB_ISSUES)>(t!(home_status_issues))</a>
            </p>
        </div>
    })
}
