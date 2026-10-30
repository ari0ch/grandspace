//! Messages between the Grandspace server and its thin clients.
//!
//! The simulation crate stays dependency-free, so the wire format lives here
//! as plain serde types with conversions from the simulation's per-faction
//! views. Messages travel as JSON text over a WebSocket: easy to debug and
//! readable from a future browser client.
//!
//! The server is authoritative. Clients send intents ([`ClientMsg`]) and
//! receive only what their faction can see ([`ServerMsg::State`]).

use grandspace_sim as sim;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

// --- Client to server -----------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message on a connection. `faction` asks for a particular
    /// faction; `None` takes the first one nobody is playing.
    Hello {
        version: u32,
        faction: Option<u32>,
    },
    Order {
        action: WireAction,
    },
    SetSpeed {
        days_per_second: f32,
    },
    Pause,
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireAction {
    Build { planet: u32, building: WireBuilding },
    Campaign { planet: u32 },
    Sabotage { planet: u32, building_id: u32 },
    Agent { planet: u32 },
    Bribe { planet: u32 },
}

// --- Server to client -----------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        version: u32,
        faction: u32,
        galaxy: WireGalaxy,
    },
    /// Sent after every simulated day (and on joining).
    State {
        view: WireView,
        /// New events this faction heard about since the last state.
        events: Vec<WireEvent>,
    },
    OrderAccepted {
        id: u64,
        arrives_day: u32,
    },
    OrderRejected {
        reason: String,
    },
    Clock {
        days_per_second: f32,
        paused: bool,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WirePower {
    Mafia,
    Church,
    Unions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireRace {
    Ostrani,
    Merrow,
    Haldane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WirePlanetKind {
    Terran,
    Ocean,
    Desert,
    Ice,
    Barren,
    Station,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireBuilding {
    Outpost,
    Hall,
    Income,
    Relay,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireGalaxy {
    pub regions: Vec<WireRegion>,
    pub systems: Vec<WireSystem>,
    pub planets: Vec<WirePlanetInfo>,
    pub factions: Vec<WireFaction>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireRegion {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireSystem {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub region: u32,
    pub planets: Vec<u32>,
    pub lanes: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WirePlanetInfo {
    pub id: u32,
    pub name: String,
    pub system: u32,
    pub kind: WirePlanetKind,
    pub race: WireRace,
    pub population: f32,
    pub slots: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireFaction {
    pub id: u32,
    pub name: String,
    pub power: WirePower,
    pub home: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "faction", rename_all = "snake_case")]
pub enum WireControl {
    Unclaimed,
    Contested,
    Controlled(u32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireBuildingView {
    pub id: u32,
    pub owner: u32,
    pub kind: WireBuilding,
    pub headquarters: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WirePlanetView {
    pub id: u32,
    /// True when the numbers are exact (you have holdings or an agent).
    pub exact: bool,
    pub used_slots: u8,
    pub buildings: Vec<WireBuildingView>,
    pub influence: Vec<f32>,
    pub control: WireControl,
    pub outpost_reach: bool,
    pub delay_days: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireOrder {
    pub id: u64,
    pub action: WireAction,
    pub arrives_day: u32,
    pub via_relay: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireView {
    pub day: u32,
    pub date: String,
    pub me: u32,
    pub currency: String,
    pub funds: f32,
    pub income_per_day: f32,
    pub heat: f32,
    pub exposed_rivals: Vec<u32>,
    pub relay_systems: Vec<u32>,
    pub agents: Vec<u32>,
    pub planets: Vec<WirePlanetView>,
    pub orders: Vec<WireOrder>,
    pub system_leaders: Vec<Option<u32>>,
    pub region_leaders: Vec<Option<u32>>,
    pub costs: WireCosts,
}

/// What each order costs this faction, so the client can show prices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireCosts {
    pub outpost: f32,
    pub hall: f32,
    pub income: f32,
    pub relay: f32,
    pub campaign: f32,
    pub sabotage: f32,
    pub agent: f32,
    pub bribe: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireEvent {
    pub seq: u64,
    pub day: u32,
    pub date: String,
    pub planet: Option<u32>,
    /// Human-readable, written from the receiving faction's point of view.
    pub text: String,
}

// --- Conversions ------------------------------------------------------------

impl From<sim::Power> for WirePower {
    fn from(p: sim::Power) -> Self {
        match p {
            sim::Power::Mafia => WirePower::Mafia,
            sim::Power::Church => WirePower::Church,
            sim::Power::Unions => WirePower::Unions,
        }
    }
}

impl From<WirePower> for sim::Power {
    fn from(p: WirePower) -> Self {
        match p {
            WirePower::Mafia => sim::Power::Mafia,
            WirePower::Church => sim::Power::Church,
            WirePower::Unions => sim::Power::Unions,
        }
    }
}

impl From<sim::Race> for WireRace {
    fn from(r: sim::Race) -> Self {
        match r {
            sim::Race::Ostrani => WireRace::Ostrani,
            sim::Race::Merrow => WireRace::Merrow,
            sim::Race::Haldane => WireRace::Haldane,
        }
    }
}

impl From<WireRace> for sim::Race {
    fn from(r: WireRace) -> Self {
        match r {
            WireRace::Ostrani => sim::Race::Ostrani,
            WireRace::Merrow => sim::Race::Merrow,
            WireRace::Haldane => sim::Race::Haldane,
        }
    }
}

impl From<sim::PlanetKind> for WirePlanetKind {
    fn from(k: sim::PlanetKind) -> Self {
        match k {
            sim::PlanetKind::Terran => WirePlanetKind::Terran,
            sim::PlanetKind::Ocean => WirePlanetKind::Ocean,
            sim::PlanetKind::Desert => WirePlanetKind::Desert,
            sim::PlanetKind::Ice => WirePlanetKind::Ice,
            sim::PlanetKind::Barren => WirePlanetKind::Barren,
            sim::PlanetKind::Station => WirePlanetKind::Station,
        }
    }
}

impl From<sim::BuildingKind> for WireBuilding {
    fn from(k: sim::BuildingKind) -> Self {
        match k {
            sim::BuildingKind::Outpost => WireBuilding::Outpost,
            sim::BuildingKind::Hall => WireBuilding::Hall,
            sim::BuildingKind::Income => WireBuilding::Income,
            sim::BuildingKind::Relay => WireBuilding::Relay,
        }
    }
}

impl From<WireBuilding> for sim::BuildingKind {
    fn from(k: WireBuilding) -> Self {
        match k {
            WireBuilding::Outpost => sim::BuildingKind::Outpost,
            WireBuilding::Hall => sim::BuildingKind::Hall,
            WireBuilding::Income => sim::BuildingKind::Income,
            WireBuilding::Relay => sim::BuildingKind::Relay,
        }
    }
}

impl From<sim::ControlState> for WireControl {
    fn from(c: sim::ControlState) -> Self {
        match c {
            sim::ControlState::Unclaimed => WireControl::Unclaimed,
            sim::ControlState::Contested => WireControl::Contested,
            sim::ControlState::Controlled(f) => WireControl::Controlled(f.0),
        }
    }
}

impl From<sim::Action> for WireAction {
    fn from(a: sim::Action) -> Self {
        match a {
            sim::Action::Build { planet, kind } => WireAction::Build {
                planet: planet.0,
                building: kind.into(),
            },
            sim::Action::Campaign { planet } => WireAction::Campaign { planet: planet.0 },
            sim::Action::Sabotage { planet, building } => WireAction::Sabotage {
                planet: planet.0,
                building_id: building.0,
            },
            sim::Action::Agent { planet } => WireAction::Agent { planet: planet.0 },
            sim::Action::Bribe { planet } => WireAction::Bribe { planet: planet.0 },
        }
    }
}

impl From<WireAction> for sim::Action {
    fn from(a: WireAction) -> Self {
        let p = sim::PlanetId;
        match a {
            WireAction::Build { planet, building } => sim::Action::Build {
                planet: p(planet),
                kind: building.into(),
            },
            WireAction::Campaign { planet } => sim::Action::Campaign { planet: p(planet) },
            WireAction::Sabotage {
                planet,
                building_id,
            } => sim::Action::Sabotage {
                planet: p(planet),
                building: sim::BuildingId(building_id),
            },
            WireAction::Agent { planet } => sim::Action::Agent { planet: p(planet) },
            WireAction::Bribe { planet } => sim::Action::Bribe { planet: p(planet) },
        }
    }
}

impl From<&sim::view::GalaxyInfo> for WireGalaxy {
    fn from(g: &sim::view::GalaxyInfo) -> Self {
        Self {
            regions: g
                .regions
                .iter()
                .map(|r| WireRegion {
                    id: r.id.0,
                    name: r.name.clone(),
                })
                .collect(),
            systems: g
                .systems
                .iter()
                .map(|s| WireSystem {
                    id: s.id.0,
                    name: s.name.clone(),
                    x: s.pos.0 as f32,
                    y: s.pos.1 as f32,
                    region: s.region.0,
                    planets: s.planets.iter().map(|p| p.0).collect(),
                    lanes: s.lanes.iter().map(|l| l.0).collect(),
                })
                .collect(),
            planets: g
                .planets
                .iter()
                .map(|p| WirePlanetInfo {
                    id: p.id.0,
                    name: p.name.clone(),
                    system: p.system.0,
                    kind: p.kind.into(),
                    race: p.race.into(),
                    population: p.population as f32,
                    slots: p.slots,
                })
                .collect(),
            factions: g
                .factions
                .iter()
                .map(|f| WireFaction {
                    id: f.id.0,
                    name: f.name.clone(),
                    power: f.power.into(),
                    home: f.home.0,
                })
                .collect(),
        }
    }
}

impl WireView {
    pub fn from_view(v: &sim::view::FactionView, power: sim::Power) -> Self {
        use sim::balance::*;
        use sim::BuildingKind as K;
        Self {
            day: v.day,
            date: v.date.clone(),
            me: v.me.0,
            currency: power.currency().to_string(),
            funds: v.funds as f32,
            income_per_day: v.income_per_day as f32,
            heat: v.heat as f32,
            exposed_rivals: v
                .rivals
                .iter()
                .filter(|r| r.exposed)
                .map(|r| r.id.0)
                .collect(),
            relay_systems: v.relay_systems.iter().map(|s| s.0).collect(),
            agents: v.agents.iter().map(|(p, _)| p.0).collect(),
            planets: v
                .planets
                .iter()
                .map(|p| WirePlanetView {
                    id: p.id.0,
                    exact: p.intel == sim::view::Intel::Exact,
                    used_slots: p.used_slots,
                    buildings: p
                        .buildings
                        .iter()
                        .map(|b| WireBuildingView {
                            id: b.id.0,
                            owner: b.owner.0,
                            kind: b.kind.into(),
                            headquarters: b.headquarters,
                        })
                        .collect(),
                    influence: p.influence.iter().map(|&x| x as f32).collect(),
                    control: p.control.into(),
                    outpost_reach: p.outpost_reach,
                    delay_days: p.delay_days,
                })
                .collect(),
            orders: v
                .orders
                .iter()
                .map(|o| WireOrder {
                    id: o.id,
                    action: o.action.into(),
                    arrives_day: o.arrives_day,
                    via_relay: o.via_relay,
                })
                .collect(),
            system_leaders: v.system_leaders.iter().map(|l| l.map(|f| f.0)).collect(),
            region_leaders: v.region_leaders.iter().map(|l| l.map(|f| f.0)).collect(),
            costs: WireCosts {
                outpost: building_cost(K::Outpost, power) as f32,
                hall: building_cost(K::Hall, power) as f32,
                income: building_cost(K::Income, power) as f32,
                relay: building_cost(K::Relay, power) as f32,
                campaign: campaign_cost(power) as f32,
                sabotage: sabotage_cost(power) as f32,
                agent: AGENT_COST as f32,
                bribe: bribe_cost(power) as f32,
            },
        }
    }
}

/// Render an event as one line of text from `viewer`'s point of view.
pub fn describe_event(s: &sim::Sim, viewer: sim::FactionId, e: &sim::Event) -> WireEvent {
    use sim::EventKind as E;
    let who = |f: sim::FactionId| {
        if f == viewer {
            "You".to_string()
        } else {
            s.faction(f).name.clone()
        }
    };
    let planet_name = |p: sim::PlanetId| s.galaxy.planet(p).name.clone();
    let building = |f: sim::FactionId, k: sim::BuildingKind| k.name(s.faction(f).power);
    let (planet, text) = match &e.kind {
        E::Built {
            faction,
            planet,
            kind,
        } => (
            Some(*planet),
            format!(
                "{} built a {} on {}",
                who(*faction),
                building(*faction, *kind),
                planet_name(*planet)
            ),
        ),
        E::Destroyed {
            by,
            victim,
            planet,
            kind,
        } => (
            Some(*planet),
            match by {
                Some(by) => format!(
                    "{} sabotaged {}'s {} on {}",
                    who(*by),
                    who(*victim),
                    building(*victim, *kind),
                    planet_name(*planet)
                ),
                None => format!(
                    "Officials shut down {}'s {} on {}",
                    who(*victim),
                    building(*victim, *kind),
                    planet_name(*planet)
                ),
            },
        ),
        E::SabotageFailed {
            faction,
            victim,
            planet,
            foiled,
        } => (
            Some(*planet),
            if *foiled {
                format!(
                    "{} saw {}'s sabotage coming on {} and stopped it",
                    who(*victim),
                    who(*faction),
                    planet_name(*planet)
                )
            } else {
                format!(
                    "{}'s sabotage against {} failed on {}",
                    who(*faction),
                    who(*victim),
                    planet_name(*planet)
                )
            },
        ),
        E::Campaign { faction, planet } => (
            Some(*planet),
            format!(
                "{} ran a {} on {}",
                who(*faction),
                s.faction(*faction).power.campaign_name().to_lowercase(),
                planet_name(*planet)
            ),
        ),
        E::AgentPlaced { faction, planet } => (
            Some(*planet),
            format!(
                "{} placed an agent on {}",
                who(*faction),
                planet_name(*planet)
            ),
        ),
        E::Bribed { faction, planet } => (
            Some(*planet),
            format!(
                "{} paid off officials on {}; the heat eases",
                who(*faction),
                planet_name(*planet)
            ),
        ),
        E::ControlChanged { planet, from, to } => (
            Some(*planet),
            match (from, to) {
                (_, sim::ControlState::Controlled(f)) => {
                    format!("{} now controls {}", who(*f), planet_name(*planet))
                }
                (sim::ControlState::Controlled(f), _) => {
                    format!("{} lost control of {}", who(*f), planet_name(*planet))
                }
                _ => format!("{} changed hands", planet_name(*planet)),
            },
        ),
        E::Crackdown {
            faction,
            region,
            buildings_lost,
        } => (
            None,
            format!(
                "Government crackdown on {} in the {}: {} holdings raided",
                who(*faction),
                s.galaxy.regions[region.idx()].name,
                buildings_lost
            ),
        ),
        E::Exposed { faction } => (
            None,
            format!("{} drew the authorities' attention", who(*faction)),
        ),
        E::Intercepted {
            sender,
            planet,
            action,
            ..
        } => (
            Some(*planet),
            format!(
                "Your relay intercepted {}'s order to {} on {}",
                who(*sender),
                action.describe(s.faction(*sender).power),
                planet_name(*planet)
            ),
        ),
        E::OrderFailed { planet, reason, .. } => (
            Some(*planet),
            format!("Your order on {} failed: {}", planet_name(*planet), reason),
        ),
        E::Abandoned {
            faction,
            planet,
            kind,
        } => (
            Some(*planet),
            format!(
                "{} could not pay upkeep and abandoned a {} on {}",
                who(*faction),
                building(*faction, *kind),
                planet_name(*planet)
            ),
        ),
        E::RegionLeaderChanged { region, from, to } => {
            let r = &s.galaxy.regions[region.idx()].name;
            (
                None,
                match (from, to) {
                    (_, Some(t)) => format!("{} now dominates the {}", who(*t), r),
                    (Some(f), None) => format!("{} lost its grip on the {}", who(*f), r),
                    (None, None) => format!("The {} is up for grabs", r),
                },
            )
        }
    };
    WireEvent {
        seq: e.seq,
        day: e.day,
        date: sim::GameDate::from_day(e.day).to_string(),
        planet: planet.map(|p| p.0),
        text,
    }
}

pub fn encode<T: Serialize>(msg: &T) -> String {
    serde_json::to_string(msg).expect("protocol messages always serialise")
}

pub fn decode_client(text: &str) -> Result<ClientMsg, serde_json::Error> {
    serde_json::from_str(text)
}

pub fn decode_server(text: &str) -> Result<ServerMsg, serde_json::Error> {
    serde_json::from_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_messages_round_trip() {
        let msgs = vec![
            ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                faction: Some(2),
            },
            ClientMsg::Order {
                action: WireAction::Build {
                    planet: 5,
                    building: WireBuilding::Hall,
                },
            },
            ClientMsg::SetSpeed {
                days_per_second: 4.0,
            },
            ClientMsg::Pause,
        ];
        for m in msgs {
            assert_eq!(decode_client(&encode(&m)).unwrap(), m);
        }
    }

    #[test]
    fn json_is_readable() {
        let m = ClientMsg::Order {
            action: WireAction::Campaign { planet: 3 },
        };
        assert_eq!(
            encode(&m),
            r#"{"type":"order","action":{"kind":"campaign","planet":3}}"#
        );
    }

    #[test]
    fn full_state_round_trips() {
        let s = sim::Sim::new(sim::SimConfig::default());
        let me = sim::FactionId(0);
        let galaxy = WireGalaxy::from(&sim::view::GalaxyInfo::from_sim(&s));
        let view = WireView::from_view(&sim::view::FactionView::build(&s, me), s.faction(me).power);
        let welcome = ServerMsg::Welcome {
            version: PROTOCOL_VERSION,
            faction: 0,
            galaxy,
        };
        assert_eq!(decode_server(&encode(&welcome)).unwrap(), welcome);
        let state = ServerMsg::State {
            view,
            events: vec![],
        };
        assert_eq!(decode_server(&encode(&state)).unwrap(), state);
    }
}
