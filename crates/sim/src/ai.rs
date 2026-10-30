//! Simple utility AI for factions without a human at the helm.
//!
//! Every few days each AI faction scores the orders open to it and issues
//! the best one it can afford. Scores lean on each power's style: the Mafia
//! likes money and sabotage, the Church builds chapels and preaches, the
//! Unions spread wide and hire. Rivals gang up on factions that are exposed
//! (decision 14).

use crate::balance::*;
use crate::model::*;
use crate::orders::Action;
use crate::rng::hash_signed_unit;
use crate::Sim;

struct Style {
    expand: f64,
    hall: f64,
    income: f64,
    campaign: f64,
    sabotage: f64,
    /// Heat above which this power stops doing risky things.
    caution: f64,
}

fn style(power: Power) -> Style {
    match power {
        Power::Mafia => Style {
            expand: 1.0,
            hall: 0.9,
            income: 1.3,
            campaign: 0.9,
            sabotage: 2.0,
            caution: 55.0,
        },
        Power::Church => Style {
            expand: 0.9,
            hall: 1.3,
            income: 0.9,
            campaign: 1.2,
            sabotage: 0.4,
            caution: 40.0,
        },
        Power::Unions => Style {
            expand: 1.2,
            hall: 1.0,
            income: 1.2,
            campaign: 1.0,
            sabotage: 0.7,
            caution: 45.0,
        },
    }
}

