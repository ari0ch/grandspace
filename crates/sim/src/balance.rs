//! Every tuning number in one place, so balancing never means hunting
//! through game logic. Units: influence points, currency, days.

use crate::model::{BuildingKind, Power};

/// Days in a game year (12 months of 30 days keeps dates readable).
pub const DAYS_PER_YEAR: u32 = 360;
pub const DAYS_PER_MONTH: u32 = 30;
pub const START_YEAR: u32 = 2340;

/// Fraction of influence that fades each day. Equilibrium influence from a
/// steady yield is `yield / decay`.
pub fn influence_decay(power: Power) -> f64 {
    match power {
        // Faith is sticky.
        Power::Church => 0.0049,
        _ => 0.005,
    }
}

/// Minimum influence before a faction can control a planet at all.
pub const CONTROL_FLOOR: f64 = 5.0;
/// Influence below this is treated as no presence when deciding whether a
/// planet is contested.
pub const PRESENCE_FLOOR: f64 = 1.0;

/// Daily influence yield of a building, before race affinity and bonuses.
pub fn building_yield(kind: BuildingKind, power: Power) -> f64 {
    match (kind, power) {
        (BuildingKind::Outpost, _) => 0.05,
        (BuildingKind::Hall, Power::Church) => 0.26,
        (BuildingKind::Hall, _) => 0.25,
        (BuildingKind::Income, _) => 0.05,
        (BuildingKind::Relay, _) => 0.02,
    }
}

pub fn building_cost(kind: BuildingKind, power: Power) -> f64 {
    match (kind, power) {
        (BuildingKind::Outpost, Power::Mafia) => 35.0,
        (BuildingKind::Outpost, _) => 40.0,
        (BuildingKind::Hall, Power::Church) => 120.0,
        (BuildingKind::Hall, Power::Mafia) => 90.0,
        (BuildingKind::Hall, _) => 100.0,
        (BuildingKind::Income, _) => 80.0,
        (BuildingKind::Relay, _) => 300.0,
    }
}

/// Daily upkeep per building, before the sprawl multiplier.
pub fn building_upkeep(kind: BuildingKind) -> f64 {
    match kind {
        BuildingKind::Relay => 1.0,
        _ => 0.12,
    }
}

/// Upkeep grows with network size: total upkeep is multiplied by
/// `1 + buildings / UPKEEP_SPRAWL`. Big organisations are expensive to run.
pub const UPKEEP_SPRAWL: f64 = 60.0;

/// Holdings are shared (decision 2): a faction may own at most this many
/// buildings on one planet, except on its power base.
pub const MAX_BUILDINGS_PER_HOLDING: usize = 2;

/// Base daily stipend every faction receives from its wider organisation.
pub const BASE_INCOME: f64 = 3.5;

/// Daily currency from an income building on a planet of `population`
/// millions.
pub fn income_yield(power: Power, population: f64) -> f64 {
    let base = 0.6 + population / 60.0;
    match power {
        Power::Unions => base * 1.2,
        _ => base,
    }
}

/// Mafia income buildings earn more where nobody is in charge (smuggling;
/// decision 12).
pub const MAFIA_CONTESTED_INCOME_MULT: f64 = 1.5;
/// Influence actions bite harder on contested planets (decision 12).
pub const CONTESTED_ACTION_MULT: f64 = 1.25;

// --- Actions -------------------------------------------------------------

/// Mafia intimidation lands harder than a sermon or a rally.
pub fn campaign_strength(power: Power) -> f64 {
    match power {
        Power::Mafia => 1.25,
        _ => 1.0,
    }
}

pub fn campaign_cost(power: Power) -> f64 {
    match power {
        Power::Unions => 50.0,
        _ => 60.0,
    }
}
/// Instant influence from a campaign (Intimidation / Sermon / Rally).
pub const CAMPAIGN_INFLUENCE: f64 = 15.0;

pub fn sabotage_cost(power: Power) -> f64 {
    match power {
        Power::Mafia => 70.0,
        _ => 100.0,
    }
}
pub fn sabotage_success(power: Power) -> f64 {
    match power {
        Power::Mafia => 0.7,
        _ => 0.5,
    }
}

