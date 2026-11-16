use topcoat::{
    Result,
    context::Cx,
    router::{href, page},
    view::{View, view},
};

use crate::{app::t, floor};

// Placeholder until the project page is rewritten: the 3D walk is gone.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let label = t!(walk_floor_label, seed: floor::WALK_SEED, size: "medium")
        .resolve(cx)
        .into_owned();
    let map = floor::walk_svg(label);
    Ok(view! {
        <section class="plain">
            <h1>(t!(hero_title))</h1>
            <p>(t!(hero_text))</p>
            (map)
            <p>
                <a href=(href!(crate::app::lab::page))>(t!(cta_lab))</a>
                " · "
                <a href=(href!(crate::app::docs::_page::page))>(t!(cta_docs))</a>
            </p>
        </section>
    })
}
