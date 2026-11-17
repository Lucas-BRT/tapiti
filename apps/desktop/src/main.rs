//! Native desktop frontend (egui) for Tapiti.
//!
//! Unlike the browser build, this binary calls the Rust generator directly with the
//! `parallel` feature enabled. Generation runs on a background OS thread so the
//! window remains responsive while Rayon handles the generator's parallel phases.

// A release build on Windows is a GUI program: without this it opens a console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod charts;
mod command;
mod iso;
mod map;
mod panels;
mod params_ui;
mod pick;
mod prefs;
mod relevance;
mod settings;
mod solid;
mod theme;

use eframe::egui::{self, Key, Modifiers};
use params_ui::ParamGroup;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tapiti_core::cancel::Cancel;
use tapiti_core::generate::{Floor, GenerationEvent, Role};
use tapiti_core::geom::Shape;
use tapiti_core::metrics::{metrics_of, Metrics};
use tapiti_core::par;
use tapiti_core::params::{params_of_size, Params, Size, TopologyProfile, TopologySpec};
use tapiti_core::stage;

pub const APP_ID: &str = "dev.lucasbrt.Tapiti";
/// Pause after the last edit before the map regenerates.
const APPLY_DELAY: Duration = Duration::from_millis(180);
/// A generation shorter than this swaps the map in one step; a longer one shows its progress.
const PREVIEW_AFTER: Duration = Duration::from_millis(120);

#[derive(Clone)]
pub struct GeneratedMap {
    pub floor: Arc<Floor>,
    pub metrics: Metrics,
    pub violations: Vec<String>,
    pub draws: u32,
    pub elapsed_ms: f64,
    pub size: Size,
    pub seed: u32,
    pub phases: String,
}

impl GeneratedMap {
    /// The box of the rooms and corridors that exist, with a margin, so Fit shows the floor and not the empty grid around it. `None`
    /// when nothing is drawn.
    pub fn occupied_bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        let mut grow = |p: [f64; 2]| {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        };
        for room in self.floor.rooms.iter().filter(|r| r.alive) {
            grow([room.bounds.lo[0], room.bounds.lo[1]]);
            grow([room.bounds.hi[0], room.bounds.hi[1]]);
        }
        for edge in &self.floor.edges {
            for p in &edge.pts {
                grow(*p);
            }
        }
        if lo[0] > hi[0] || lo[1] > hi[1] {
            return None;
        }
        let margin = 0.04 * (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0) + self.floor.params.width;
        Some((
            [lo[0] - margin, lo[1] - margin],
            [hi[0] + margin, hi[1] + margin],
        ))
    }
}

#[derive(Default)]
pub struct BuildingMap {
    seed: u32,
    draw: u32,
    grid_x: Vec<f64>,
    grid_y: Vec<f64>,
    corridor_width: f64,
    rooms: Vec<Shape>,
    edges: Vec<Vec<[f64; 2]>>,
    connections: usize,
}

impl BuildingMap {
    fn has_layout(&self) -> bool {
        self.grid_x.len() >= 2 && self.grid_y.len() >= 2
    }

    fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        Some((
            [*self.grid_x.first()?, *self.grid_y.first()?],
            [*self.grid_x.last()?, *self.grid_y.last()?],
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Experiment {
    Shapes,
    Cycles,
    Junctions,
    CircularRooms,
}

#[derive(Debug, Clone, Copy)]
pub enum BenchmarkKind {
    Forty,
    Batch,
}

/// How the floor is shown on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// The plan, from above.
    Plan,
    /// Obliquely, rooms as slabs at their height (see `iso`).
    Iso,
    /// In three dimensions, from outside or on foot (see `solid`).
    Solid,
}

impl Overlays {
    const NAMES: [&'static str; 11] = [
        "cells",
        "height",
        "water",
        "temperature",
        "light",
        "events",
        "labels",
        "ports",
        "pairs",
        "candidates",
        "legend",
    ];

    fn flags(&mut self) -> [&mut bool; 11] {
        [
            &mut self.cells,
            &mut self.height,
            &mut self.water,
            &mut self.temperature,
            &mut self.light,
            &mut self.events,
            &mut self.labels,
            &mut self.ports,
            &mut self.pairs,
            &mut self.candidates,
            &mut self.legend,
        ]
    }

    /// The layers that are on, by name, as kept in the preferences.
    pub fn to_text(mut self) -> String {
        let on: Vec<&str> = self
            .flags()
            .into_iter()
            .zip(Self::NAMES)
            .filter(|(flag, _)| **flag)
            .map(|(_, name)| name)
            .collect();
        on.join(",")
    }

    /// The layers named in `text` on and the others off; names it does not know are ignored.
    pub fn set_from_text(&mut self, text: &str) {
        let names: Vec<&str> = text.split(',').map(str::trim).collect();
        for (flag, name) in self.flags().into_iter().zip(Self::NAMES) {
            *flag = names.contains(&name);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Overlays {
    pub cells: bool,
    pub height: bool,
    pub water: bool,
    pub temperature: bool,
    pub light: bool,
    pub events: bool,
    /// The level of a stacked floor on show (0 is the first); a floor with one level ignores it.
    pub level: Option<usize>,
    pub labels: bool,
    pub ports: bool,
    pub pairs: bool,
    pub candidates: bool,
    pub legend: bool,
}

/// What a worker thread tells the UI; always tagged with the work id it belongs to.
#[allow(clippy::large_enum_variant)]
enum Event {
    Progress(Vec<GenerationEvent>),
    Generated(Result<GeneratedMap, String>),
    /// The generation was stopped by the user before it was done.
    Cancelled,
    /// The result of a benchmark, or `None` when it was stopped.
    Benchmark(Option<String>),
    /// A file dialog ended: what the file said, or why it could not be read or written.
    Opened(String, Result<String, String>),
    Saved(String, Result<(), String>),
}

/// A message for the person at the keyboard, not a progress line.
pub struct Notice {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

/// The size, the seed and the Λ: what Undo and Redo bring back.
#[derive(Clone)]
pub struct Snapshot {
    size: Size,
    seed: u32,
    params: Params,
    /// The floor that size, seed and Λ drew, when it was on the screen and Settings allow keeping it: Undo and Redo then show it
    /// at once instead of generating it again.
    generated: Option<GeneratedMap>,
}

impl Snapshot {
    /// Forgets the floor, keeping the Λ: Undo then generates it again.
    pub fn drop_floor(&mut self) {
        self.generated = None;
    }
}

pub struct App {
    ctx: egui::Context,
    tx: Sender<(u64, Event)>,
    rx: Receiver<(u64, Event)>,
    pub size: Size,
    pub seed: u32,
    pub seed_draft: String,
    pub params: Params,
    pub defaults: Value,
    /// The settings Λ leaves out of its JSON while they are off, with their values then.
    pub skipped_defaults: Value,
    /// Paths of the parameters that differ from the size default.
    pub changed: HashSet<String>,
    pub json_text: String,
    pub json_dirty: bool,
    pub json_error: Option<String>,
    pub show_json: bool,
    pub filter: String,
    pub overlays: Overlays,
    pub zoom: f64,
    pub view_mode: ViewMode,
    /// The view mode last written to the preferences.
    saved_view: ViewMode,
    pub iso: iso::IsoCamera,
    pub solid: solid::Solid,
    pub pan: [f64; 2],
    pub busy: bool,
    pub queued: bool,
    /// Stops the generation that is running, when there is one.
    cancel: Option<Cancel>,
    work_id: u64,
    launched_at: Instant,
    show_building: bool,
    show_shortcuts: bool,
    show_settings: bool,
    apply_at: Option<Instant>,
    pub status: String,
    /// What the last command, file operation or failure said, apart from the progress in `status`: errors stay until dismissed.
    pub notice: Option<Notice>,
    /// The last generation failed or was cancelled, so the floor on the screen is not the one the parameters describe.
    pub generation_failed: bool,
    /// The Λ before the last change (a size, a reset, a preset, a file, an edit), for Edit > Undo; and the one Undo left, for Redo.
    pub previous: Option<Snapshot>,
    pub redo: Option<Snapshot>,
    /// The layers last written to the preferences.
    saved_layers: String,
    /// The title the window has, so it is set again only when it changes.
    window_title: String,
    /// When the Λ was last remembered for a single edit, so a drag of a value is one step back.
    last_remember: Option<Instant>,
    /// The window size last written to the preferences, and the one the window has.
    saved_window: Option<[f32; 2]>,
    /// A benchmark is running (it builds floors that are not shown): the map keeps the floor on the screen.
    pub benchmarking: bool,
    pub generated: Option<GeneratedMap>,
    pub building: BuildingMap,
    pub groups: Vec<ParamGroup>,
    /// `TAPITI_SCREENSHOT=file.ppm`: writes the window once the first floor is drawn and quits (for the pictures of the docs and for checks).
    shot: Option<Shot>,
    /// `TAPITI_SCROLL`: where the side panel starts, for a screenshot of the part below the fold.
    pub scroll_to: Option<f32>,
    /// A group of parameters to open and scroll to once (the screenshots of the docs ask for it).
    pub focus_group: Option<String>,
    /// What is typed in the command line, and what the last command said.
    pub command: String,
    /// The commands run, oldest first, and where Up and Down are in them.
    pub command_history: Vec<String>,
    pub command_cursor: Option<usize>,
    /// Compact or comfortable spacing, kept between runs.
    pub density: theme::Density,
    /// The reading column: `None` follows the window (shown from 1180 points wide), `Some` is the user's choice.
    pub reading_open: Option<bool>,
    /// The floor on show is identified by its seed and size; when that changes the plan is revealed from the entrance.
    pub reveal_key: Option<(u32, Size)>,
    /// When the reveal began, in the clock of the window; `None` when nothing is being revealed.
    pub reveal_start: Option<f64>,
    /// Set while the progressive preview is on, so the floor that follows it is not revealed a second time.
    pub skip_reveal: bool,
    /// How long the interface's own transitions last, and how long a new floor takes to be revealed; both kept between runs.
    pub ui_motion: settings::Timing,
    pub reveal_motion: settings::Timing,
    pub text_scale: settings::TextScale,
    /// Whether the floor of the last change is kept in memory for Undo and Redo.
    pub keep_floors: settings::KeepFloors,
    /// The text size slider was moved and the window has not been rescaled yet (it waits for the pointer to be let go).
    pub text_scale_pending: bool,
    /// A choice in Settings changed and is not written to the preferences yet.
    pub settings_dirty: bool,
    pub reading_width: settings::ReadingWidth,
    /// Light, dark or the system's, kept between runs, and what the system says now.
    pub appearance: theme::Appearance,
    pub system_theme: theme::SystemTheme,
    /// The room picked on the plan (an index of the floor's rooms).
    pub selected: Option<usize>,
    /// A room to pick as soon as the first floor is on screen (`TAPITI_SELECT`, for pictures).
    pub select_after: Option<usize>,
    /// Where the reading column starts (`TAPITI_READ_SCROLL`), for a picture of the part below the fold.
    pub read_scroll: Option<f32>,
}

struct Shot {
    path: String,
    frames: u32,
    requested: bool,
    /// The window size at the last frame and for how many frames it has not changed: a window that is still being resized would give a
    /// picture of two layouts.
    size: [f32; 2],
    still: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        let params = params_of_size(Size::Small);
        let loaded_view = match prefs::load("view").as_deref() {
            Some("iso") => ViewMode::Iso,
            Some("solid") => ViewMode::Solid,
            _ => ViewMode::Plan,
        };
        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            tx,
            rx,
            size: Size::Small,
            seed: 1,
            seed_draft: "1".into(),
            defaults: serde_json::to_value(&params).unwrap_or(Value::Null),
            skipped_defaults: skipped_defaults(),
            params,
            changed: HashSet::new(),
            json_text: String::new(),
            json_dirty: false,
            json_error: None,
            show_json: false,
            filter: String::new(),
            overlays: Overlays {
                cells: true,
                height: true,
                water: true,
                temperature: false,
                light: false,
                events: true,
                level: Some(0),
                labels: false,
                ports: false,
                pairs: false,
                candidates: false,
                legend: true,
            },
            zoom: 1.0,
            view_mode: loaded_view,
            saved_view: loaded_view,
            iso: iso::IsoCamera::default(),
            solid: solid::Solid::default(),
            pan: [0.0, 0.0],
            busy: false,
            queued: false,
            cancel: None,
            work_id: 0,
            launched_at: Instant::now(),
            show_building: false,
            show_shortcuts: false,
            show_settings: false,
            apply_at: None,
            notice: None,
            generation_failed: false,
            previous: None,
            redo: None,
            saved_layers: String::new(),
            window_title: String::new(),
            last_remember: None,
            saved_window: None,
            status: "Preparing the generator…".into(),
            benchmarking: false,
            generated: None,
            building: BuildingMap::default(),
            groups: params_ui::groups(),
            focus_group: None,
            command_history: Vec::new(),
            command_cursor: None,
            command: String::new(),
            density: theme::Density::saved(),
            reading_open: settings::Reading::saved().open(),
            reveal_key: None,
            reveal_start: None,
            skip_reveal: false,
            ui_motion: settings::Timing::interface(),
            reveal_motion: settings::Timing::reveal(),
            text_scale: settings::TextScale::saved(),
            keep_floors: settings::KeepFloors::saved(),
            text_scale_pending: false,
            settings_dirty: false,
            reading_width: settings::ReadingWidth::saved(),
            appearance: theme::Appearance::saved(),
            system_theme: theme::SystemTheme::watch(&cc.egui_ctx),
            selected: None,
            read_scroll: std::env::var("TAPITI_READ_SCROLL")
                .ok()
                .and_then(|n| n.parse().ok()),
            select_after: std::env::var("TAPITI_SELECT")
                .ok()
                .and_then(|n| n.parse().ok()),
            scroll_to: std::env::var("TAPITI_SCROLL")
                .ok()
                .and_then(|v| v.parse().ok()),
            shot: std::env::var("TAPITI_SCREENSHOT").ok().map(|path| Shot {
                path,
                frames: 0,
                requested: false,
                size: [0.0, 0.0],
                still: 0,
            }),
        };
        app.pick_from_env();
        if app.shot.is_none() {
            app.restore_last();
            if let Some(text) = prefs::load("layers") {
                app.overlays.set_from_text(&text);
                app.saved_layers = text;
            }
        }
        app.sync_json();
        if !app.busy {
            app.launch_generate();
        }
        app
    }

    /// `TAPITI_LAYOUT`, `TAPITI_SIZE` (1 to 4) and `TAPITI_TAB` (analysis, parameters or the id of a group) choose what a screenshot shows.
    fn pick_from_env(&mut self) {
        self.show_settings = std::env::var_os("TAPITI_SETTINGS").is_some();
        if self.shot.is_none() {
            return;
        }
        if let Ok(size) = std::env::var("TAPITI_SIZE") {
            if let Ok(n) = size.parse::<usize>() {
                self.size = Size::of_index(n);
                self.params = params_of_size(self.size);
                self.defaults = serde_json::to_value(&self.params).unwrap_or(Value::Null);
            }
        }
        match std::env::var("TAPITI_LAYOUT").as_deref() {
            Ok("growth") => self.params.layout = tapiti_core::params::Layout::Growth,
            Ok("bsp") => self.params.layout = tapiti_core::params::Layout::Bsp,
            _ => {}
        }
        if let Ok(intent) = std::env::var("TAPITI_INTENT") {
            if let Ok(profile) = serde_json::from_value(Value::String(intent)) {
                self.params.apply_intent(profile);
            }
        }
        if std::env::var("TAPITI_HEIGHT").is_ok() {
            self.params.environment.height.enabled = true;
        }
        match std::env::var("TAPITI_VIEW").as_deref() {
            Ok("iso") => self.view_mode = ViewMode::Iso,
            Ok("solid") => self.view_mode = ViewMode::Solid,
            Ok("walk") => {
                self.view_mode = ViewMode::Solid;
                self.solid.mode = solid::Mode::Walk;
            }
            _ => {}
        }
        if let Ok(n) = std::env::var("TAPITI_LEVELS") {
            self.params.levels.count = n.parse().unwrap_or(2);
            self.params.levels.stairs = 2;
        }
        if std::env::var("TAPITI_EVENTS").is_ok() {
            use tapiti_core::environment::{EventSpec, Placement};
            let ev = &mut self.params.environment.events;
            ev.enabled = true;
            ev.list = vec![
                EventSpec {
                    name: "trap".into(),
                    count: 3,
                    placement: Placement::DeadEnds,
                    ..EventSpec::default()
                },
                EventSpec {
                    name: "guard".into(),
                    count: 3,
                    placement: Placement::MainPath,
                    ..EventSpec::default()
                },
                EventSpec {
                    name: "camp".into(),
                    count: 2,
                    placement: Placement::FarFromEntrance,
                    min_gap: 2,
                    ..EventSpec::default()
                },
            ];
        }
        if std::env::var("TAPITI_LIGHT").is_ok() {
            self.params.environment.light.enabled = true;
            self.params.environment.light.torches = 3;
            self.params.environment.light.time = 0.0;
            self.overlays.light = true;
        }
        if std::env::var("TAPITI_CLIMATE").is_ok() {
            self.params.environment.height.enabled = true;
            self.params.environment.climate.enabled = true;
            self.params.environment.climate.season = 1.0;
            self.overlays.temperature = true;
        }
        if std::env::var("TAPITI_WATER").is_ok() {
            self.params.environment.height.enabled = true;
            self.params.environment.water.enabled = true;
        }
        match std::env::var("TAPITI_THEME").as_deref() {
            Ok("light") => self.appearance = theme::Appearance::Light,
            Ok("dark") => self.appearance = theme::Appearance::Dark,
            _ => {}
        }
        if let Some(zoom) = std::env::var("TAPITI_ZOOM")
            .ok()
            .and_then(|z| z.parse::<f32>().ok())
        {
            self.ctx.set_zoom_factor(zoom);
        }
        // `TAPITI_TAB=<id of a group>` opens that group of the tree; the other values name what is always on screen now.
        if let Ok(tab) = std::env::var("TAPITI_TAB") {
            if !matches!(
                tab.as_str(),
                "floor" | "analysis" | "design" | "advanced" | "parameters"
            ) {
                self.focus_group = Some(tab);
            }
        }
        self.refresh_changed();
    }

    /// Asks the window for a picture once the first floor is on it, writes it as a PPM and quits.
    /// Whether a screenshot was asked for with `TAPITI_SCREENSHOT`: then nothing animates, so the picture is of the finished frame.
    pub fn capturing(&self) -> bool {
        self.shot.is_some()
    }

    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let ready = !self.busy && self.generated.is_some();
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        shot.frames += 1;
        let rect = ctx.content_rect();
        let size = [rect.width(), rect.height()];
        shot.still = if size == shot.size { shot.still + 1 } else { 0 };
        shot.size = size;
        if ready && shot.frames > 20 && shot.still >= 15 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            ctx.request_screenshot(move |image| {
                let [w, h] = image.size;
                let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
                for pixel in &image.pixels {
                    out.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
                }
                let _ = std::fs::write(&path, out);
                std::process::exit(0);
            });
        }
    }

    // ---- generation -------------------------------------------------------------------------

    /// Starts a generation now, or after the running one when there is one.
    pub fn request_generate(&mut self) {
        if self.busy {
            self.queued = true;
        } else {
            self.launch_generate();
        }
    }

    fn launch_generate(&mut self) {
        if let Err(error) = validate_params(&self.params) {
            self.queued = false;
            let error = self.plain_error(&error);
            self.notify(error, true);
            return;
        }
        self.busy = true;
        self.queued = false;
        self.work_id = self.work_id.wrapping_add(1);
        self.launched_at = Instant::now();
        self.show_building = false;
        self.building = BuildingMap::default();
        self.status = "Generating the map…".into();
        let (work_id, size, seed, params) =
            (self.work_id, self.size, self.seed, self.params.clone());
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let cancel = Cancel::new();
        self.cancel = Some(cancel.clone());
        std::thread::spawn(move || {
            // One message per frame keeps generation fast and avoids waking the UI for every room and corridor.
            let frame = Duration::from_millis(33);
            let mut events = Vec::new();
            let mut last_flush = Instant::now();
            let mut progress = |event| {
                events.push(event);
                if last_flush.elapsed() >= frame {
                    let _ = tx.send((work_id, Event::Progress(std::mem::take(&mut events))));
                    ctx.request_repaint();
                    last_flush = Instant::now();
                }
            };
            let result = generate_map(size, seed, params, &cancel, &mut progress);
            drop(progress);
            if !events.is_empty() {
                let _ = tx.send((work_id, Event::Progress(events)));
            }
            let _ = tx.send((
                work_id,
                match result {
                    Some(result) => Event::Generated(result),
                    None => Event::Cancelled,
                },
            ));
            ctx.request_repaint();
        });
    }

    /// Whether a generation is running that the Cancel button can stop (a benchmark is not).
    pub fn can_cancel(&self) -> bool {
        self.cancel.is_some()
    }

    /// Stops the generation that is running (a very large floor can take a long time) and forgets the one queued after it.
    pub fn cancel_generation(&mut self) {
        self.queued = false;
        self.apply_at = None;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
            self.status = "Cancelling the generation…".into();
        }
    }

    pub fn start_benchmark(&mut self, kind: BenchmarkKind) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.benchmarking = true;
        self.work_id = self.work_id.wrapping_add(1);
        self.status = match kind {
            BenchmarkKind::Forty => "Benchmark: building 40 floors…".into(),
            BenchmarkKind::Batch => {
                "Benchmark: building 800 floors, one thread and then all…".into()
            }
        };
        let (work_id, size) = (self.work_id, self.size);
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let cancel = Cancel::new();
        self.cancel = Some(cancel.clone());
        std::thread::spawn(move || {
            let _ = tx.send((
                work_id,
                Event::Benchmark(run_benchmark(size, kind, &cancel)),
            ));
            ctx.request_repaint();
        });
    }

