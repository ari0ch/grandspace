//! Whole-simulation tests: determinism and long-run sanity.

use grandspace_sim::*;

#[test]
fn same_seed_same_history() {
    let cfg = SimConfig {
        seed: 42,
        ..SimConfig::default()
    };
    let mut a = Sim::new(cfg.clone());
    let mut b = Sim::new(cfg);
    for _ in 0..5 {
        a.run_days(360);
        b.run_days(360);
        assert_eq!(a.fingerprint(), b.fingerprint());
    }
}

#[test]
fn different_seeds_diverge() {
    let mut a = Sim::new(SimConfig {
        seed: 1,
        ..SimConfig::default()
    });
    let mut b = Sim::new(SimConfig {
        seed: 2,
        ..SimConfig::default()
    });
    a.run_days(100);
    b.run_days(100);
    assert_ne!(a.fingerprint(), b.fingerprint());
}

#[test]
fn fifty_years_stay_sane() {
    let mut sim = Sim::new(SimConfig {
        seed: 7,
        ..SimConfig::default()
    });
    sim.run_days(50 * 360);
    for p in &sim.galaxy.planets {
        assert!(
            p.buildings.len() <= p.slots as usize,
            "{} over slots",
            p.name
        );
        assert!(p.influence.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
    for f in &sim.factions {
        assert!(f.funds.is_finite());
        assert!((0.0..=100.0).contains(&f.heat));
        // Headquarters survive everything.
        assert!(
            sim.galaxy.planet(f.home).has_presence(f.id),
            "{} lost HQ",
            f.name
        );
    }
    // The AI actually played.
    assert!(sim.stats.buildings_built > 50);
}

#[test]
fn orders_travel_by_courier_outside_relay_network() {
    let mut sim = Sim::new(SimConfig::default());
    let f = FactionId(0);
    let home_system = sim.galaxy.planet(sim.faction(f).home).system;
    // Find a planet one hop away.
    let neighbour = sim.galaxy.system(home_system).lanes[0];
    let target = sim.galaxy.system(neighbour).planets[0];
    sim.set_human(f, true);
    let free = sim.galaxy.planet(target).free_slots() > 0;
    if !free {
        return;
    }
    sim.issue(
        f,
        Action::Build {
            planet: target,
            kind: BuildingKind::Outpost,
        },
    )
    .expect("outpost in reach");
    let order = sim.orders.iter().find(|o| o.faction == f).unwrap();
    assert!(!order.via_relay);
    assert_eq!(order.arrives, sim.day + balance::COURIER_DAYS_PER_HOP);
}

#[test]
fn orders_inside_relay_network_arrive_next_day() {
    let mut sim = Sim::new(SimConfig::default());
    let f = FactionId(1);
    sim.set_human(f, true);
    let home = sim.faction(f).home;
    sim.issue(f, Action::Campaign { planet: home }).unwrap();
    let order = sim.orders.iter().find(|o| o.faction == f).unwrap();
    assert!(order.via_relay);
    assert_eq!(order.arrives, sim.day + 1);
    let before = sim.galaxy.planet(home).influence[1];
    sim.tick();
    assert!(sim.galaxy.planet(home).influence[1] > before + 5.0);
}

#[test]
fn cannot_sabotage_headquarters_or_own_buildings() {
    let sim = Sim::new(SimConfig::default());
    let f = FactionId(0);
    let home = sim.faction(f).home;
    let own = sim.galaxy.planet(home).buildings[0].id;
    assert_eq!(
        sim.check(
            f,
            Action::Sabotage {
                planet: home,
                building: own
            }
        ),
        Err(OrderError::InvalidTarget)
    );
}
