//! The documentation: the repository's Markdown files (`docs/`) rendered once at startup, plus a generated parameter
//! reference. Math is written as `$…$` / `$$…$$` and becomes MathML on the server, so the pages need no script and no font files.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use pulldown_latex::{
    Parser as LatexParser, Storage,
    config::{DisplayMode, RenderConfig},
    push_mathml,
};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::OnceLock;
use tapiti_schema::{FieldKind, ParamGroup};

pub const GITHUB_BLOB: &str = "https://github.com/Lucas-BRT/tapiti/blob/main";

/// A documentation page: where its Markdown lives.
struct Source {
    slug: &'static str,
    en: &'static str,
    /// Path under `docs/`, for the "edit on GitHub" link.
    file: &'static str,
}

const SOURCES: &[Source] = &[
    Source {
        slug: "getting-started",
        en: include_str!("../../../docs/getting-started.md"),
        file: "getting-started.md",
    },
    Source {
        slug: "model",
        en: include_str!("../../../docs/model.md"),
        file: "model.md",
    },
    Source {
        slug: "parameters",
        en: "",
        file: "",
    },
    Source {
        slug: "performance",
        en: include_str!("../../../docs/performance-memory-review.md"),
        file: "performance-memory-review.md",
    },
    Source {
        slug: "desktop",
        en: include_str!("../../../docs/desktop-generation-rendering.md"),
        file: "desktop-generation-rendering.md",
    },
    Source {
        slug: "development",
        en: include_str!("../../../docs/development.md"),
        file: "development.md",
    },
    Source {
        slug: "roadmap",
        en: include_str!("../../../docs/roadmap.md"),
        file: "roadmap.md",
    },
];

/// A group of the sidebar: the pages, by what a reader is doing. Every page is in exactly one group (a test checks it).
pub struct Group {
    pub id: &'static str,
    pub slugs: &'static [&'static str],
}

pub const GROUPS: &[Group] = &[
    Group {
        id: "start",
        slugs: &["getting-started"],
    },
    Group {
        id: "use",
        slugs: &["desktop", "parameters"],
    },
    Group {
        id: "how",
        slugs: &["model", "performance"],
    },
    Group {
        id: "project",
        slugs: &["development", "roadmap"],
    },
];

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// One heading and the text under it, for search.
#[derive(Clone, Debug)]
pub struct Section {
    pub heading: String,
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub slug: &'static str,
    pub title: String,
    pub html: String,
    pub toc: Vec<TocEntry>,
    pub sections: Vec<Section>,
    /// Number of math spans in the source and number of `<math>` elements produced (they must match).
    #[allow(dead_code)] // read by the tests that pin the math pipeline
    pub math_spans: usize,
    #[allow(dead_code)]
    pub math_rendered: usize,
    /// Where to read or edit the source on GitHub.
    pub source_url: String,
    /// What the page says first, in a sentence or two, for the page's own description in the search engines and the link previews.
    pub description: String,
}

pub struct Docs {
    pages: Vec<Page>,
}

pub fn docs() -> &'static Docs {
    static DOCS: OnceLock<Docs> = OnceLock::new();
    DOCS.get_or_init(|| Docs {
        pages: SOURCES.iter().map(build).collect(),
    })
}

impl Docs {
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// The sidebar: each group with its pages, in the order of [`GROUPS`].
    pub fn groups(&self) -> Vec<(&'static str, Vec<&Page>)> {
        GROUPS
            .iter()
            .map(|group| {
                let pages = group
                    .slugs
                    .iter()
                    .filter_map(|slug| self.page(slug))
                    .collect();
                (group.id, pages)
            })
            .collect()
    }

    pub fn page(&self, slug: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.slug == slug)
    }
}

fn build(source: &Source) -> Page {
    if source.slug == "parameters" {
        return parameters_page();
    }
    let mut page = render(source.en, source.slug);
    page.source_url = format!("{GITHUB_BLOB}/docs/{}", source.file);
    page
}