    fn poll_events(&mut self) {
        while let Ok((id, event)) = self.rx.try_recv() {
            // The answers of the file dialogs do not belong to a generation.
            match event {
                Event::Opened(path, text) => {
                    match text.and_then(|text| self.load_text(&text)) {
                        Ok(()) => self.notify(format!("Opened {path}."), false),
                        Err(error) => self.notify(format!("Could not open {path}: {error}"), true),
                    }
                    continue;
                }
                Event::Saved(path, result) => {
                    match result {
                        Ok(()) => self.notify(format!("Saved {path}."), false),
                        Err(error) => self.notify(format!("Could not save {path}: {error}"), true),
                    }
                    continue;
                }
                event => {
                    if id != self.work_id {
                        continue;
                    }
                    self.handle_event(event);
                    continue;
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        {
            match event {
                Event::Progress(events) => self.apply_progress(events),
                Event::Generated(result) => {
                    self.busy = false;
                    self.cancel = None;
                    self.show_building = false;
                    self.building = BuildingMap::default();
                    match result {
                        Ok(map) => {
                            self.status = format!(
                                "Floor generated in {ms:.1} ms · {draws} attempt{s}.",
                                ms = map.elapsed_ms,
                                draws = map.draws,
                                s = if map.draws == 1 { "" } else { "s" }
                            );
                            self.generated = Some(map);
                            // A room's index means another room in another floor.
                            self.selected = None;
                            self.generation_failed = false;
                            // A failure is answered by the next floor; the other messages stay until they fade.
                            if self.notice.as_ref().is_some_and(|n| n.error) {
                                self.notice = None;
                            }
                            self.keep_last();
                        }
                        Err(error) => {
                            self.generation_failed = true;
                            self.notify(format!("Generation failed: {error}"), true);
                        }
                    }
                    if self.queued {
                        self.launch_generate();
                    }
                }
                Event::Cancelled => {
                    self.busy = false;
                    self.cancel = None;
                    self.show_building = false;
                    self.building = BuildingMap::default();
                    self.generation_failed = true;
                    self.notify(
                        "Generation cancelled; the floor on the screen is the previous one.",
                        false,
                    );
                    if self.queued {
                        self.launch_generate();
                    }
                }
                Event::Benchmark(text) => {
                    self.busy = false;
                    self.cancel = None;
                    self.benchmarking = false;
                    match text {
                        Some(text) => self.notify(text, false),
                        None => self.notify("Benchmark stopped.", false),
                    }
                    self.status = "Benchmark done.".into();
                }
                // The file dialogs are answered in `poll_events`.
                Event::Opened(..) | Event::Saved(..) => {}
            }
        }
    }

    fn apply_progress(&mut self, events: Vec<GenerationEvent>) {
        // A quick generation replaces the map in one step; only a slow one shows its building.
        if self.launched_at.elapsed() >= PREVIEW_AFTER {
            self.show_building = true;
        }
        for event in events {
            match event {
                GenerationEvent::AttemptStarted { draw, seed } => {
                    self.building = BuildingMap {
                        draw,
                        seed,
                        ..BuildingMap::default()
                    };
                }
                GenerationEvent::Layout {
                    grid_x,
                    grid_y,
                    corridor_width,
                } => {
                    self.building.grid_x = grid_x;
                    self.building.grid_y = grid_y;
                    self.building.corridor_width = corridor_width;
                }
                GenerationEvent::RoomAdded(shape) => self.building.rooms.push(shape),
                GenerationEvent::EdgeUpdated { id, pts } => {
                    let b = &mut self.building;
                    if b.edges.len() <= id {
                        b.edges.resize_with(id + 1, Vec::new);
                    }
                    if b.edges[id].is_empty() {
                        b.connections += 1;
                    }
                    b.edges[id] = pts;
                }
            }
        }
        let b = &self.building;
        self.status = format!(
            "Attempt {draw} · seed {seed} · {rooms} rooms · {links} connections",
            draw = b.draw,
            seed = b.seed,
            rooms = b.rooms.len(),
            links = b.connections
        );
    }

    // ---- parameters -------------------------------------------------------------------------

    pub fn sync_json(&mut self) {
        self.json_text = serde_json::to_string_pretty(&self.params).unwrap_or_default();
        self.json_dirty = false;
        self.json_error = None;
    }

    fn refresh_changed(&mut self) {
        let current = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        self.changed = self
            .groups
            .iter()
            .flat_map(|group| &group.fields)
            .filter(|field| {
                // A setting the JSON leaves out is at its default.
                get_path(&current, &field.path)
                    .is_some_and(|value| Some(value) != self.default_ref(&field.path))
            })
            .map(|field| field.path.clone())
            .collect();
    }

    /// Call after only the settings of the environment layers changed: they run after the layout, so the floor on the screen is kept and
    /// the layers run again on it, at once. Falls back to a regeneration when there is no floor, when a layer that runs before the
    /// layout is involved (it changes the rooms), or when a generation is running.
    pub fn environment_edited(&mut self, was_before: bool) {
        let before_height =
            |p: &Params| p.environment.height.phase == tapiti_core::environment::LayerPhase::Before;
        let reuse = !self.busy
            && !was_before
            && !before_height(&self.params)
            && self
                .generated
                .as_ref()
                .is_some_and(|g| g.floor.params.levels == self.params.levels);
        if !reuse {
            self.params_edited();
            return;
        }
        self.refresh_changed();
        self.sync_json();
        if let Some(g) = self.generated.as_mut() {
            let floor = Arc::make_mut(&mut g.floor);
            tapiti_core::environment::reevaluate(floor, &self.params);
            g.violations = tapiti_core::validate::validate(floor);
            g.metrics = metrics_of(floor);
            self.status = "Environment layers updated on the floor on the screen.".into();
        }
    }

    /// Call after `params` changed from outside the JSON editor: refreshes derived state and schedules a regeneration.
    pub fn params_edited(&mut self) {
        if self.json_dirty {
            self.notify("The JSON editor had text that was not applied yet: the change on the screen replaced it.", false);
        }
        self.refresh_changed();
        self.sync_json();
        self.apply_at = Some(Instant::now() + APPLY_DELAY);
    }

    /// An edit is waiting to be applied.
    pub fn pending(&self) -> bool {
        self.apply_at.is_some()
    }

    /// File > Copy JSON, and Ctrl+C outside a text field.
    pub fn copy_json(&mut self, ctx: &egui::Context) {
        ctx.copy_text(self.export_json());
        self.notify("Parameters copied as JSON.", false);
    }

    /// File > Copy SVG, and Ctrl+Shift+C outside a text field.
    pub fn copy_svg(&mut self, ctx: &egui::Context) {
        let Some(generated) = &self.generated else {
            return;
        };
        let style = tapiti_render::Style {
            theme: if ctx.global_style().visuals.dark_mode {
                tapiti_render::Theme::Dark
            } else {
                tapiti_render::Theme::Light
            },
            grid: self.overlays.cells,
            labels: self.overlays.labels,
            ..tapiti_render::Style::default()
        };
        ctx.copy_text(tapiti_render::floor_svg(&generated.floor, &style));
        self.notify("Floor copied as SVG.", false);
    }

    /// Larger (`1`), smaller (`-1`) or normal (`0`) text, kept for the next run.
    pub fn step_text(&mut self, ctx: &egui::Context, direction: i32) {
        self.text_scale = self.text_scale.stepped(direction);
        self.text_scale.save();
        ctx.set_zoom_factor(self.text_scale.factor());
    }

    pub fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            error,
            at: Instant::now(),
        });
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            size: self.size,
            seed: self.seed,
            params: self.params.clone(),
            generated: self.kept_floor(),
        }
    }

    /// The floor on the screen, when it is the one the size, seed and Λ describe now and Settings allow keeping it.
    fn kept_floor(&self) -> Option<GeneratedMap> {
        let g = self.generated.as_ref()?;
        let rooms = g.floor.rooms.iter().filter(|r| r.alive).count();
        let same = g.seed == self.seed
            && g.size == self.size
            && !self.pending()
            && serde_json::to_value(&g.floor.params).ok()
                == serde_json::to_value(&self.params).ok();
        (same && self.keep_floors.allows(rooms)).then(|| g.clone())
    }

    /// Keeps the Λ as it is now, to come back to with Restore previous Λ. Only one step is kept.
    pub fn remember(&mut self) {
        self.previous = Some(self.snapshot());
        self.redo = None;
        self.last_remember = Some(Instant::now());
    }

    /// Remembers before a single edit, but only once for a run of them (a drag of a value is one step back).
    pub fn remember_for_edit(&mut self) {
        if self
            .last_remember
            .is_none_or(|at| at.elapsed() > Duration::from_millis(1500))
        {
            self.remember();
        }
        self.last_remember = Some(Instant::now());
    }

    /// Goes back to the Λ before the last change. One step is kept; what was undone can be redone until something else changes.
    pub fn undo(&mut self) {
        let Some(previous) = self.previous.take() else {
            self.notify("There is nothing to undo.", false);
            return;
        };
        self.redo = Some(self.snapshot());
        self.apply_snapshot(previous);
        self.notify("Undone. Redo (Ctrl+Y) brings it back.", false);
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo.take() else {
            self.notify("There is nothing to redo.", false);
            return;
        };
        self.previous = Some(self.snapshot());
        self.apply_snapshot(next);
        self.notify("Redone. Undo (Ctrl+Z) goes back.", false);
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.size = snapshot.size;
        self.seed = snapshot.seed;
        self.seed_draft = snapshot.seed.to_string();
        self.defaults = serde_json::to_value(params_of_size(snapshot.size)).unwrap_or(Value::Null);
        self.params = snapshot.params;
        self.refresh_changed();
        self.sync_json();
        match snapshot.generated {
            // The floor is still in memory: it is shown at once, with nothing generated and no reveal.
            Some(map) if !self.busy => {
                self.apply_at = None;
                self.queued = false;
                self.generated = Some(map);
                self.selected = None;
                self.generation_failed = false;
                self.skip_reveal = true;
                self.status = "Floor taken from memory: nothing was generated.".into();
                self.keep_last();
            }
            _ => self.request_generate(),
        }
    }

    /// Another size. The adjustments that mean the same in the new size are kept; those the size sets for itself (its grid, its
    /// counts) take the new size's values.
    pub fn set_size(&mut self, size: Size) {
        self.remember();
        let profile = self.params.topology.profile;
        let old_defaults = self.defaults.clone();
        let old_current = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let kept_from = self.changed.clone();
        self.load_size(size);
        self.params.apply_intent(profile);
        let mut next = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        let (mut kept, mut dropped) = (0, 0);
        for path in &kept_from {
            let same = get_path(&old_defaults, path) == get_path(&self.defaults, path);
            match (same, get_path(&old_current, path)) {
                (true, Some(value)) => {
                    set_path(&mut next, path, value.clone());
                    kept += 1;
                }
                _ => dropped += 1,
            }
        }
        if let Ok(params) = serde_json::from_value::<Params>(next) {
            self.params = params;
        }
        self.refresh_changed();
        self.sync_json();
        if kept + dropped > 0 {
            self.notify(
                format!(
                    "Size {}: {kept} adjustment(s) kept, {dropped} took the size's own value. Undo (Ctrl+Z) goes back.",
                    size_label(size).to_lowercase()
                ),
                false,
            );
        }
        self.request_generate();
    }

    /// The defaults of a size, with nothing carried over.
    fn load_size(&mut self, size: Size) {
        self.size = size;
        self.params = params_of_size(size);
        self.defaults = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        self.zoom = 1.0;
        self.pan = [0.0, 0.0];
    }

    pub fn set_intent(&mut self, profile: TopologyProfile) {
        self.remember();
        self.params.topology = TopologySpec::default();
        self.params.apply_intent(profile);
        self.params.topology = self.params.topology.requirements();
        self.params_edited();
    }

    pub fn reset_all(&mut self) {
        self.remember();
        self.load_size(self.size);
        self.refresh_changed();
        self.sync_json();
        self.notify(
            "Parameters reset to the defaults of the size. Undo (Ctrl+Z) goes back.",
            false,
        );
        self.request_generate();
    }

    /// The size, the seed and the Λ as the JSON that File > Save and Copy JSON write.
    pub fn export_json(&self) -> String {
        let export = json!({ "size": size_label(self.size).to_lowercase(), "seed": self.seed, "params": self.params });
        serde_json::to_string_pretty(&export).unwrap_or_default()
    }

    /// File > Save Λ…: asks for a file and writes the export there. (The dialog blocks the window while it is open.)
    /// File > Save Λ…: the dialog runs on a thread of its own, so the window keeps drawing (and the compositor keeps it as responding).
    pub fn save_file(&mut self) {
        let (text, tx, ctx) = (self.export_json(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Save the Λ")
                .set_file_name("tapiti.json")
                .add_filter("JSON", &["json"])
                .save_file()
            else {
                return;
            };
            let result = std::fs::write(&path, text).map_err(|e| e.to_string());
            let _ = tx.send((0, Event::Saved(path.display().to_string(), result)));
            ctx.request_repaint();
        });
    }

    /// File > Open Λ…: reads a file written by Save or Copy JSON (or a bare Λ) and generates with it.
    pub fn open_file(&mut self) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let Some(path) = rfd::FileDialog::new()
                .set_title("Open a Λ")
                .add_filter("JSON", &["json"])
                .pick_file()
            else {
                return;
            };
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string());
            let _ = tx.send((0, Event::Opened(path.display().to_string(), text)));
            ctx.request_repaint();
        });
    }

    /// Takes the JSON that Save and Copy JSON write (or a bare Λ) as the size, seed and Λ, and generates with it.
    fn load_text(&mut self, text: &str) -> Result<(), String> {
        let value = serde_json::from_str::<Value>(text).map_err(|e| e.to_string())?;
        let params = value.get("params").unwrap_or(&value).clone();
        let params = serde_json::from_value::<Params>(params).map_err(|e| e.to_string())?;
        validate_params(&params)?;
        self.remember();
        if let Some(size) = value.get("size").and_then(Value::as_str).and_then(|name| {
            Size::ALL
                .into_iter()
                .find(|s| size_label(*s).to_lowercase() == name)
        }) {
            self.size = size;
            self.defaults = serde_json::to_value(params_of_size(size)).unwrap_or(Value::Null);
        }
        if let Some(seed) = value
            .get("seed")
            .and_then(Value::as_u64)
            .and_then(|s| u32::try_from(s).ok())
        {
            self.seed = seed;
            self.seed_draft = seed.to_string();
        }
        self.params = params;
        self.refresh_changed();
        self.sync_json();
        self.request_generate();
        Ok(())
    }

    /// The size, seed and Λ of the last run, which the app comes back to; a missing or unreadable file starts from Small and seed 1.
    fn restore_last(&mut self) {
        if let Some(text) = prefs::load("last") {
            if self.load_text(&text).is_ok() {
                self.previous = None;
            }
        }
    }

    /// Writes the size, seed and Λ on the screen, so the next start is where this one stopped.
    fn keep_last(&mut self) {
        prefs::save("last", &self.export_json());
    }

    /// The window's title says what is on the screen, for the task switcher: size, seed and whether the floor is valid.
    fn keep_title(&mut self, ctx: &egui::Context) {
        let title = match &self.generated {
            Some(g) => format!(
                "Tapiti · {} · seed {} · {}",
                size_label(g.size).to_lowercase(),
                g.seed,
                if g.violations.is_empty() {
                    "valid"
                } else {
                    "violations"
                }
            ),
            None => "Tapiti".to_string(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Writes the layers shown on the map when they changed, so they are as they were left at the next start.
    fn keep_layers(&mut self) {
        let text = self.overlays.to_text();
        if text != self.saved_layers && !self.capturing() {
            prefs::save("layers", &text);
            self.saved_layers = text;
        }
    }

    /// Writes the window's size when it changed, so the next start has the window as it was left (not while maximized).
    fn keep_window(&mut self, ctx: &egui::Context) {
        let size = ctx.input(|i| i.viewport().inner_rect.map(|r| [r.width(), r.height()]));
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if let Some(size) = size.filter(|_| !maximized && !self.capturing()) {
            let moved = self
                .saved_window
                .is_none_or(|s| (s[0] - size[0]).abs() > 2.0 || (s[1] - size[1]).abs() > 2.0);
            if moved {
                prefs::save("window", &format!("{:.0},{:.0}", size[0], size[1]));
                self.saved_window = Some(size);
            }
        }
    }

    /// Words a validation message in the labels of the screen instead of the keys of the JSON.
    pub fn plain_error(&self, message: &str) -> String {
        let key = message.split([' ', ':']).next().unwrap_or("");
        let field =
            self.groups.iter().flat_map(|g| &g.fields).find(|f| {
                f.path == key || f.path.rsplit('.').next().is_some_and(|last| last == key)
            });
        match field {
            Some(f) => message.replacen(key, f.label.as_str(), 1),
            None => message.to_string(),
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
        self.seed_draft = seed.to_string();
        self.request_generate();
    }

    pub fn random_seed(&mut self) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or_default();
        self.set_seed(seed);
    }

    pub fn commit_seed_draft(&mut self) {
        match self.seed_draft.trim().parse::<u32>() {
            Ok(seed) => self.set_seed(seed),
            // The text stays, in the error colour, to be corrected.
            Err(_) => self.notify(
                "The seed must be an integer between 0 and 4,294,967,295.",
                true,
            ),
        }
    }

    /// Takes the JSON editor's text as the new Λ, if it is valid.
    fn commit_json(&mut self) -> bool {
        let parsed = serde_json::from_str::<Params>(&self.json_text)
            .map_err(|error| format!("Invalid Λ; check the JSON: {error}", error = error))
            .and_then(|params| validate_params(&params).map(|()| params));
        match parsed {
            Ok(params) => {
                self.params = params;
                self.json_dirty = false;
                self.json_error = None;
                self.refresh_changed();
                true
            }
            Err(error) => {
                self.json_error = Some(error);
                false
            }
        }
    }

    /// The default of a parameter: from the size's Λ, or, for a setting its JSON leaves out while it is off, the value it has then.
    pub fn default_ref(&self, path: &str) -> Option<&Value> {
        get_path(&self.defaults, path).or_else(|| get_path(&self.skipped_defaults, path))
    }

    pub fn change_param(&mut self, path: &str, value: Value) {
        self.remember_for_edit();
        let was_before =
            self.params.environment.height.phase == tapiti_core::environment::LayerPhase::Before;
        let mut params = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        set_path(&mut params, path, value);
        match serde_json::from_value::<Params>(params) {
            Ok(mut params) => {
                // Turning zones on also gives rectangle-only weights a mix, so the zones can look different.
                // Water runs down the ground, so turning it on turns Height on.
                if (path == "environment.water.enabled" && params.environment.water.enabled)
                    || (path == "environment.climate.enabled" && params.environment.climate.enabled)
                {
                    params.environment.height.enabled = true;
                }
                if path == "zones.count" && self.params.zones.count < 2 && params.zones.count >= 2 {
                    params.set_zones(params.zones.count, params.zones.bottleneck);
                }
                self.params = params;
                if path.starts_with("environment.") {
                    self.environment_edited(was_before);
                } else {
                    self.params_edited();
                }
            }
            Err(error) => {
                let error = self.plain_error(&error.to_string());
                self.notify(format!("Invalid parameter: {error}"), true);
            }
        }
    }

    pub fn reset_group(&mut self, id: &str, only: &[String]) {
        let Some(group) = self.groups.iter().find(|group| group.id == id) else {
            return;
        };
        let mut value = serde_json::to_value(&self.params).unwrap_or(Value::Null);
        // Reset tuning without changing the selected exploration intent.
        for field in group
            .fields
            .iter()
            .filter(|f| f.path != "topology.profile" && only.contains(&f.path))
        {
            if let Some(default) = self.default_ref(&field.path).cloned() {
                set_path(&mut value, &field.path, default);
            }
        }
        let title = group.title.as_str().to_string();
        self.remember();
        if let Ok(mut params) = serde_json::from_value::<Params>(value) {
            params.apply_intent(self.params.topology.profile);
            self.params = params;
            self.params_edited();
            self.notify(
                format!("{title} parameters restored. Undo (Ctrl+Z) goes back."),
                false,
            );
        }
    }

    pub fn set_preset(&mut self, preset: Experiment) {
        self.remember();
        let p = &mut self.params;
        let note = match preset {
            Experiment::Shapes => {
                p.weights.rect = 1.0;
                p.weights.disc = 1.0;
                p.weights.oct = 1.0;
                p.weights.l = 1.0;
                p.weights.star = 1.0;
                p.m = 8;
                "varied shapes and diagonal directions"
            }
            Experiment::Cycles => {
                p.delta = 0.8;
                "high cycle density"
            }
            Experiment::Junctions => {
                p.p_t = p.p_t.max(0.25);
                p.p_x = p.p_x.max(0.15);
                "T and X junctions favored"
            }
            Experiment::CircularRooms => {
                p.weights.rect = 0.0;
                p.weights.disc = 1.0;
                p.weights.oct = 0.0;
                p.weights.l = 0.0;
                p.weights.star = 0.0;
                "preference for circular rooms"
            }
        };
        self.params_edited();
        self.notify(
            format!("Experiment applied: {note}. Undo (Ctrl+Z) goes back."),
            false,
        );
    }

    pub fn map_bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        match (&self.generated, self.show_building) {
            (Some(map), false) => map.occupied_bounds().or(Some((
                [*map.floor.grid_x.first()?, *map.floor.grid_y.first()?],
                [*map.floor.grid_x.last()?, *map.floor.grid_y.last()?],
            ))),
            _ => self.building.bounds().or_else(|| {
                let d = &self.generated.as_ref()?.floor;
                Some((
                    [*d.grid_x.first()?, *d.grid_y.first()?],
                    [*d.grid_x.last()?, *d.grid_y.last()?],
                ))
            }),
        }
    }

    pub fn showing_building(&self) -> bool {
        (self.show_building || self.generated.is_none()) && self.building.has_layout()
    }

    // ---- frame ------------------------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let alt = |ctx: &egui::Context, key| ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key));
        for (index, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4]
            .into_iter()
            .enumerate()
        {
            if alt(ctx, key) {
                self.set_size(Size::ALL[index]);
            }
        }
        if self.busy && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.cancel_generation();
        }
        if alt(ctx, Key::R) {
            self.random_seed();
        }
        // `/` jumps to the filter of the tree and `:` to the command line, unless a field already has the keyboard.
        if !ctx.egui_wants_keyboard_input() {
            let typed =
                |ctx: &egui::Context, key, mods| ctx.input_mut(|i| i.consume_key(mods, key));
            if typed(ctx, Key::Slash, Modifiers::NONE) {
                ctx.memory_mut(|m| m.request_focus(theme::filter_id()));
            } else if typed(ctx, Key::Colon, Modifiers::NONE)
                || typed(ctx, Key::Semicolon, Modifiers::SHIFT)
                || typed(ctx, Key::K, Modifiers::COMMAND)
            {
                ctx.memory_mut(|m| m.request_focus(theme::command_id()));
            }
        }
        let command =
            |ctx: &egui::Context, key| ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, key));
        if command(ctx, Key::Comma) {
            self.show_settings = true;
        }
        if command(ctx, Key::O) {
            self.open_file();
        }
        if command(ctx, Key::S) {
            self.save_file();
        }
        if command(ctx, Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Ctrl+Z and Ctrl+Y are the text field's own while one has the keyboard.
        if !ctx.egui_wants_keyboard_input() {
            let shift_z =
                ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z));
            if shift_z || command(ctx, Key::Y) {
                self.redo();
            } else if command(ctx, Key::Z) {
                self.undo();
            }
            // Ctrl+C is a Copy event, not a key, and shift is told apart by the modifiers of the frame.
            let (copied, shift) = ctx.input(|i| {
                (
                    i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                    i.modifiers.shift,
                )
            });
            if copied {
                if shift {
                    self.copy_svg(ctx);
                } else {
                    self.copy_json(ctx);
                }
            }
        }
        if command(ctx, Key::F) {
            ctx.memory_mut(|m| m.request_focus(theme::filter_id()));
        }
        if command(ctx, Key::L) {
            ctx.memory_mut(|m| m.request_focus(theme::seed_id()));
        }
        if command(ctx, Key::N) {
            if self.changed.is_empty() {
                self.notify("The parameters are already the size's defaults.", false);
            } else {
                self.reset_all();
            }
        }
        if command(ctx, Key::B) {
            let shown = self
                .reading_open
                .unwrap_or_else(|| ctx.content_rect().width() >= 1180.0);
            self.reading_open = Some(!shown);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
        for (index, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4]
            .into_iter()
            .enumerate()
        {
            if command(ctx, key) {
                self.set_size(Size::ALL[index]);
            }
        }
        if command(ctx, Key::Num0) {
            self.step_text(ctx, 0);
        }
        if command(ctx, Key::Plus) || command(ctx, Key::Equals) {
            self.step_text(ctx, 1);
        }
        if command(ctx, Key::Minus) {
            self.step_text(ctx, -1);
        }
        if command(ctx, Key::W) {
            self.show_json = false;
            self.show_shortcuts = false;
            self.show_settings = false;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1)) {
            self.show_shortcuts = true;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) || command(ctx, Key::Enter) {
            self.generate_now();
        }
        if alt(ctx, Key::ArrowLeft) {
            self.set_seed(self.seed.saturating_sub(1));
        }
        if alt(ctx, Key::ArrowRight) {
            self.set_seed(self.seed.saturating_add(1));
        }
    }

    fn apply_due(&mut self, ctx: &egui::Context) {
        let Some(at) = self.apply_at else { return };
        let now = Instant::now();
        if now < at {
            ctx.request_repaint_after(at - now);
            return;
        }
        self.apply_at = None;
        if self.json_dirty && !self.commit_json() {
            return;
        }
        self.request_generate();
    }

    /// The JSON editor changed: validate and regenerate after the usual pause.
    pub fn apply_at_edit(&mut self) {
        self.apply_at = Some(Instant::now() + APPLY_DELAY);
    }

    pub fn generate_now(&mut self) {
        self.apply_at = None;
        if self.json_dirty && !self.commit_json() {
            self.notify("The parameter JSON has errors; fix it to generate.", true);
            return;
        }
        if self.json_dirty {
            self.sync_json();
        }
        self.request_generate();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_events();
        theme::apply_appearance(&ctx, self.appearance, &self.system_theme);
        self.shortcuts(&ctx);
        self.apply_due(&ctx);
        self.keep_window(&ctx);
        self.keep_title(&ctx);
        self.keep_layers();
        // A message that is not an error fades after a few seconds.
        if let Some(notice) = &self.notice {
            if !notice.error && notice.at.elapsed() > Duration::from_secs(8) {
                self.notice = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
        panels::menu_bar(self, ui);
        panels::top_bar(self, ui);
        panels::command_bar(self, ui);
        panels::tree_panel(self, ui);
        panels::reading_panel(self, ui);
        panels::central(self, ui);
        panels::shortcuts_window(self, &ctx);
        panels::json_window(self, &ctx);
        settings::window(self, &ctx);
        self.take_screenshot(&ctx);
    }
}

/// The settings Λ leaves out of its JSON while they are off, with the values they have then: the desktop compares a Λ with its
/// defaults and must be able to reset a setting that is not in the JSON.
/// The window size of the last run, when it is one that fits the minimum.
fn saved_window_size() -> Option<[f32; 2]> {
    let text = prefs::load("window")?;
    let (w, h) = text.split_once(',')?;
    let (w, h) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
    (w >= 1000.0 && h >= 640.0 && w <= 8000.0 && h <= 8000.0).then_some([w, h])
}

fn skipped_defaults() -> Value {
    serde_json::json!({
        "zones": tapiti_core::params::ZoneSpec::default(),
        "environment": tapiti_core::environment::EnvSpec::default(),
        "levels": tapiti_core::params::LevelsSpec::default(),
        "topology": { "minDetours": 0 },
    })
}

fn setup_style(ctx: &egui::Context) {
    theme::apply(ctx);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title("Tapiti")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
                    .expect("assets/icon.png is a valid PNG"),
            )
            .with_inner_size(saved_window_size().unwrap_or([1360.0, 860.0]))
            // Only the screenshot runs fill the screen (maximized, so the title bar stays): a normal start is a normal window.
            .with_maximized(std::env::var_os("TAPITI_SCREENSHOT").is_some())
            .with_min_inner_size([1000.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tapiti",
        options,
        Box::new(|cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(App::new(cc)))
        }),
    )
}

