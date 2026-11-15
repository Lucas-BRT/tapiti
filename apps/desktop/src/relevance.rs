//! Which parameters can change the floor under the current intent and settings: the others are not listed.

use tapiti_core::params::{Params, TopologyProfile};

/// Whether the parameter at `path` can affect what the generator draws with `p`. Search still lists every parameter.
pub fn applies(path: &str, p: &Params) -> bool {
    let profile = p.topology.profile;
    match path {
        // Without an entrance there is no exit; without an exit nothing counts its doors or its distance.
        "exitRoom" if !p.entrance => false,
        "exitDoors" | "exitMinHops" if !p.entrance || !p.exit_room => false,
        // Every intent but Classic builds its structure from the settings of its own card, not from the loose minimums.
        _ if path.starts_with("topology.") && path != "topology.profile" => {
            profile == TopologyProfile::Classic
        }
        // A binary partition places every room itself: only how many leaves it cuts is a setting.
        "growth.start" | "growth.islands" | "growth.branch" | "growth.depth" | "growth.cluster"
        | "growth.reach"
            if p.layout == tapiti_core::params::Layout::Bsp =>
        {
            false
        }
        // Zones joined by single corridors start in the first strip, and the zones are the clumps.
        "growth.start" | "growth.islands" if p.bottleneck_on() => false,
        "zones.bottleneck" => p.zone_count() > 1 && p.layout.places_freely(),
        // The planned intents place the first room and the one island themselves.
        "growth.start" | "growth.islands" => {
            !matches!(
                profile,
                TopologyProfile::Ring | TopologyProfile::HubAndWings
            ) && !(profile == TopologyProfile::SpineWithDetours && path == "growth.islands")
        }
        // A ring and a hub with wings keep their own structure: no extra cycles are added, so the density does nothing.
        "delta" => !matches!(
            profile,
            TopologyProfile::Ring | TopologyProfile::HubAndWings
        ),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::params::{params_of_size, Size};

    fn with(profile: TopologyProfile) -> Params {
        let mut p = params_of_size(Size::Medium);
        p.apply_intent(profile);
        p
    }

    #[test]
    fn a_planned_intent_hides_what_it_overrides() {
        for profile in [TopologyProfile::Ring, TopologyProfile::HubAndWings] {
            let p = with(profile);
            for path in [
                "delta",
                "growth.start",
                "growth.islands",
                "topology.minBranches",
            ] {
                assert!(!applies(path, &p), "{profile:?} {path}");
            }
            assert!(applies("growth.depth", &p));
            assert!(applies("exitMinHops", &p));
        }
        assert!(applies("delta", &with(TopologyProfile::SpineWithDetours)));
        assert!(!applies(
            "growth.islands",
            &with(TopologyProfile::SpineWithDetours)
        ));
    }

    #[test]
    fn bsp_and_zones_hide_the_growth_settings_they_replace() {
        let mut bsp = params_of_size(Size::Medium);
        bsp.layout = tapiti_core::params::Layout::Bsp;
        for path in [
            "growth.start",
            "growth.islands",
            "growth.branch",
            "growth.depth",
            "growth.cluster",
            "growth.reach",
        ] {
            assert!(!applies(path, &bsp), "{path}");
        }
        assert!(applies("growth.rooms", &bsp) && applies("delta", &bsp));
        let mut zoned = params_of_size(Size::Large);
        zoned.set_zones(3, true);
        zoned.layout = tapiti_core::params::Layout::Growth;
        assert!(!applies("growth.start", &zoned) && !applies("growth.islands", &zoned));
        assert!(applies("zones.bottleneck", &zoned) && applies("growth.branch", &zoned));
        zoned.zones.count = 0;
        assert!(!applies("zones.bottleneck", &zoned));
    }

    #[test]
    fn classic_lists_everything_and_exit_settings_follow_the_exit() {
        let mut p = params_of_size(Size::Medium);
        for path in [
            "delta",
            "growth.start",
            "growth.islands",
            "topology.minBranches",
            "exitDoors",
        ] {
            assert!(applies(path, &p), "{path}");
        }
        p.exit_room = false;
        assert!(!applies("exitDoors", &p) && !applies("exitMinHops", &p));
        p.entrance = false;
        assert!(!applies("exitRoom", &p));
    }
}