// ---- Markdown ------------------------------------------------------------------------------------

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
}

/// MathML for a TeX formula, or `None` when it does not parse.
fn mathml(tex: &str, display: bool) -> Option<String> {
    let storage = Storage::new();
    let mut events = Vec::new();
    for event in LatexParser::new(tex, &storage) {
        events.push(event.ok()?);
    }
    let mut out = String::new();
    let config = RenderConfig {
        display_mode: if display {
            DisplayMode::Block
        } else {
            DisplayMode::Inline
        },
        annotation: Some(tex),
        ..RenderConfig::default()
    };
    push_mathml(
        &mut out,
        events.into_iter().map(Ok::<_, std::convert::Infallible>),
        config,
    )
    .ok()?;
    Some(out)
}

/// The Greek letters the docs use, so that searching for "β" finds a section that only writes `\beta`.
fn greek_aliases(tex: &str) -> String {
    const LETTERS: &[(&str, &str)] = &[
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\xi", "ξ"),
        ("\\rho", "ρ"),
        ("\\sigma", "σ"),
        ("\\theta", "θ"),
        ("\\lambda", "λ"),
        ("\\varphi", "φ"),
        ("\\omega", "ω"),
        ("\\Lambda", "Λ"),
        ("\\Phi", "Φ"),
        ("\\Pi", "Π"),
        ("\\iota", "ι"),
        ("\\tau", "τ"),
        ("\\kappa", "κ"),
        ("\\ell", "ℓ"),
    ];
    LETTERS
        .iter()
        .filter(|(command, _)| tex.contains(command))
        .map(|(_, letter)| *letter)
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Rewrites a link of the Markdown sources: page slugs become routes, repository paths become GitHub URLs.
fn rewrite_link(dest: &str) -> String {
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with('#')
        || dest.starts_with("mailto:")
    {
        return dest.to_string();
    }
    let trimmed = dest.trim_start_matches("../");
    if trimmed.starts_with("crates/")
        || trimmed.starts_with("apps/")
        || trimmed.starts_with("docs/")
        || trimmed.starts_with("src/")
    {
        return format!("{GITHUB_BLOB}/{trimmed}");
    }
    let (slug, fragment) = dest.split_once('#').map_or((dest, ""), |(s, f)| (s, f));
    if SOURCES.iter().any(|s| s.slug == slug) {
        let mut url = format!("/docs/{slug}");
        if !fragment.is_empty() {
            let _ = write!(url, "#{fragment}");
        }
        return url;
    }
    dest.to_string()
}

fn render(markdown: &str, slug: &'static str) -> Page {
    let mut math_spans = 0;
    let mut math_rendered = 0;
    let mut events: Vec<Event> = Vec::new();
    for event in Parser::new_ext(markdown, options()) {
        events.push(match event {
            Event::InlineMath(tex) | Event::DisplayMath(tex)
                if {
                    math_spans += 1;
                    false
                } =>
            {
                Event::Text(tex)
            }
            Event::InlineMath(tex) => match mathml(&tex, false) {
                Some(m) => {
                    math_rendered += 1;
                    Event::Html(format!("{m}\u{200b}").into())
                }
                None => Event::Html(
                    format!("<code class=\"math-error\">{}</code>", escape(&tex)).into(),
                ),
            },
            Event::DisplayMath(tex) => match mathml(&tex, true) {
                Some(m) => {
                    math_rendered += 1;
                    Event::Html(
                        format!("<div class=\"math-display\" tabindex=\"0\">{m}</div>").into(),
                    )
                }
                None => {
                    Event::Html(format!("<pre class=\"math-error\">{}</pre>", escape(&tex)).into())
                }
            },
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: CowStr::from(rewrite_link(&dest_url)),
                title,
                id,
            }),
            other => other,
        });
    }
    // Search text: plain text per section, plus the TeX of math (with Greek letters spelled as letters).
    let mut title = String::new();
    let mut toc = Vec::new();
    let mut sections: Vec<Section> = vec![Section {
        heading: String::new(),
        id: String::new(),
        text: String::new(),
    }];
    let mut used_ids: HashMap<String, usize> = HashMap::new();
    let mut out_events: Vec<Event> = Vec::new();
    let mut i = 0;
    let mut raw_math = markdown_math(markdown).into_iter();
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let level_number = heading_number(*level);
                let mut j = i + 1;
                let mut text = String::new();
                while j < events.len() && !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                    if let Event::Text(t) | Event::Code(t) = &events[j] {
                        text.push_str(t);
                    }
                    j += 1;
                }
                let base = slugify(&text);
                let count = used_ids.entry(base.clone()).or_insert(0);
                let id = if *count == 0 {
                    base.clone()
                } else {
                    format!("{base}-{count}")
                };
                *count += 1;
                if level_number == 1 && title.is_empty() {
                    title = text.clone();
                    out_events.push(Event::Html(format!("<h1 id=\"{id}\">").into()));
                } else {
                    out_events.push(Event::Html(format!("<h{level_number} id=\"{id}\">").into()));
                }
                out_events.extend(events[i + 1..j].iter().cloned());
                if level_number > 1 {
                    out_events.push(Event::Html(
                        format!("<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\" tabindex=\"-1\">#</a>")
                            .into(),
                    ));
                    toc.push(TocEntry {
                        level: level_number,
                        id: id.clone(),
                        text: text.clone(),
                    });
                    sections.push(Section {
                        heading: text,
                        id,
                        text: String::new(),
                    });
                }
                out_events.push(Event::Html(format!("</h{level_number}>").into()));
                i = j + 1;
            }
            event => {
                let section = sections.last_mut().expect("there is always a section");
                match event {
                    Event::Text(t) | Event::Code(t) => {
                        section.text.push_str(t);
                        section.text.push(' ');
                    }
                    Event::SoftBreak | Event::HardBreak => section.text.push(' '),
                    Event::Html(h) if h.contains("<math") => {
                        if let Some(tex) = raw_math.next() {
                            section.text.push_str(&greek_aliases(&tex));
                            section.text.push(' ');
                        }
                    }
                    _ => {}
                }
                out_events.push(event.clone());
                i += 1;
            }
        }
    }
    let mut html_out = String::new();
    html::push_html(&mut html_out, out_events.into_iter());
    let html_out = html_out
        .replace(
            "<table>",
            "<div class=\"table-wrap\" tabindex=\"0\"><table>",
        )
        .replace("<pre>", "<pre tabindex=\"0\">")
        .replace("</table>", "</table></div>");
    let description = first_paragraph(&html_out);
    Page {
        slug,
        title,
        html: html_out,
        toc,
        sections,
        math_spans,
        math_rendered,
        source_url: String::new(),
        description,
    }
}