/// Pick the order this faction should issue now, if any.
pub fn decide(sim: &Sim, f: FactionId) -> Option<Action> {
    let fac = sim.faction(f);
    let budget = fac.funds - AI_RESERVE;
    if budget <= 0.0 {
        return None;
    }
    let power = fac.power;
    let st = style(power);
    let fi = f.idx();
    let presence = sim.presence_systems(f);
    let hops = sim.galaxy.hops_from(&presence);
    let relays = sim.relay_systems(f);
    let exposed: Vec<bool> = sim.factions.iter().map(|x| x.exposed).collect();
    let cautious = fac.heat >= st.caution;
    let sabotage_in_flight = sim
        .orders
        .iter()
        .any(|o| o.faction == f && matches!(o.action, Action::Sabotage { .. }));
    let net = sim.net_income(f);
    // Only sprawl when the economy can carry more upkeep.
    let economy = (net / 4.0).clamp(0.25, 1.0);
    // Don't grow past the size where the network itself draws scrutiny.
    let buildings: usize = sim
        .galaxy
        .planets
        .iter()
        .map(|p| p.buildings.iter().filter(|b| b.owner == f).count())
        .sum();
    let room_to_grow = structural_heat(buildings + 1) < st.caution;
    let lie_low = if room_to_grow { 1.0 } else { 0.0 };

    let mut candidates: Vec<(f64, Action)> = Vec::new();
    for p in &sim.galaxy.planets {
        let h = hops[p.system.idx()];
        if h > 2 {
            continue;
        }
        if sim
            .orders
            .iter()
            .any(|o| o.faction == f && o.action.planet() == p.id)
        {
            continue;
        }
        let noise = 1.0 + 0.25 * hash_signed_unit(&[sim.day as u64, fi as u64, p.id.0 as u64]);
        let mine = p.influence[fi];
        let others = p.total_influence() - mine;
        let affinity = p.race.affinity(power);
        let control = sim.control[p.id.idx()];
        let present = p.has_presence(f);

        if !present {
            if h <= 1 && p.free_slots() > 0 {
                let situation = match control {
                    ControlState::Unclaimed => 1.0,
                    ControlState::Contested if power == Power::Mafia => 1.6,
                    ControlState::Contested => 1.2,
                    ControlState::Controlled(o) if exposed[o.idx()] => 1.1,
                    ControlState::Controlled(_) => 0.6,
                };
                let score = (1.0 + p.population / 50.0)
                    * affinity
                    * situation
                    * st.expand
                    * economy
                    * lie_low
                    / (1.0 + h as f64 * 0.6);
                candidates.push((
                    score * noise,
                    Action::Build {
                        planet: p.id,
                        kind: BuildingKind::Outpost,
                    },
                ));
            }
            if h <= 2 && !sim.has_agent(f, p.id) && others > 20.0 {
                candidates.push((0.25 * noise, Action::Agent { planet: p.id }));
            }
            continue;
        }

        let secure =
            matches!(control, ControlState::Controlled(c) if c == f) && mine > 2.0 * others;
        if p.free_slots() > 0 && !sim.at_holding_limit(f, p) {
            let hall_need = if secure { 0.35 } else { 1.5 };
            let score = affinity * (1.0 + p.population / 80.0) * hall_need * st.hall * lie_low;
            candidates.push((
                score * noise,
                Action::Build {
                    planet: p.id,
                    kind: BuildingKind::Hall,
                },
            ));

            if p.count_owned(f, BuildingKind::Income) == 0 {
                let mut inc = income_yield(power, p.population);
                if control == ControlState::Contested && power == Power::Mafia {
                    inc *= MAFIA_CONTESTED_INCOME_MULT;
                }
                let need_money = if net < 2.0 {
                    3.0
                } else if fac.funds < 200.0 {
                    1.8
                } else {
                    0.8
                };
                candidates.push((
                    inc * need_money * st.income * lie_low * noise,
                    Action::Build {
                        planet: p.id,
                        kind: BuildingKind::Income,
                    },
                ));
            }

            let mine_here = p.buildings.iter().filter(|b| b.owner == f).count();
            let home_system = sim.galaxy.planet(fac.home).system;
            let relay_score = if p.id == fac.home && relays.is_empty() {
                // Rebuild the network's anchor first.
                3.0
            } else if !relays.contains(&p.system)
                && sim.delivery_delay(f, p.system) >= 2 * COURIER_DAYS_PER_HOP
                && mine_here >= 2
                && p.system != home_system
            {
                1.6
            } else {
                0.0
            };
            if relay_score > 0.0 && !sim.has_relay_in_system(f, p.system) {
                candidates.push((
                    relay_score * noise,
                    Action::Build {
                        planet: p.id,
                        kind: BuildingKind::Relay,
                    },
                ));
            }
        }

        // Push for control where it is within reach.
        if !secure {
            let need = (others - mine).max(0.0) + 1.0;
            let push = CAMPAIGN_INFLUENCE * affinity;
            let reachable = if need <= push * 1.5 { 1.4 } else { 0.4 };
            let caution = if cautious { 0.5 } else { 1.0 };
            candidates.push((
                reachable * st.campaign * caution * noise,
                Action::Campaign { planet: p.id },
            ));
        }

        // Sabotage the strongest rival here, preferring exposed ones. One
        // covert operation at a time.
        if !cautious && !sabotage_in_flight {
            let target = p
                .buildings
                .iter()
                .filter(|b| b.owner != f && !sim.is_headquarters(p, b))
                .max_by(|a, b| {
                    let sa =
                        p.influence[a.owner.idx()] * if exposed[a.owner.idx()] { 2.0 } else { 1.0 };
                    let sb =
                        p.influence[b.owner.idx()] * if exposed[b.owner.idx()] { 2.0 } else { 1.0 };
                    sa.total_cmp(&sb).then(b.id.cmp(&a.id))
                });
            if let Some(t) = target {
                let rival_controls = control == ControlState::Controlled(t.owner);
                let mut score = 0.5 * st.sabotage;
                if rival_controls {
                    score *= 1.6;
                }
                if exposed[t.owner.idx()] {
                    score *= 1.8;
                }
                if t.kind == BuildingKind::Hall || t.kind == BuildingKind::Relay {
                    score *= 1.2;
                }
                candidates.push((
                    score * noise,
                    Action::Sabotage {
                        planet: p.id,
                        building: t.id,
                    },
                ));
            }
        }
    }

    // Pay off officials when the heat is on and there is money to spare.
    if fac.heat >= st.caution - 5.0 {
        let score = 1.5 + (fac.heat - st.caution).max(0.0) / 5.0;
        candidates.push((score, Action::Bribe { planet: fac.home }));
    }
    candidates.retain(|c| c.0 > 0.0);
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    // Take the best valid option. If it is too expensive right now, save up
    // for it rather than frittering money on cheaper, worse options.
    for (_, action) in candidates.into_iter().take(12) {
        match sim.check(f, action) {
            Ok(cost) if cost <= budget => return Some(action),
            Ok(_) | Err(crate::orders::OrderError::NotEnoughFunds) => return None,
            Err(_) => continue,
        }
    }
    None
}
