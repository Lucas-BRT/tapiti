//! The window's layout: top bar, bottom bar, the tabbed side panel and the map.

use crate::analysis::analysis_tab;
use crate::map::{self, Palette, Xf};
use crate::params_ui::{field_row, FieldKind, ParamField};
use crate::pick::map_object_at;
use crate::theme;
use crate::{get_path, set_path, size_description, size_label, App, Experiment, Tab};
use eframe::egui::{
    self, Align, Button, Color32, CursorIcon, Frame, Grid, Layout, Panel, RichText, ScrollArea,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
use serde_json::Value;
use tapiti_core::par;
use tapiti_core::params::{Size, TopologyProfile};

pub(crate) fn muted(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .small()
        .color(ui.visuals().weak_text_color())
}

pub(crate) fn card(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    theme::card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(15.0));
        ui.add_space(theme::GAP);
        body(ui);
    });
    ui.add_space(theme::GAP + 2.0);
}

fn primary(ui: &Ui, text: &str) -> Button<'static> {
    Button::new(
        RichText::new(text.to_string())
            .strong()
            .color(theme::ON_PRIMARY),
    )
    .fill(theme::of(ui).primary)
}

pub fn menu_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("menu")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Λ…").clicked() {
                        ui.close();
                        app.open_file();
                    }
                    if ui.button("Save Λ…").clicked() {
                        ui.close();
                        app.save_file();
                    }
                    ui.separator();
                    if ui
                        .button("Copy JSON")
                        .on_hover_text("Size, seed and the full Λ")
                        .clicked()
                    {
                        ui.ctx().copy_text(app.export_json());
                        app.status = "Parameters copied as JSON.".into();
                        ui.close();
                    }
                    if ui
                        .add_enabled(app.generated.is_some(), Button::new("Copy SVG"))
                        .on_hover_text(
                            "The floor as an SVG image, with the grid and labels as shown",
                        )
                        .clicked()
                    {
                        if let Some(generated) = &app.generated {
                            let style = tapiti_render::Style {
                                theme: if ui.visuals().dark_mode {
                                    tapiti_render::Theme::Dark
                                } else {
                                    tapiti_render::Theme::Light
                                },
                                grid: app.overlays.cells,
                                labels: app.overlays.labels,
                                ..tapiti_render::Style::default()
                            };
                            ui.ctx()
                                .copy_text(tapiti_render::floor_svg(&generated.floor, &style));
                            app.status = "Floor copied as SVG.".into();
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(!app.changed.is_empty(), Button::new("Reset all settings"))
                        .on_hover_text("Restore Classic and the defaults of the current size")
                        .clicked()
                    {
                        app.reset_all();
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    let mut theme = ui.options(|o| o.theme_preference);
                    for (value, name) in [
                        (egui::ThemePreference::Dark, "Dark theme"),
                        (egui::ThemePreference::Light, "Light theme"),
                        (egui::ThemePreference::System, "Follow the system"),
                    ] {
                        ui.radio_value(&mut theme, value, name);
                    }
                    theme::set_preference(ui.ctx(), theme);
                });
                ui.menu_button("Tools", |ui| {
                    ui.label(muted(ui, "Guided experiments"));
                    for (preset, name) in [
                        (Experiment::Shapes, "Shapes and diagonals"),
                        (Experiment::Cycles, "More routes"),
                        (Experiment::Junctions, "T/X junctions"),
                        (Experiment::CircularRooms, "Circular rooms"),
                    ] {
                        if ui.button(name).clicked() {
                            app.set_preset(preset);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("JSON editor…").clicked() {
                        app.show_json = true;
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Shortcuts").clicked() {
                        app.show_shortcuts = true;
                        ui.close();
                    }
                });
            });
        });
}

pub fn shortcuts_window(app: &mut App, ctx: &egui::Context) {
    egui::Window::new("Shortcuts")
        .open(&mut app.show_shortcuts)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            for line in [
                "Alt+1–4 switches the size · Alt+R random seed",
                "Alt+Left / Right: previous / next seed",
                "Esc cancels a generation",
                "Drag moves the map · wheel zooms at the cursor",
                "Double-click fits the map to the window",
            ] {
                ui.label(line);
            }
        });
}

pub fn top_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("top")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Tapiti");
                ui.separator();
                size_picker(app, ui);
                ui.separator();
                ui.label("Seed");
                if ui
                    .button("<")
                    .on_hover_text("Previous seed (Alt+Left)")
                    .clicked()
                {
                    app.set_seed(app.seed.saturating_sub(1));
                }
                let seed = ui.add(
                    TextEdit::singleline(&mut app.seed_draft)
                        .desired_width(92.0)
                        .hint_text("0–4294967295"),
                );
                if seed.lost_focus() && app.seed_draft != app.seed.to_string() {
                    app.commit_seed_draft();
                }
                if ui
                    .button(">")
                    .on_hover_text("Next seed (Alt+Right)")
                    .clicked()
                {
                    app.set_seed(app.seed.saturating_add(1));
                }
                if ui
                    .button("Random")
                    .on_hover_text("New random seed (Alt+R)")
                    .clicked()
                {
                    app.random_seed();
                }
                ui.separator();
                if app.busy && app.can_cancel() {
                    if ui
                        .add(
                            Button::new(RichText::new("Cancel").strong().color(Color32::WHITE))
                                .fill(Palette::new(ui.visuals().dark_mode).danger),
                        )
                        .on_hover_text("Stop this generation (Esc). The floor on the screen stays.")
                        .clicked()
                    {
                        app.cancel_generation();
                    }
                } else if ui
                    .add_enabled(
                        !app.busy && app.params.intent_error().is_none(),
                        primary(ui, "Generate"),
                    )
                    .on_hover_text("Generate again with the current Λ")
                    .clicked()
                {
                    app.generate_now();
                }
                if app.busy {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let dark = ui.visuals().dark_mode;
                    if ui
                        .button(if dark { "Light" } else { "Dark" })
                        .on_hover_text("Switch between the paper theme and the dark theme")
                        .clicked()
                    {
                        let next = if dark {
                            egui::ThemePreference::Light
                        } else {
                            egui::ThemePreference::Dark
                        };
                        theme::set_preference(ui.ctx(), next);
                    }
                });
            });
            ui.add_space(4.0);
        });
}