// ---- helpers kept from the previous frontend ----------------------------------------------------

pub fn translate_violation(message: &str) -> String {
    if let Some((code, detail)) = message.split_once(' ') {
        let meaning = match code {
            "V1" => "room reach",
            "V2" => "distance and collisions",
            "V3" => "doors",
            "V4" => "corridor geometry",
            "V5" => "entrance and exit",
            "T1" | "T2" | "T3" | "T4" | "T5" | "T6" => "topology requirement",
            _ => "model rule",
        };
        format!("{meaning}: {detail}")
    } else {
        message.to_string()
    }
}

fn translate_log(message: &str) -> String {
    if let Some(value) = message.strip_prefix("tree stuck: near ") {
        return format!(
            "The search widened the connection radius to {value}.",
            value = value
        );
    }
    if let Some(value) = message.strip_prefix("layout jitter limited to ") {
        return format!("Grid irregularity was limited to {value}.", value = value);
    }
    message.to_string()
}

fn class_label(class: &str) -> &str {
    match class {
        "rect" => "Rectangular",
        "disc" => "Circular",
        "oct" => "Octagonal",
        "L" => "L shape",
        "star" => "Truncated stars",
        _ => class,
    }
}

fn json_value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn get_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}

fn set_path(value: &mut Value, path: &str, next: Value) {
    let mut parts = path.split('.').peekable();
    let mut cursor = value;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Some(object) = cursor.as_object_mut() {
                object.insert(part.to_string(), next);
            }
            return;
        }
        let Some(object) = cursor.as_object_mut() else {
            return;
        };
        cursor = object
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
}

