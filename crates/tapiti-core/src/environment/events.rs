//! Events: things the user names and scatters over the floor, each with a count and a distribution over the rooms.
//!
//! An event kind is just a name, a number of occurrences and a [`Placement`] that says where they are likely (anywhere, near or far from
//! the entrance, on the main path, in dead ends, on high or low ground), with optional limits: a zone of the map, a minimum number of
//! corridors between two of the same kind, and keeping off the entrance and the exit. The layer decides nothing about what an event is
//! or does: it only says in which rooms it happens. Kinds are placed in the order of the list, each from the stream of the layer.

use super::{
    placement::{place, Placement},
    Environment, Layer, LayerPhase,
};
use crate::{
    generate::{Floor, Node},
    rng::Mulberry,
};
use serde::{Deserialize, Serialize};

/// One kind of event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventSpec {
    pub name: String,
    /// How many occurrences to place (fewer when the floor has no room left that fits).
    pub count: usize,
    pub placement: Placement,
    /// The fewest corridors between two occurrences of this kind, 0 for no limit.
    pub min_gap: usize,
    /// Only rooms of this zone (0 is the first strip), when zones are on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<usize>,
    /// Keep off the entrance and the exit.
    pub avoid_entrance: bool,
}

impl Default for EventSpec {
    fn default() -> Self {
        Self {
            name: "event".into(),
            count: 1,
            placement: Placement::Uniform,
            min_gap: 0,
            zone: None,
            avoid_entrance: true,
        }
    }
}

/// The settings of the layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventsSpec {
    pub enabled: bool,
    /// At most one event in a room, whatever its kind.
    pub exclusive: bool,
    pub list: Vec<EventSpec>,
}

impl Default for EventsSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            exclusive: true,
            list: Vec::new(),
        }
    }
}

impl EventsSpec {
    /// What is wrong with the settings, if anything.
    pub fn error(&self) -> Option<&'static str> {
        if self.list.len() > 16 {
            return Some("Events: at most 16 kinds.");
        }
        for (i, e) in self.list.iter().enumerate() {
            let name = e.name.trim();
            if name.is_empty() || name.chars().count() > 32 {
                return Some("Events: a name has from 1 to 32 characters.");
            }
            if self.list[..i].iter().any(|o| o.name.trim() == name) {
                return Some("Events: two kinds have the same name.");
            }
            if e.count > 256 || e.min_gap > 64 {
                return Some("Events: at most 256 occurrences and a gap of 64 corridors.");
            }
        }
        None
    }
}

/// Where one kind of event happens.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventKind {
    pub name: String,
    /// The nodes (rooms) with an occurrence, in the order drawn.
    pub nodes: Vec<usize>,
}

/// Every kind, in the order of the list.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsField {
    pub kinds: Vec<EventKind>,
}

/// Corridors from `src` to every node; `usize::MAX` where nothing reaches.
pub fn hops_from(floor: &Floor, src: usize) -> Vec<usize> {
    let n = floor.nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &floor.edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    let mut hops = vec![usize::MAX; n];
    hops[src] = 0;
    let mut queue = std::collections::VecDeque::from([src]);
    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if hops[v] == usize::MAX {
                hops[v] = hops[u] + 1;
                queue.push_back(v);
            }
        }
    }
    hops
}

fn node_of_room(floor: &Floor, room: usize) -> Option<usize> {
    floor
        .nodes
        .iter()
        .position(|n| matches!(n, Node::Room { room: r } if *r == room))
}

/// The nodes of the entrance and of the exit.
fn doors(floor: &Floor) -> Vec<usize> {
    [floor.entrance, floor.exit]
        .into_iter()
        .flatten()
        .filter_map(|r| node_of_room(floor, r))
        .collect()
}

/// The zone a node stands in (the one of its room's centre); `None` for a junction.
fn zone_of_node(floor: &Floor, i: usize) -> Option<usize> {
    match floor.nodes[i] {
        Node::Room { room } => Some(floor.params.zone_of(floor.rooms[room].center)),
        Node::Junction { .. } => None,
    }
}