pub fn bottom_bar(app: &mut App, ui: &mut Ui) {
    Panel::bottom("bottom")
        .frame(theme::bar_frame(ui))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(muted(ui, &app.status));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(muted(ui, format!("{} threads", par::threads())));
                    if !app.changed.is_empty() {
                        ui.separator();
                        if ui
                            .small_button("Reset")
                            .on_hover_text("Restore Classic and the defaults of the current size")
                            .clicked()
                        {
                            app.reset_all();
                        }
                        ui.label(muted(ui, format!("{} changed", app.changed.len())));
                    }
                });
            });
        });
}

/// Size changes keep the exploration intent; geometry remains in the side panel.
fn size_picker(app: &mut App, ui: &mut Ui) {
    let mut size = None;
    ui.label(RichText::new("Size").strong());
    for (n, s) in Size::ALL.into_iter().enumerate() {
        let hint = format!("{}\nAlt+{}", size_description(s), n + 1);
        if ui
            .selectable_label(app.size == s, size_label(s))
            .on_hover_text(hint)
            .clicked()
        {
            size = Some(s);
        }
    }
    if let Some(s) = size {
        app.set_size(s);
    }
}

fn intent_description(profile: TopologyProfile) -> &'static str {
    profile.guide().makes
}

fn intent_fields(profile: TopologyProfile) -> &'static [&'static str] {
    match profile {
        TopologyProfile::SpineWithDetours => &[
            "exitMinHops",
            "topology.minBranches",
            "topology.minBranchDepth",
            "topology.minDetours",
        ],
        TopologyProfile::HubAndWings => &["exitMinHops", "topology.minHubDegree"],
        TopologyProfile::Ring => &["exitMinHops"],
        TopologyProfile::Backbone => &[
            "exitMinHops",
            "topology.minBranches",
            "topology.minBranchDepth",
        ],
        TopologyProfile::Hub => &["topology.minHubDegree"],
        TopologyProfile::RedundantExit => &["exitMinHops", "topology.minExitRoutes"],
        TopologyProfile::LongLoop => &["topology.minCycleLength"],
        TopologyProfile::Classic => &["exitMinHops", "delta"],
    }
}

/// The settings the layout card lists, as the schema paths; the rest stay in the advanced groups.
fn layout_fields(layout: tapiti_core::params::Layout) -> &'static [&'static str] {
    match layout {
        tapiti_core::params::Layout::Grid => &["layoutJitter", "drop"],
        tapiti_core::params::Layout::Growth => &["growth.rooms", "growth.start"],
        tapiti_core::params::Layout::Bsp => &["growth.rooms"],
    }
}

