//! Orders, events and game dates.

use std::fmt;

use crate::balance::{DAYS_PER_MONTH, DAYS_PER_YEAR, START_YEAR};
use crate::model::*;

/// Something a faction wants done at a planet. Orders travel to their
/// target (instantly over your relays, by courier otherwise) and are
/// carried out on arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Build {
        planet: PlanetId,
        kind: BuildingKind,
    },
    /// Intimidation / Sermon / Rally: an instant push of influence.
    Campaign { planet: PlanetId },
    /// Destroy a rival's building.
    Sabotage {
        planet: PlanetId,
        building: BuildingId,
    },
    /// Place an agent for exact intel on a planet you have no holdings on.
    Agent { planet: PlanetId },
    /// Pay off officials where you have holdings to cool your heat.
    Bribe { planet: PlanetId },
}

impl Action {
    pub fn planet(self) -> PlanetId {
        match self {
            Action::Build { planet, .. }
            | Action::Campaign { planet }
            | Action::Sabotage { planet, .. }
            | Action::Agent { planet }
            | Action::Bribe { planet } => planet,
        }
    }

    pub fn describe(self, power: Power) -> String {
        match self {
            Action::Build { kind, .. } => format!("build a {}", kind.name(power)),
            Action::Campaign { .. } => power.campaign_name().to_lowercase(),
            Action::Sabotage { .. } => "sabotage".to_string(),
            Action::Agent { .. } => "place an agent".to_string(),
            Action::Bribe { .. } => "bribe officials".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingOrder {
    pub id: u64,
    pub faction: FactionId,
    pub action: Action,
    pub issued: u32,
    pub arrives: u32,
    /// True if sent over the faction's own relay network.
    pub via_relay: bool,
    pub cost: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderError {
    UnknownTarget,
    NotEnoughFunds,
    NoFreeSlot,
    /// You need a building on this planet first.
    NoPresence,
    /// Too far from your holdings.
    OutOfReach,
    /// One relay per faction per system.
    RelayAlreadyInSystem,
    /// You already have the most buildings allowed on this holding.
    HoldingLimit,
    /// You cannot target your own buildings (or a rival's headquarters).
    InvalidTarget,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            OrderError::UnknownTarget => "no such target",
            OrderError::NotEnoughFunds => "not enough funds",
            OrderError::NoFreeSlot => "no free building slot",
            OrderError::NoPresence => "you need a building on this planet first",
            OrderError::OutOfReach => "too far from your holdings",
            OrderError::RelayAlreadyInSystem => "you already have a relay in this system",
            OrderError::HoldingLimit => "you already have the most buildings allowed here",
            OrderError::InvalidTarget => "invalid target",
        })
    }
}

impl std::error::Error for OrderError {}

#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Built {
        faction: FactionId,
        planet: PlanetId,
        kind: BuildingKind,
    },
    /// `by` is `None` when a government crackdown did it.
    Destroyed {
        by: Option<FactionId>,
        victim: FactionId,
        planet: PlanetId,
        kind: BuildingKind,
    },
    SabotageFailed {
        faction: FactionId,
        victim: FactionId,
        planet: PlanetId,
        /// True if the victim read the order over its relay and was ready.
        foiled: bool,
    },
    Campaign {
        faction: FactionId,
        planet: PlanetId,
    },
    AgentPlaced {
        faction: FactionId,
        planet: PlanetId,
    },
    Bribed {
        faction: FactionId,
        planet: PlanetId,
    },
    ControlChanged {
        planet: PlanetId,
        from: ControlState,
        to: ControlState,
    },
    Crackdown {
        faction: FactionId,
        region: RegionId,
        buildings_lost: usize,
    },
    Exposed {
        faction: FactionId,
    },
    Intercepted {
        interceptor: FactionId,
        sender: FactionId,
        planet: PlanetId,
        action: Action,
    },
    OrderFailed {
        faction: FactionId,
        planet: PlanetId,
        reason: OrderError,
    },
    Abandoned {
        faction: FactionId,
        planet: PlanetId,
        kind: BuildingKind,
    },
    RegionLeaderChanged {
        region: RegionId,
        from: Option<FactionId>,
        to: Option<FactionId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Monotonic sequence number, for clients catching up.
    pub seq: u64,
    pub day: u32,
    pub kind: EventKind,
}

impl Event {
    /// Whether a faction hears about this event. Control changes, crackdowns,
    /// exposure and region leadership are public; covert acts are known only
    /// to the people involved.
    pub fn visible_to(&self, f: FactionId) -> bool {
        match &self.kind {
            EventKind::Built { faction, .. }
            | EventKind::Campaign { faction, .. }
            | EventKind::AgentPlaced { faction, .. }
            | EventKind::Bribed { faction, .. }
            | EventKind::OrderFailed { faction, .. }
            | EventKind::Abandoned { faction, .. } => *faction == f,
            EventKind::Destroyed { by, victim, .. } => *victim == f || *by == Some(f),
            EventKind::SabotageFailed {
                faction, victim, ..
            } => *faction == f || *victim == f,
            EventKind::Intercepted { interceptor, .. } => *interceptor == f,
            EventKind::ControlChanged { .. }
            | EventKind::Crackdown { .. }
            | EventKind::Exposed { .. }
            | EventKind::RegionLeaderChanged { .. } => true,
        }
    }
}

/// Calendar position of a simulation day (one tick = one day; decision 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameDate {
    pub year: u32,
    /// 1-based.
    pub month: u32,
    /// 1-based.
    pub day: u32,
}

impl GameDate {
    pub fn from_day(day: u32) -> Self {
        Self {
            year: START_YEAR + day / DAYS_PER_YEAR,
            month: (day % DAYS_PER_YEAR) / DAYS_PER_MONTH + 1,
            day: day % DAYS_PER_MONTH + 1,
        }
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(GameDate::from_day(0).to_string(), "2340-01-01");
        assert_eq!(GameDate::from_day(29).to_string(), "2340-01-30");
        assert_eq!(GameDate::from_day(30).to_string(), "2340-02-01");
        assert_eq!(GameDate::from_day(360).to_string(), "2341-01-01");
    }
}
