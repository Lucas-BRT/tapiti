pub mod _page;
pub mod docs;
pub mod lab;
mod legacy;
mod missing;
pub mod search;

use std::path::PathBuf;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt, asset},
    context::{BaseUrl, Cx},
    font::{Font, fontsource::fontsource_font},
    l10n::{fluent::messages, locale},
    metadata::{Metadata, OpenGraph, Url},
    router::{
        Body, HeaderName, HeaderValue, Next, Router, RouterBuilderDirectoryExt,
        RouterBuilderDiscoverExt, Slot,
        content::sitemap::{ChangeFrequency, Sitemap, SitemapUrl},
        header::CONTENT_TYPE,
        href, layer, layout, module_router,
        response::Response,
        route,
    },
    view::{View, component, view},
};

// Loads `src/l10n/en.ftl` and creates `t!`. The site is English only.
messages!("./l10n/{locale}.ftl");

/// A message of this file as an owned `String`, for components that take text.
macro_rules! s {
    ($($t:tt)*) => {
        t!($($t)*).resolve_in(&locale!("en")).into_owned()
    };
}

pub fn router() -> Router {
    // Drawn now, at startup, so the first visitor does not wait for the specimens.
    crate::floor::gallery();
    module_router!()
        .with(BaseUrl::new(site_url()).expect("TAPITI_SITE_URL must be an absolute URL such as https://tapiti.example"))
        .discover()
        .assets(AssetBundle::load().expect("asset bundle missing: run `topcoat dev`, or `topcoat asset bundle --package tapiti-site` after building"))
        // The lab's WebAssembly module and its glue live at fixed paths: the glue finds its `.wasm` next to itself, so they cannot be hashed assets.
        // The two big files are routes of their own (below): `serve_dir` answers with `Accept-Ranges`, which keeps the response from being
        // compressed, and the module is a megabyte. The rest of the folder (the glue's snippets) is served as it is.
        .serve_dir("/lab/pkg/{*file}", pkg_dir())
        .build()
}

/// The public address of the site (for canonical links and social previews): `TAPITI_SITE_URL`, else the local one.
pub(crate) fn site_url() -> String {
    std::env::var("TAPITI_SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into())
}

/// Where the WebAssembly module of the lab is: `TAPITI_LAB_PKG`, else the nearest `apps/site/pkg` above the executable (what `scripts/build-wasm.sh` fills).
fn pkg_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TAPITI_LAB_PKG") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.ancestors()
                .map(|dir| dir.join("apps/site/pkg"))
                .find(|dir| dir.join("tapiti.js").is_file())
        })
        .unwrap_or_else(|| PathBuf::from("apps/site/pkg"))
}

/// A file of the lab's module folder, compressed by the router like a page, and kept by the browser for a few minutes (the paths are
/// fixed, not hashed, so a long lifetime would hold on to an old module after an update).
async fn pkg_file(
    name: &str,
    mime: &'static str,
) -> Result<([(HeaderName, HeaderValue); 2], Vec<u8>)> {
    let bytes =
        std::fs::read(pkg_dir().join(name)).map_err(|_| topcoat::router::error::not_found())?;
    Ok((
        [
            (CONTENT_TYPE, HeaderValue::from_static(mime)),
            (
                HeaderName::from_static("cache-control"),
                HeaderValue::from_static("public, max-age=300, stale-while-revalidate=86400"),
            ),
        ],
        bytes,
    ))
}

#[route(GET "/lab/pkg/tapiti_bg.wasm")]
async fn lab_wasm(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 2], Vec<u8>)> {
    pkg_file("tapiti_bg.wasm", "application/wasm").await
}

#[route(GET "/lab/pkg/tapiti.js")]
async fn lab_glue(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 2], Vec<u8>)> {
    pkg_file("tapiti.js", "text/javascript; charset=utf-8").await
}