/// The layout is the first choice: it decides how rooms are placed, and some intents only work with one of them.
fn layout_card(app: &mut App, ui: &mut Ui) {
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let layout = app.params.layout;
    let mut edits = Vec::new();
    card(ui, "Layout", |ui| {
        Grid::new("layout_controls")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                let levels = app.params.levels.count > 1;
                let paths = std::iter::once("layout")
                    .chain(
                        layout_fields(layout)
                            .iter()
                            .copied()
                            .filter(|p| crate::relevance::applies(p, &app.params)),
                    )
                    .chain(["levels.count"])
                    .chain(
                        ["levels.stairs", "levels.storey"]
                            .into_iter()
                            .filter(|_| levels),
                    );
                for path in paths {
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        let default = app.default_ref(path);
                        if let Some(next) = field_row(ui, field, &value, default) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.label(muted(ui, layout.guide()));
    });
    for (path, value) in edits {
        app.change_param(&path, value);
    }
    // Leaving Growth drops an intent that only Growth can run.
    if !app.params.layout.allows(app.params.topology.profile) {
        app.set_intent(TopologyProfile::Classic);
    }
}

fn intent_card(app: &mut App, ui: &mut Ui) {
    let profile = app.params.topology.profile;
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let mut selected = None;
    let mut edits = Vec::new();
    card(ui, "Exploration intent", |ui| {
        if let Some(field) = app
            .groups
            .iter()
            .flat_map(|g| &g.fields)
            .find(|f| f.path == "topology.profile")
        {
            if let FieldKind::Choice(options) = &field.kind {
                let value = get_path(&current, &field.path)
                    .and_then(Value::as_str)
                    .unwrap_or("classic");
                let name = options
                    .iter()
                    .find(|(key, _)| key == value)
                    .map_or(value, |(_, label)| label.as_str());
                egui::ComboBox::from_id_salt("exploration_intent")
                    .width(ui.available_width())
                    .selected_text(RichText::new(name).strong())
                    .show_ui(ui, |ui| {
                        for (key, label) in options {
                            if let Ok(option) = serde_json::from_value::<TopologyProfile>(
                                Value::String(key.clone()),
                            ) {
                                // A layout other than Growth cannot run an intent that plans its own rooms.
                                if !app.params.layout.allows(option) {
                                    continue;
                                }
                                if ui
                                    .selectable_label(profile == option, label)
                                    .on_hover_text(intent_description(option))
                                    .clicked()
                                    && profile != option
                                {
                                    selected = Some(option);
                                }
                            }
                        }
                    });
            }
        }
        let guide = profile.guide();
        ui.label(muted(ui, guide.makes));
        ui.add_space(4.0);
        for (title, text) in [
            ("Needs", guide.needs),
            ("Sets for you", guide.sets),
            ("With zones", guide.zones),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(format!("{title}:")).small().strong());
                ui.label(RichText::new(text).small());
            });
        }
        ui.add_space(theme::GAP);
        Grid::new("intent_controls")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in intent_fields(profile) {
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == *path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, None) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.add_space(theme::GAP);
        ui.label(RichText::new("Zones").strong());
        let blocker = app.params.zone_bottleneck_blocker();
        Grid::new("zone_controls")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in ["zones.count", "zones.bottleneck"] {
                    let shown =
                        path == "zones.count" || (app.params.zone_count() > 1 && blocker.is_none());
                    if !shown {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, None) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        if app.params.zone_count() > 1 {
            if let Some(reason) = blocker {
                ui.label(muted(ui, reason));
            }
        }
        if let Some(error) = app.params.intent_error() {
            ui.add_space(theme::GAP);
            ui.colored_label(theme::of(ui).error, error);
        } else if let Some(map) = app
            .generated
            .as_ref()
            .filter(|m| m.floor.params.topology.profile == profile)
        {
            ui.separator();
            ui.label(muted(ui, "Last generated floor"));
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} steps to exit",
                    map.metrics
                        .hops_to_exit
                        .map_or("—".to_string(), |h| format!("{h:.0}"))
                ));
                ui.label(format!("{} side branches", map.metrics.side_branches));
                ui.label(format!("Depth {}", map.metrics.deepest_branch));
            });
            if !map.violations.is_empty() {
                ui.colored_label(
                    theme::of(ui).error,
                    "Some requirements did not fit. See Analysis.",
                );
            }
        }
    });
    if let Some(profile) = selected {
        app.set_intent(profile);
    } else {
        for (path, value) in edits {
            app.change_param(&path, value);
        }
    }
}

/// Whether a group of parameters applies to the layout: the growth settings only to the grown layout.
fn group_applies(id: &str, layout: tapiti_core::params::Layout) -> bool {
    id != "growth" || layout == tapiti_core::params::Layout::Growth
}

pub fn side_panel(app: &mut App, ui: &mut Ui) {
    Panel::right("side")
        .frame(theme::panel_frame(ui))
        .default_size(400.0)
        .size_range(340.0..=600.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            // Intent and tuning share one panel; analysis keeps the generated result.
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    for (tab, title) in [
                        (Tab::Settings, "Design"),
                        (Tab::Advanced, "Advanced"),
                        (Tab::Analysis, "Analysis"),
                    ] {
                        if theme::tab(ui, title, app.tab == tab).clicked() {
                            app.tab = tab;
                        }
                    }
                });
            });
            ui.add_space(theme::GAP);
            // The clear button first, from the right, and the box takes what is left: a box sized as "the width left minus a guess" is
            // a few pixels too wide, and in a side panel (which grows to fit its content) that made the panel widen at every repaint.
            // (Inside a `horizontal`, so the right-to-left layout takes the height of one row, not what is left of the panel.)
            // The search is for the parameters only.
            if app.tab == Tab::Advanced {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(!app.filter.is_empty(), Button::new("×"))
                            .on_hover_text("Clear the search")
                            .clicked()
                        {
                            app.filter.clear();
                        }
                        ui.add(
                            TextEdit::singleline(&mut app.filter)
                                .hint_text("Search every parameter…")
                                .desired_width(ui.available_width()),
                        );
                    });
                });
            }
            ui.separator();
            // Each tab keeps its own scroll position and starts at the top: with one shared area, leaving a long tab (the Analysis, scrolled
            // down) for a short one left the content to slide back up to fit.
            let mut area = ScrollArea::vertical()
                .id_salt(app.tab as u8)
                .auto_shrink(false);
            if app.generated.is_some() {
                if let Some(offset) = app.scroll_to.take() {
                    area = area.vertical_scroll_offset(offset);
                }
            }
            area.show(ui, |ui| match app.tab {
                Tab::Settings => design_tab(app, ui),
                Tab::Advanced => advanced_tab(app, ui),
                Tab::Analysis => analysis_tab(app, ui),
            });
        });
}

// ---- Settings: one list of collapsible groups --------------------------------------------------

/// Intent comes first; advanced groups open only on demand or when a search matches them.
/// The value of a parameter in the Λ JSON; a setting the JSON leaves out while it is off has its default.
fn value_of(app: &App, current: &Value, path: &str) -> Value {
    get_path(current, path)
        .or_else(|| app.default_ref(path))
        .cloned()
        .unwrap_or(Value::Null)
}

