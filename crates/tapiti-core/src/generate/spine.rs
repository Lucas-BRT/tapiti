//! Plan a main route, returning wings and dead ends before ordinary growth fills the remaining space.

use super::*;

pub(super) struct SpinePlan {
    hops: usize,
    branches: usize,
    depth: usize,
    detours: usize,
    completed: usize,
    root: usize,
    wing: Vec<usize>,
    /// Hub and wings: the room count of each wing after the first (the first is the main route), all rooted at the hub.
    wings: Option<Vec<usize>>,
}

impl SpinePlan {
    pub(super) fn new(p: &Params) -> Self {
        let req = p.topology_requirements();
        let hops = p.exit_min_hops.max(1) as usize;
        let wings =
            (p.topology.profile == TopologyProfile::HubAndWings).then(|| p.hub_wing_depths());
        Self {
            hops,
            branches: wings.as_ref().map_or(req.min_branches, |w| w.len()),
            depth: req.min_branch_depth.max(1),
            detours: if wings.is_some() { 0 } else { req.min_detours },
            wings,
            completed: 0,
            root: 0,
            wing: Vec::new(),
        }
    }

    pub(super) fn minimum_rooms(&self) -> usize {
        match &self.wings {
            Some(wings) => self.hops + 1 + wings.iter().sum::<usize>(),
            None => self.hops + 1 + self.detours + self.branches.saturating_mul(self.depth),
        }
    }

    pub(super) fn finished(&self, g: &Gen<'_>) -> bool {
        g.spine.len() > self.hops && self.completed >= self.branches + self.detours
    }

    pub(super) fn parent(&mut self, g: &mut Gen<'_>, open: &[usize]) -> Option<usize> {
        if g.spine.len() <= self.hops {
            return g.spine.last().copied().filter(|r| open.contains(r));
        }
        if let Some(&tip) = self.wing.last() {
            return open.contains(&tip).then_some(tip);
        }
        if self.wings.is_some() {
            // Every wing starts at the hub, through a door of its own.
            let hub = 0;
            let free = open.contains(&hub) && g.rooms[hub].ports.iter().any(|q| g.port_free(q));
            self.root = hub;
            return free.then_some(hub);
        }
        let roots: Vec<_> = g
            .spine
            .iter()
            .copied()
            .filter(|r| open.contains(r) && !g.locked_full(*r))
            .filter(|r| g.rooms[*r].ports.iter().any(|q| g.port_free(q)))
            .collect();
        let root = *roots.get((g.rng.next() * roots.len() as f64).floor() as usize)?;
        self.root = root;
        Some(root)
    }

    pub(super) fn needs_return(&self, g: &Gen<'_>) -> bool {
        g.spine.len() > self.hops && self.completed < self.detours
    }

    /// Both corridors must fit together before the returning room is accepted.
    pub(super) fn return_link(&self, g: &Gen<'_>, incoming: &Candidate) -> Option<Candidate> {
        let root_pos = g.spine.iter().position(|r| *r == self.root)?;
        let mut options = Vec::new();
        for (pos, &room) in g.spine.iter().enumerate() {
            if room == self.root || g.locked_full(room) || pos.abs_diff(root_pos) > 2 {
                continue;
            }
            for c in candidates_for_indexed(g.p, &g.rooms, incoming.b, room, 4, &g.room_grid) {
                if g.fits_now(&c)
                    && !region_within(&c.region, &incoming.region, g.p.clear)
                    && !shape_within(
                        &exit_band(c.ports[0].p, c.ports[0].k, g.p, 0.0),
                        &exit_band(incoming.ports[1].p, incoming.ports[1].k, g.p, 0.0),
                        g.p.clear,
                    )
                {
                    options.push(c);
                }
            }
        }
        options.into_iter().min_by(|a, b| a.cost.total_cmp(&b.cost))
    }

    pub(super) fn placed(&mut self, g: &mut Gen<'_>, id: usize) {
        if g.spine.len() <= self.hops {
            g.spine.push(id);
            if g.spine.len() > self.hops {
                g.exit_room = id;
            }
        } else {
            self.wing.push(id);
            g.reserved_rooms.insert(id);
            let length = match &self.wings {
                Some(wings) => wings[self.completed],
                None if self.completed < self.detours => 1,
                None => self.depth,
            };
            if self.wing.len() >= length {
                self.completed += 1;
                self.wing.clear();
            }
        }
    }
}