/// Browsers ask for this path on their own, whatever the page links to.
#[route(GET "/favicon.ico")]
async fn favicon(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 1], Vec<u8>)> {
    let icon = include_bytes!("../assets/brand/favicon.ico").to_vec();
    Ok((
        [(CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        icon,
    ))
}

/// Tells crawlers where the sitemap is. The sitemap needs absolute URLs, which come from `TAPITI_SITE_URL`.
#[route(GET "/robots.txt")]
async fn robots(cx: &Cx) -> Result<([(HeaderName, HeaderValue); 1], String)> {
    let text = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        site_url().trim_end_matches('/')
    );
    Ok((
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        text,
    ))
}

/// Every page: the landing page, the lab and each documentation page.
#[route(GET "/sitemap.xml")]
async fn sitemap(cx: &Cx) -> Result<Sitemap> {
    let mut sitemap = Sitemap::new()
        .url(
            SitemapUrl::new("/")
                .change_frequency(ChangeFrequency::Monthly)
                .priority(1.0),
        )
        .url(
            SitemapUrl::new("/lab")
                .change_frequency(ChangeFrequency::Monthly)
                .priority(0.8),
        );
    for page in crate::docs::docs().pages() {
        sitemap = sitemap.url(
            SitemapUrl::new(format!("/docs/{}", page.slug))
                .change_frequency(ChangeFrequency::Monthly)
                .priority(0.7),
        );
    }
    Ok(sitemap)
}

/// Liveness probe for the container.
#[route(GET "/healthz")]
async fn healthz(cx: &Cx) -> Result<&'static str> {
    Ok("ok")
}

/// Container health probe, run as `tapiti-site --healthcheck` (the image has no curl): asks this machine for `/healthz`.
pub fn healthcheck() -> bool {
    use std::io::{Read, Write};
    use std::time::Duration;
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let timeout = Some(Duration::from_secs(3));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    if stream
        .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).is_ok() && head.ends_with(b" 200")
}

const GITHUB: &str = "https://github.com/Lucas-BRT/tapiti";
pub(crate) const GITHUB_ISSUES: &str = "https://github.com/Lucas-BRT/tapiti/issues";

// Self-hosted: the font files are bundled as assets and served by this app. IBM Plex, as in the desktop app: Mono for the interface and
// the numbers, Sans Condensed for titles, Sans for the prose of the documentation.
// rustfmt and `topcoat fmt` wrap this macro differently; keep the Topcoat layout.
#[rustfmt::skip]
const PLEX_MONO: Font =
    fontsource_font!(
        IBM_PLEX_MONO,
        weight: [400, 500],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const PLEX_CONDENSED: Font =
    fontsource_font!(
        IBM_PLEX_SANS_CONDENSED,
        weight: [600],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const PLEX_SANS: Font =
    fontsource_font!(
        IBM_PLEX_SANS,
        weight: [400, 600],
        style: Normal,
        subset: [Latin],
        host: Asset,
    );
#[rustfmt::skip]
const PLEX_SANS_ITALIC: Font =
    fontsource_font!(
        IBM_PLEX_SANS,
        weight: [400],
        style: Italic,
        subset: [Latin],
        host: Asset,
    );

/// The headers every response carries. The page runs its own scripts and styles, the module and its workers, and nothing from anywhere
/// else; it is not framed, and it asks for no camera, microphone or position. (Inline `style` attributes are allowed: the bars and the
/// chips of the readout carry their widths and colours.)
const SECURITY_HEADERS: [(&str, &str); 5] = [
    (
        "content-security-policy",
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
    ),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "strict-origin-when-cross-origin"),
    (
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()",
    ),
    ("cross-origin-opener-policy", "same-origin"),
];

// Every request is rendered in English (Topcoat's `t!` needs a language in the context) and every response carries the security headers.
#[layer]
async fn language(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let cx = cx.with(locale!("en"));
    let mut response = next.run(&cx, body).await?;
    for (name, value) in SECURITY_HEADERS {
        response.headers_mut().insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    Ok(response)
}

#[layout(
    with(Metadata::new()
        .title(t!(title))
        .description(t!(description))
        .canonical(Url::Current)
        .open_graph(
            OpenGraph::new().site_name("Tapiti").image(asset!("assets/brand/og.png")),
        ))
)]
async fn layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta name="color-scheme" content="light dark">
                <meta
                    name="theme-color"
                    content="#f3f5f7"
                    media="(prefers-color-scheme: light)"
                >
                <meta
                    name="theme-color"
                    content="#131619"
                    media="(prefers-color-scheme: dark)"
                >
                <script src=(asset!("assets/theme.js"))></script>
                topcoat::metadata::tags()
                topcoat::font::link(font: PLEX_MONO)
                topcoat::font::link(font: PLEX_CONDENSED)
                topcoat::font::link(font: PLEX_SANS)
                topcoat::font::link(font: PLEX_SANS_ITALIC)
                <link
                    rel="icon"
                    href=(asset!("assets/brand/icon.svg"))
                    type="image/svg+xml"
                >
                <link rel="icon" href="/favicon.ico" sizes="32x32">
                <link
                    rel="apple-touch-icon"
                    href=(asset!("assets/brand/apple-touch-icon.png"))
                >
                <link rel="stylesheet" href=(asset!("assets/styles.css"))>
                <script src=(asset!("assets/site.js")) defer="defer"></script>
                <script type="module" src=(asset!("assets/settings.js"))></script>
                topcoat::dev::script()
            </head>
            <body>
                <a class="skip-link" href="#main">(t!(skip_link))</a>
                header()
                <main id="main">(slot)</main>
                // The lab is a tool that fills the window, not a page to scroll to a footer.
                if !topcoat::router::request::try_uri(cx).is_some_and(
                    |uri| uri.path().starts_with("/lab"),
                ) {
                    footer()
                } else {
                    settings_dialog()
                }
            </body>
        </html>
    })
}