/// The environment layers, each off until turned on; the settings of a layer show only while it is on.
fn environment_card(app: &mut App, ui: &mut Ui) {
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let mut edits = Vec::new();
    let mut new_events = None;
    card(ui, "Environment", |ui| {
        ui.label(muted(
            ui,
            "Layers laid over the floor. All are off until you turn them on.",
        ));
        ui.add_space(4.0);
        let on = app.params.environment.height.enabled;
        Grid::new("environment_height")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in [
                    "environment.height.enabled",
                    "environment.height.phase",
                    "environment.height.style",
                    "environment.height.relief",
                    "environment.height.maxSlope",
                ] {
                    if path != "environment.height.enabled" && !on {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, app.default_ref(path)) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.add_space(4.0);
        let water_on = app.params.environment.water.enabled;
        Grid::new("environment_water")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in [
                    "environment.water.enabled",
                    "environment.water.sources",
                    "environment.water.flow",
                    "environment.water.reach",
                ] {
                    if path != "environment.water.enabled" && !water_on {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, app.default_ref(path)) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.add_space(4.0);
        let climate_on = app.params.environment.climate.enabled;
        Grid::new("environment_climate")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in [
                    "environment.climate.enabled",
                    "environment.climate.climate",
                    "environment.climate.season",
                    "environment.climate.openings",
                    "environment.climate.reach",
                    "environment.climate.gradient",
                ] {
                    if path != "environment.climate.enabled" && !climate_on {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, app.default_ref(path)) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.add_space(4.0);
        let light_on = app.params.environment.light.enabled;
        Grid::new("environment_light")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in LIGHT_FIELDS {
                    if path != "environment.light.enabled" && !light_on {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, app.default_ref(path)) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        ui.add_space(4.0);
        let events_on = app.params.environment.events.enabled;
        Grid::new("environment_events")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for path in ["environment.events.enabled", "environment.events.exclusive"] {
                    if path != "environment.events.enabled" && !events_on {
                        continue;
                    }
                    if let Some(field) = app
                        .groups
                        .iter()
                        .flat_map(|g| &g.fields)
                        .find(|f| f.path == path)
                    {
                        let value = value_of(app, &current, path);
                        if let Some(next) = field_row(ui, field, &value, app.default_ref(path)) {
                            edits.push((field.path.clone(), next));
                        }
                    }
                }
            });
        if events_on {
            new_events = events_editor(
                ui,
                &app.params.environment.events.list,
                app.params.zone_count(),
                &app.params.environment,
            );
        }
    });
    for (path, value) in edits {
        app.change_param(&path, value);
    }
    if let Some(list) = new_events {
        app.params.environment.events.list = list;
        app.environment_edited(false);
    }
}

/// The events you name: one row each, with the count, where they go, the gap between them and a zone. Returns the new list when
/// something changed.
fn events_editor(
    ui: &mut Ui,
    list: &[tapiti_core::environment::EventSpec],
    zones: usize,
    env: &tapiti_core::environment::EnvSpec,
) -> Option<Vec<tapiti_core::environment::EventSpec>> {
    use tapiti_core::environment::{Condition, ConditionField as F, EventSpec, Placement};
    // What a condition can look at: the layers that are on (and the level, always).
    let fields: Vec<(F, &str, bool)> = vec![
        (F::Height, "height", env.height.enabled),
        (F::WaterDepth, "water depth", env.water.enabled),
        (F::Humidity, "humidity", env.water.enabled),
        (F::Temperature, "temperature", env.climate.enabled),
        (F::Light, "light", env.light.enabled),
        (F::Level, "level", true),
    ];
    let options = tapiti_schema::placement_options();
    let key = |p: Placement| {
        serde_json::to_value(p)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    };
    let mut list = list.to_vec();
    let mut changed = false;
    let mut remove = None;
    for (i, event) in list.iter_mut().enumerate() {
        ui.separator();
        ui.horizontal(|ui| {
            changed |= ui
                .add(TextEdit::singleline(&mut event.name).desired_width(120.0))
                .on_hover_text("The name of this kind of event")
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut event.count)
                        .range(0..=256)
                        .suffix(" times"),
                )
                .on_hover_text("How many occurrences to place")
                .changed();
            if ui.small_button("remove").clicked() {
                remove = Some(i);
            }
        });
        ui.horizontal(|ui| {
            let current = key(event.placement);
            let label = options
                .iter()
                .find(|(k, _)| *k == current)
                .map_or(current.as_str(), |(_, l)| l.as_str())
                .to_string();
            egui::ComboBox::from_id_salt(("event_placement", i))
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (k, l) in &options {
                        if let Ok(p) =
                            serde_json::from_value::<Placement>(Value::String((*k).into()))
                        {
                            changed |= ui
                                .selectable_value(&mut event.placement, p, l.as_str())
                                .changed();
                        }
                    }
                });
            changed |= ui
                .add(
                    egui::DragValue::new(&mut event.min_gap)
                        .range(0..=64)
                        .prefix("gap "),
                )
                .on_hover_text("The fewest corridors between two of this kind; 0 for no limit")
                .changed();
        });
        ui.horizontal(|ui| {
            if zones > 1 {
                let label = event
                    .zone
                    .map_or("any zone".to_string(), |z| format!("zone {}", z + 1));
                egui::ComboBox::from_id_salt(("event_zone", i))
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(&mut event.zone, None, "any zone")
                            .changed();
                        for z in 0..zones {
                            changed |= ui
                                .selectable_value(
                                    &mut event.zone,
                                    Some(z),
                                    format!("zone {}", z + 1),
                                )
                                .changed();
                        }
                    });
            }
            changed |= ui
                .checkbox(&mut event.avoid_entrance, "keep off the entrance and exit")
                .changed();
        });
        // Where the layers say it may happen: "only where the light is below 0.1".
        let mut drop_condition = None;
        for (c, cond) in event.conditions.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("only where");
                let name = fields
                    .iter()
                    .find(|(f, _, _)| *f == cond.field)
                    .map_or("?", |(_, n, _)| *n);
                egui::ComboBox::from_id_salt(("event_condition", i, c))
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for (f, n, on) in &fields {
                            if *on || *f == cond.field {
                                changed |= ui.selectable_value(&mut cond.field, *f, *n).changed();
                            }
                        }
                    });
                for (limit, label) in [(&mut cond.min, "from"), (&mut cond.max, "to")] {
                    let mut on = limit.is_some();
                    if ui.checkbox(&mut on, label).changed() {
                        *limit = on.then_some(limit.unwrap_or(0.0));
                        changed = true;
                    }
                    if let Some(v) = limit {
                        changed |= ui.add(egui::DragValue::new(v).speed(0.05)).changed();
                    }
                }
                if ui
                    .small_button("x")
                    .on_hover_text("Remove this condition")
                    .clicked()
                {
                    drop_condition = Some(c);
                }
            });
        }
        if let Some(c) = drop_condition {
            event.conditions.remove(c);
            changed = true;
        }
        if event.conditions.len() < 8 && ui.small_button("Add a condition").clicked() {
            let first = fields
                .iter()
                .find(|(_, _, on)| *on)
                .map_or(F::Level, |(f, _, _)| *f);
            event.conditions.push(Condition {
                field: first,
                min: Some(0.0),
                max: None,
            });
            changed = true;
        }
    }
    ui.separator();
    if ui.button("Add an event").clicked() {
        let mut next = EventSpec::default();
        let mut n = list.len() + 1;
        while list.iter().any(|e| e.name == format!("event {n}")) {
            n += 1;
        }
        next.name = format!("event {n}");
        list.push(next);
        changed = true;
    }
    if let Some(i) = remove {
        list.remove(i);
        changed = true;
    }
    changed.then_some(list)
}

