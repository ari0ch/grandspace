//! Grandspace simulation.
//!
//! A deterministic, headless model of a galaxy where three hidden powers
//! (the Mafia, the Church and the Unions) compete for influence over
//! planets they never officially rule. One call to [`Sim::tick`] advances
//! the galaxy by one day.
//!
//! The crate has no dependencies. Servers, local single-player hosts and the
//! headless runner all drive the same [`Sim`]; clients only ever see a
//! per-faction [`view::FactionView`], which hides what that faction cannot
//! know.

pub mod ai;
pub mod balance;
pub mod galaxy;
pub mod model;
pub mod names;
pub mod orders;
pub mod rng;
pub mod view;

use std::collections::VecDeque;

use balance::*;
pub use galaxy::GalaxyConfig;
pub use model::*;
pub use orders::*;
use rng::Rng;

/// How many recent events the simulation keeps in memory.
pub const EVENT_BUFFER: usize = 10_000;

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub seed: u64,
    pub galaxy: GalaxyConfig,
    /// Factions per power; 2 to 4 is the intended range (decision 7).
    pub factions_per_power: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            galaxy: GalaxyConfig::default(),
            factions_per_power: 3,
        }
    }
}

/// Running totals for the headless history report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub buildings_built: u64,
    pub sabotages: u64,
    pub sabotages_failed: u64,
    pub crackdowns: u64,
    pub interceptions: u64,
    pub control_changes: u64,
    pub abandoned: u64,
}

#[derive(Clone, Debug)]
pub struct Sim {
    pub config: SimConfig,
    /// Days elapsed since the start. One tick is one day.
    pub day: u32,
    pub galaxy: Galaxy,
    pub factions: Vec<Faction>,
    pub orders: Vec<PendingOrder>,
    /// Control state per planet, indexed by `PlanetId`.
    pub control: Vec<ControlState>,
    pub system_leaders: Vec<Option<FactionId>>,
    pub region_leaders: Vec<Option<FactionId>>,
    pub events: VecDeque<Event>,
    pub stats: Stats,
    rng: Rng,
    next_order: u64,
    next_building: u32,
    next_event: u64,
}

impl Sim {
    pub fn new(config: SimConfig) -> Self {
        let mut rng = Rng::new(config.seed);
        let per_power = config.factions_per_power.max(1);
        let faction_count = per_power * Power::ALL.len();
        let galaxy = galaxy::generate(&mut rng, &config.galaxy, faction_count);

        // Interleave powers so neighbouring power bases tend to differ.
        let powers: Vec<Power> = (0..faction_count).map(|i| Power::ALL[i % 3]).collect();
        let homes = pick_homes(&galaxy, faction_count, &mut rng);

        let factions: Vec<Faction> = powers
            .iter()
            .zip(&homes)
            .enumerate()
            .map(|(i, (&power, &home))| Faction {
                id: FactionId(i as u32),
                name: names::faction_name(&mut rng, power),
                power,
                home,
                funds: STARTING_FUNDS,
                heat: 0.0,
                exposed: false,
                agents: Vec::new(),
                human: false,
            })
            .collect();

        let planet_count = galaxy.planets.len();
        let mut sim = Sim {
            day: 0,
            control: vec![ControlState::Unclaimed; planet_count],
            system_leaders: vec![None; galaxy.systems.len()],
            region_leaders: vec![None; galaxy.regions.len()],
            galaxy,
            factions,
            orders: Vec::new(),
            events: VecDeque::new(),
            stats: Stats::default(),
            rng,
            next_order: 1,
            next_building: 1,
            next_event: 1,
            config,
        };

        // Power bases start with a full set of buildings and a head start.
        for i in 0..sim.factions.len() {
            let f = FactionId(i as u32);
            let home = sim.factions[i].home;
            for kind in BuildingKind::ALL {
                sim.place_building(f, home, kind);
            }
            sim.galaxy.planet_mut(home).influence[i] = STARTING_HOME_INFLUENCE;
        }
        sim.refresh_control(false);
        sim.refresh_leaders(false);
        sim
    }

