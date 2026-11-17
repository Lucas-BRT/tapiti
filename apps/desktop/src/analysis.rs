//! The reading: what the generated floor is, in the short labels of a readout, with the long explanation on hover.
//! Rules first, then the measures, the shapes, the doors, the generation phases and the layers the floor has.

use crate::charts::{self, Bar};
use crate::map::{Palette, SHAPE_CLASSES};
use crate::panels::{card, muted};
use crate::{class_label, summary, translate_log, translate_violation, App, GeneratedMap};
use eframe::egui::{self, Color32, Grid, RichText, Stroke, Ui};

/// One line of the readout: a short label, the value, and when the value is part of something a bar showing its share, with the
/// longer explanation for the hover.
pub struct Measure {
    pub label: &'static str,
    pub value: String,
    /// The share of a whole this value is, from 0 to 1, when there is a whole.
    pub share: Option<f32>,
    pub tip: String,
}

impl Measure {
    fn new(label: &'static str, value: impl Into<String>, tip: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            share: None,
            tip: tip.into(),
        }
    }

    fn of(mut self, whole: f64, part: f64) -> Self {
        self.share = (whole > 0.0).then(|| (part / whole).clamp(0.0, 1.0) as f32);
        self
    }
}

/// The measures of a floor, from `metrics_of`, in the order they are read. A bar is drawn only where the value is a share of a whole
/// that exists: loops of what fits, dead ends of the rooms, bridges of the corridors, the way to the exit of the diameter.
pub fn measure_rows(m: &tapiti_core::metrics::Metrics) -> Vec<Measure> {
    tapiti_render::readout::rows(m)
        .into_iter()
        .map(|row| Measure {
            label: row.label,
            value: row.value,
            share: row.share,
            tip: row.tip,
        })
        .collect()
}

/// Draws the rows of a readout, each of the exact width of the column: the label on the left, the share as a bar in the middle when
/// there is one, the value on the right.
fn measures(ui: &mut Ui, rows: &[Measure]) {
    let c = crate::theme::of(ui);
    let label_font = egui::FontId::proportional(12.0);
    let value_font = egui::FontId::new(12.0, crate::theme::medium());
    for row in rows {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Label,
                true,
                format!("{}: {}", row.label, row.value),
            )
        });
        let painter = ui.painter_at(rect);
        let mid = rect.center().y;
        painter.text(
            egui::pos2(rect.left(), mid),
            egui::Align2::LEFT_CENTER,
            row.label,
            label_font.clone(),
            c.text_muted,
        );
        let value = painter.text(
            egui::pos2(rect.right(), mid),
            egui::Align2::RIGHT_CENTER,
            &row.value,
            value_font.clone(),
            c.text,
        );
        if let Some(share) = row.share {
            let from = rect.left() + (rect.width() * 0.34).max(90.0);
            let to = (value.left() - 8.0).max(from);
            let track =
                egui::Rect::from_min_max(egui::pos2(from, mid - 3.0), egui::pos2(to, mid + 3.0));
            painter.rect_filled(track, 0.0, c.border_variant);
            let fill = egui::Rect::from_min_size(
                track.min,
                egui::vec2(
                    (track.width() * share).max(if share > 0.0 { 2.0 } else { 0.0 }),
                    6.0,
                ),
            );
            painter.rect_filled(fill, 0.0, c.measure);
        }
        if !row.tip.is_empty() {
            response.on_hover_text(&row.tip);
        }
    }
}

/// The short name of a phase of the generator.
fn phase_name(name: &str) -> &'static str {
    match name {
        "rooms" => "rooms",
        "candidates" => "candidates",
        "tree" => "tree",
        "β_max" => "β max",
        "cycles" => "cycles",
        "leaf chambers" => "leaf chambers",
        "stairs" => "stairs",
        "other levels" => "other levels",
        _ => "other",
    }
}

/// The five rules and what each one checks, in few words.
use tapiti_render::readout::RULES;

