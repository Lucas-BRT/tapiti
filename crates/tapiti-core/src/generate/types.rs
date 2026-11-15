//! Doors, rooms, edges, candidates and the finished `Floor`.

use super::*;

/// A door (§3): where on the room's outline a corridor meets it and the direction (0..7, ×45°) it leaves in.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Port {
    pub room: usize,
    pub p: V,
    pub k: i32,
    pub edge: i32,
    pub embed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Entrance,
    Exit,
    Plain,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Room {
    pub id: usize,
    pub cls: ShapeClass,
    pub shape: Shape,
    #[serde(skip)]
    pub bounds: Bounds,
    pub center: V,
    pub area: f64,
    #[serde(rename = "box")]
    pub rect: Rect,
    /// Admissible doors, sampled `portStep` px apart along each edge (the set is continuous; these are the positions tried).
    pub ports: Vec<Port>,
    pub used: Vec<Port>,
    pub leaf: bool,
    pub alive: bool,
    pub role: Role,
    /// The level of a stacked floor the room is on; left out of the JSON for level 0.
    #[serde(skip_serializing_if = "is_level_zero")]
    pub level: usize,
}

fn is_level_zero(level: &usize) -> bool {
    *level == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Room { room: usize },
    Junction { p: Box<V> },
}

/// A branch of a corridor between two nodes of H (§5.1): a room or a junction at each end.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: usize,
    pub u: usize,
    pub v: usize,
    pub pts: Vec<V>,
    pub ends: [Option<Port>; 2],
    /// A stair between two levels of a stacked floor, not a corridor: it has no doors and no geometry of its own.
    #[serde(skip_serializing_if = "is_false")]
    pub stair: bool,
    /// Collision geometry is kept in Rust for generation/validation but is not
    /// needed by the viewer's map representation.
    #[serde(skip)]
    pub region: Vec<Bounded>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RouteType {
    I,
    L,
    Z,
    W,
}

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub a: usize,
    pub b: usize,
    pub ports: [Port; 2],
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
    pub cost: f64,
    pub weight: f64,
    pub region: Vec<Bounded>,
}

/// The compact candidate geometry sent to the viewer. Search-only ports,
/// weights and collision regions are released after generation.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateView {
    pub a: usize,
    pub b: usize,
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Floor {
    pub params: Params,
    pub seed: u32,
    pub rooms: Vec<Room>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// The entrance room, when the Λ asks for one.
    pub entrance: Option<usize>,
    /// The exit room: only with an entrance, and when the Λ asks for one.
    pub exit: Option<usize>,
    /// Node IDs on the designated entrance-to-exit path in H, before leaf rooms are added (empty without both).
    pub main_path: Vec<usize>,
    /// Grid line coordinates; interior lines may be jittered while the outer bounds stay fixed.
    pub grid_x: Vec<f64>,
    pub grid_y: Vec<f64>,
    pub beta_max: i32,
    pub beta_target: i32,
    pub candidates: Vec<CandidateView>,
    pub pairs: Vec<[usize; 2]>,
    pub dropped: Vec<V>,
    pub log: Vec<String>,
    /// What the environment layers computed; absent while none ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<crate::environment::Environment>,
    /// How a stacked floor is made of its levels; absent with one level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<StackInfo>,
    /// Milliseconds each phase of the generator took, in order (rooms, candidates, tree, β_max, cycles, leaf chambers).
    pub phases: Vec<(String, f64)>,
}

/// Where each level of a stacked floor starts in the ids of the whole, and the anchors of its own.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelInfo {
    pub room_offset: usize,
    pub rooms: usize,
    pub node_offset: usize,
    pub nodes: usize,
    pub edge_offset: usize,
    pub edges: usize,
    /// The level's own entrance and exit (room ids of the whole), and its path between them (node ids of the whole).
    pub entrance: Option<usize>,
    pub exit: Option<usize>,
    pub main_path: Vec<usize>,
}

/// A stacked floor: levels generated one by one in the same world, joined by stairs where rooms of consecutive levels overlap.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackInfo {
    pub levels: Vec<LevelInfo>,
}

/// A lightweight event stream for drawing a floor while it is being built.
#[derive(Clone, Debug)]
pub enum GenerationEvent {
    AttemptStarted {
        draw: u32,
        seed: u32,
    },
    Layout {
        grid_x: Vec<f64>,
        grid_y: Vec<f64>,
        corridor_width: f64,
    },
    RoomAdded(Shape),
    EdgeUpdated {
        id: usize,
        pts: Vec<V>,
    },
}

impl Floor {
    /// The entrance, or the first live room when the floor has none: where reading the floor from one room starts.
    pub fn origin(&self) -> usize {
        self.entrance
            .or_else(|| self.rooms.iter().find(|r| r.alive).map(|r| r.id))
            .unwrap_or(0)
    }
}