    pub fn date(&self) -> GameDate {
        GameDate::from_day(self.day)
    }

    pub fn faction(&self, f: FactionId) -> &Faction {
        &self.factions[f.idx()]
    }

    pub fn set_human(&mut self, f: FactionId, human: bool) {
        if let Some(fac) = self.factions.get_mut(f.idx()) {
            fac.human = human;
        }
    }

    /// Advance the galaxy by one day.
    pub fn tick(&mut self) {
        self.day += 1;
        self.deliver_orders();
        self.collect_income();
        self.spread_influence();
        self.refresh_control(true);
        self.update_heat();
        self.expire_agents();
        if self.day % DAYS_PER_MONTH == 0 {
            self.refresh_leaders(true);
        }
        self.run_ai();
    }

    pub fn run_days(&mut self, days: u32) {
        for _ in 0..days {
            self.tick();
        }
    }

    // --- Orders --------------------------------------------------------

    /// Validate an order, pay for it and send it on its way.
    pub fn issue(&mut self, f: FactionId, action: Action) -> Result<u64, OrderError> {
        let cost = self.check(f, action)?;
        let target_system = self.galaxy.planet(action.planet()).system;
        let delay = self.delivery_delay(f, target_system);
        let fac = &mut self.factions[f.idx()];
        fac.funds -= cost;
        let id = self.next_order;
        self.next_order += 1;
        self.orders.push(PendingOrder {
            id,
            faction: f,
            action,
            issued: self.day,
            arrives: self.day + delay.max(1),
            via_relay: delay == 0,
            cost,
        });
        Ok(id)
    }

    /// Check an order without issuing it. Returns its cost.
    pub fn check(&self, f: FactionId, action: Action) -> Result<f64, OrderError> {
        let fac = self
            .factions
            .get(f.idx())
            .ok_or(OrderError::UnknownTarget)?;
        let planet = self
            .galaxy
            .planets
            .get(action.planet().idx())
            .ok_or(OrderError::UnknownTarget)?;
        let cost = match action {
            Action::Build { kind, .. } => {
                if planet.free_slots() == 0 {
                    return Err(OrderError::NoFreeSlot);
                }
                if kind == BuildingKind::Outpost {
                    if !self.in_outpost_reach(f, planet.system) {
                        return Err(OrderError::OutOfReach);
                    }
                } else if !planet.has_presence(f) {
                    return Err(OrderError::NoPresence);
                }
                if kind == BuildingKind::Relay && self.has_relay_in(f, planet.system) {
                    return Err(OrderError::RelayAlreadyInSystem);
                }
                if self.at_holding_limit(f, planet) {
                    return Err(OrderError::HoldingLimit);
                }
                building_cost(kind, fac.power)
            }
            Action::Campaign { .. } => {
                if !planet.has_presence(f) {
                    return Err(OrderError::NoPresence);
                }
                campaign_cost(fac.power)
            }
            Action::Sabotage { building, .. } => {
                let target = planet
                    .buildings
                    .iter()
                    .find(|b| b.id == building)
                    .ok_or(OrderError::UnknownTarget)?;
                if target.owner == f || self.is_headquarters(planet, target) {
                    return Err(OrderError::InvalidTarget);
                }
                if !planet.has_presence(f) && !self.has_agent(f, planet.id) {
                    return Err(OrderError::NoPresence);
                }
                sabotage_cost(fac.power)
            }
            Action::Bribe { .. } => {
                if !planet.has_presence(f) {
                    return Err(OrderError::NoPresence);
                }
                bribe_cost(fac.power)
            }
            Action::Agent { .. } => {
                let hops = self.galaxy.hops_from(&self.presence_systems(f));
                if hops[planet.system.idx()] > 2 {
                    return Err(OrderError::OutOfReach);
                }
                AGENT_COST
            }
        };
        if fac.funds < cost {
            return Err(OrderError::NotEnoughFunds);
        }
        Ok(cost)
    }