/// The light settings the Environment card lists; the others (daylight, source power, range, door transmission) are in Advanced.
const LIGHT_FIELDS: [&str; 5] = [
    "environment.light.enabled",
    "environment.light.time",
    "environment.light.openings",
    "environment.light.torches",
    "environment.light.placement",
];

/// Whether the Environment card lists the setting, so Advanced does not.
fn in_environment_card(path: &str) -> bool {
    !path.starts_with("environment.light.") || LIGHT_FIELDS.contains(&path)
}

fn design_tab(app: &mut App, ui: &mut Ui) {
    layout_card(app, ui);
    intent_card(app, ui);
    environment_card(app, ui);
    ui.add_space(theme::GAP);
    ui.label(muted(
        ui,
        "Everything else that shapes the space, rooms and connections is in the Advanced tab.",
    ));
}

/// Every parameter group the Design tab does not already show, collapsed; the search finds any parameter, shown elsewhere or not.
fn advanced_tab(app: &mut App, ui: &mut Ui) {
    let needle = app.filter.trim().to_lowercase();
    if needle.is_empty() {
        ui.label(muted(ui, "Shape the space, rooms and connections."));
        ui.add_space(theme::GAP);
    }
    let groups = std::mem::take(&mut app.groups);
    let current = serde_json::to_value(&app.params).unwrap_or(Value::Null);
    let layout = app.params.layout;
    let focus = app.focus_group.take();
    let mut edits: Vec<(String, Value)> = Vec::new();
    let mut reset = None;
    let mut shown = 0;
    for group in groups
        .iter()
        .filter(|g| !needle.is_empty() || group_applies(&g.id, layout))
    {
        // Search includes controls already shown in the intent card, and inactive layout settings.
        let fields: Vec<&ParamField> = group
            .fields
            .iter()
            .filter(|f| {
                !needle.is_empty()
                    || (f.path != "topology.profile"
                        && !f.path.starts_with("zones.")
                        && !(f.path.starts_with("environment.") && in_environment_card(&f.path))
                        && !f.path.starts_with("levels.")
                        && f.path != "layout"
                        && !layout_fields(app.params.layout).contains(&f.path.as_str())
                        && !intent_fields(app.params.topology.profile).contains(&f.path.as_str())
                        && crate::relevance::applies(&f.path, &app.params))
            })
            .filter(|f| {
                needle.is_empty()
                    || f.label.as_str().to_lowercase().contains(&needle)
                    || f.path.to_lowercase().contains(&needle)
            })
            .collect();
        if fields.is_empty() {
            continue;
        }
        shown += 1;
        let changed = group
            .fields
            .iter()
            .filter(|f| {
                fields.iter().any(|shown| shown.path == f.path) && app.changed.contains(&f.path)
            })
            .count();
        let title = if changed > 0 {
            format!("{} · {changed}", group.title.as_str())
        } else {
            group.title.as_str().to_string()
        };
        let focused = focus.as_deref() == Some(group.id.as_str());
        let forced = (focused || !needle.is_empty()).then_some(true);
        let response = egui::CollapsingHeader::new(RichText::new(title).strong())
            .id_salt(("advanced_group", &group.id))
            .default_open(false)
            .open(forced)
            .show(ui, |ui| {
                ui.label(muted(ui, group.intro.as_str()));
                ui.add_space(4.0);
                Grid::new(&group.id)
                    .num_columns(3)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        for field in fields {
                            let value = value_of(app, &current, &field.path);
                            if let Some(next) =
                                field_row(ui, field, &value, app.default_ref(&field.path))
                            {
                                edits.push((field.path.clone(), next));
                            }
                        }
                    });
                if changed > 0 && ui.small_button("Reset this group").clicked() {
                    reset = Some(group.id.clone());
                }
                ui.add_space(6.0);
            });
        if focused {
            response.header_response.scroll_to_me(Some(Align::TOP));
        }
    }
    app.groups = groups;
    if shown == 0 {
        ui.label(muted(ui, "No parameter matches the search."));
    }
    if let Some(id) = reset {
        app.reset_group(&id);
    } else if !edits.is_empty() {
        let mut value = current;
        for (path, next) in edits {
            set_path(&mut value, &path, next);
        }
        match serde_json::from_value::<tapiti_core::params::Params>(value) {
            Ok(params) => {
                if params.topology.profile != app.params.topology.profile {
                    let profile = params.topology.profile;
                    app.params = params;
                    app.set_intent(profile);
                } else {
                    app.params = params;
                    app.params_edited();
                }
            }
            Err(error) => app.status = format!("Invalid parameter: {error}", error = error),
        }
    }
}

