//! What one faction can see. Clients only ever receive these types, so the
//! server never leaks rivals' secrets (decision 13: exact influence only
//! where you have holdings or agents, estimates elsewhere).

use crate::balance::DAYS_PER_MONTH;
use crate::model::*;
use crate::orders::{Action, GameDate};
use crate::rng::hash_signed_unit;
use crate::{control_state, Sim};

/// How far an estimate can stray from the truth (±35%).
pub const ESTIMATE_ERROR: f64 = 0.35;

/// The static map, sent once when a client joins.
#[derive(Clone, Debug, PartialEq)]
pub struct GalaxyInfo {
    pub regions: Vec<RegionInfo>,
    pub systems: Vec<SystemInfo>,
    pub planets: Vec<PlanetInfo>,
    pub factions: Vec<FactionInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegionInfo {
    pub id: RegionId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemInfo {
    pub id: SystemId,
    pub name: String,
    pub pos: (f64, f64),
    pub region: RegionId,
    pub planets: Vec<PlanetId>,
    pub lanes: Vec<SystemId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetInfo {
    pub id: PlanetId,
    pub name: String,
    pub system: SystemId,
    pub kind: PlanetKind,
    pub race: Race,
    pub population: f64,
    pub slots: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FactionInfo {
    pub id: FactionId,
    pub name: String,
    pub power: Power,
    /// Power bases are common knowledge.
    pub home: PlanetId,
}

impl GalaxyInfo {
    pub fn from_sim(sim: &Sim) -> Self {
        Self {
            regions: sim
                .galaxy
                .regions
                .iter()
                .map(|r| RegionInfo {
                    id: r.id,
                    name: r.name.clone(),
                })
                .collect(),
            systems: sim
                .galaxy
                .systems
                .iter()
                .map(|s| SystemInfo {
                    id: s.id,
                    name: s.name.clone(),
                    pos: s.pos,
                    region: s.region,
                    planets: s.planets.clone(),
                    lanes: s.lanes.clone(),
                })
                .collect(),
            planets: sim
                .galaxy
                .planets
                .iter()
                .map(|p| PlanetInfo {
                    id: p.id,
                    name: p.name.clone(),
                    system: p.system,
                    kind: p.kind,
                    race: p.race,
                    population: p.population,
                    slots: p.slots,
                })
                .collect(),
            factions: sim
                .factions
                .iter()
                .map(|f| FactionInfo {
                    id: f.id,
                    name: f.name.clone(),
                    power: f.power,
                    home: f.home,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intel {
    /// You have holdings or an agent here: numbers are exact.
    Exact,
    /// Rumour and estimates: numbers may be off by up to ±35%.
    Estimate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildingView {
    pub id: BuildingId,
    pub owner: FactionId,
    pub kind: BuildingKind,
    /// Headquarters can't be sabotaged.
    pub headquarters: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetView {
    pub id: PlanetId,
    pub intel: Intel,
    pub used_slots: u8,
    /// Every building, when intel is exact; empty otherwise.
    pub buildings: Vec<BuildingView>,
    /// Influence per faction (exact or estimated, see `intel`).
    pub influence: Vec<f64>,
    /// Control as it appears from these numbers.
    pub control: ControlState,
    /// Whether you could order an outpost here right now.
    pub outpost_reach: bool,
    /// Days your orders take to arrive here.
    pub delay_days: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderView {
    pub id: u64,
    pub action: Action,
    pub arrives_day: u32,
    pub via_relay: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RivalView {
    pub id: FactionId,
    /// Publicly known: this faction is drawing government attention.
    pub exposed: bool,
}

/// One faction's picture of the galaxy on one day.
#[derive(Clone, Debug, PartialEq)]
pub struct FactionView {
    pub day: u32,
    pub date: String,
    pub me: FactionId,
    pub funds: f64,
    pub income_per_day: f64,
    pub heat: f64,
    pub relay_systems: Vec<SystemId>,
    pub agents: Vec<(PlanetId, u32)>,
    pub rivals: Vec<RivalView>,
    pub planets: Vec<PlanetView>,
    pub orders: Vec<OrderView>,
    pub system_leaders: Vec<Option<FactionId>>,
    pub region_leaders: Vec<Option<FactionId>>,
}

impl FactionView {
    pub fn build(sim: &Sim, me: FactionId) -> Self {
        let fac = sim.faction(me);
        let month = (sim.day / DAYS_PER_MONTH) as u64;
        let presence = sim.presence_systems(me);
        let near = sim.galaxy.hops_from(&presence);
        let mut relay_sources = sim.relay_systems(me);
        if relay_sources.is_empty() {
            relay_sources.push(sim.galaxy.planet(fac.home).system);
        }
        let comm = sim.galaxy.hops_from(&relay_sources);

        let planets = sim
            .galaxy
            .planets
            .iter()
            .map(|p| {
                let exact = p.has_presence(me) || sim.has_agent(me, p.id);
                let influence: Vec<f64> = if exact {
                    p.influence.clone()
                } else {
                    p.influence
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| {
                            let n =
                                hash_signed_unit(&[me.0 as u64, p.id.0 as u64, i as u64, month]);
                            (v * (1.0 + ESTIMATE_ERROR * n)).round().max(0.0)
                        })
                        .collect()
                };
                let control = if exact {
                    sim.control[p.id.idx()]
                } else {
                    control_state(&influence)
                };
                let buildings = if exact {
                    p.buildings
                        .iter()
                        .map(|b| BuildingView {
                            id: b.id,
                            owner: b.owner,
                            kind: b.kind,
                            headquarters: sim.is_headquarters(p, b),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let s = p.system.idx();
                PlanetView {
                    id: p.id,
                    intel: if exact { Intel::Exact } else { Intel::Estimate },
                    used_slots: p.buildings.len() as u8,
                    buildings,
                    influence,
                    control,
                    outpost_reach: near[s] <= 1,
                    delay_days: comm[s].saturating_mul(crate::balance::COURIER_DAYS_PER_HOP),
                }
            })
            .collect();

        Self {
            day: sim.day,
            date: GameDate::from_day(sim.day).to_string(),
            me,
            funds: fac.funds,
            income_per_day: sim.net_income(me),
            heat: fac.heat,
            relay_systems: sim.relay_systems(me),
            agents: fac.agents.clone(),
            rivals: sim
                .factions
                .iter()
                .filter(|f| f.id != me)
                .map(|f| RivalView {
                    id: f.id,
                    exposed: f.exposed,
                })
                .collect(),
            planets,
            orders: sim
                .orders
                .iter()
                .filter(|o| o.faction == me)
                .map(|o| OrderView {
                    id: o.id,
                    action: o.action,
                    arrives_day: o.arrives,
                    via_relay: o.via_relay,
                })
                .collect(),
            system_leaders: sim.system_leaders.clone(),
            region_leaders: sim.region_leaders.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimConfig;

    #[test]
    fn rivals_home_is_estimated_own_home_is_exact() {
        let mut sim = Sim::new(SimConfig::default());
        sim.run_days(60);
        let me = FactionId(0);
        let view = FactionView::build(&sim, me);
        let home = sim.faction(me).home;
        assert_eq!(view.planets[home.idx()].intel, Intel::Exact);
        assert_eq!(
            view.planets[home.idx()].influence,
            sim.galaxy.planet(home).influence
        );
        let rival_home = sim.faction(FactionId(4)).home;
        if !sim.galaxy.planet(rival_home).has_presence(me) && !sim.has_agent(me, rival_home) {
            let pv = &view.planets[rival_home.idx()];
            assert_eq!(pv.intel, Intel::Estimate);
            assert!(pv.buildings.is_empty());
            let truth = sim.galaxy.planet(rival_home).influence[4];
            let shown = pv.influence[4];
            assert!((shown - truth).abs() <= truth * ESTIMATE_ERROR + 1.0);
        }
    }
}