    /// Days for an order to reach a system: zero inside your relay network,
    /// courier hops outside it.
    pub fn delivery_delay(&self, f: FactionId, target: SystemId) -> u32 {
        let mut sources = self.relay_systems(f);
        if sources.is_empty() {
            sources.push(self.galaxy.planet(self.factions[f.idx()].home).system);
        }
        let hops = self.galaxy.hops_from(&sources)[target.idx()];
        if hops == u32::MAX {
            u32::MAX / 2
        } else {
            hops * COURIER_DAYS_PER_HOP
        }
    }

    pub fn relay_systems(&self, f: FactionId) -> Vec<SystemId> {
        let mut out: Vec<SystemId> = self
            .galaxy
            .planets
            .iter()
            .filter(|p| p.count_owned(f, BuildingKind::Relay) > 0)
            .map(|p| p.system)
            .collect();
        out.sort();
        out.dedup();
        out
    }

    pub fn presence_systems(&self, f: FactionId) -> Vec<SystemId> {
        let mut out: Vec<SystemId> = self
            .galaxy
            .planets
            .iter()
            .filter(|p| p.has_presence(f))
            .map(|p| p.system)
            .collect();
        out.push(self.galaxy.planet(self.factions[f.idx()].home).system);
        out.sort();
        out.dedup();
        out
    }

    /// Outposts can go in any system with or next to one of your holdings.
    pub fn in_outpost_reach(&self, f: FactionId, system: SystemId) -> bool {
        let presence = self.presence_systems(f);
        presence.contains(&system)
            || presence
                .iter()
                .any(|s| self.galaxy.system(*s).lanes.contains(&system))
    }

    pub fn has_relay_in_system(&self, f: FactionId, system: SystemId) -> bool {
        self.has_relay_in(f, system)
    }

    fn has_relay_in(&self, f: FactionId, system: SystemId) -> bool {
        self.galaxy
            .system(system)
            .planets
            .iter()
            .any(|&p| self.galaxy.planet(p).count_owned(f, BuildingKind::Relay) > 0)
    }

    /// Holdings are shared: outside its power base a faction may own only
    /// `MAX_BUILDINGS_PER_HOLDING` buildings per planet.
    pub fn at_holding_limit(&self, f: FactionId, planet: &Planet) -> bool {
        self.factions[f.idx()].home != planet.id
            && planet.buildings.iter().filter(|b| b.owner == f).count() >= MAX_BUILDINGS_PER_HOLDING
    }

    pub fn has_agent(&self, f: FactionId, p: PlanetId) -> bool {
        self.factions[f.idx()].agents.iter().any(|(ap, _)| *ap == p)
    }

    /// A faction's headquarters (its home outpost) can't be sabotaged or
    /// raided, so nobody is ever wiped off the map entirely.
    pub fn is_headquarters(&self, planet: &Planet, b: &Building) -> bool {
        b.kind == BuildingKind::Outpost && self.factions[b.owner.idx()].home == planet.id && {
            // Only the first outpost there counts as the HQ.
            planet
                .buildings
                .iter()
                .find(|x| x.owner == b.owner && x.kind == BuildingKind::Outpost)
                .map(|x| x.id)
                == Some(b.id)
        }
    }

    fn deliver_orders(&mut self) {
        let today = self.day;
        let mut due: Vec<PendingOrder> = Vec::new();
        self.orders.retain(|o| {
            if o.arrives <= today {
                due.push(o.clone());
                false
            } else {
                true
            }
        });
        due.sort_by_key(|o| o.id);
        for order in due {
            self.execute(order);
        }
    }