pub fn json_window(app: &mut App, ctx: &egui::Context) {
    let mut open = app.show_json;
    egui::Window::new("JSON editor")
        .open(&mut open)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label(muted(
                ui,
                "Size, seed and the full Λ. A valid edit regenerates the floor after a short pause.",
            ));
            let editor = ui.add(
                TextEdit::multiline(&mut app.json_text)
                    .code_editor()
                    .desired_rows(18)
                    .desired_width(ui.available_width()),
            );
            if editor.changed() {
                app.json_dirty = true;
                app.apply_at_edit();
            }
            match (&app.json_error, app.json_dirty) {
                (Some(error), _) => {
                    ui.colored_label(Palette::new(ui.visuals().dark_mode).danger, error);
                }
                (None, true) => {
                    ui.label(muted(ui, "Validating before regenerating…"));
                }
                _ => {}
            }
        });
    app.show_json = open;
}

// ---- Map ----------------------------------------------------------------------------------------

pub fn central(app: &mut App, ui: &mut Ui) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::of(ui).panel).inner_margin(10.0))
        .show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            let pal = Palette::new(ui.visuals().dark_mode);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 10.0, pal.background);
            painter.rect_stroke(
                rect,
                10.0,
                Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );

            let Some((min, max)) =
                (if app.view_mode == crate::ViewMode::Iso && !app.showing_building() {
                    app.generated
                        .as_ref()
                        .map(|g| crate::iso::bounds(&g.floor, &app.iso))
                } else {
                    None
                })
                .or_else(|| app.map_bounds())
            else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Generating the first floor…",
                    egui::FontId::proportional(16.0),
                    pal.ink,
                );
                return;
            };
            let mut xf = Xf::new(rect, min, max, app.zoom, app.pan);
            let ctx = ui.ctx().clone();

            if response.double_clicked() {
                app.zoom = 1.0;
                app.pan = [0.0, 0.0];
            }
            if response.hovered() && app.view_mode != crate::ViewMode::Solid {
                let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta()));
                let factor = f64::from((scroll * 0.0012).exp() * pinch);
                if (factor - 1.0).abs() > 1e-6 {
                    if let Some(pos) = response.hover_pos() {
                        zoom_at(app, &xf, pos, factor);
                        xf = Xf::new(rect, min, max, app.zoom, app.pan);
                    }
                }
            }
            if response.dragged_by(egui::PointerButton::Secondary)
                && app.view_mode == crate::ViewMode::Iso
            {
                // The right button turns the view: across for the yaw, up and down for the tilt.
                let delta = response.drag_delta();
                app.iso.yaw = (app.iso.yaw + f64::from(delta.x) * 0.4).rem_euclid(360.0);
                app.iso.tilt = (app.iso.tilt - f64::from(delta.y) * 0.3).clamp(15.0, 90.0);
            } else if response.dragged() && app.view_mode != crate::ViewMode::Solid {
                let delta = response.drag_delta();
                let scale = f64::from(xf.scale).max(1e-6);
                app.pan[0] -= f64::from(delta.x) / scale;
                app.pan[1] -= f64::from(delta.y) / scale;
                xf = Xf::new(rect, min, max, app.zoom, app.pan);
            }
            if response.hovered() {
                ctx.set_cursor_icon(if response.dragged() {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Grab
                });
            }

            // While the map moves the detail layers are dropped, so each frame stays cheap.
            let mut overlays = app.overlays;
            if response.dragged() {
                overlays.labels = false;
                overlays.ports = false;
                overlays.pairs = false;
                overlays.candidates = false;
            }
            if app.showing_building() {
                map::draw_building(&painter, &xf, &app.building, overlays, &pal);
            } else if let Some(floor) = app.generated.as_ref().map(|g| g.floor.clone()) {
                match app.view_mode {
                    crate::ViewMode::Solid => {
                        app.solid.show(ui, rect, &response, &floor, overlays, &pal);
                    }
                    crate::ViewMode::Iso => {
                        crate::iso::draw(&painter, &xf, &floor, &app.iso, overlays, &pal);
                    }
                    crate::ViewMode::Plan => map::draw_floor(&painter, &xf, &floor, overlays, &pal),
                }
            }
            if app.overlays.legend && rect.width() > 520.0 && rect.height() > 380.0 {
                if let Some(generated) = &app.generated {
                    let present: Vec<&str> = generated
                        .floor
                        .rooms
                        .iter()
                        .filter(|room| room.alive)
                        .map(|room| room.cls.name())
                        .collect();
                    map::legend(&painter, rect, &pal, ui.visuals().text_color(), &present);
                    map::scales(
                        &painter,
                        rect,
                        ui.visuals().text_color(),
                        &generated.floor,
                        app.overlays,
                    );
                }
            }
            map_controls(app, ui, rect);
            if app.busy {
                painter.text(
                    rect.right_top() + Vec2::new(-14.0, 16.0),
                    egui::Align2::RIGHT_CENTER,
                    "Generating…",
                    egui::FontId::proportional(13.0),
                    pal.ink.gamma_multiply(0.7),
                );
            }

            if response.hovered()
                && !response.dragged()
                && !app.showing_building()
                && app.view_mode != crate::ViewMode::Solid
            {
                if let (Some(pos), Some(generated)) = (response.hover_pos(), &app.generated) {
                    let info = if app.view_mode == crate::ViewMode::Iso {
                        // The room under the pointer in the oblique view, described as the plan describes it.
                        crate::iso::pick(&generated.floor, &xf, &app.iso, pos)
                            .map(|id| {
                                let r = &generated.floor.rooms[id];
                                map_object_at(&generated.floor, r.center, app.zoom, Some(r.level))
                            })
                            .unwrap_or_default()
                    } else {
                        map_object_at(
                            &generated.floor,
                            xf.to_world(pos),
                            app.zoom,
                            app.overlays.level,
                        )
                    };
                    if !info.is_empty() {
                        response.on_hover_text_at_pointer(info);
                    }
                }
            }
        });
}