/// The text of the first paragraph of a page's HTML, without tags, cut at a word near 170 characters.
fn first_paragraph(html: &str) -> String {
    let Some(start) = html.find("<p>") else {
        return String::new();
    };
    let rest = &html[start + 3..];
    let end = rest.find("</p>").unwrap_or(rest.len());
    let mut text = String::new();
    let mut in_tag = false;
    for c in rest[..end].chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= 170 {
        return text;
    }
    let cut: String = text.chars().take(170).collect();
    format!(
        "{}…",
        cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head)
    )
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The TeX source of each math span, in document order (used only to index the Greek letters for search).
fn markdown_math(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, options())
        .filter_map(|event| match event {
            Event::InlineMath(tex) | Event::DisplayMath(tex) => Some(tex.to_string()),
            _ => None,
        })
        .collect()
}

// ---- The parameter reference ------------------------------------------------------------------------

fn parameters_page() -> Page {
    let title = "Parameters".to_string();
    let mut html = String::new();
    let _ = write!(html, "<h1 id=\"parameters\">{title}</h1>");
    let _ = write!(
        html,
        "<p>Every parameter of Λ, as the desktop app and the lab show them. Names in code are the JSON field names, so a Λ copied from one frontend loads in the other. Pass a path like <code>growth.rooms</code> to read or set it.</p>"
    );
    let mut toc = Vec::new();
    let mut sections = vec![Section {
        heading: String::new(),
        id: String::new(),
        text: title.clone(),
    }];
    for group in tapiti_schema::groups() {
        let group_title = group.title.as_str().to_string();
        let id = slugify(&group.id);
        let _ = write!(
            html,
            "<h2 id=\"{id}\">{}<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\" tabindex=\"-1\">#</a></h2>",
            escape(&group_title)
        );
        let _ = write!(html, "<p>{}</p>", escape(group.intro.as_str()));
        group_table(&mut html, &group);
        let mut body = group.intro.as_str().to_string();
        for field in &group.fields {
            let _ = write!(
                body,
                " {} {} {}",
                field.path,
                field.label.as_str(),
                field.help.as_str()
            );
        }
        toc.push(TocEntry {
            level: 2,
            id: id.clone(),
            text: group_title.clone(),
        });
        sections.push(Section {
            heading: group_title,
            id,
            text: body,
        });
    }
    Page {
        slug: "parameters",
        title,
        html,
        toc,
        sections,
        math_spans: 0,
        math_rendered: 0,
        source_url: format!("{GITHUB_BLOB}/crates/tapiti-schema/src/lib.rs"),
        description: "Every parameter of Λ: its name in code, what it accepts, its default in each size and what it does.".to_string(),
    }
}