fn view_box(min: [f64; 2], max: [f64; 2], zoom: f64, pan: [f64; 2]) -> (f64, f64, f64, f64) {
    let width = (max[0] - min[0]).max(1.0);
    let height = (max[1] - min[1]).max(1.0);
    let margin = width.max(height) * 0.035;
    let width = (width + margin * 2.0) / zoom.max(0.05);
    let height = (height + margin * 2.0) / zoom.max(0.05);
    let center_x = (min[0] + max[0]) / 2.0 + pan[0];
    let center_y = (min[1] + max[1]) / 2.0 + pan[1];
    (
        center_x - width / 2.0,
        center_y - height / 2.0,
        width,
        height,
    )
}

fn run_benchmark(size: Size, kind: BenchmarkKind, cancel: &Cancel) -> Option<String> {
    let params = params_of_size(size);
    match kind {
        BenchmarkKind::Forty => {
            let started = Instant::now();
            let mut invalid = 0usize;
            for seed in 1..=40u32 {
                if cancel.is_cancelled() {
                    return None;
                }
                invalid += usize::from(!stage::stage(seed, &params, 8).violations.is_empty());
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            Some(format!(
                "Benchmark, 40 floors: average: {ms:.2} ms per map · {invalid} invalid of 40 · {threads} threads available.",
                ms = elapsed / 40.0,
                invalid = invalid,
                threads = par::threads()
            ))
        }
        BenchmarkKind::Batch => {
            let seeds: Vec<u32> = (1..=800).collect();
            let one_started = Instant::now();
            let one_invalid = seeds
                .iter()
                .take_while(|_| !cancel.is_cancelled())
                .map(|seed| usize::from(!stage::stage(*seed, &params, 8).violations.is_empty()))
                .sum::<usize>();
            let one_ms = one_started.elapsed().as_secs_f64() * 1000.0;
            let parallel_started = Instant::now();
            if cancel.is_cancelled() {
                return None;
            }
            let parallel_invalid = par::pmap(&seeds, |seed| {
                if cancel.is_cancelled() {
                    return 0;
                }
                usize::from(!stage::stage(*seed, &params, 8).violations.is_empty())
            })
            .into_iter()
            .sum::<usize>();
            let parallel_ms = parallel_started.elapsed().as_secs_f64() * 1000.0;
            if cancel.is_cancelled() {
                return None;
            }
            Some(format!(
                "Benchmark, 800 floors: 1 thread {one:.0} ms · {threads} threads {many:.0} ms · invalid: {a}/{b}.",
                one = one_ms,
                threads = par::threads(),
                many = parallel_ms,
                a = one_invalid,
                b = parallel_invalid
            ))
        }
    }
}

/// Multi-line description of a floor, built on demand so it follows the active language.
pub fn summary(map: &GeneratedMap) -> String {
    let m = &map.metrics;
    let validity = if map.violations.is_empty() {
        "valid"
    } else {
        "with violations"
    };
    let warnings = if map.violations.is_empty() {
        String::new()
    } else {
        format!("\nWarnings: {}", map.violations.join("; "))
    };
    format!(
        "{size} size · seed {seed} · {validity} · one floor generated in {ms:.1} ms · {draws} attempt(s)\n{rooms} rooms · {edges} corridors · β={beta} · δ={delta:.2} · {routes} routes to the exit\n{phases}{warnings}",
        size = size_label(map.size),
        seed = map.seed,
        validity = validity,
        ms = map.elapsed_ms,
        draws = map.draws,
        rooms = m.rooms,
        edges = m.edges,
        beta = m.beta,
        delta = m.delta,
        routes = m.routes_to_exit.map_or("no".to_string(), |n| n.to_string()),
        phases = map.phases,
        warnings = warnings
    )
}

/// `None` when the user cancelled before the floor was done.
fn generate_map(
    size: Size,
    seed: u32,
    params: Params,
    cancel: &Cancel,
    mut progress: impl FnMut(GenerationEvent),
) -> Option<Result<GeneratedMap, String>> {
    let started = Instant::now();
    let stage = stage::stage_cancellable(seed, &params, 8, cancel, &mut progress)?;
    if cancel.is_cancelled() {
        return None;
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let measures = metrics_of(&stage.d);
    let chosen_seed = stage.d.seed;
    let phases = stage
        .d
        .phases
        .iter()
        .map(|(name, ms)| format!("{name} {ms:.1} ms"))
        .collect::<Vec<_>>()
        .join(" · ");
    Some(Ok(GeneratedMap {
        floor: Arc::new(stage.d),
        metrics: measures,
        size,
        seed: chosen_seed,
        phases,
        violations: stage.violations,
        draws: stage.draws,
        elapsed_ms,
    }))
}

fn validate_params(p: &Params) -> Result<(), String> {
    fn float(name: &str, value: f64, min: f64, max: f64) -> Result<(), String> {
        if value.is_finite() && (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between {min} and {max}.",
                name = name,
                min = min,
                max = max
            ))
        }
    }

    fn int(name: &str, value: i32, min: i32, max: i32) -> Result<(), String> {
        if (min..=max).contains(&value) {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between {min} and {max}.",
                name = name,
                min = min,
                max = max
            ))
        }
    }

    fn count(name: &str, value: usize, max: usize) -> Result<(), String> {
        if value <= max {
            Ok(())
        } else {
            Err(format!(
                "{name} must be between 0 and {max}.",
                name = name,
                max = max
            ))
        }
    }

    fn by_class(name: &str, values: [(&str, f64); 5], min: f64, max: f64) -> Result<(), String> {
        for (class, value) in values {
            float(&format!("{name}.{class}"), value, min, max)?;
        }
        Ok(())
    }

    int("cols", p.cols, 1, 64)?;
    int("rows", p.rows, 1, 64)?;
    float("cellW", p.cell_w, 1.0, 5000.0)?;
    float("cellH", p.cell_h, 1.0, 5000.0)?;
    float("gap", p.gap, 0.0, 1000.0)?;
    int("big", p.big, 0, 1000)?;
    float("drop", p.drop, 0.0, 1.0)?;
    float("layoutJitter", p.layout_jitter, 0.0, 0.45)?;
    float("homophily", p.homophily, -1.0, 1.0)?;
    float("portStep", p.port_step, 0.1, 10000.0)?;
    float("width", p.width, 0.1, 10000.0)?;
    float("clear", p.clear, 0.0, 1000.0)?;
    float("exit", p.exit, 0.1, 10000.0)?;
    float("seg", p.seg, 0.1, 10000.0)?;
    float("minLen", p.min_len, 0.1, 10000.0)?;
    float("near", p.near, 0.1, 10000.0)?;
    float("delta", p.delta, 0.0, 1.0)?;
    if !matches!(p.m, 4 | 8) {
        return Err("m must be 4 or 8.".into());
    }
    if !matches!(p.disc_ports, 4 | 8 | 16) {
        return Err("discPorts must be 4, 8 or 16.".into());
    }
    if !matches!(p.max_bends, 0 | 1 | 2 | 4 | 5 | 6) {
        return Err("maxBends must be 0, 1, 2, 4, 5 or 6.".into());
    }
    int("growth.rooms", p.growth.rooms, 0, 2000)?;
    int("growth.branch", p.growth.branch, 1, 16)?;
    int("growth.depth", p.growth.depth, 1, 200)?;
    int("growth.islands", p.growth.islands, 1, 16)?;
    float("growth.cluster", p.growth.cluster, -10.0, 10.0)?;
    float("growth.reach", p.growth.reach, 0.0, 20.0)?;
    int("starCells", p.star_cells, 1, 1000)?;
    for (name, value) in [
        ("pI", p.p_i),
        ("pL", p.p_l),
        ("pZ", p.p_z),
        ("pW", p.p_w),
        ("pT", p.p_t),
        ("pX", p.p_x),
        ("costBend", p.cost_bend),
        ("costLen", p.cost_len),
        ("cost45", p.cost45),
        ("costFacing", p.cost_facing),
    ] {
        float(name, value, 0.0, 1000.0)?;
    }
    by_class(
        "weights",
        [
            ("rect", p.weights.rect),
            ("disc", p.weights.disc),
            ("oct", p.weights.oct),
            ("L", p.weights.l),
            ("star", p.weights.star),
        ],
        0.0,
        1000.0,
    )?;
    by_class(
        "maxOf",
        [
            ("rect", p.max_of.rect),
            ("disc", p.max_of.disc),
            ("oct", p.max_of.oct),
            ("L", p.max_of.l),
            ("star", p.max_of.star),
        ],
        0.0,
        10000.0,
    )?;
    for (class, value) in [
        ("rect", p.max_of.rect),
        ("disc", p.max_of.disc),
        ("oct", p.max_of.oct),
        ("L", p.max_of.l),
        ("star", p.max_of.star),
    ] {
        if value.fract() != 0.0 {
            return Err(format!("maxOf.{class} must be an integer.", class = class));
        }
    }
    by_class(
        "leafClasses",
        [
            ("rect", p.leaf_classes.rect),
            ("disc", p.leaf_classes.disc),
            ("oct", p.leaf_classes.oct),
            ("L", p.leaf_classes.l),
            ("star", p.leaf_classes.star),
        ],
        0.0,
        1000.0,
    )?;
    for (name, value) in &p.mix {
        float(&format!("mix.{name}"), *value, 0.0, 1.0)?;
    }
    if let Some(value) = p.junction {
        float("junction", value, 0.0, 10000.0)?;
    }
    int("exitDoors", p.exit_doors, 1, 1000)?;
    int("exitMinHops", p.exit_min_hops, 1, 1000)?;
    int("leaves", p.leaves, 0, 1000)?;
    count("topology.minBranches", p.topology.min_branches, 1000)?;
    count("topology.minBranchDepth", p.topology.min_branch_depth, 1000)?;
    count("topology.minExitRoutes", p.topology.min_exit_routes, 1000)?;
    count("topology.minCycleLength", p.topology.min_cycle_length, 1000)?;
    count("topology.minHubDegree", p.topology.min_hub_degree, 1000)?;
    count("topology.minDetours", p.topology.min_detours, 1000)?;
    if let Some(error) = p.intent_error() {
        return Err(error.into());
    }
    Ok(())
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Entrance => "entrance",
        Role::Exit => "exit",
        Role::Plain => "plain",
    }
}

