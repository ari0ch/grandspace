//! Procedural galaxy generation (decision 17: procedural map; handcrafted
//! key systems can be layered on later).

use crate::model::*;
use crate::names;
use crate::rng::Rng;

/// Map extents in abstract units.
pub const MAP_WIDTH: f64 = 1000.0;
pub const MAP_HEIGHT: f64 = 700.0;

#[derive(Clone, Debug)]
pub struct GalaxyConfig {
    pub systems: usize,
    pub regions: usize,
    pub min_planets: usize,
    pub max_planets: usize,
}

impl Default for GalaxyConfig {
    fn default() -> Self {
        Self {
            systems: 40,
            regions: 5,
            min_planets: 2,
            max_planets: 5,
        }
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

pub fn generate(rng: &mut Rng, cfg: &GalaxyConfig, factions: usize) -> Galaxy {
    assert!(cfg.systems >= 2, "need at least two systems");
    let regions_n = cfg.regions.clamp(1, cfg.systems);

    // Scatter systems with a minimum spacing (relaxed if the map is crowded).
    let mut positions: Vec<(f64, f64)> = Vec::with_capacity(cfg.systems);
    let area = MAP_WIDTH * MAP_HEIGHT;
    let mut min_gap = (area / cfg.systems as f64).sqrt() * 0.6;
    let margin = 40.0;
    while positions.len() < cfg.systems {
        let mut placed = false;
        for _ in 0..200 {
            let p = (
                rng.range_f64(margin, MAP_WIDTH - margin),
                rng.range_f64(margin, MAP_HEIGHT - margin),
            );
            if positions.iter().all(|&q| dist(p, q) >= min_gap) {
                positions.push(p);
                placed = true;
                break;
            }
        }
        if !placed {
            min_gap *= 0.9;
        }
    }

    // Lanes: a minimum spanning tree guarantees connectivity, then each
    // system also links to its nearest neighbours for alternative routes.
    let n = positions.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let add_lane = |a: usize, b: usize, adj: &mut Vec<Vec<usize>>| {
        if a != b && !adj[a].contains(&b) {
            adj[a].push(b);
            adj[b].push(a);
        }
    };
    let mut in_tree = vec![false; n];
    let mut best = vec![(f64::INFINITY, usize::MAX); n];
    in_tree[0] = true;
    for j in 1..n {
        best[j] = (dist(positions[0], positions[j]), 0);
    }
    for _ in 1..n {
        let mut next = usize::MAX;
        let mut next_d = f64::INFINITY;
        for j in 0..n {
            if !in_tree[j] && best[j].0 < next_d {
                next_d = best[j].0;
                next = j;
            }
        }
        in_tree[next] = true;
        add_lane(next, best[next].1, &mut adj);
        for j in 0..n {
            if !in_tree[j] {
                let d = dist(positions[next], positions[j]);
                if d < best[j].0 {
                    best[j] = (d, next);
                }
            }
        }
    }
    let lane_reach = (area / n as f64).sqrt() * 1.5;
    for i in 0..n {
        let mut near: Vec<(f64, usize)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (dist(positions[i], positions[j]), j))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(d, j) in near.iter().take(2) {
            if d <= lane_reach {
                add_lane(i, j, &mut adj);
            }
        }
    }

    // Regions: farthest-point seeds, then nearest-seed assignment.
    let mut seeds = vec![rng.range_usize(0, n)];
    while seeds.len() < regions_n {
        let far = (0..n)
            .max_by(|&a, &b| {
                let da = seeds
                    .iter()
                    .map(|&s| dist(positions[a], positions[s]))
                    .fold(f64::INFINITY, f64::min);
                let db = seeds
                    .iter()
                    .map(|&s| dist(positions[b], positions[s]))
                    .fold(f64::INFINITY, f64::min);
                da.total_cmp(&db)
            })
            .unwrap();
        seeds.push(far);
    }
    let regions: Vec<Region> = (0..regions_n)
        .map(|i| Region {
            id: RegionId(i as u32),
            name: names::region_name(rng),
        })
        .collect();

    // Each region leans towards one race, with mixing.
    let region_race: Vec<Race> = (0..regions_n)
        .map(|i| Race::ALL[(i + rng.range_usize(0, 3)) % 3])
        .collect();

    let mut systems = Vec::with_capacity(n);
    let mut planets: Vec<Planet> = Vec::new();
    for i in 0..n {
        let region = (0..regions_n)
            .min_by(|&a, &b| {
                dist(positions[i], positions[seeds[a]])
                    .total_cmp(&dist(positions[i], positions[seeds[b]]))
            })
            .unwrap();
        let name = names::system_name(rng);
        let count = rng.range_usize(cfg.min_planets, cfg.max_planets + 1);
        let mut planet_ids = Vec::with_capacity(count);
        for k in 0..count {
            let kind = *rng.pick(&PlanetKind::ALL).unwrap();
            let race = if rng.chance(0.65) {
                region_race[region]
            } else {
                *rng.pick(&Race::ALL).unwrap()
            };
            let (lo, hi) = kind.population_range();
            let id = PlanetId(planets.len() as u32);
            let pname = if kind == PlanetKind::Station {
                format!("{} Station {}", name, names::roman(k))
            } else {
                format!("{} {}", name, names::roman(k))
            };
            planets.push(Planet {
                id,
                name: pname,
                system: SystemId(i as u32),
                kind,
                race,
                population: rng.range_f64(lo, hi).round(),
                slots: kind.slots(),
                buildings: Vec::new(),
                influence: vec![0.0; factions],
            });
            planet_ids.push(id);
        }
        let mut lanes: Vec<SystemId> = adj[i].iter().map(|&j| SystemId(j as u32)).collect();
        lanes.sort();
        systems.push(StarSystem {
            id: SystemId(i as u32),
            name,
            pos: positions[i],
            region: RegionId(region as u32),
            planets: planet_ids,
            lanes,
        });
    }

    Galaxy {
        regions,
        systems,
        planets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galaxy_is_connected() {
        for seed in 0..20 {
            let mut rng = Rng::new(seed);
            let g = generate(&mut rng, &GalaxyConfig::default(), 9);
            let hops = g.hops_from(&[SystemId(0)]);
            assert!(hops.iter().all(|&h| h != u32::MAX), "seed {seed}");
        }
    }

    #[test]
    fn every_system_has_planets_and_region() {
        let mut rng = Rng::new(3);
        let cfg = GalaxyConfig::default();
        let g = generate(&mut rng, &cfg, 9);
        assert_eq!(g.systems.len(), cfg.systems);
        for s in &g.systems {
            assert!(s.planets.len() >= cfg.min_planets);
            assert!(s.region.idx() < g.regions.len());
        }
    }
}