/// The default of a parameter in each of the four sizes, as text: one value when the sizes agree, else `small 1 · medium 2 · …`.
fn defaults_text(field: &tapiti_schema::ParamField, sizes: &[serde_json::Value]) -> String {
    let show = |value: Option<&serde_json::Value>| match (value, &field.kind) {
        (None, _) | (Some(serde_json::Value::Null), _) => "—".to_string(),
        (Some(serde_json::Value::Bool(b)), _) => if *b { "on" } else { "off" }.to_string(),
        (Some(serde_json::Value::String(text)), FieldKind::Choice(options)) => options
            .iter()
            .find(|(value, _)| value == text)
            .map_or(text.clone(), |(_, name)| name.as_str().to_string()),
        (Some(value), FieldKind::Choice(options)) => {
            let text = value.to_string();
            options
                .iter()
                .find(|(v, _)| *v == text)
                .map_or(text, |(_, name)| name.as_str().to_string())
        }
        (Some(value), _) => value.to_string(),
    };
    let at = |size: &'_ serde_json::Value| -> Option<serde_json::Value> {
        field
            .path
            .split('.')
            .try_fold(size, |value, key| value.get(key))
            .cloned()
    };
    let texts: Vec<String> = sizes.iter().map(|size| show(at(size).as_ref())).collect();
    if texts.iter().all(|t| *t == texts[0]) {
        return texts[0].clone();
    }
    ["small", "medium", "large", "huge"]
        .iter()
        .zip(&texts)
        .map(|(name, text)| format!("{name} {text}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn group_table(html: &mut String, group: &ParamGroup) {
    let sizes: Vec<serde_json::Value> = (1..=4).map(tapiti_render::readout::full_params).collect();
    let head = ["Parameter", "Name", "Values", "Default", "What it does"];
    let _ = write!(
        html,
        "<div class=\"table-wrap\" tabindex=\"0\"><table><thead><tr>"
    );
    for h in head {
        let _ = write!(html, "<th>{h}</th>");
    }
    let _ = write!(html, "</tr></thead><tbody>");
    for field in &group.fields {
        let values = match &field.kind {
            FieldKind::Number { min, max, integer } => {
                if *integer {
                    format!("{min:.0} – {max:.0}")
                } else {
                    format!("{min} – {max}")
                }
            }
            FieldKind::Boolean => "on / off".into(),
            FieldKind::Choice(options) => options
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        };
        let _ = write!(
            html,
            "<tr id=\"param-{}\"><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            field.path.replace('.', "-"),
            escape(&field.path),
            escape(field.label.as_str()),
            escape(&values),
            escape(&defaults_text(field, &sizes)),
            escape(field.help.as_str())
        );
    }
    let _ = write!(html, "</tbody></table></div>");
}

// ---- Search ------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Hit {
    pub slug: &'static str,
    pub page_title: String,
    pub heading: String,
    pub id: String,
    /// The snippet of the section, escaped, with the words that were searched for in `<mark>`.
    pub snippet_html: String,
}

