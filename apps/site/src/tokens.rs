//! The colour tokens of `assets/styles.css`, read as text and checked: every piece of text is at least 7:1 on every surface it sits on,
//! in the light theme and in both places the dark theme is written (the system's, and the chosen one), and those two agree.

use std::collections::BTreeMap;

const CSS: &str = include_str!("../assets/styles.css");

/// The `--name: #hex` declarations of the block that opens with `opening` (up to its closing brace).
fn block(opening: &str) -> BTreeMap<String, String> {
    let start = CSS
        .find(opening)
        .unwrap_or_else(|| panic!("no block {opening}"))
        + opening.len();
    let end = start + CSS[start..].find('}').unwrap();
    CSS[start..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(';');
            let (name, rest) = line.split_once(':')?;
            let value = rest.split("/*").next()?.trim().trim_end_matches(';').trim();
            (name.starts_with("--") && value.starts_with('#') && value.len() == 7)
                .then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}

fn light() -> BTreeMap<String, String> {
    block("  :root {\n    color-scheme: light;")
}

fn dark_by_system() -> BTreeMap<String, String> {
    let mut all = light();
    all.extend(block(
        ":root:not([data-theme=\"light\"]) {\n      color-scheme: dark;",
    ));
    all
}

fn dark_chosen() -> BTreeMap<String, String> {
    let mut all = light();
    all.extend(block(
        ":root[data-theme=\"dark\"] {\n    color-scheme: dark;",
    ));
    all
}

fn channel(hex: &str, at: usize) -> f64 {
    let v = f64::from(u8::from_str_radix(&hex[at..at + 2], 16).unwrap()) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(hex: &str) -> f64 {
    0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5)
}

fn ratio(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

const TEXT: [&str; 6] = [
    "--text",
    "--muted",
    "--amber",
    "--measure",
    "--ok",
    "--error",
];
const SURFACES: [&str; 5] = ["--bar", "--panel", "--bg", "--element", "--element-hover"];

#[test]
fn all_text_is_seven_to_one_on_every_surface_in_every_theme() {
    for (name, theme) in [
        ("light", light()),
        ("dark by system", dark_by_system()),
        ("dark chosen", dark_chosen()),
    ] {
        for text in TEXT {
            for surface in SURFACES {
                let r = ratio(&theme[text], &theme[surface]);
                assert!(r >= 7.0, "{name}: {text} on {surface} is {r:.2}");
            }
        }
    }
}

#[test]
fn controls_keep_their_outline_and_the_main_action_its_text() {
    for (name, theme) in [
        ("light", light()),
        ("dark by system", dark_by_system()),
        ("dark chosen", dark_chosen()),
    ] {
        for surface in ["--panel", "--bg", "--bar"] {
            let r = ratio(&theme["--border"], &theme[surface]);
            assert!(r >= 3.0, "{name}: border on {surface} is {r:.2}");
            let r = ratio(&theme["--amber"], &theme[surface]);
            assert!(r >= 3.0, "{name}: the focus ring on {surface} is {r:.2}");
        }
        let r = ratio(&theme["--on-primary"], &theme["--primary"]);
        assert!(r >= 7.0, "{name}: text on the main action is {r:.2}");
    }
}

#[test]
fn the_two_dark_blocks_say_the_same() {
    assert_eq!(dark_by_system(), dark_chosen());
}