#[component]
async fn header(cx: &Cx) -> Result<impl View> {
    let path = topcoat::router::request::try_uri(cx).map_or("", |uri| uri.path());
    let in_docs = path.starts_with("/docs") || path.starts_with("/search");
    let in_lab = path.starts_with("/lab");
    Ok(view! {
        <header class="site-header">
            <a class="brand" href=(href!(_page::page))>
                <img
                    src=(asset!("assets/brand/logo.svg"))
                    alt=""
                    width="17"
                    height="20"
                >
                "tapiti"
            </a>
            <nav class="site-nav" aria-label=(t!(nav_label))>
                <a
                    href=(href!(docs::_page::page))
                    if in_docs {
                        aria-current="page"
                    }
                >
                    (t!(nav_docs))
                </a>
                <a
                    href=(href!(lab::page))
                    if in_lab {
                        aria-current="page"
                    }
                >
                    (t!(nav_lab))
                </a>
                <a href=(GITHUB)>"GitHub"</a>
                // The lab has the settings (they are about the lab: its panel, its undo, its keys); the other pages only the theme.
                if in_lab {
                    <button
                        class="settings-button"
                        type="button"
                        hidden=(true)
                        aria-haspopup="dialog"
                        aria-controls="settings-dialog"
                    >
                        (t!(settings_button))
                    </button>
                } else {
                    <button
                        class="theme-toggle"
                        type="button"
                        hidden=(true)
                        data-label=(t!(settings_theme))
                        data-system=(t!(settings_system))
                        data-light=(t!(settings_light))
                        data-dark=(t!(settings_dark))
                    ></button>
                }
            </nav>
        </header>
    })
}

#[component]
async fn footer(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <footer class="site-footer">
            <div class="footer-inner">
                <div class="footer-about">
                    <a class="brand" href=(href!(_page::page))>
                        <img
                            src=(asset!("assets/brand/logo.svg"))
                            alt=""
                            width="30"
                            height="36"
                        >
                        "tapiti"
                    </a>
                    <p>(t!(footer_about))</p>
                    <p class="footer-small">(t!(footer_license))</p>
                </div>
                <nav aria-label=(t!(footer_explore))>
                    <h2>(t!(footer_explore))</h2>
                    <a href=(href!(docs::_page::page))>(t!(nav_docs))</a>
                    <a href=(href!(lab::page))>(t!(nav_lab))</a>
                </nav>
                <nav aria-label=(t!(footer_project))>
                    <h2>(t!(footer_project))</h2>
                    <a href=(GITHUB)>(t!(footer_source))</a>
                    <a href=(GITHUB_ISSUES)>(t!(footer_issues))</a>
                </nav>
            </div>
        </footer>
    })
}

/// One option of the settings: a set of presets as radio buttons, and, when it has one, a value of the user's own.
#[component]
async fn setting_option(
    cx: &Cx,
    id: &str,
    legend: &str,
    help: &str,
    choices: &[(&str, String)],
    custom: Option<(&str, &str, &str)>,
) -> Result<impl View> {
    Ok(view! {
        <fieldset class="setting" data-option=(id)>
            <legend>(legend)</legend>
            <p class="setting-help" id=(format!("setting-{id}-help"))>(help)</p>
            <div class="setting-choices">
                for (value, label) in choices {
                    <label>
                        <input type="radio" name=(id) value=(*value)>
                        <span>(label.as_str())</span>
                    </label>
                }
                if custom.is_some() {
                    <label>
                        <input type="radio" name=(id) value="custom">
                        <span>(t!(settings_custom))</span>
                    </label>
                }
            </div>
            if let Some((min, max, unit)) = custom {
                <label class="setting-custom" hidden=(true)>
                    <span class="sr-only">(legend)</span>
                    <input
                        type="number"
                        data-custom=(id)
                        min=(min)
                        max=(max)
                        step="1"
                        aria-describedby=(format!("setting-{id}-help"))
                    >
                    <span>(unit)</span>
                </label>
            }
        </fieldset>
    })
}