fn zoom_at(app: &mut App, xf: &Xf, anchor: egui::Pos2, factor: f64) {
    let world = xf.to_world(anchor);
    let zoom = (app.zoom * factor).clamp(0.3, 20.0);
    if (zoom - app.zoom).abs() < 1e-9 {
        return;
    }
    app.zoom = zoom;
    // Shift the pan so the world point under the cursor stays under it at the new zoom.
    if let Some((min, max)) = app.map_bounds() {
        let moved = Xf::new(xf.rect(), min, max, app.zoom, app.pan).to_world(anchor);
        app.pan[0] += world[0] - moved[0];
        app.pan[1] += world[1] - moved[1];
    }
}

/// What floats over the map: the layers at the top left, the zoom at the bottom right.
fn map_controls(app: &mut App, ui: &mut Ui, rect: egui::Rect) {
    let margin = Vec2::splat(10.0);
    // The level of a stacked floor on show.
    let levels = app
        .generated
        .as_ref()
        .and_then(|g| g.floor.stack.as_ref())
        .map_or(0, |s| s.levels.len());
    // In the oblique view the levels are drawn together, so the selector of one is for the plan only.
    let plan_levels = levels > 1 && app.view_mode == crate::ViewMode::Plan;
    // The way the floor is shown, and in the oblique view where it is looked at from.
    egui::Area::new(ui.scope_id().with("map_view"))
        .fixed_pos(
            rect.left_top() + margin + Vec2::new(84.0 + if plan_levels { 100.0 } else { 0.0 }, 0.0),
        )
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("map_view_mode")
                    .selected_text(match app.view_mode {
                        crate::ViewMode::Plan => "2D",
                        crate::ViewMode::Iso => "2.5D",
                        crate::ViewMode::Solid => "3D",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.view_mode, crate::ViewMode::Plan, "2D");
                        ui.selectable_value(&mut app.view_mode, crate::ViewMode::Iso, "2.5D");
                        ui.selectable_value(&mut app.view_mode, crate::ViewMode::Solid, "3D");
                    });
                if app.view_mode == crate::ViewMode::Solid {
                    ui.menu_button("Camera", |ui| {
                        use crate::solid::Mode;
                        let before = app.solid.mode;
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut app.solid.mode,
                                Mode::Orbit,
                                "Circle the floor",
                            );
                            ui.selectable_value(&mut app.solid.mode, Mode::Walk, "Walk in it");
                        });
                        if app.solid.mode != before && app.solid.mode == Mode::Walk {
                            app.solid.respawn();
                        }
                        ui.add(
                            egui::Slider::new(&mut app.solid.wall_height, 20.0..=140.0)
                                .text("walls"),
                        );
                        ui.add(
                            egui::Slider::new(&mut app.solid.exaggeration, 0.5..=6.0)
                                .text("height ×"),
                        );
                        ui.add(egui::Slider::new(&mut app.solid.quality, 0.3..=1.5).text("detail"));
                        if levels > 1 {
                            ui.add(
                                egui::Slider::new(&mut app.solid.explode, 0.0..=3.0)
                                    .text("levels apart"),
                            );
                            let mut all = app.solid.up_to.is_none();
                            if ui.checkbox(&mut all, "all levels").changed() {
                                app.solid.up_to = if all { None } else { Some(levels - 1) };
                            }
                            if let Some(up) = app.solid.up_to.as_mut() {
                                ui.add(egui::Slider::new(up, 0..=levels - 1).text("up to level"));
                            }
                        }
                        let walking = app.solid.mode == Mode::Walk;
                        if ui
                            .button(if walking {
                                "Back to the entrance"
                            } else {
                                "Reset the camera"
                            })
                            .clicked()
                        {
                            if walking {
                                app.solid.respawn();
                            } else {
                                app.solid.reset_orbit();
                            }
                        }
                        ui.label(muted(
                            ui,
                            if walking {
                                "W A S D to walk, Shift to run, drag to look, E at a stair."
                            } else {
                                "Drag to turn, wheel to come near or go far."
                            },
                        ));
                    });
                }
                if app.view_mode == crate::ViewMode::Iso {
                    ui.menu_button("Camera", |ui| {
                        ui.add(egui::Slider::new(&mut app.iso.yaw, 0.0..=360.0).text("turn"));
                        ui.add(egui::Slider::new(&mut app.iso.tilt, 15.0..=90.0).text("tilt"));
                        ui.add(
                            egui::Slider::new(&mut app.iso.exaggeration, 0.5..=6.0)
                                .text("height ×"),
                        );
                        if levels > 1 {
                            ui.add(
                                egui::Slider::new(&mut app.iso.explode, 0.0..=3.0)
                                    .text("levels apart"),
                            );
                            let mut all = app.iso.up_to.is_none();
                            ui.checkbox(&mut all, "all levels").changed().then(|| {
                                app.iso.up_to = if all { None } else { Some(levels - 1) };
                            });
                            if let Some(up) = app.iso.up_to.as_mut() {
                                ui.add(egui::Slider::new(up, 0..=levels - 1).text("up to level"));
                            }
                        }
                        if ui.button("Reset the camera").clicked() {
                            app.iso = crate::iso::IsoCamera::default();
                        }
                        ui.label(muted(
                            ui,
                            "Right button: turn and tilt. Left: move. Wheel: zoom.",
                        ));
                    });
                }
            });
        });
    if plan_levels {
        egui::Area::new(ui.scope_id().with("map_levels"))
            .fixed_pos(rect.left_top() + margin + Vec2::new(84.0, 0.0))
            .show(ui.ctx(), |ui| {
                let shown = app.overlays.level.unwrap_or(0).min(levels - 1);
                egui::ComboBox::from_id_salt("map_level")
                    .selected_text(format!("Level {}", shown + 1))
                    .show_ui(ui, |ui| {
                        for k in 0..levels {
                            ui.selectable_value(
                                &mut app.overlays.level,
                                Some(k),
                                format!("Level {}", k + 1),
                            );
                        }
                    });
            });
    }
    egui::Area::new(ui.scope_id().with("map_layers"))
        .fixed_pos(rect.left_top() + margin)
        .show(ui.ctx(), |ui| {
            ui.menu_button("Layers", |ui| {
                let o = &mut app.overlays;
                for (flag, name, hint) in [
                    (&mut o.cells, "Grid", "Grid blocks of the floor"),
                    (
                        &mut o.height,
                        "Height",
                        "Tint rooms by their height, blue low to amber high",
                    ),
                    (
                        &mut o.water,
                        "Water",
                        "Rooms with water over their floor, deeper in a stronger blue",
                    ),
                    (
                        &mut o.events,
                        "Events",
                        "A badge in each room that holds an event, by kind",
                    ),
                    (
                        &mut o.light,
                        "Light",
                        "Darken rooms the light does not reach and mark direct sources",
                    ),
                    (
                        &mut o.temperature,
                        "Temperature",
                        "Tint rooms by temperature, blue cold to red hot",
                    ),
                    (
                        &mut o.labels,
                        "Labels",
                        "Number, shape and role of each room",
                    ),
                    (&mut o.ports, "Doors", "Every possible door position"),
                    (&mut o.pairs, "Pairs", "Candidate room pairs for connection"),
                    (&mut o.candidates, "Routes", "Candidate corridor routes"),
                    (&mut o.legend, "Legend", "Shape and role colors"),
                ] {
                    ui.checkbox(flag, name).on_hover_text(hint);
                }
            });
        });
    egui::Area::new(ui.scope_id().with("map_zoom"))
        .fixed_pos(rect.right_bottom() - margin)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button("−").on_hover_text("Zoom out").clicked() {
                    app.zoom = (app.zoom * 0.8).max(0.3);
                }
                ui.label(format!("{}%", (app.zoom * 100.0).round() as u32));
                if ui.button("+").on_hover_text("Zoom in").clicked() {
                    app.zoom = (app.zoom * 1.25).min(20.0);
                }
                if ui
                    .button("Fit")
                    .on_hover_text("Double-click on the map")
                    .clicked()
                {
                    app.zoom = 1.0;
                    app.pan = [0.0, 0.0];
                }
            });
        });
}