    fn execute(&mut self, order: PendingOrder) {
        let f = order.faction;
        let pid = order.action.planet();
        let system = self.galaxy.planet(pid).system;

        // Rival relays in the destination system may read the order.
        let mut foiled_by: Option<FactionId> = None;
        let mut rivals: Vec<FactionId> = self
            .galaxy
            .system(system)
            .planets
            .iter()
            .flat_map(|&p| self.galaxy.planet(p).buildings.iter())
            .filter(|b| b.kind == BuildingKind::Relay && b.owner != f)
            .map(|b| b.owner)
            .collect();
        rivals.sort();
        rivals.dedup();
        for r in rivals {
            if self.rng.chance(INTERCEPT_CHANCE) {
                self.stats.interceptions += 1;
                self.add_heat(f, HEAT_INTERCEPTED);
                self.push_event(EventKind::Intercepted {
                    interceptor: r,
                    sender: f,
                    planet: pid,
                    action: order.action,
                });
                if let Action::Sabotage { building, .. } = order.action {
                    let owner = self
                        .galaxy
                        .planet(pid)
                        .buildings
                        .iter()
                        .find(|b| b.id == building)
                        .map(|b| b.owner);
                    if owner == Some(r) {
                        foiled_by = Some(r);
                    }
                }
                break;
            }
        }

        let power = self.factions[f.idx()].power;
        match order.action {
            Action::Build { kind, .. } => {
                let p = self.galaxy.planet(pid);
                let reason = if p.free_slots() == 0 {
                    Some(OrderError::NoFreeSlot)
                } else if kind != BuildingKind::Outpost && !p.has_presence(f) {
                    Some(OrderError::NoPresence)
                } else if kind == BuildingKind::Relay && self.has_relay_in(f, system) {
                    Some(OrderError::RelayAlreadyInSystem)
                } else if self.at_holding_limit(f, p) {
                    Some(OrderError::HoldingLimit)
                } else {
                    None
                };
                match reason {
                    Some(reason) => self.fail_order(&order, reason),
                    None => {
                        self.place_building(f, pid, kind);
                        self.stats.buildings_built += 1;
                        self.push_event(EventKind::Built {
                            faction: f,
                            planet: pid,
                            kind,
                        });
                    }
                }
            }
            Action::Campaign { .. } => {
                let p = self.galaxy.planet(pid);
                if !p.has_presence(f) {
                    self.fail_order(&order, OrderError::NoPresence);
                    return;
                }
                let mut gain =
                    CAMPAIGN_INFLUENCE * campaign_strength(power) * p.race.affinity(power);
                if self.control[pid.idx()] == ControlState::Contested {
                    gain *= CONTESTED_ACTION_MULT;
                }
                self.galaxy.planet_mut(pid).influence[f.idx()] += gain;
                self.add_heat(f, HEAT_CAMPAIGN);
                self.push_event(EventKind::Campaign {
                    faction: f,
                    planet: pid,
                });
            }
            Action::Sabotage { building, .. } => {
                let target = self
                    .galaxy
                    .planet(pid)
                    .buildings
                    .iter()
                    .find(|b| b.id == building)
                    .cloned();
                let Some(target) = target else {
                    self.fail_order(&order, OrderError::UnknownTarget);
                    return;
                };
                let success = foiled_by.is_none() && self.rng.chance(sabotage_success(power));
                if success {
                    self.galaxy
                        .planet_mut(pid)
                        .buildings
                        .retain(|b| b.id != building);
                    self.stats.sabotages += 1;
                    self.add_heat(f, HEAT_SABOTAGE_SUCCESS);
                    self.push_event(EventKind::Destroyed {
                        by: Some(f),
                        victim: target.owner,
                        planet: pid,
                        kind: target.kind,
                    });
                } else {
                    self.stats.sabotages_failed += 1;
                    self.add_heat(f, HEAT_SABOTAGE_FAIL);
                    self.push_event(EventKind::SabotageFailed {
                        faction: f,
                        victim: target.owner,
                        planet: pid,
                        foiled: foiled_by.is_some(),
                    });
                }
            }
            Action::Bribe { .. } => {
                if !self.galaxy.planet(pid).has_presence(f) {
                    self.fail_order(&order, OrderError::NoPresence);
                    return;
                }
                let fac = &mut self.factions[f.idx()];
                fac.heat = (fac.heat - BRIBE_HEAT_RELIEF).max(0.0);
                self.refresh_exposure(f);
                self.push_event(EventKind::Bribed {
                    faction: f,
                    planet: pid,
                });
            }
            Action::Agent { .. } => {
                let expires = self.day + AGENT_DAYS;
                let fac = &mut self.factions[f.idx()];
                fac.agents.retain(|(p, _)| *p != pid);
                fac.agents.push((pid, expires));
                self.add_heat(f, HEAT_AGENT);
                self.push_event(EventKind::AgentPlaced {
                    faction: f,
                    planet: pid,
                });
            }
        }
    }

