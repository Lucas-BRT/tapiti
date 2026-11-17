pub mod _page;
pub mod slug;

use topcoat::{
    Result,
    asset::asset,
    context::Cx,
    router::href,
    view::{Child, View, component, view},
};

use super::search;
use crate::{app::t, docs::docs};

/// The name of a sidebar group.
fn group_label(cx: &Cx, id: &str) -> String {
    match id {
        "start" => t!(docs_group_start),
        "use" => t!(docs_group_use),
        "how" => t!(docs_group_how),
        _ => t!(docs_group_project),
    }
    .resolve(cx)
    .into_owned()
}

/// The documentation's frame: the search box and the list of pages beside whatever `child` shows. `current` is the slug being read.
#[component]
pub async fn frame(cx: &Cx, current: &str, #[default] child: Child<'_>) -> Result<impl View> {
    let groups = docs().groups();
    Ok(view! {
        <link rel="stylesheet" href=(asset!("assets/docs.css"))>
        <div class="docs">
            <aside class="docs-nav">
                // The search page has its own box; the others have this one.
                if !current.is_empty() {
                    <form
                        class="docs-search"
                        role="search"
                        method="get"
                        action=(href!(search::page))
                    >
                        <label class="visually-hidden" for="docs-q">
                            (t!(docs_search_label))
                        </label>
                        <input
                            id="docs-q"
                            type="search"
                            name="q"
                            maxlength="80"
                            placeholder=(t!(docs_search_placeholder))
                        >
                    </form>
                }
                <nav aria-labelledby="docs-nav-title">
                    <p id="docs-nav-title" class="docs-side-title">
                        (t!(docs_nav_label))
                    </p>
                    <ul class="docs-groups">
                        for (id, pages) in groups {
                            <li class="docs-group">
                                <p
                                    class="docs-group-title"
                                    id=(format!("docs-group-{id}"))
                                >
                                    (group_label(cx, id))
                                </p>
                                <ul aria-labelledby=(format!("docs-group-{id}"))>
                                    for page in pages {
                                        <li>
                                            <a
                                                href=(href!(slug::_page::page, slug: page.slug))
                                                if page.slug == current {
                                                    aria-current="page"
                                                }
                                            >
                                                (page.title.as_str())
                                            </a>
                                        </li>
                                    }
                                </ul>
                            </li>
                        }
                    </ul>
                </nav>
            </aside>
            <div class="docs-main">(child)</div>
        </div>
    })
}