/// What the settings dialog lists for one option.
struct SettingSpec {
    id: &'static str,
    legend: String,
    help: String,
    choices: Vec<(&'static str, String)>,
    custom: Option<(&'static str, &'static str, &'static str)>,
}

/// The settings of the site, in a dialog the header's button opens. The page script (`settings.js`) fills it from, and saves it to, the
/// browser's storage; every option starts from what the system says (its colour scheme, its wish for less motion).
#[component]
async fn settings_dialog(cx: &Cx) -> Result<impl View> {
    let spec = |id, legend: String, help: String, choices: Vec<(&'static str, String)>, custom| {
        SettingSpec {
            id,
            legend,
            help,
            choices,
            custom,
        }
    };
    let site = vec![
        spec(
            "theme",
            s!(settings_theme),
            s!(settings_theme_help),
            vec![
                ("system", s!(settings_system)),
                ("light", s!(settings_light)),
                ("dark", s!(settings_dark)),
            ],
            None,
        ),
        spec(
            "text",
            s!(settings_text),
            s!(settings_text_help),
            vec![
                ("small", s!(settings_small)),
                ("normal", s!(settings_normal)),
                ("large", s!(settings_large)),
                ("larger", s!(settings_larger)),
            ],
            Some(("70", "200", "%")),
        ),
        spec(
            "motion",
            s!(settings_motion),
            s!(settings_motion_help),
            vec![
                ("off", s!(settings_off)),
                ("fast", s!(settings_fast)),
                ("normal", s!(settings_normal)),
                ("slow", s!(settings_slow)),
            ],
            Some(("0", "600", "ms")),
        ),
    ];
    let lab = vec![
        spec(
            "reveal",
            s!(settings_reveal),
            s!(settings_reveal_help),
            vec![
                ("off", s!(settings_off)),
                ("fast", s!(settings_fast)),
                ("normal", s!(settings_normal)),
                ("slow", s!(settings_slow)),
            ],
            Some(("0", "3000", "ms")),
        ),
        spec(
            "reading",
            s!(settings_reading),
            s!(settings_reading_help),
            vec![
                ("automatic", s!(settings_automatic)),
                ("shown", s!(settings_shown)),
                ("hidden", s!(settings_hidden)),
            ],
            None,
        ),
        spec(
            "width",
            s!(settings_width),
            s!(settings_width_help),
            vec![
                ("narrow", s!(settings_narrow)),
                ("normal", s!(settings_normal)),
                ("wide", s!(settings_wide)),
            ],
            Some(("240", "560", "px")),
        ),
        spec(
            "memory",
            s!(settings_memory),
            s!(settings_memory_help),
            vec![
                ("off", s!(settings_off)),
                ("small", s!(settings_small_floors)),
                ("always", s!(settings_always)),
            ],
            Some(("10", "1000", "rooms")),
        ),
        spec(
            "shortcuts",
            s!(settings_shortcuts),
            s!(settings_shortcuts_help),
            vec![("on", s!(settings_on)), ("off", s!(settings_off))],
            None,
        ),
    ];
    Ok(view! {
        <dialog
            id="settings-dialog"
            class="settings-dialog"
            aria-labelledby="settings-title"
        >
            <form method="dialog">
                <div class="settings-head">
                    <h2 id="settings-title">(t!(settings_title))</h2>
                    <button type="submit" class="settings-close">
                        (t!(settings_close))
                    </button>
                </div>
                <p class="settings-intro">(t!(settings_intro))</p>
                <h3>(t!(settings_site))</h3>
                for o in &site {
                    setting_option(
                        id: o.id,
                        legend: &o.legend,
                        help: &o.help,
                        choices: &o.choices,
                        custom: o.custom
                    )
                }
                <h3>(t!(settings_lab))</h3>
                for o in &lab {
                    setting_option(
                        id: o.id,
                        legend: &o.legend,
                        help: &o.help,
                        choices: &o.choices,
                        custom: o.custom
                    )
                }
                <p class="settings-note">
                    <button type="button" class="secondary-button" id="settings-reset">
                        (t!(settings_reset))
                    </button>
                </p>
            </form>
        </dialog>
    })
}