    fn fail_order(&mut self, order: &PendingOrder, reason: OrderError) {
        // Half the money comes back; the rest was spent getting there.
        self.factions[order.faction.idx()].funds += order.cost * 0.5;
        self.push_event(EventKind::OrderFailed {
            faction: order.faction,
            planet: order.action.planet(),
            reason,
        });
    }

    fn place_building(&mut self, f: FactionId, p: PlanetId, kind: BuildingKind) {
        let id = BuildingId(self.next_building);
        self.next_building += 1;
        self.galaxy
            .planet_mut(p)
            .buildings
            .push(Building { id, owner: f, kind });
    }

    // --- Daily systems -------------------------------------------------

    /// Net currency a faction gains per day right now (income minus upkeep).
    pub fn net_income(&self, f: FactionId) -> f64 {
        let power = self.factions[f.idx()].power;
        let mut total = BASE_INCOME;
        total += REGION_LEADER_DAILY_FUNDS
            * self
                .region_leaders
                .iter()
                .filter(|l| **l == Some(f))
                .count() as f64;
        let mut upkeep = 0.0;
        let mut count = 0usize;
        for p in &self.galaxy.planets {
            let contested = self.control[p.id.idx()] == ControlState::Contested;
            let leads_system = self.system_leaders[p.system.idx()] == Some(f);
            for b in p.buildings.iter().filter(|b| b.owner == f) {
                upkeep += building_upkeep(b.kind);
                count += 1;
                if b.kind == BuildingKind::Income {
                    let mut inc = income_yield(power, p.population);
                    if contested && power == Power::Mafia {
                        inc *= MAFIA_CONTESTED_INCOME_MULT;
                    }
                    if leads_system {
                        inc *= 1.0 + SYSTEM_LEADER_INCOME_BONUS;
                    }
                    total += inc;
                }
            }
        }
        total - upkeep * (1.0 + count as f64 / UPKEEP_SPRAWL)
    }

    fn collect_income(&mut self) {
        let nf = self.factions.len();
        let delta: Vec<f64> = (0..nf)
            .map(|i| self.net_income(FactionId(i as u32)))
            .collect();
        for (fac, d) in self.factions.iter_mut().zip(delta) {
            fac.funds += d;
        }

        // A faction deep in debt loses a building (never its headquarters).
        for i in 0..nf {
            if self.factions[i].funds < -100.0 {
                let f = FactionId(i as u32);
                if let Some((pid, bid, kind)) = self.pick_disposable_building(f, None) {
                    self.galaxy
                        .planet_mut(pid)
                        .buildings
                        .retain(|b| b.id != bid);
                    self.factions[i].funds += 50.0;
                    self.stats.abandoned += 1;
                    self.push_event(EventKind::Abandoned {
                        faction: f,
                        planet: pid,
                        kind,
                    });
                }
            }
        }
    }

    /// A random non-headquarters building of `f`, optionally within a region.
    fn pick_disposable_building(
        &mut self,
        f: FactionId,
        region: Option<RegionId>,
    ) -> Option<(PlanetId, BuildingId, BuildingKind)> {
        let this: &Sim = self;
        let mut candidates: Vec<(PlanetId, BuildingId, BuildingKind)> = Vec::new();
        for p in &this.galaxy.planets {
            if region.is_some_and(|r| this.galaxy.system(p.system).region != r) {
                continue;
            }
            for b in &p.buildings {
                if b.owner == f && !this.is_headquarters(p, b) {
                    candidates.push((p.id, b.id, b.kind));
                }
            }
        }
        self.rng.pick(&candidates).copied()
    }