/// Longest query accepted, in characters.
pub const MAX_QUERY: usize = 80;

/// Sections containing every word of `query`, best first. Plain substring search, case-insensitive.
pub fn search(query: &str) -> Vec<Hit> {
    let query: String = query.chars().take(MAX_QUERY).collect();
    let words: Vec<String> = query
        .split_whitespace()
        .take(5)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, Hit)> = Vec::new();
    for page in docs().pages() {
        for section in &page.sections {
            let heading = section.heading.to_lowercase();
            let body = section.text.to_lowercase();
            if !words
                .iter()
                .all(|w| heading.contains(w) || body.contains(w))
            {
                continue;
            }
            let score: usize = words
                .iter()
                .map(|w| 5 * heading.matches(w).count() + body.matches(w).count())
                .sum();
            scored.push((
                score,
                Hit {
                    slug: page.slug,
                    page_title: page.title.clone(),
                    heading: if section.heading.is_empty() {
                        page.title.clone()
                    } else {
                        section.heading.clone()
                    },
                    id: section.id.clone(),
                    snippet_html: mark_words(&snippet(&section.text, &words[0]), &words),
                },
            ));
        }
    }
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().take(30).map(|(_, hit)| hit).collect()
}

/// About 160 characters of `text` around the first occurrence of `word`.
/// `text` escaped, with each of `words` (any case) wrapped in `<mark>`.
fn mark_words(text: &str, words: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    if lower.len() != chars.len() {
        return escape(text);
    }
    let needles: Vec<Vec<char>> = words.iter().map(|w| w.chars().collect()).collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let hit = needles
            .iter()
            .filter(|n| !n.is_empty() && lower[i..].starts_with(n))
            .map(Vec::len)
            .max();
        match hit {
            Some(len) => {
                let word: String = chars[i..i + len].iter().collect();
                out.push_str(&format!("<mark>{}</mark>", escape(&word)));
                i += len;
            }
            None => {
                out.push_str(&escape(&chars[i].to_string()));
                i += 1;
            }
        }
    }
    out
}

