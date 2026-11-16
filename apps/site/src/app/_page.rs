use topcoat::{
    Result,
    context::Cx,
    router::{href, page},
    view::{View, component, view},
};

use crate::{
    app::{GITHUB_ISSUES, t},
    floor::{self, Figure, HERO_SEED},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/Lucas-BRT/tapiti/main/install.sh | bash";

/// One plan on the sheet: the floor on a grid, and under it a title block (an index cell, a title, the facts).
#[component]
async fn plan(cx: &Cx, id: String, title: String, fig: &Figure, meta: String) -> Result<impl View> {
    let svg = floor::TrustedHtml(fig.svg.0.clone());
    Ok(view! {
        <figure class="plan">
            <div class="plan-art">(svg)</div>
            <figcaption class="block">
                <span class="block-id">(id)</span>
                <span class="block-title">(title)</span>
                <span class="block-meta">(meta)</span>
            </figcaption>
        </figure>
    })
}

fn facts(cx: &Cx, fig: &Figure) -> String {
    format!(
        "seed {} · {}",
        fig.seed,
        t!(home_block_rooms, rooms: fig.rooms).resolve(cx)
    )
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let g = floor::gallery();
    let m = &g.hero.metrics;
    let hero_meta = if g.hero.valid {
        format!(
            "{} · {}",
            facts(cx, &g.hero),
            t!(home_block_valid).resolve(cx)
        )
    } else {
        facts(cx, &g.hero)
    };
    let model = |fragment: &'static str| {
        href!(crate::app::docs::slug::_page::page, crate::app::docs::slug::slug: "model")
            .fragment(fragment)
    };
    Ok(view! {
        <div class="sheet">
            <section class="hero">
                <div class="hero-text">
                    <p class="meta">(t!(home_meta, version: VERSION))</p>
                    <h1>(t!(home_title))</h1>
                    <p class="lead">(t!(home_lead))</p>
                    <form
                        class="seed-form"
                        action=(href!(crate::app::lab::page))
                        method="get"
                    >
                        <label for="home-seed">(t!(home_seed_label))</label>
                        <input
                            id="home-seed"
                            name="seed"
                            type="number"
                            inputmode="numeric"
                            min="0"
                            max="4294967295"
                            required=(true)
                            value=(HERO_SEED)
                        >
                        <button type="submit">(t!(home_seed_submit))</button>
                        <a href=(href!(crate::app::lab::page))>(t!(home_lab))</a>
                    </form>
                </div>
                plan(
                    id: "P-0".to_string(),
                    title: t!(home_fig_hero, seed: HERO_SEED).resolve(cx).into_owned(),
                    fig: &g.hero,
                    meta: hero_meta
                )
            </section>

            <ul class="shapes" aria-label=(t!(home_shapes_title))>
                <li data-shape="rect">(t!(home_shape_rect))</li>
                <li data-shape="disc">(t!(home_shape_disc))</li>
                <li data-shape="oct">(t!(home_shape_oct))</li>
                <li data-shape="L">(t!(home_shape_L))</li>
                <li data-shape="star">(t!(home_shape_star))</li>
            </ul>

            <section class="band">
                <div class="band-head">
                    <h2>(t!(home_layouts_title))</h2>
                    <p>(t!(home_layouts_note))</p>
                </div>
                <div class="cols-3">
                    for (i, (layout, fig)) in g.layouts.iter().enumerate() {
                        <div class="cell">
                            plan(
                                id: format!("L-{}", i + 1),
                                title: floor::LAYOUTS[i].1.to_string(),
                                fig: fig,
                                meta: facts(cx, fig)
                            )
                            <p class="plan-note">(layout.guide())</p>
                        </div>
                    }
                </div>
            </section>

            <section class="band">
                <div class="band-head">
                    <h2>(t!(home_intents_title))</h2>
                    <p>(t!(home_intents_note))</p>
                </div>
                <div class="cols-4">
                    for (i, (profile, name, fig)) in g.intents.iter().enumerate() {
                        <div class="cell">
                            plan(
                                id: format!("I-{}", i + 1),
                                title: name.to_string(),
                                fig: fig,
                                meta: facts(cx, fig)
                            )
                            <p class="plan-note">(profile.guide().makes)</p>
                        </div>
                    }
                </div>
                <h3 class="subhead">(t!(home_checked_title))</h3>
                <dl class="checked">
                    for (profile, name) in floor::CHECKED {
                        <div>
                            <dt>(name)</dt>
                            <dd>(profile.guide().makes)</dd>
                        </div>
                    }
                </dl>
            </section>

            <section class="band">
                <div class="band-head">
                    <h2>(t!(home_layers_title))</h2>
                    <p>(t!(home_layers_note))</p>
                </div>
                <div class="cols-4">
                    for (i, (_, fig)) in g.layers.iter().enumerate() {
                        <div class="cell">
                            plan(
                                id: format!("E-{}", i + 1),
                                title: fig
                                    .range
                                    .as_ref()
                                    .map_or(String::new(), |r| r.label.to_string()),
                                fig: fig,
                                meta: facts(cx, fig)
                            )
                            if let Some(range) = &fig.range {
                                <div
                                    class="ramp"
                                    style=(format!("background: {}", range.gradient))
                                ></div>
                                <p class="plan-note ramp-range">
                                    (t!(
                                        home_layers_range,
                                        low: range.low.clone(),
                                        high: range.high.clone(),
                                    ))
                                </p>
                            }
                        </div>
                    }
                </div>
            </section>

            <section class="band two">
                <div>
                    <div class="band-head"><h2>(t!(home_read_title))</h2></div>
                    <p class="plan-note">(t!(home_read_note))</p>
                    <dl class="facts">
                        <div>
                            <dt>
                                (t!(home_read_rooms))
                                <small>(t!(home_read_rooms_text))</small>
                            </dt>
                            <dd>(g.hero.rooms)</dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_edges))
                                <small>(t!(home_read_edges_text))</small>
                            </dt>
                            <dd>(m.edges)</dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_loops))
                                <small>(t!(home_read_loops_text))</small>
                            </dt>
                            <dd>(m.beta)</dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_leaves))
                                <small>(t!(home_read_leaves_text))</small>
                            </dt>
                            <dd>(m.leaves)</dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_hops))
                                <small>(t!(home_read_hops_text))</small>
                            </dt>
                            <dd>
                                (m.hops_to_exit.map_or(
                                    "—".to_string(),
                                    |h| format!("{h:.0}"),
                                ))
                            </dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_routes))
                                <small>(t!(home_read_routes_text))</small>
                            </dt>
                            <dd>
                                (m.routes_to_exit.map_or(
                                    "—".to_string(),
                                    |r| r.to_string(),
                                ))
                            </dd>
                        </div>
                        <div>
                            <dt>
                                (t!(home_read_bridges))
                                <small>(t!(home_read_bridges_text))</small>
                            </dt>
                            <dd>(m.bridges)</dd>
                        </div>
                    </dl>
                    <p><a href=(model("6-navigable-graph"))>(t!(home_read_link))</a></p>
                </div>
                <div>
                    <div class="band-head"><h2>(t!(home_rules_title))</h2></div>
                    <p class="plan-note">(t!(home_rules_note))</p>
                    <ol class="rules">
                        <li>
                            <b>(t!(home_rule_1))</b>
                            (t!(home_rule_1_text))
                        </li>
                        <li>
                            <b>(t!(home_rule_2))</b>
                            (t!(home_rule_2_text))
                        </li>
                        <li>
                            <b>(t!(home_rule_3))</b>
                            (t!(home_rule_3_text))
                        </li>
                        <li>
                            <b>(t!(home_rule_4))</b>
                            (t!(home_rule_4_text))
                        </li>
                        <li>
                            <b>(t!(home_rule_5))</b>
                            (t!(home_rule_5_text))
                        </li>
                    </ol>
                    <p>
                        <a href=(model("65-when-a-floor-is-valid"))>
                            (t!(home_rules_link))
                        </a>
                    </p>
                </div>
            </section>

            <section class="band two">
                <div>
                    <div class="band-head"><h2>(t!(home_same_title))</h2></div>
                    <p>(t!(home_same_text))</p>
                </div>
                <div>
                    <div class="band-head"><h2>(t!(home_params_title))</h2></div>
                    <p>(t!(home_params_text))</p>
                </div>
            </section>

            <section class="band">
                <div class="band-head"><h2>(t!(home_run_title))</h2></div>
                <dl class="run">
                    <dt>
                        <a href=(href!(crate::app::lab::page))>(t!(home_run_lab))</a>
                    </dt>
                    <dd>(t!(home_run_lab_text))</dd>
                    <dt>(t!(home_run_desktop))</dt>
                    <dd>
                        (t!(home_run_desktop_text))
                        <pre tabindex="0"><code>(INSTALL_COMMAND)</code></pre>
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
                        <pre tabindex="0">
                            <code>
                                "tapiti-core = { git = \"https://github.com/Lucas-BRT/tapiti\" }"
                            </code>
                        </pre>
                    </dd>
                    <dt>(t!(home_run_docker))</dt>
                    <dd>
                        (t!(home_run_docker_text))
                        <pre tabindex="0">
                            <code>
                                "docker compose -f compose.local.yml up --build"
                            </code>
                        </pre>
                    </dd>
                </dl>
            </section>

            <section class="band">
                <div class="band-head"><h2>(t!(home_status_title))</h2></div>
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
            </section>
        </div>
    })
}