    fn spread_influence(&mut self) {
        let nf = self.factions.len();
        let powers: Vec<Power> = self.factions.iter().map(|f| f.power).collect();
        let decay: Vec<f64> = powers.iter().map(|&p| influence_decay(p)).collect();
        for p in &mut self.galaxy.planets {
            let system = &self.galaxy.systems[p.system.idx()];
            let sys_leader = self.system_leaders[p.system.idx()];
            let reg_leader = self.region_leaders[system.region.idx()];
            let mut gain = vec![0.0; nf];
            for b in &p.buildings {
                let fi = b.owner.idx();
                let mut y = building_yield(b.kind, powers[fi]) * p.race.affinity(powers[fi]);
                let mut bonus = 1.0;
                if sys_leader == Some(b.owner) {
                    bonus += SYSTEM_LEADER_YIELD_BONUS;
                }
                if reg_leader == Some(b.owner) {
                    bonus += REGION_LEADER_YIELD_BONUS;
                }
                y *= bonus;
                gain[fi] += y;
            }
            for i in 0..nf {
                let v = p.influence[i] * (1.0 - decay[i]) + gain[i];
                p.influence[i] = if v < 0.01 { 0.0 } else { v };
            }
        }
    }

    /// Recompute the control rule for every planet (decision 3).
    fn refresh_control(&mut self, emit: bool) {
        for i in 0..self.galaxy.planets.len() {
            let new = control_state(&self.galaxy.planets[i].influence);
            let old = self.control[i];
            if new != old {
                self.control[i] = new;
                let involves_control = matches!(old, ControlState::Controlled(_))
                    || matches!(new, ControlState::Controlled(_));
                if emit && involves_control {
                    self.stats.control_changes += 1;
                    self.push_event(EventKind::ControlChanged {
                        planet: PlanetId(i as u32),
                        from: old,
                        to: new,
                    });
                }
            }
        }
    }

    /// Recompute system and region leaders (decision 4).
    fn refresh_leaders(&mut self, emit: bool) {
        let nf = self.factions.len();
        let mut region_totals = vec![vec![0.0; nf]; self.galaxy.regions.len()];
        for s in &self.galaxy.systems {
            let mut totals = vec![0.0; nf];
            for &pid in &s.planets {
                for (t, v) in totals.iter_mut().zip(&self.galaxy.planet(pid).influence) {
                    *t += v;
                }
            }
            for (rt, t) in region_totals[s.region.idx()].iter_mut().zip(&totals) {
                *rt += t;
            }
            self.system_leaders[s.id.idx()] =
                leader(&totals, SYSTEM_LEAD_FLOOR, SYSTEM_LEAD_MARGIN);
        }
        for (r, totals) in region_totals.iter().enumerate() {
            let new = leader(totals, REGION_LEAD_FLOOR, SYSTEM_LEAD_MARGIN);
            let old = self.region_leaders[r];
            if new != old {
                self.region_leaders[r] = new;
                if emit {
                    self.push_event(EventKind::RegionLeaderChanged {
                        region: RegionId(r as u32),
                        from: old,
                        to: new,
                    });
                }
            }
        }
    }

    fn building_counts(&self) -> Vec<usize> {
        let mut counts = vec![0usize; self.factions.len()];
        for p in &self.galaxy.planets {
            for b in &p.buildings {
                counts[b.owner.idx()] += 1;
            }
        }
        counts
    }

    fn update_heat(&mut self) {
        let counts = self.building_counts();
        for (i, &count) in counts.iter().enumerate() {
            let target = structural_heat(count);
            let fac = &mut self.factions[i];
            fac.heat = (fac.heat + (target - fac.heat) * HEAT_RELAX_RATE).clamp(0.0, 100.0);
            let heat = fac.heat;
            let f = FactionId(i as u32);
            self.refresh_exposure(f);
            if heat > EXPOSED_HEAT {
                let chance = (heat - EXPOSED_HEAT) / CRACKDOWN_DIVISOR;
                if self.rng.chance(chance) {
                    self.crackdown(f);
                }
            }
        }
    }

