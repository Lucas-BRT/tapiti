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

/// One plate of the page: the floor on a grid, and under it a one-line caption (an index, a title, the facts).
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
    let readout = floor::readout(m);
    let groups = floor::tree_groups();
    let hero_svg = floor::TrustedHtml(g.hero.svg.0.clone());
    // The five rules, each with a few words on what it checks (the model has the rest).
    let rules = [
        (
            "V1",
            t!(home_rule_1).resolve(cx).into_owned(),
            t!(home_rule_d1).resolve(cx).into_owned(),
            t!(home_rule_1_text).resolve(cx).into_owned(),
        ),
        (
            "V2",
            t!(home_rule_2).resolve(cx).into_owned(),
            t!(home_rule_d2).resolve(cx).into_owned(),
            t!(home_rule_2_text).resolve(cx).into_owned(),
        ),
        (
            "V3",
            t!(home_rule_3).resolve(cx).into_owned(),
            t!(home_rule_d3).resolve(cx).into_owned(),
            t!(home_rule_3_text).resolve(cx).into_owned(),
        ),
        (
            "V4",
            t!(home_rule_4).resolve(cx).into_owned(),
            t!(home_rule_d4).resolve(cx).into_owned(),
            t!(home_rule_4_text).resolve(cx).into_owned(),
        ),
        (
            "V5",
            t!(home_rule_5).resolve(cx).into_owned(),
            t!(home_rule_d5).resolve(cx).into_owned(),
            t!(home_rule_5_text).resolve(cx).into_owned(),
        ),
    ];
    let model = |fragment: &'static str| {
        href!(crate::app::docs::slug::_page::page, crate::app::docs::slug::slug: "model")
            .fragment(fragment)
    };
    Ok(view! {
        <div class="landing">
            <section class="intro">
                <p class="meta">(t!(home_meta, version: VERSION))</p>
                <h1>(t!(home_title))</h1>
                <p class="lead">(t!(home_lead))</p>
                <p class="links">
                    <a href=(href!(crate::app::lab::page))>(t!(home_lab))</a>
                    " · "
                    <a href=(href!(crate::app::docs::_page::page))>(t!(nav_docs))</a>
                </p>
            </section>

            <section class="bench" aria-label=(t!(home_bench_label))>
                <div class="bench-bar">
                    <span>
                        (t!(home_bar_seed))
                        <b>(HERO_SEED)</b>
                    </span>
                    <span>(t!(home_bar_size))</span>
                    <span>
                        (t!(home_bar_rooms))
                        <b>(g.hero.rooms)</b>
                    </span>
                    <span class="sp"></span>
                    <span>
                        "V1 V2 V3 V4 V5 "
                        if g.hero.valid {
                            <span class="valid-word">(t!(home_bar_valid))</span>
                        } else {
                            <span>(t!(home_bar_invalid))</span>
                        }
                    </span>
                </div>
                <div class="bench-cols">
                    <aside class="bench-tree" aria-label=(t!(home_tree_label))>
                        <div class="group-bar">
                            "Λ"
                            <span>(groups.len())</span>
                        </div>
                        <ul class="tree-list">
                            for (title, count) in &groups {
                                <li>
                                    <a
                                        href=(href!(
                                            crate::app::docs::slug::_page::page,
                                            crate::app::docs::slug::slug: "parameters",
                                        ))
                                    >
                                        (title.as_str())
                                        <span>(*count)</span>
                                    </a>
                                </li>
                            }
                        </ul>
                    </aside>
                    <div class="bench-canvas">
                        (hero_svg)
                        <div class="bench-hud">
                            <span data-shape="rect">
                                <i></i>
                                (t!(home_shape_rect))
                            </span>
                            <span data-shape="disc">
                                <i></i>
                                (t!(home_shape_disc))
                            </span>
                            <span data-shape="oct">
                                <i></i>
                                (t!(home_shape_oct))
                            </span>
                            <span data-shape="L">
                                <i></i>
                                (t!(home_shape_L))
                            </span>
                            <span data-shape="star">
                                <i></i>
                                (t!(home_shape_star))
                            </span>
                            <span class="in">
                                <i></i>
                                (t!(home_hud_entrance))
                            </span>
                            <span class="out">
                                <i></i>
                                (t!(home_hud_exit))
                            </span>
                        </div>
                    </div>
                    <aside class="bench-read" aria-label=(t!(home_read_label))>
                        <div class="group-bar">(t!(home_rules_group))</div>
                        <div class="readout">
                            for (code, name, detail, long) in &rules {
                                <div class="rule-row" title=(long.as_str())>
                                    <svg class="tick" viewBox="0 0 12 12" aria-hidden="true">
                                        <path d="M1.5 6.5 4.5 9.5 10.5 2.5"></path>
                                    </svg>
                                    <b>(*code)</b>
                                    <span>(name.as_str())</span>
                                    <span>(detail.as_str())</span>
                                </div>
                            }
                            <p class="plan-note">
                                (t!(home_rules_note))
                                " "
                                <a href=(model("65-when-a-floor-is-valid"))>
                                    (t!(home_rules_link))
                                </a>
                            </p>
                        </div>
                        <div class="group-bar">(t!(home_measures_group))</div>
                        <ul class="readout">
                            for row in &readout {
                                <li
                                    class=(if row.share.is_some() { "has-bar" } else { "" })
                                >
                                    <span>(row.label)</span>
                                    if let Some(share) = row.share {
                                        <span class="track">
                                            <i style=(format!("width: {:.0}%", share * 100.0))></i>
                                        </span>
                                    }
                                    <span class="val">(row.value.as_str())</span>
                                </li>
                            }
                        </ul>
                    </aside>
                </div>
                <form
                    class="bench-cmd"
                    action=(href!(crate::app::lab::page))
                    method="get"
                >
                    <span class="prompt" aria-hidden="true">"›"</span>
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
                </form>
            </section>

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