/// A check mark or a cross, drawn: Plex Mono has neither glyph.
fn mark(ui: &mut Ui, ok: bool) {
    let c = crate::theme::of(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    let p = |x: f32, y: f32| rect.min + egui::vec2(x, y);
    let (color, points): (Color32, Vec<egui::Pos2>) = if ok {
        (c.success, vec![p(1.5, 6.5), p(4.5, 9.5), p(10.5, 2.5)])
    } else {
        (c.error, vec![p(2.0, 2.0), p(10.0, 10.0)])
    };
    let stroke = Stroke::new(1.8, color);
    ui.painter().add(egui::Shape::line(points, stroke));
    if !ok {
        ui.painter()
            .line_segment([p(10.0, 2.0), p(2.0, 10.0)], stroke);
    }
}

/// Which of the five rules a violation breaks, by the code its message starts with.
fn broken(map: &GeneratedMap, code: &str) -> bool {
    map.violations.iter().any(|v| v.starts_with(code))
}

fn rules_section(ui: &mut Ui, map: &GeneratedMap) {
    let c = crate::theme::of(ui);
    card(ui, "Rules", |ui| {
        ui.horizontal(|ui| {
            let (word, color) = if map.violations.is_empty() {
                ("valid", c.success)
            } else {
                ("violations", c.error)
            };
            ui.label(
                RichText::new(word)
                    .font(egui::FontId::new(13.0, crate::theme::medium()))
                    .color(color),
            );
            // At the right: what identifies the floor on the screen. Two windows with the same hash show the same floor.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(crate::panels::floor_hash(map))
                        .font(egui::FontId::new(12.0, crate::theme::medium()))
                        .color(c.text),
                )
                .on_hover_text(
                    "A hash of the seed and the full Λ: the same hash is the same floor",
                );
                ui.label(muted(ui, "hash"));
            });
        });
        for (code, name, detail) in RULES {
            ui.horizontal(|ui| {
                mark(ui, !broken(map, code));
                ui.label(RichText::new(format!("{code} {name}")).color(c.text));
                ui.label(muted(ui, detail));
            });
        }
        // Whatever else the validator said (the intent, the layers, the stairs), in words.
        let others: Vec<&String> = map
            .violations
            .iter()
            .filter(|v| !RULES.iter().any(|(code, _, _)| v.starts_with(code)))
            .collect();
        let shown_total = map.violations.len();
        for violation in others
            .into_iter()
            .chain(
                map.violations
                    .iter()
                    .filter(|v| RULES.iter().any(|(code, _, _)| v.starts_with(code))),
            )
            .take(12)
        {
            ui.label(RichText::new(translate_violation(violation)).color(c.error));
        }
        if shown_total > 12 {
            ui.label(muted(ui, format!("and {} more", shown_total - 12)));
        }
    });
}