    /// Government raids hit the region where the faction is most visible.
    fn crackdown(&mut self, f: FactionId) {
        let mut per_region = vec![0usize; self.galaxy.regions.len()];
        for p in &self.galaxy.planets {
            let n = p
                .buildings
                .iter()
                .filter(|b| b.owner == f && !self.is_headquarters(p, b))
                .count();
            per_region[self.galaxy.system(p.system).region.idx()] += n;
        }
        let Some((region, _)) = per_region
            .iter()
            .enumerate()
            .filter(|(_, &n)| n > 0)
            .max_by_key(|(i, &n)| (n, std::cmp::Reverse(*i)))
        else {
            self.factions[f.idx()].heat -= CRACKDOWN_HEAT_RELIEF;
            return;
        };
        let region = RegionId(region as u32);
        let total: usize = per_region.iter().sum();
        let target = CRACKDOWN_MIN_BUILDINGS.max((total as f64 * CRACKDOWN_SHARE).ceil() as usize);
        let mut lost = 0;
        while lost < target {
            // Start in the most visible region, then spill over elsewhere.
            let pick = self
                .pick_disposable_building(f, Some(region))
                .or_else(|| self.pick_disposable_building(f, None));
            let Some((pid, bid, _)) = pick else { break };
            let planet = self.galaxy.planet_mut(pid);
            planet.buildings.retain(|b| b.id != bid);
            planet.influence[f.idx()] *= 0.5;
            lost += 1;
        }
        let fac = &mut self.factions[f.idx()];
        fac.heat = (fac.heat - CRACKDOWN_HEAT_RELIEF).max(0.0);
        self.stats.crackdowns += 1;
        self.push_event(EventKind::Crackdown {
            faction: f,
            region,
            buildings_lost: lost,
        });
    }

    fn add_heat(&mut self, f: FactionId, amount: f64) {
        let fac = &mut self.factions[f.idx()];
        fac.heat = (fac.heat + amount * heat_multiplier(fac.power)).min(100.0);
        self.refresh_exposure(f);
    }

    /// Exposure has hysteresis so the public status doesn't flicker.
    fn refresh_exposure(&mut self, f: FactionId) {
        let fac = &mut self.factions[f.idx()];
        if !fac.exposed && fac.heat >= EXPOSED_HEAT {
            fac.exposed = true;
            self.push_event(EventKind::Exposed { faction: f });
        } else if fac.exposed && fac.heat < UNEXPOSED_HEAT {
            fac.exposed = false;
        }
    }

    fn expire_agents(&mut self) {
        let today = self.day;
        for fac in &mut self.factions {
            fac.agents.retain(|(_, exp)| *exp > today);
        }
    }

    fn run_ai(&mut self) {
        for i in 0..self.factions.len() {
            if self.factions[i].human {
                continue;
            }
            if (self.day + i as u32) % AI_THINK_INTERVAL_DAYS != 0 {
                continue;
            }
            let f = FactionId(i as u32);
            // Richer factions get more done each time they think.
            let orders = 1 + (self.factions[i].funds / AI_FUNDS_PER_EXTRA_ORDER) as usize;
            for _ in 0..orders.min(AI_MAX_ORDERS_PER_THINK) {
                match ai::decide(self, f) {
                    // The AI only proposes valid orders, but a failure here
                    // is harmless: it simply tries again next time.
                    Some(action) => {
                        let _ = self.issue(f, action);
                    }
                    None => break,
                }
            }
        }
    }

    fn push_event(&mut self, kind: EventKind) {
        let seq = self.next_event;
        self.next_event += 1;
        self.events.push_back(Event {
            seq,
            day: self.day,
            kind,
        });
        while self.events.len() > EVENT_BUFFER {
            self.events.pop_front();
        }
    }

    /// Events with a sequence number greater than `after`.
    pub fn events_since(&self, after: u64) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |e| e.seq > after)
    }

    /// Total influence per faction across the galaxy.
    pub fn influence_totals(&self) -> Vec<f64> {
        let mut t = vec![0.0; self.factions.len()];
        for p in &self.galaxy.planets {
            for (a, v) in t.iter_mut().zip(&p.influence) {
                *a += v;
            }
        }
        t
    }

    pub fn controlled_counts(&self) -> Vec<usize> {
        let mut c = vec![0usize; self.factions.len()];
        for s in &self.control {
            if let ControlState::Controlled(f) = s {
                c[f.idx()] += 1;
            }
        }
        c
    }

    pub fn buildings_per_faction(&self) -> Vec<usize> {
        self.building_counts()
    }

    /// A cheap fingerprint of the whole state, for determinism checks.
    pub fn fingerprint(&self) -> u64 {
        let mut parts: Vec<u64> = vec![self.day as u64];
        for f in &self.factions {
            parts.push(f.funds.to_bits());
            parts.push(f.heat.to_bits());
        }
        for p in &self.galaxy.planets {
            parts.push(p.buildings.len() as u64);
            for v in &p.influence {
                parts.push(v.to_bits());
            }
        }
        rng::hash64(&parts)
    }
}