pub const AGENT_COST: f64 = 30.0;
pub const AGENT_DAYS: u32 = 180;

// --- Exposure (decision 14) ---------------------------------------------

/// Heat drifts towards a structural level set by the size of your network:
/// `100 * (buildings / HEAT_SIZE_SCALE)^2`. Small networks stay under the
/// radar; past roughly 55 buildings you are exposed just by existing
/// ("visibility scales with power").
pub const HEAT_SIZE_SCALE: f64 = 75.0;
/// Fraction of the gap between heat and its structural level closed each
/// day. Spikes from risky actions fade with a half-life of about 70 days.
pub const HEAT_RELAX_RATE: f64 = 0.01;

/// Structural heat for a network of `buildings`.
pub fn structural_heat(buildings: usize) -> f64 {
    (100.0 * (buildings as f64 / HEAT_SIZE_SCALE).powi(2)).min(100.0)
}

/// Bribing officials buys down heat. Money talks loudest for the Mafia.
pub fn bribe_cost(power: Power) -> f64 {
    match power {
        Power::Mafia => 100.0,
        _ => 150.0,
    }
}
pub const BRIBE_HEAT_RELIEF: f64 = 10.0;

/// A crackdown raids at least this share of the faction's buildings.
pub const CRACKDOWN_SHARE: f64 = 0.1;
pub fn heat_multiplier(power: Power) -> f64 {
    match power {
        Power::Church => 1.0,
        Power::Mafia => 1.0,
        Power::Unions => 1.0,
    }
}
pub const HEAT_CAMPAIGN: f64 = 1.5;
pub const HEAT_SABOTAGE_SUCCESS: f64 = 14.0;
pub const HEAT_SABOTAGE_FAIL: f64 = 25.0;
pub const HEAT_AGENT: f64 = 1.0;
pub const HEAT_INTERCEPTED: f64 = 3.0;
/// Heat above which crackdowns can happen and rivals see you as exposed.
pub const EXPOSED_HEAT: f64 = 60.0;
/// Heat a faction must cool below before it stops counting as exposed.
pub const UNEXPOSED_HEAT: f64 = 45.0;
/// Chance per day of a crackdown is `(heat - EXPOSED_HEAT) / this`.
pub const CRACKDOWN_DIVISOR: f64 = 2000.0;
pub const CRACKDOWN_HEAT_RELIEF: f64 = 25.0;
/// A crackdown always raids at least this many buildings.
pub const CRACKDOWN_MIN_BUILDINGS: usize = 3;

// --- Communication (decision 10) ----------------------------------------

/// Courier travel time per lane hop outside your relay network.
pub const COURIER_DAYS_PER_HOP: u32 = 4;
/// Chance that a rival's relay in the destination system reads an order.
pub const INTERCEPT_CHANCE: f64 = 0.3;

// --- Cluster bonuses (decision 4) ---------------------------------------

/// A system leader must beat the runner-up's total by this factor.
pub const SYSTEM_LEAD_MARGIN: f64 = 1.1;
/// Minimum total influence in a system to lead it.
pub const SYSTEM_LEAD_FLOOR: f64 = 20.0;
pub const SYSTEM_LEADER_YIELD_BONUS: f64 = 0.15;
pub const SYSTEM_LEADER_INCOME_BONUS: f64 = 0.10;
pub const REGION_LEAD_FLOOR: f64 = 60.0;
pub const REGION_LEADER_YIELD_BONUS: f64 = 0.05;
pub const REGION_LEADER_DAILY_FUNDS: f64 = 3.0;

// --- Start of game -------------------------------------------------------

pub const STARTING_FUNDS: f64 = 300.0;
/// Influence a faction starts with on its home planet.
pub const STARTING_HOME_INFLUENCE: f64 = 60.0;

/// How often AI factions consider a new order.
pub const AI_THINK_INTERVAL_DAYS: u32 = 5;
/// Funds an AI keeps in reserve.
pub const AI_RESERVE: f64 = 40.0;
/// An AI issues one extra order per think for every this much in funds.
pub const AI_FUNDS_PER_EXTRA_ORDER: f64 = 400.0;
pub const AI_MAX_ORDERS_PER_THINK: usize = 3;