fn snippet(text: &str, word: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = word.chars().collect();
    let at = if lower.len() == chars.len() {
        (0..lower.len().saturating_sub(needle.len().saturating_sub(1)))
            .find(|&i| lower[i..].starts_with(&needle))
            .unwrap_or(0)
    } else {
        0
    };
    let start = at.saturating_sub(60);
    let end = (start + 160).min(chars.len());
    let mut out: String = chars[start..end]
        .iter()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_math_span_becomes_mathml() {
        for page in docs().pages() {
            let counted = page.html.matches("<math").count();
            assert_eq!(
                page.math_spans, page.math_rendered,
                "{}: a formula failed to parse",
                page.slug
            );
            assert_eq!(
                page.math_spans, counted,
                "{}: math spans and <math> elements differ",
                page.slug
            );
            assert!(!page.html.contains("math-error"), "{}", page.slug);
        }
    }

    #[test]
    fn the_model_page_has_all_its_formulas() {
        let page = docs().page("model").unwrap();
        assert!(page.math_spans > 200, "{}", page.math_spans);
    }

    /// A link into the repository (`../crates/...`) must point at a file or folder that exists, or GitHub shows a 404.
    #[test]
    fn repository_links_point_at_files_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut missing = Vec::new();
        for source in SOURCES {
            for part in source.en.split("](../").skip(1) {
                let path = part.split([')', '#', ' ']).next().unwrap_or("");
                if !root.join(path).exists() {
                    missing.push(format!("{}: ../{path}", source.file));
                }
            }
        }
        assert!(missing.is_empty(), "broken repository links: {missing:?}");
    }

    #[test]
    fn every_page_is_in_exactly_one_group() {
        let mut seen: Vec<&str> = GROUPS
            .iter()
            .flat_map(|g| g.slugs.iter().copied())
            .collect();
        for slug in &seen {
            assert!(
                docs().page(slug).is_some(),
                "{slug} is in a group but is not a page"
            );
        }
        seen.sort_unstable();
        let total = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), total, "a page is in two groups");
        for page in docs().pages() {
            assert!(seen.contains(&page.slug), "{} is in no group", page.slug);
        }
    }

    #[test]
    fn heading_ids_are_unique_and_links_resolve() {
        for page in docs().pages() {
            let mut ids = std::collections::HashSet::new();
            for entry in &page.toc {
                assert!(
                    ids.insert(entry.id.clone()),
                    "{}: duplicate id {}",
                    page.slug,
                    entry.id
                );
                assert!(page.html.contains(&format!("id=\"{}\"", entry.id)));
            }
            for href in page
                .html
                .split("href=\"")
                .skip(1)
                .map(|s| s.split('"').next().unwrap())
            {
                if let Some(rest) = href.strip_prefix("/docs/") {
                    let (slug, fragment) = rest.split_once('#').unwrap_or((rest, ""));
                    let target = docs()
                        .page(slug)
                        .unwrap_or_else(|| panic!("{}: broken link {href}", page.slug));
                    assert!(
                        fragment.is_empty() || target.html.contains(&format!("id=\"{fragment}\"")),
                        "{}: missing anchor in {href}",
                        page.slug
                    );
                } else if let Some(fragment) = href.strip_prefix('#') {
                    assert!(
                        page.html.contains(&format!("id=\"{fragment}\"")),
                        "{}: missing anchor {href}",
                        page.slug
                    );
                } else {
                    assert!(
                        href.starts_with("https://")
                            || href.starts_with("mailto:")
                            || href.starts_with("http://localhost"),
                        "{}: unresolved link {href}",
                        page.slug
                    );
                }
            }
        }
    }

    #[test]
    fn no_rendered_link_has_a_locale_prefix() {
        for page in docs().pages() {
            assert!(!page.html.contains("href=\"/en/"), "{}", page.slug);
            assert!(!page.html.contains("/pt-BR/"), "{}", page.slug);
        }
    }

    #[test]
    fn titles_exist() {
        for page in docs().pages() {
            assert!(!page.title.is_empty(), "{}", page.slug);
        }
    }

    #[test]
    fn search_finds_greek_letters_and_words() {
        assert!(!search("β").is_empty());
        assert!(!search("corridor").is_empty());
        assert!(search("xyzzyplugh").is_empty());
        assert!(search(&"a".repeat(500)).len() <= 30);
    }

    #[test]
    fn the_parameter_reference_lists_every_field() {
        let page = docs().page("parameters").unwrap();
        for group in tapiti_schema::groups() {
            for field in group.fields {
                assert!(
                    page.html.contains(&format!("<code>{}</code>", field.path)),
                    "{}",
                    field.path
                );
            }
        }
        // Every parameter has a row of its own to link to, and says its default.
        for group in tapiti_schema::groups() {
            for field in &group.fields {
                let id = format!("id=\"param-{}\"", field.path.replace('.', "-"));
                assert!(page.html.contains(&id), "{}", field.path);
            }
        }
        assert!(page.html.contains("<th>Default</th>"));
    }
}
