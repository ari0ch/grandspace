//! Core data types: powers, races, factions, the galaxy and its holdings.

use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $name {
            pub fn idx(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(SystemId);
id_type!(PlanetId);
id_type!(RegionId);
id_type!(FactionId);
id_type!(BuildingId);

/// The three hidden powers. Every playable faction is a branch of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Power {
    Mafia,
    Church,
    Unions,
}

impl Power {
    pub const ALL: [Power; 3] = [Power::Mafia, Power::Church, Power::Unions];

    pub fn name(self) -> &'static str {
        match self {
            Power::Mafia => "Mafia",
            Power::Church => "Church",
            Power::Unions => "Unions",
        }
    }

    /// Each power spends its own currency (decision 11).
    pub fn currency(self) -> &'static str {
        match self {
            Power::Mafia => "cash",
            Power::Church => "faith",
            Power::Unions => "labour",
        }
    }

    /// Name of the instant influence push for this power.
    pub fn campaign_name(self) -> &'static str {
        match self {
            Power::Mafia => "Intimidation",
            Power::Church => "Sermon",
            Power::Unions => "Rally",
        }
    }
}

impl fmt::Display for Power {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Races are independent of factions; each favours a different power's
/// approach (decision 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Race {
    /// Devout: responds to faith and ritual. Favours the Church.
    Ostrani,
    /// Transactional: everything has a price. Favours the Mafia.
    Merrow,
    /// Industrial: organised, collective labour. Favours the Unions.
    Haldane,
}

impl Race {
    pub const ALL: [Race; 3] = [Race::Ostrani, Race::Merrow, Race::Haldane];

    pub fn name(self) -> &'static str {
        match self {
            Race::Ostrani => "Ostrani",
            Race::Merrow => "Merrow",
            Race::Haldane => "Haldane",
        }
    }

    pub fn temperament(self) -> &'static str {
        match self {
            Race::Ostrani => "devout",
            Race::Merrow => "transactional",
            Race::Haldane => "industrial",
        }
    }

    /// Multiplier on influence a power gains among this race.
    pub fn affinity(self, power: Power) -> f64 {
        match (self, power) {
            (Race::Ostrani, Power::Church) => 1.4,
            (Race::Ostrani, _) => 0.85,
            (Race::Merrow, Power::Mafia) => 1.4,
            (Race::Merrow, Power::Church) => 0.8,
            (Race::Merrow, Power::Unions) => 0.95,
            (Race::Haldane, Power::Unions) => 1.4,
            (Race::Haldane, _) => 0.85,
        }
    }
}

impl fmt::Display for Race {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlanetKind {
    Terran,
    Ocean,
    Desert,
    Ice,
    Barren,
    Station,
}

impl PlanetKind {
    pub const ALL: [PlanetKind; 6] = [
        PlanetKind::Terran,
        PlanetKind::Ocean,
        PlanetKind::Desert,
        PlanetKind::Ice,
        PlanetKind::Barren,
        PlanetKind::Station,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlanetKind::Terran => "Terran",
            PlanetKind::Ocean => "Ocean",
            PlanetKind::Desert => "Desert",
            PlanetKind::Ice => "Ice",
            PlanetKind::Barren => "Barren",
            PlanetKind::Station => "Station",
        }
    }

    /// Building slots shared by every faction on this holding.
    pub fn slots(self) -> u8 {
        match self {
            PlanetKind::Terran => 6,
            PlanetKind::Ocean => 5,
            PlanetKind::Desert => 4,
            PlanetKind::Ice => 3,
            PlanetKind::Barren => 3,
            PlanetKind::Station => 3,
        }
    }

    /// Population range in millions (rough flavour; drives income).
    pub fn population_range(self) -> (f64, f64) {
        match self {
            PlanetKind::Terran => (40.0, 120.0),
            PlanetKind::Ocean => (20.0, 80.0),
            PlanetKind::Desert => (10.0, 50.0),
            PlanetKind::Ice => (5.0, 25.0),
            PlanetKind::Barren => (2.0, 12.0),
            PlanetKind::Station => (8.0, 30.0),
        }
    }
}