/// The layer: it runs after the layout, and uses the heights for the placements that need them.
pub struct EventsLayer(pub EventsSpec);

impl Layer for EventsLayer {
    fn name(&self) -> &'static str {
        "events"
    }
    fn tag(&self) -> u32 {
        // "EVNT"
        0x4556_4e54
    }
    fn phase(&self) -> LayerPhase {
        LayerPhase::After
    }
    fn after(&self, floor: &Floor, rng: &mut Mulberry, out: &mut Environment) {
        let spec = &self.0;
        let n = floor.nodes.len();
        let heights = out.height.as_ref().map(|h| h.nodes.as_slice());
        let doors = doors(floor);
        let mut taken: Vec<usize> = Vec::new();
        let mut kinds = Vec::new();
        for event in &spec.list {
            let mut base: Vec<usize> = Vec::new();
            if event.avoid_entrance {
                base.extend(&doors);
            }
            if spec.exclusive {
                base.extend(&taken);
            }
            if let Some(zone) = event.zone {
                base.extend((0..n).filter(|&i| zone_of_node(floor, i) != Some(zone)));
            }
            let mut nodes: Vec<usize> = Vec::new();
            for _ in 0..event.count {
                let mut exclude = base.clone();
                for &chosen in &nodes {
                    exclude.push(chosen);
                    if event.min_gap > 0 {
                        let hops = hops_from(floor, chosen);
                        exclude.extend((0..n).filter(|&i| hops[i] <= event.min_gap));
                    }
                }
                match place(floor, heights, event.placement, 1, &exclude, rng).first() {
                    Some(&i) => nodes.push(i),
                    None => break,
                }
            }
            taken.extend(&nodes);
            kinds.push(EventKind {
                name: event.name.trim().to_string(),
                nodes,
            });
        }
        out.events = Some(EventsField { kinds });
    }
}

/// The validator's check of the events (E5): every occurrence is in a live room, within the count, with the gap, the zone and the
/// exclusions its kind asks, and no room holds two events when they are exclusive.
pub fn check(d: &Floor, field: &EventsField) -> Vec<String> {
    let spec = &d.params.environment.events;
    let mut out = Vec::new();
    if field.kinds.len() != spec.list.len() {
        out.push("E5 events: there is not one entry for each kind".to_string());
        return out;
    }
    let doors = doors(d);
    let mut seen: Vec<usize> = Vec::new();
    for (kind, event) in field.kinds.iter().zip(&spec.list) {
        let tag = format!("E5 events: {}", kind.name);
        if kind.nodes.len() > event.count {
            out.push(format!("{tag} has more occurrences than asked"));
        }
        for (k, &i) in kind.nodes.iter().enumerate() {
            let live = matches!(d.nodes.get(i), Some(Node::Room { room }) if d.rooms[*room].alive);
            if !live {
                out.push(format!(
                    "{tag} stands at node {i}, which is not a live room"
                ));
                continue;
            }
            if kind.nodes[..k].contains(&i) {
                out.push(format!("{tag} happens twice at node {i}"));
            }
            if event.avoid_entrance && doors.contains(&i) {
                out.push(format!("{tag} stands on the entrance or the exit"));
            }
            if event.zone.is_some_and(|z| zone_of_node(d, i) != Some(z)) {
                out.push(format!("{tag} stands outside its zone"));
            }
            if spec.exclusive && seen.contains(&i) {
                out.push(format!("{tag} shares node {i} with another kind"));
            }
            if event.min_gap > 0 {
                let hops = hops_from(d, i);
                if kind.nodes[..k].iter().any(|&j| hops[j] <= event.min_gap) {
                    out.push(format!(
                        "{tag} is closer than {} corridors to another",
                        event.min_gap + 1
                    ));
                }
            }
        }
        seen.extend(&kind.nodes);
    }
    out
}
