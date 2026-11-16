use topcoat::{
    Result,
    context::Cx,
    metadata::{Metadata, Robots},
    router::page,
    view::{View, view},
};

use crate::app::{missing::not_found, t};

#[page(
    with(Metadata::new()
        .title(t!(missing_title))
        .robots(Robots::new().index(false).follow(false)))
)]
pub async fn page(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        (topcoat::router::StatusCode::NOT_FOUND)
        not_found()
    })
}