/// The control rule: control means more influence than all others combined.
pub fn control_state(influence: &[f64]) -> ControlState {
    let total: f64 = influence.iter().sum();
    let present = influence.iter().filter(|&&v| v >= PRESENCE_FLOOR).count();
    if let Some((i, &top)) = influence
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(&a.0)))
    {
        if top >= CONTROL_FLOOR && top > total - top {
            return ControlState::Controlled(FactionId(i as u32));
        }
    }
    if present >= 2 {
        ControlState::Contested
    } else {
        ControlState::Unclaimed
    }
}

fn leader(totals: &[f64], floor: f64, margin: f64) -> Option<FactionId> {
    let mut order: Vec<usize> = (0..totals.len()).collect();
    order.sort_by(|&a, &b| totals[b].total_cmp(&totals[a]).then(a.cmp(&b)));
    let first = *order.first()?;
    let second = order.get(1).map(|&i| totals[i]).unwrap_or(0.0);
    if totals[first] >= floor && totals[first] >= second * margin {
        Some(FactionId(first as u32))
    } else {
        None
    }
}

/// Spread power bases out with farthest-point sampling over systems that
/// have a planet large enough for a full set of buildings.
fn pick_homes(g: &Galaxy, count: usize, rng: &mut Rng) -> Vec<PlanetId> {
    let candidates: Vec<(SystemId, PlanetId)> = g
        .systems
        .iter()
        .filter_map(|s| {
            s.planets
                .iter()
                .copied()
                .filter(|&p| g.planet(p).slots >= 4)
                .max_by(|&a, &b| {
                    g.planet(a)
                        .population
                        .total_cmp(&g.planet(b).population)
                        .then(b.cmp(&a))
                })
                .map(|p| (s.id, p))
        })
        .collect();
    assert!(
        candidates.len() >= count,
        "galaxy too small for {count} power bases; add systems"
    );
    let mut chosen: Vec<(SystemId, PlanetId)> = vec![*rng.pick(&candidates).unwrap()];
    while chosen.len() < count {
        let used: Vec<SystemId> = chosen.iter().map(|c| c.0).collect();
        let hops = g.hops_from(&used);
        let next = candidates
            .iter()
            .filter(|c| !used.contains(&c.0))
            .max_by_key(|c| (hops[c.0.idx()], std::cmp::Reverse(c.0)))
            .copied()
            .unwrap();
        chosen.push(next);
    }
    chosen.into_iter().map(|c| c.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_rule() {
        assert_eq!(control_state(&[0.0, 0.0]), ControlState::Unclaimed);
        assert_eq!(
            control_state(&[10.0, 3.0, 3.0]),
            ControlState::Controlled(FactionId(0))
        );
        // 10 is not more than 6 + 4 combined.
        assert_eq!(control_state(&[10.0, 6.0, 4.0]), ControlState::Contested);
        // A lone faction below the floor does not control.
        assert_eq!(control_state(&[3.0, 0.0]), ControlState::Unclaimed);
    }

    #[test]
    fn new_sim_has_power_bases() {
        let sim = Sim::new(SimConfig::default());
        assert_eq!(sim.factions.len(), 9);
        for f in &sim.factions {
            let home = sim.galaxy.planet(f.home);
            assert_eq!(home.buildings.iter().filter(|b| b.owner == f.id).count(), 4);
            assert_eq!(sim.control[f.home.idx()], ControlState::Controlled(f.id));
        }
    }
}