pub fn analysis_tab(app: &mut App, ui: &mut Ui) {
    let Some(map) = app.generated.clone() else {
        ui.label(muted(ui, "No floor yet."));
        return;
    };
    let pal = Palette::new(ui.visuals().dark_mode);
    let m = &map.metrics;

    // The room that is picked on the plan, described where the rest of the floor is read.
    if let Some(room) = app.selected.and_then(|id| map.floor.rooms.get(id)) {
        card(ui, "Selected room", |ui| {
            ui.label(RichText::new(crate::map::describe(&map.floor, room)).color(pal.accent));
            ui.label(muted(
                ui,
                format!(
                    "{} door{} used · [ and ] pick another, Esc lets go",
                    room.used.len(),
                    if room.used.len() == 1 { "" } else { "s" }
                ),
            ));
        });
    }
    rules_section(ui, &map);

    card(ui, "Measures", |ui| measures(ui, &measure_rows(m)));

    // ---- layers: only those the floor has ----
    let num = |label: &'static str, value: String, tip: String| Measure::new(label, value, tip);
    if let Some(h) = &m.height {
        card(ui, "Height", |ui| {
            measures(
                ui,
                &[
                    num("range", format!("{:.0}", h.range), format!("From {:.0} to {:.0}.", h.min, h.max)),
                    num(
                        "steepest",
                        format!("{:.2}", h.max_slope),
                        format!(
                            "Rise over run of the steepest corridor; the average is {:.2}, the limit {}.",
                            h.mean_slope, map.floor.params.environment.height.max_slope
                        ),
                    ),
                ],
            );
        });
    }
    if let (Some(e), Some(field)) = (
        &m.events,
        map.floor
            .environment
            .as_ref()
            .and_then(|e| e.events.as_ref()),
    ) {
        card(ui, "Events", |ui| {
            for (k, (kind, spec)) in field
                .kinds
                .iter()
                .zip(&map.floor.params.environment.events.list)
                .enumerate()
            {
                ui.horizontal(|ui| {
                    let (swatch, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(swatch, 0.0, crate::map::event_color(k));
                    ui.label(format!(
                        "{}: {} of {}",
                        kind.name,
                        kind.nodes.len(),
                        spec.count
                    ));
                });
            }
            if e.missing > 0 {
                ui.label(muted(
                    ui,
                    format!("{} not placed: no room that fits was left", e.missing),
                ));
            }
        });
    }
    if let Some(l) = &m.light {
        card(ui, "Light", |ui| {
            measures(
                ui,
                &[
                    num(
                        "dark rooms",
                        l.dark_rooms.to_string(),
                        format!(
                            "Rooms with less than a twentieth of full light; the average is {:.0}%.",
                            100.0 * l.mean
                        ),
                    ),
                    num(
                        "sources",
                        format!("{} + {}", l.openings, l.torches),
                        "Openings to the surface plus direct sources: where light comes from.".to_string(),
                    ),
                ],
            );
        });
    }
    if let Some(c) = &m.climate {
        card(ui, "Temperature", |ui| {
            measures(
                ui,
                &[
                    num(
                        "range",
                        format!("{:.0}° to {:.0}°", c.coldest, c.hottest),
                        format!("The average is {:.1}°.", c.mean),
                    ),
                    num(
                        "openings",
                        c.openings.to_string(),
                        "Openings to the surface: the entrance and the rooms that open outside."
                            .to_string(),
                    ),
                ],
            );
        });
    }
    if let Some(w) = &m.water {
        card(ui, "Water", |ui| {
            measures(
                ui,
                &[
                    num(
                        "wet rooms",
                        w.wet_rooms.to_string(),
                        format!(
                            "{} sources pour; the deepest pool is {:.0}.",
                            w.sources, w.deepest
                        ),
                    ),
                    num(
                        "humidity",
                        format!("{:.0}%", 100.0 * w.mean_humidity),
                        "The average over the rooms, falling with the distance to water."
                            .to_string(),
                    ),
                ],
            );
        });
    }

    // ---- shapes ----
    card(ui, "Shapes", |ui| {
        let bars: Vec<Bar> = SHAPE_CLASSES
            .iter()
            .filter_map(|class| {
                let n = *m.by_class.get(*class)?;
                Some(Bar::new(class_label(class), n as f64, pal.shape(class)))
            })
            .collect();
        let max = bars.iter().map(|b| b.value).fold(1.0, f64::max);
        charts::hbars(ui, &bars, max);
    });

    // ---- doors per room ----
    card(ui, "Doors per room", |ui| {
        let mut counts = [0usize; 5];
        for room in map.floor.rooms.iter().filter(|r| r.alive) {
            counts[room.used.len().min(4)] += 1;
        }
        let labels = ["0 doors", "1 door", "2 doors", "3 doors", "4+ doors"];
        let measure = crate::theme::of(ui).measure;
        let bars: Vec<Bar> = counts
            .iter()
            .enumerate()
            .filter(|(i, n)| *i > 0 || **n > 0)
            .map(|(i, n)| Bar::new(labels[i], *n as f64, measure))
            .collect();
        let max = bars.iter().map(|b| b.value).fold(1.0, f64::max);
        ui.label(muted(ui, "Rooms, by the doors they use"));
        ui.scope(|ui| charts::columns(ui, &bars, max, 96.0))
            .response
            .on_hover_text(
                "One door is a dead end, two make a hallway, three or more a crossroads.",
            );
    });

    // ---- phases ----
    if !map.floor.phases.is_empty() {
        let total: f64 = map.floor.phases.iter().map(|(_, ms)| *ms).sum();
        let longest = map
            .floor
            .phases
            .iter()
            .map(|(_, ms)| *ms)
            .fold(0.0, f64::max);
        card(ui, "Phases", |ui| {
            let rows: Vec<Measure> = map
                .floor
                .phases
                .iter()
                .map(|(name, ms)| {
                    Measure::new(phase_name(name), format!("{ms:.1} ms"), "The last attempt; a floor that fails a check is built again from a nearby seed.")
                        .of(longest, *ms)
                })
                .collect();
            measures(ui, &rows);
            ui.label(muted(
                ui,
                format!(
                    "{total:.1} ms in all, {} attempt{}; each bar is relative to the longest phase",
                    map.draws,
                    if map.draws == 1 { "" } else { "s" }
                ),
            ));
        });
    }

    // ---- the details ----
    let sections = metric_sections(&map);
    let count = sections.iter().map(|(_, rows)| rows.len()).sum();
    let bar = crate::theme::GroupBar {
        title: "All the numbers",
        count,
        changed: 0,
        default_open: false,
    };
    crate::theme::group(ui, ui.make_persistent_id("all-numbers"), bar, None, |ui| {
        for (title, rows) in sections {
            ui.label(RichText::new(title).font(egui::FontId::new(13.0, crate::theme::display())));
            Grid::new(title)
                .num_columns(2)
                .striped(true)
                .min_col_width(ui.available_width() / 2.0 - 12.0)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (label, value, help) in rows {
                        ui.label(label).on_hover_text(help);
                        ui.label(
                            RichText::new(value)
                                .font(egui::FontId::new(12.0, crate::theme::medium())),
                        );
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
        }
        ui.label(muted(ui, summary(&map)));
    });
    if !map.floor.log.is_empty() {
        let bar = crate::theme::GroupBar {
            title: "Generator log",
            count: map.floor.log.len(),
            changed: 0,
            default_open: false,
        };
        crate::theme::group(
            ui,
            ui.make_persistent_id("generator-log"),
            bar,
            None,
            |ui| {
                for line in &map.floor.log {
                    ui.label(muted(ui, translate_log(line)));
                }
            },
        );
    }
}

type Row = (&'static str, String, &'static str);

fn metric_sections(map: &GeneratedMap) -> Vec<(&'static str, Vec<Row>)> {
    let m = &map.metrics;
    let opt = |v: Option<f64>| v.map(|v| format!("{v:.0}")).unwrap_or_else(|| "—".into());
    vec![
        (
            "Map size",
            vec![
                (
                    "Main rooms",
                    m.rooms.to_string(),
                    "Active rooms, not counting leaf chambers.",
                ),
                (
                    "Leaf chambers",
                    m.leaf_chambers.to_string(),
                    "Extra rooms linked by a single corridor.",
                ),
                (
                    "T/X junctions",
                    m.junctions.to_string(),
                    "Points where three or four arms meet.",
                ),
                (
                    "Corridor segments",
                    m.edges.to_string(),
                    "Graph edges; each junction arm counts as a segment.",
                ),
            ],
        ),
        (
            "Cycles and density",
            vec![
                (
                    "Current cycles β",
                    m.beta.to_string(),
                    "Number of independent cycles; a tree has zero.",
                ),
                (
                    "Cycle target",
                    m.beta_target.to_string(),
                    "Target computed from the density and the generator's reference.",
                ),
                (
                    "Reference βmax",
                    m.beta_max.to_string(),
                    "Greedy estimate of the cycles that fit, including junctions.",
                ),
                (
                    "Density δ",
                    format!("{:.2}", m.delta),
                    "Ratio between current cycles and the βmax reference.",
                ),
            ],
        ),
        (
            "Paths and connectivity",
            vec![
                (
                    "Doors used per room",
                    format!("{:.2}", m.mean_degree),
                    "Average number of corridors linked to each room.",
                ),
                (
                    "Single-door rooms",
                    m.leaves.to_string(),
                    "Rooms with only one connection; includes leaf chambers, the entrance and the exit.",
                ),
                (
                    "Bridges",
                    format!("{} · {:.0}%", m.bridges, m.bridge_share * 100.0),
                    "Segments whose removal would disconnect part of the map.",
                ),
                (
                    "Articulation rooms",
                    m.articulations.to_string(),
                    "Rooms whose removal would split the graph.",
                ),
                (
                    "Steps to the exit",
                    opt(m.hops_to_exit),
                    "Number of rooms on the shortest path from the entrance to the exit.",
                ),
                (
                    "Diameter",
                    format!("{:.0}", m.diameter),
                    "Largest minimum distance between two rooms, in steps.",
                ),
                (
                    "Independent routes to the exit",
                    m.routes_to_exit.map_or("—".into(), |n| n.to_string()),
                    "Paths between the entrance and the exit that share no corridors.",
                ),
                (
                    "Shortest cycle",
                    m.cycle_girth
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".into()),
                    "Number of segments in the map's shortest cycle.",
                ),
                (
                    "Side branches",
                    m.side_branches.to_string(),
                    "Optional components attached to the main path by a single passage.",
                ),
                (
                    "Branch depth",
                    m.deepest_branch.to_string(),
                    "Largest number of rooms in a side branch.",
                ),
                (
                    "Highest degree",
                    m.max_node_degree.to_string(),
                    "Largest number of arms joined at one point.",
                ),
                (
                    "Geometric path",
                    m.geometric_path_to_exit
                        .map_or("—".into(), |l| format!("{l:.0} px")),
                    "Physical length of the shortest path to the exit.",
                ),
                (
                    "Connectivity λ₂",
                    format!("{:.3}", m.lambda2),
                    "Larger values tend to indicate less dependence on bottlenecks.",
                ),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::metrics::metrics_of;
    use tapiti_core::params::{params_of_size, rich_of_size, Size};
    use tapiti_core::stage::stage;

    fn row<'a>(rows: &'a [Measure], label: &str) -> &'a Measure {
        rows.iter().find(|r| r.label == label).unwrap()
    }

    #[test]
    fn every_number_of_the_readout_is_the_one_the_metrics_hold() {
        let floor = stage(279, &rich_of_size(Size::Medium), 8).d;
        let m = metrics_of(&floor);
        let rows = measure_rows(&m);
        assert_eq!(
            row(&rows, "rooms").value,
            format!("{}+{}", m.rooms, m.leaf_chambers)
        );
        assert_eq!(row(&rows, "corridors").value, m.edges.to_string());
        assert_eq!(
            row(&rows, "loops β").value,
            format!("{} / {}", m.beta, m.beta_max)
        );
        assert_eq!(row(&rows, "single-door rooms").value, m.leaves.to_string());
        assert!(row(&rows, "bridges")
            .value
            .starts_with(&m.bridges.to_string()));
        assert_eq!(
            row(&rows, "steps to exit").value,
            format!("{:.0}", m.hops_to_exit.unwrap())
        );
        assert_eq!(
            row(&rows, "routes → exit").value,
            m.routes_to_exit.unwrap().to_string()
        );
        assert_eq!(row(&rows, "λ₂").value, format!("{:.3}", m.lambda2));
    }

    #[test]
    fn the_explanations_quote_the_numbers_of_the_floor_they_explain() {
        for seed in 270..275 {
            let m = metrics_of(&stage(seed, &rich_of_size(Size::Medium), 8).d);
            let rows = measure_rows(&m);
            let rooms = &row(&rows, "rooms").tip;
            assert!(
                rooms.contains(&format!("{} main rooms", m.rooms)),
                "{seed}: {rooms}"
            );
            assert!(
                rooms.contains(&format!("{} leaf chambers", m.leaf_chambers)),
                "{seed}: {rooms}"
            );
            let loops = &row(&rows, "loops β").tip;
            for part in [
                format!("{} now", m.beta),
                format!("{} aimed for", m.beta_target),
                format!("room for {}", m.beta_max),
            ] {
                assert!(loops.contains(&part), "{seed}: {loops}");
            }
        }
    }

    #[test]
    fn a_bar_is_a_share_of_a_whole_that_exists() {
        let floor = stage(279, &rich_of_size(Size::Medium), 8).d;
        let m = metrics_of(&floor);
        let rows = measure_rows(&m);
        for r in &rows {
            if let Some(share) = r.share {
                assert!((0.0..=1.0).contains(&share), "{}: {share}", r.label);
            }
        }
        let loops = row(&rows, "loops β").share.unwrap();
        assert!((loops - m.beta as f32 / m.beta_max as f32).abs() < 1e-6);
        let bridges = row(&rows, "bridges").share.unwrap();
        assert!((f64::from(bridges) - m.bridge_share).abs() < 1e-6);
        // Counts that are not part of a whole have no bar.
        for label in [
            "rooms",
            "corridors",
            "diameter",
            "λ₂",
            "doors per room",
            "routes → exit",
        ] {
            assert!(row(&rows, label).share.is_none(), "{label}");
        }
    }

    #[test]
    fn a_floor_without_an_exit_shows_dashes_and_no_bar_for_the_way_out() {
        let mut p = params_of_size(Size::Small);
        p.entrance = false;
        p.exit_room = false;
        let m = metrics_of(&stage(1, &p, 8).d);
        let rows = measure_rows(&m);
        let hops = row(&rows, "steps to exit");
        assert_eq!(hops.value, "—");
        assert!(hops.share.is_none());
        assert_eq!(row(&rows, "routes → exit").value, "—");
    }
}