fn size_label(size: Size) -> &'static str {
    ["Small", "Medium", "Large", "Huge"][size as usize]
}

fn size_description(size: Size) -> String {
    let (cols, rows) = size.grid();
    format!(
        "{} world of {cols} × {rows} cells. Keeps the intent and resets its tuning.",
        size_label(size)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_round_trip_through_their_text() {
        let mut layers = Overlays {
            cells: true,
            height: false,
            water: true,
            temperature: false,
            light: false,
            events: false,
            level: None,
            labels: true,
            ports: false,
            pairs: false,
            candidates: false,
            legend: true,
        };
        let text = layers.to_text();
        assert_eq!(text, "cells,water,labels,legend");
        layers.set_from_text("height,legend,unknown");
        assert_eq!(layers.to_text(), "height,legend");
    }

    #[test]
    fn spine_intent_defaults_pass_desktop_validation_in_every_size() {
        for size in Size::ALL {
            let mut p = params_of_size(size);
            p.apply_intent(TopologyProfile::SpineWithDetours);
            assert!(validate_params(&p).is_ok(), "{}", size.name());
            p.entrance = false;
            assert!(validate_params(&p).is_err());
        }
    }

    #[test]
    fn ring_intent_defaults_pass_desktop_validation_in_every_size() {
        for size in Size::ALL {
            let mut p = params_of_size(size);
            p.apply_intent(TopologyProfile::Ring);
            assert!(validate_params(&p).is_ok(), "{}", size.name());
            p.growth.start = tapiti_core::params::GrowthStart::Center;
            assert!(validate_params(&p).is_err());
        }
    }

    #[test]
    fn imported_detour_counts_are_bounded_before_generation() {
        let mut p = params_of_size(Size::Medium);
        p.apply_intent(TopologyProfile::SpineWithDetours);
        let mut value = serde_json::to_value(p).unwrap();
        set_path(&mut value, "topology.minDetours", Value::from(1001));
        let imported = serde_json::from_value(value).unwrap();
        assert!(validate_params(&imported)
            .unwrap_err()
            .contains("topology.minDetours"));
    }
}