/// What a faction can build on a shared holding (decision 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingKind {
    /// Foothold: presence, exact intel and a little influence.
    Outpost,
    /// The power's main influence building.
    Hall,
    /// Generates the power's currency.
    Income,
    /// Ansible relay: instant orders to this system, and taps rivals'
    /// traffic arriving here (decision 10).
    Relay,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 4] = [
        BuildingKind::Outpost,
        BuildingKind::Hall,
        BuildingKind::Income,
        BuildingKind::Relay,
    ];

    /// Flavour name as each power calls it.
    pub fn name(self, power: Power) -> &'static str {
        match (self, power) {
            (BuildingKind::Outpost, Power::Mafia) => "Safehouse",
            (BuildingKind::Outpost, Power::Church) => "Mission",
            (BuildingKind::Outpost, Power::Unions) => "Union Office",
            (BuildingKind::Hall, Power::Mafia) => "Protection Racket",
            (BuildingKind::Hall, Power::Church) => "Chapel",
            (BuildingKind::Hall, Power::Unions) => "Union Hall",
            (BuildingKind::Income, Power::Mafia) => "Casino",
            (BuildingKind::Income, Power::Church) => "Tithe House",
            (BuildingKind::Income, Power::Unions) => "Hiring Hall",
            (BuildingKind::Relay, _) => "Ansible Relay",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub id: RegionId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StarSystem {
    pub id: SystemId,
    pub name: String,
    /// Map position in abstract units (roughly 0..1000 by 0..700).
    pub pos: (f64, f64),
    pub region: RegionId,
    pub planets: Vec<PlanetId>,
    /// Lane neighbours, sorted.
    pub lanes: Vec<SystemId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    pub id: BuildingId,
    pub owner: FactionId,
    pub kind: BuildingKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planet {
    pub id: PlanetId,
    pub name: String,
    pub system: SystemId,
    pub kind: PlanetKind,
    pub race: Race,
    /// Millions.
    pub population: f64,
    pub slots: u8,
    pub buildings: Vec<Building>,
    /// Influence per faction, indexed by `FactionId`.
    pub influence: Vec<f64>,
}

impl Planet {
    pub fn free_slots(&self) -> usize {
        (self.slots as usize).saturating_sub(self.buildings.len())
    }

    pub fn has_presence(&self, f: FactionId) -> bool {
        self.buildings.iter().any(|b| b.owner == f)
    }

    pub fn count_owned(&self, f: FactionId, kind: BuildingKind) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.owner == f && b.kind == kind)
            .count()
    }

    pub fn total_influence(&self) -> f64 {
        self.influence.iter().sum()
    }
}

/// How a planet stands under the control rule (decision 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlState {
    /// Nobody has meaningful influence here.
    Unclaimed,
    /// Several factions present, none with a majority over all others
    /// combined. These are the hot spots (decision 12).
    Contested,
    /// One faction's influence exceeds every other faction's combined.
    Controlled(FactionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Faction {
    pub id: FactionId,
    pub name: String,
    pub power: Power,
    /// The planet the faction starts from; its relay network is anchored
    /// here and couriers leave from it.
    pub home: PlanetId,
    /// The power's currency (cash, faith or labour).
    pub funds: f64,
    /// Exposure, 0..=100. High heat brings crackdowns (decision 14).
    pub heat: f64,
    /// Publicly known to be under government scrutiny. Set when heat
    /// reaches `EXPOSED_HEAT`, cleared below `UNEXPOSED_HEAT`.
    pub exposed: bool,
    /// Planets where an agent is giving exact intel, with expiry day.
    pub agents: Vec<(PlanetId, u32)>,
    /// True when a human (or remote client) gives this faction's orders.
    pub human: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Galaxy {
    pub regions: Vec<Region>,
    pub systems: Vec<StarSystem>,
    pub planets: Vec<Planet>,
}

impl Galaxy {
    pub fn system(&self, id: SystemId) -> &StarSystem {
        &self.systems[id.idx()]
    }

    pub fn planet(&self, id: PlanetId) -> &Planet {
        &self.planets[id.idx()]
    }

    pub fn planet_mut(&mut self, id: PlanetId) -> &mut Planet {
        &mut self.planets[id.idx()]
    }

    /// Lane hops from every system to the nearest system in `sources`.
    /// Unreachable systems get `u32::MAX`.
    pub fn hops_from(&self, sources: &[SystemId]) -> Vec<u32> {
        let mut dist = vec![u32::MAX; self.systems.len()];
        let mut queue = std::collections::VecDeque::new();
        for &s in sources {
            if dist[s.idx()] != 0 {
                dist[s.idx()] = 0;
                queue.push_back(s);
            }
        }
        while let Some(s) = queue.pop_front() {
            let d = dist[s.idx()];
            for &n in &self.systems[s.idx()].lanes {
                if dist[n.idx()] == u32::MAX {
                    dist[n.idx()] = d + 1;
                    queue.push_back(n);
                }
            }
        }
        dist
    }
}
