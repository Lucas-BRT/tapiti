//! Every address that matches no page: a plain 404.

pub mod _page;

use topcoat::{
    Result,
    context::Cx,
    router::href,
    view::{View, component, view},
};

use crate::app::t;

/// The body of a 404 page (the caller sets the status).
#[component]
pub async fn not_found(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <section class="plain">
            <h1>(t!(missing_title))</h1>
            <p>(t!(missing_text))</p>
            <p>
                <a href=(href!(crate::app::_page::page))>(t!(missing_back))</a>
                " · "
                <a href=(href!(crate::app::docs::_page::page))>(t!(nav_docs))</a>
                " · "
                <a href=(href!(crate::app::lab::page))>(t!(nav_lab))</a>
                " · "
                <a href=(href!(crate::app::search::page))>(t!(missing_search))</a>
            </p>
        </section>
    })
}

topcoat::router::segment!(kind: CatchAll, rename: "path");
