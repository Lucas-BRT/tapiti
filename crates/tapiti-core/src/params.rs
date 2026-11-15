//! The parameter vector Λ of the Tapiti model (§8): one generator, one Λ that a size preset or the user fills in. The JSON field names are camelCase,
//! so a Λ copied from one frontend (desktop or web) loads in the other.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeClass {
    #[serde(rename = "rect")]
    Rect,
    #[serde(rename = "disc")]
    Disc,
    #[serde(rename = "oct")]
    Oct,
    L,
    #[serde(rename = "star")]
    Star,
}

pub const CLASSES: [ShapeClass; 5] = [
    ShapeClass::Rect,
    ShapeClass::Disc,
    ShapeClass::Oct,
    ShapeClass::L,
    ShapeClass::Star,
];

impl ShapeClass {
    pub fn index(self) -> usize {
        CLASSES.iter().position(|c| *c == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        ["rect", "disc", "oct", "L", "star"][self.index()]
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ByClass {
    pub rect: f64,
    pub disc: f64,
    pub oct: f64,
    #[serde(rename = "L")]
    pub l: f64,
    pub star: f64,
}

impl ByClass {
    pub fn get(&self, c: ShapeClass) -> f64 {
        [self.rect, self.disc, self.oct, self.l, self.star][c.index()]
    }
    const fn new(rect: f64, disc: f64, oct: f64, l: f64, star: f64) -> Self {
        Self {
            rect,
            disc,
            oct,
            l,
            star,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proximity {
    Rng,
    Gabriel,
    Near,
}

fn yes() -> bool {
    true
}

/// How the rooms are laid out: one per grid block, grown one from another off a parent's door, or one per leaf of a binary
/// partition of the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    Grid,
    Growth,
    Bsp,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopologyProfile {
    #[default]
    Classic,
    Backbone,
    Hub,
    RedundantExit,
    LongLoop,
    SpineWithDetours,
    Ring,
    HubAndWings,
}

/// Stacked levels: the floor is made of `count` levels in the same world, each generated on its own, joined by `stairs` stairs between
/// each two consecutive ones where rooms of the two overlap in plan. One level (the default) is the floor as it always was; the settings
/// are then left out of the JSON.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LevelsSpec {
    pub count: usize,
    pub stairs: usize,
    /// The height between two levels, in the units of the floor.
    pub storey: f64,
}

impl Default for LevelsSpec {
    fn default() -> Self {
        Self {
            count: 1,
            stairs: 1,
            storey: 120.0,
        }
    }
}

impl LevelsSpec {
    fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

/// Zoning: the world cut into strips along its longer side, each with a shape class of its own, and optionally joined by single corridors.
/// Fewer than two zones means no zoning; the settings are then left out of the JSON, so older files keep their shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ZoneSpec {
    pub count: usize,
    /// Zones are joined by one corridor each, so removing it splits the floor. Needs the Growth layout.
    pub bottleneck: bool,
}

impl ZoneSpec {
    fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

impl Layout {
    /// Whether the layout places rooms in free space, joining them as it goes (Growth and the binary partition), not one per grid block.
    pub fn places_freely(self) -> bool {
        self != Self::Grid
    }

    /// Whether the layout can run the intent: the intents that plan their own rooms (the route, the ring, the hub) work inside Growth.
    pub fn allows(self, profile: TopologyProfile) -> bool {
        self == Self::Growth || !profile.needs_growth()
    }

    /// How the layout places rooms, in words for the apps.
    pub fn guide(self) -> &'static str {
        match self {
            Self::Grid => "Rooms sit in a grid of blocks, one per block (some merged into large blocks, some left empty). A spanning tree joins them, then cycles are added.",
            Self::Growth => "Rooms grow one from another off free doors, so positions are less aligned. The grid only sets the size of the world.",
            Self::Bsp => "The world is cut into leaves of varied proportions with one room in each, joined sibling to sibling, like a planned building.",
        }
    }
}

impl TopologyProfile {
    /// Whether the intent plans its own rooms inside the Growth layout, so no other layout can run it.
    pub fn needs_growth(self) -> bool {
        matches!(
            self,
            Self::SpineWithDetours | Self::Ring | Self::HubAndWings
        )
    }
}

/// What an intent builds, what it needs to work and what choosing it changes in Λ, in words for the apps. Kept beside
/// `apply_intent` and `intent_error` so the text follows what they do.
#[derive(Clone, Copy, Debug)]
pub struct IntentGuide {
    pub makes: &'static str,
    pub needs: &'static str,
    pub sets: &'static str,
    pub zones: &'static str,
}

impl TopologyProfile {
    pub fn guide(self) -> IntentGuide {
        const JOINS: &str =
            "Can be joined by single corridors (Growth or Binary partition layout).";
        const STYLE: &str = "Shapes only: this intent crosses every zone boundary.";
        let (makes, needs, sets, zones) = match self {
            Self::Classic => (
                "An open-ended floor: rooms and corridors shape it, then cycles are added up to the density δ.",
                "Nothing in particular.",
                "Nothing: every setting stays as it is.",
                JOINS,
            ),
            Self::Backbone => (
                "A route from the entrance to the exit with side branches to explore.",
                "An entrance and an exit.",
                "Nothing: at least two side branches are required and checked once the floor is built.",
                JOINS,
            ),
            Self::Hub => (
                "A crossroads: at least one room with three ways out.",
                "Nothing in particular.",
                "Nothing: the requirement is checked once the floor is built.",
                JOINS,
            ),
            Self::RedundantExit => (
                "Two independent routes to the exit, so no single corridor cuts it off.",
                "An entrance, an exit, one island, and a door on the exit for each route.",
                "Turns on the entrance and the exit, one island and at least two doors on the exit.",
                "Shapes only: a single-corridor join would leave one route.",
            ),
            Self::LongLoop => (
                "Only long cycles, of four rooms or more: no small loops.",
                "Nothing in particular.",
                "Nothing: the requirement is checked once the floor is built.",
                JOINS,
            ),
            Self::SpineWithDetours => (
                "A main route is built first, then dead-end branches and detours that rejoin it farther on.",
                "Growth, an entrance and an exit, one island.",
                "Turns on Growth, the entrance and the exit, one island, and a growth depth that reaches the exit.",
                STYLE,
            ),
            Self::Ring => (
                "Rooms round an empty court: two ways round to an exit opposite the entrance.",
                "Growth from the north border, an entrance and an exit, one island, and a world big enough for the ring.",
                "Turns on Growth from the north, the entrance and the exit, one island and two doors on the exit.",
                STYLE,
            ),
            Self::HubAndWings => (
                "A central hall that is the entrance, with three or four wings of different depths; the longest ends at the exit.",
                "Growth from the centre, an entrance and an exit, one island.",
                "Turns on Growth from the middle, the entrance and the exit, one island and a hub degree of three.",
                STYLE,
            ),
        };
        IntentGuide {
            makes,
            needs,
            sets,
            zones,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TopologySpec {
    pub profile: TopologyProfile,
    pub min_branches: usize,
    pub min_branch_depth: usize,
    pub min_exit_routes: usize,
    pub min_cycle_length: usize,
    pub min_hub_degree: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub min_detours: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

impl Default for TopologySpec {
    fn default() -> Self {
        Self {
            profile: TopologyProfile::Classic,
            min_branches: 0,
            min_branch_depth: 0,
            min_exit_routes: 1,
            min_cycle_length: 0,
            min_hub_degree: 0,
            min_detours: 0,
        }
    }
}

impl TopologySpec {
    /// The requirements of the profile and the explicit minimums. Side branches and independent routes are measured from the
    /// path between the entrance and the exit, so without both they cannot hold and are left out.
    pub fn requirements_with(self, anchors: bool) -> Self {
        let mut out = self.requirements();
        if !anchors {
            out.min_branches = 0;
            out.min_branch_depth = 0;
            out.min_exit_routes = 1;
            out.min_detours = 0;
        }
        out
    }

    pub fn requirements(self) -> Self {
        let mut out = self;
        if out.min_branch_depth > 0 {
            out.min_branches = out.min_branches.max(1);
        }
        match self.profile {
            TopologyProfile::Classic => {}
            TopologyProfile::Backbone => {
                out.min_branches = out.min_branches.max(2);
                out.min_branch_depth = out.min_branch_depth.max(1);
            }
            TopologyProfile::Hub => out.min_hub_degree = out.min_hub_degree.max(3),
            TopologyProfile::RedundantExit => out.min_exit_routes = out.min_exit_routes.max(2),
            TopologyProfile::LongLoop => out.min_cycle_length = out.min_cycle_length.max(4),
            TopologyProfile::HubAndWings => out.min_hub_degree = out.min_hub_degree.max(3),
            TopologyProfile::Ring => {
                out.min_exit_routes = out.min_exit_routes.max(2);
                out.min_cycle_length = out.min_cycle_length.max(4);
            }
            TopologyProfile::SpineWithDetours => {
                out.min_branches = out.min_branches.max(1);
                out.min_branch_depth = out.min_branch_depth.max(1);
                out.min_detours = out.min_detours.max(1);
            }
        }
        out
    }
}

/// Where the first room of the grown layout stands: the middle of the world, or against one of its borders (north is the top of the map).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrowthStart {
    #[default]
    Center,
    North,
    South,
    East,
    West,
}

/// Grown layout: rooms are placed one after another off a parent's door, instead of one per grid block.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GrowthSpec {
    /// Where the first room (the entrance, when there is one) stands.
    pub start: GrowthStart,
    /// Rooms to grow; 0 takes the number of live blocks the grid scaffold would have given.
    pub rooms: i32,
    /// Each room spawns 1..=branch children.
    pub branch: i32,
    /// Deepest generation a room may be in (the first room is 0).
    pub depth: i32,
    /// Placement bias: weight e^(cluster·n) with n the rooms near the spot. > 0 clumps, < 0 spreads out.
    pub cluster: f64,
    /// Mean corridor reach beyond the minimum, in cells.
    pub reach: f64,
    /// Clumps to split the rooms into; a bridge corridor three to six cells long starts each one after the first.
    pub islands: i32,
}

impl Default for GrowthSpec {
    fn default() -> Self {
        Self {
            start: GrowthStart::Center,
            rooms: 0,
            branch: 3,
            depth: 8,
            cluster: 0.0,
            reach: 1.0,
            islands: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// The layout mode; the `growth` settings apply only to `growth`.
    #[serde(default)]
    pub layout: Layout,
    // Scaffold (§7.1)
    pub cols: i32,
    pub rows: i32,
    pub cell_w: f64,
    pub cell_h: f64,
    pub gap: f64,
    pub big: i32,
    pub drop: f64,
    /// Fractional displacement of interior grid lines; 0 preserves a uniform grid.
    #[serde(default)]
    pub layout_jitter: f64,
    /// Rotate polygonal rooms by a direction from the active direction alphabet.
    #[serde(default)]
    pub rotate_rooms: bool,
    // Shape classes (§7.2)
    pub weights: ByClass,
    pub max_of: ByClass,
    pub star_cells: i32,
    pub homophily: f64,
    // Directions (§1, §3.1)
    pub m: i32,
    pub disc_ports: i32,
    pub port_step: f64,
    // Corridors (§4)
    pub width: f64,
    pub clear: f64,
    pub exit: f64,
    pub seg: f64,
    pub min_len: f64,
    pub max_bends: i32,
    pub p_i: f64,
    pub p_l: f64,
    pub p_z: f64,
    #[serde(default)]
    pub p_w: f64,
    pub p_t: f64,
    pub p_x: f64,
    pub cost_bend: f64,
    pub cost_len: f64,
    pub cost45: f64,
    pub cost_facing: f64,
    /// The mixing matrix M (§7.3.1), keys `a-b` in CLASSES order; 0 forbids, missing is 1.
    pub mix: BTreeMap<String, f64>,
    // Candidate pairs (§7.3), density (§6.1), roles (V5)
    pub proximity: Proximity,
    pub near: f64,
    pub delta: f64,
    /// Whether the floor has an entrance room.
    #[serde(default = "yes")]
    pub entrance: bool,
    /// Whether the floor has an exit room (only with an entrance).
    #[serde(default = "yes")]
    pub exit_room: bool,
    pub exit_doors: i32,
    pub exit_min_hops: i32,
    // Leaf chambers (§7.6)
    pub leaves: i32,
    pub leaf_classes: ByClass,
    /// Junction conductance ω_h (§4.2.1): null merges the arms into one space. Shown, not used by the generator.
    pub junction: Option<f64>,
    #[serde(default)]
    pub topology: TopologySpec,
    #[serde(default)]
    pub growth: GrowthSpec,
    #[serde(default, skip_serializing_if = "ZoneSpec::is_off")]
    pub zones: ZoneSpec,
    /// The levels of a stacked floor; left out of the JSON with one level.
    #[serde(default, skip_serializing_if = "LevelsSpec::is_off")]
    pub levels: LevelsSpec,
    /// The environment layers; left out of the JSON while all are off.
    #[serde(default, skip_serializing_if = "crate::environment::EnvSpec::is_off")]
    pub environment: crate::environment::EnvSpec,
}

/// The pair key of the mixing matrix, the same whatever the order.
pub fn mix_key(a: ShapeClass, b: ShapeClass) -> String {
    let (x, y) = if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    };
    format!("{}-{}", x.name(), y.name())
}

fn full_mix() -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for a in CLASSES {
        for b in CLASSES {
            out.insert(mix_key(a, b), 1.0);
        }
    }
    out
}

fn base() -> Params {
    Params {
        layout: Layout::Grid,
        cols: 4,
        rows: 3,
        cell_w: 270.0,
        cell_h: 230.0,
        gap: 56.0,
        big: 2,
        drop: 0.15,
        layout_jitter: 0.0,
        rotate_rooms: false,
        weights: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        max_of: ByClass::new(10000.0, 10000.0, 10000.0, 10000.0, 10000.0),
        star_cells: 2,
        homophily: 0.0,
        m: 4,
        disc_ports: 8,
        port_step: 12.0,
        width: 44.0,
        clear: 12.0,
        exit: 24.0,
        seg: 24.0,
        min_len: 112.0,
        max_bends: 2,
        p_i: 1.0,
        p_l: 1.0,
        p_z: 0.6,
        p_w: 0.0,
        p_t: 0.0,
        p_x: 0.0,
        cost_bend: 1.0,
        cost_len: 0.004,
        cost45: 0.5,
        cost_facing: 0.5,
        mix: full_mix(),
        proximity: Proximity::Rng,
        near: 520.0,
        delta: 0.15,
        entrance: true,
        exit_room: true,
        exit_doors: 1,
        exit_min_hops: 3,
        leaves: 0,
        leaf_classes: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        junction: None,
        topology: TopologySpec::default(),
        growth: GrowthSpec::default(),
        zones: ZoneSpec::default(),
        levels: LevelsSpec::default(),
        environment: Default::default(),
    }
}

impl Params {
    /// Apply an explicitly chosen intent and the settings it needs; importing JSON never calls this.
    pub fn apply_intent(&mut self, profile: TopologyProfile) {
        self.topology.profile = profile;
        if profile == TopologyProfile::SpineWithDetours {
            self.layout = Layout::Growth;
            self.entrance = true;
            self.exit_room = true;
            self.growth.islands = 1;
            let req = self.topology.requirements();
            self.topology.min_branches = req.min_branches;
            self.topology.min_branch_depth = req.min_branch_depth;
            self.topology.min_detours = req.min_detours;
            self.growth.depth = self
                .growth
                .depth
                .max(self.exit_min_hops + req.min_branch_depth as i32);
        }
        if profile == TopologyProfile::HubAndWings {
            self.layout = Layout::Growth;
            self.growth.start = GrowthStart::Center;
            self.entrance = true;
            self.exit_room = true;
            self.growth.islands = 1;
            self.topology.min_hub_degree = self.topology.requirements().min_hub_degree.min(4);
            self.growth.depth = self.growth.depth.max(self.exit_min_hops + 1);
        }
        if profile == TopologyProfile::Ring {
            self.layout = Layout::Growth;
            self.growth.start = GrowthStart::North;
            self.entrance = true;
            self.exit_room = true;
            self.growth.islands = 1;
            self.exit_doors = self.exit_doors.max(2);
            self.growth.depth = self.growth.depth.max(self.ring_rooms() as i32 / 2 + 1);
        }
        if profile == TopologyProfile::RedundantExit {
            self.entrance = true;
            self.exit_room = true;
            self.growth.islands = 1;
            self.exit_doors = self
                .exit_doors
                .max(self.topology.requirements().min_exit_routes as i32);
        }
    }

    /// Most zones the world holds: the strips cut along its longer side stay two cells wide, up to four.
    pub fn zone_limit(&self) -> usize {
        (self.cols.max(self.rows).max(1) as usize / 2).clamp(2, 4)
    }

    /// The zones in use: 1 without zoning.
    pub fn zone_count(&self) -> usize {
        if self.zones.count >= 2 {
            self.zones.count.min(self.zone_limit())
        } else {
            1
        }
    }

    /// The strip of zone `k` as `[x0, y0, x1, y1]`, in the world of the scaffold (from the origin, `cols` by `rows` cells).
    pub fn zone_strip(&self, k: usize) -> [f64; 4] {
        let (w, h) = (
            self.cols.max(1) as f64 * self.cell_w,
            self.rows.max(1) as f64 * self.cell_h,
        );
        let n = self.zone_count() as f64;
        let (lo, hi) = (k as f64 / n, (k + 1) as f64 / n);
        if w >= h {
            [w * lo, 0.0, w * hi, h]
        } else {
            [0.0, h * lo, w, h * hi]
        }
    }

    /// The zone a point stands in. A pure function of the point and Λ: it draws nothing from the generator.
    pub fn zone_of(&self, at: [f64; 2]) -> usize {
        let n = self.zone_count();
        let (w, h) = (
            self.cols.max(1) as f64 * self.cell_w,
            self.rows.max(1) as f64 * self.cell_h,
        );
        let t = if w >= h { at[0] / w } else { at[1] / h };
        ((t * n as f64).floor().max(0.0) as usize).min(n - 1)
    }

    /// Why zones cannot be joined by single corridors with this layout and intent, if they cannot.
    pub fn zone_bottleneck_blocker(&self) -> Option<&'static str> {
        if !self.layout.places_freely() {
            return Some(
                "Single-corridor joins need the Growth or Binary partition layout; the grid only gets zone styles.",
            );
        }
        matches!(
            self.topology.profile,
            TopologyProfile::Ring | TopologyProfile::HubAndWings | TopologyProfile::SpineWithDetours
        )
        .then_some("This intent crosses every zone boundary, so zones keep their styles but cannot be single corridors.")
    }

    /// Whether the zones are joined by single corridors (it needs a layout that places rooms in free space, zone by zone).
    pub fn bottleneck_on(&self) -> bool {
        self.zone_count() > 1 && self.zones.bottleneck && self.layout.places_freely()
    }

    /// Clumps of the grown layout joined by single corridors: the zones when they are bottlenecks, else the islands.
    pub fn clumps(&self) -> usize {
        if self.bottleneck_on() {
            self.zone_count()
        } else if self.layout == Layout::Growth {
            self.growth.islands.max(1) as usize
        } else {
            1
        }
    }

    /// Turns zoning on with `count` zones. A zone's look is its shape class, so with rectangles only the weights get a mix.
    pub fn set_zones(&mut self, count: usize, bottleneck: bool) {
        self.zones = ZoneSpec { count, bottleneck };
        let others = [
            ShapeClass::Disc,
            ShapeClass::Oct,
            ShapeClass::L,
            ShapeClass::Star,
        ];
        if count >= 2 && others.iter().all(|c| self.weights.get(*c) <= 0.0) {
            self.weights = ByClass::new(0.4, 0.2, 0.2, 0.2, 0.0);
        }
    }

    /// Rooms in each wing of the hub, the first being the route to the exit: `exitMinHops` rooms, then one fewer each, never fewer
    /// than one. The wings are as many as the hub degree asks for, three at least and four at most.
    pub fn hub_wing_depths(&self) -> Vec<usize> {
        let hops = self.exit_min_hops.max(1) as usize;
        let wings = self.topology.requirements().min_hub_degree.clamp(3, 4);
        (0..wings).map(|i| hops.saturating_sub(i).max(1)).collect()
    }

    /// Rooms on the ring: even, so the exit stands opposite the entrance at `exitMinHops` or more room hops from it.
    pub fn ring_rooms(&self) -> usize {
        (2 * self.exit_min_hops.max(2)) as usize
    }

    /// The ellipse the ring rooms centre on: the world's middle and the radii that keep the first room a `gap` from the north border.
    pub fn ring_ellipse(&self) -> ([f64; 2], [f64; 2]) {
        let (w, h) = (
            self.cols.max(1) as f64 * self.cell_w,
            self.rows.max(1) as f64 * self.cell_h,
        );
        let (bw, bh) = (self.cell_w - 2.0 * self.gap, self.cell_h - 2.0 * self.gap);
        (
            [w / 2.0, h / 2.0],
            [
                (w / 2.0 - self.gap - bw / 2.0).max(1.0),
                (h / 2.0 - self.gap - bh / 2.0).max(1.0),
            ],
        )
    }

    /// How many one-cell rooms fit round the ring ellipse, a room and a minimum corridor apart (Ramanujan's perimeter).
    pub fn ring_capacity(&self) -> usize {
        let (_, [a, b]) = self.ring_ellipse();
        let perimeter =
            std::f64::consts::PI * (3.0 * (a + b) - ((3.0 * a + b) * (a + 3.0 * b)).sqrt());
        let step = self.cell_w - 2.0 * self.gap + self.min_len;
        (perimeter / step.max(1.0)).floor() as usize
    }

    /// Combinations that cannot express the requested intent, before spending attempts on geometry.
    pub fn intent_error(&self) -> Option<&'static str> {
        if self.topology.profile == TopologyProfile::SpineWithDetours {
            if self.layout != Layout::Growth
                || !self.entrance
                || !self.exit_room
                || self.growth.islands != 1
            {
                return Some(
                    "Spine with detours needs Growth, an entrance and exit, and one island.",
                );
            }
            if self.growth.depth < self.exit_min_hops {
                return Some("Growth depth must reach the requested main route length.");
            }
        }
        if self.levels.count > 1 || self.levels != LevelsSpec::default() {
            if !(1..=8).contains(&self.levels.count) {
                return Some("Levels: from 1 to 8.");
            }
            if !(1..=4).contains(&self.levels.stairs) {
                return Some("Levels: from 1 to 4 stairs between two levels.");
            }
            if !(self.levels.storey.is_finite() && self.levels.storey > 0.0) {
                return Some("Levels: the height of a storey must be greater than 0.");
            }
            if self.levels.count > 1 && !(self.entrance && self.exit_room) {
                return Some(
                    "Levels need an entrance and an exit: the path climbs from one to the other.",
                );
            }
            if self.levels.count > 1
                && self.environment.height.enabled
                && self.environment.height.phase == crate::environment::LayerPhase::Before
            {
                return Some("Height before the layout is not available with several levels yet.");
            }
        }
        if self.zones.count >= 2 {
            if self.zones.count > self.zone_limit() {
                return Some(
                    "The world is too small for that many zones: each strip needs two cells.",
                );
            }
            if self.zones.bottleneck {
                if let Some(reason) = self.zone_bottleneck_blocker() {
                    return Some(reason);
                }
            }
        }
        if self.environment.height.enabled {
            if let Some(error) = self.environment.height.error() {
                return Some(error);
            }
        }
        if self.environment.light.enabled {
            if let Some(error) = self.environment.light.error() {
                return Some(error);
            }
        }
        if self.environment.events.enabled {
            if let Some(error) = self.environment.events.error() {
                return Some(error);
            }
            if self
                .environment
                .events
                .list
                .iter()
                .any(|e| e.zone.is_some_and(|z| z >= self.zone_count()))
            {
                return Some("Events: a kind is limited to a zone the map does not have.");
            }
            let env = &self.environment;
            for c in self
                .environment
                .events
                .list
                .iter()
                .flat_map(|e| &e.conditions)
            {
                use crate::environment::ConditionField as F;
                let missing = match c.field {
                    F::Height => !env.height.enabled,
                    F::WaterDepth | F::Humidity => !env.water.enabled,
                    F::Temperature => !env.climate.enabled,
                    F::Light => !env.light.enabled,
                    F::Level => false,
                };
                if missing {
                    return Some("Events: a condition needs the layer it looks at turned on.");
                }
            }
        }
        if self.environment.climate.enabled {
            if !self.environment.height.enabled {
                return Some("Climate needs the depth: turn Height on too.");
            }
            if let Some(error) = self.environment.climate.error() {
                return Some(error);
            }
        }
        if self.environment.water.enabled {
            if !self.environment.height.enabled {
                return Some("Water runs down the ground: turn Height on too.");
            }
            if let Some(error) = self.environment.water.error() {
                return Some(error);
            }
        }
        if self.topology.profile == TopologyProfile::HubAndWings
            && (self.layout != Layout::Growth
                || self.growth.start != GrowthStart::Center
                || !self.entrance
                || !self.exit_room
                || self.growth.islands != 1)
        {
            return Some(
                "Hub and wings needs Growth starting at the centre, an entrance and exit, and one island.",
            );
        }
        if self.topology.profile == TopologyProfile::Ring {
            if self.layout != Layout::Growth
                || self.growth.start != GrowthStart::North
                || !self.entrance
                || !self.exit_room
                || self.growth.islands != 1
            {
                return Some(
                    "Ring needs Growth starting at the north border, an entrance and exit, and one island.",
                );
            }
            if self.ring_rooms() > self.ring_capacity() {
                return Some("The ring needs more rooms than fit around this world.");
            }
        }
        if self.entrance
            && self.exit_room
            && self.topology_requirements().min_exit_routes > self.exit_doors.max(0) as usize
        {
            return Some("The exit needs at least one door per independent route.");
        }
        None
    }

    /// The topology requirements this Λ can meet: those that need the path between an entrance and an exit are left out without
    /// both, and independent routes to the exit are left out with several islands, whose bridges are single corridors.
    pub fn topology_requirements(&self) -> TopologySpec {
        let mut out = self
            .topology
            .requirements_with(self.entrance && self.exit_room);
        if self.clumps() > 1 {
            out.min_exit_routes = 1;
        }
        out
    }
}

/// A starting size for the floor: the grid and how many large blocks and leaf chambers go with it. A quick way to begin;
/// every other parameter stays at its default and any value can be changed afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
    Huge,
}

impl Size {
    pub const ALL: [Size; 4] = [Size::Small, Size::Medium, Size::Large, Size::Huge];

    pub fn name(self) -> &'static str {
        ["small", "medium", "large", "huge"][self as usize]
    }

    /// The size at position `n` (1 to 4) of `ALL`, clamped.
    pub fn of_index(n: usize) -> Size {
        Size::ALL[n.clamp(1, 4) - 1]
    }

    /// Columns by rows of the grid.
    pub fn grid(self) -> (i32, i32) {
        [(4, 3), (6, 5), (10, 8), (16, 12)][self as usize]
    }
}

/// Λ of a size: the grid, about one large block in seven cells, one leaf chamber in eight and the exit distance that fits.
pub fn params_of_size(size: Size) -> Params {
    let mut p = base();
    let (cols, rows) = size.grid();
    let cells = cols * rows;
    p.cols = cols;
    p.rows = rows;
    p.big = (cells + 3) / 7;
    p.leaves = [1, 4, 10, 24][size as usize];
    // A small floor has few rooms between its two ends: the exit must be at least this many room hops from the entrance.
    p.exit_min_hops = [2, 3, 4, 5][size as usize];
    p
}

/// `params_of_size` with every shape, junction and connection option switched on, for tests, benchmarks and demos: a floor that
/// uses the whole generator. It is not a preset of the apps, which start from `params_of_size`.
pub fn rich_of_size(size: Size) -> Params {
    let mut p = params_of_size(size);
    p.m = 8;
    p.weights = ByClass::new(0.3, 0.2, 0.15, 0.2, 0.15);
    p.proximity = Proximity::Near;
    p.delta = 0.55;
    p.p_t = 0.25;
    p.p_x = 0.15;
    p.exit_doors = 2;
    p.exit_min_hops = 4;
    p.leaf_classes = ByClass::new(1.0, 1.0, 1.0, 0.0, 0.0);
    p
}
