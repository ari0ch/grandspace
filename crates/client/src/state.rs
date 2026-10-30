//! Client-side game state: the last thing the server told us, plus what the
//! player has selected. The client never simulates anything itself.

use std::collections::VecDeque;

use bevy::prelude::*;
use grandspace_protocol::*;
use grandspace_sim::{BuildingKind, Power};

use crate::net::{Net, NetEvent};

pub const LOG_LINES: usize = 8;

#[derive(Resource, Default)]
pub struct Game {
    pub galaxy: Option<WireGalaxy>,
    pub me: Option<u32>,
    pub view: Option<WireView>,
    pub log: VecDeque<String>,
    pub selected_system: Option<u32>,
    pub selected_planet: Option<u32>,
    pub speed: f32,
    pub paused: bool,
    pub notice: Option<String>,
    pub disconnected: Option<String>,
    /// Something changed that the UI should show.
    pub dirty: bool,
}

impl Game {
    pub fn my_power(&self) -> Option<Power> {
        let g = self.galaxy.as_ref()?;
        let me = self.me?;
        g.factions.get(me as usize).map(|f| f.power.into())
    }

    pub fn faction_name(&self, id: u32) -> String {
        match (&self.galaxy, self.me) {
            (_, Some(me)) if me == id => "You".to_string(),
            (Some(g), _) => g
                .factions
                .get(id as usize)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("Faction {id}")),
            _ => format!("Faction {id}"),
        }
    }

    pub fn faction_power(&self, id: u32) -> Option<Power> {
        self.galaxy
            .as_ref()?
            .factions
            .get(id as usize)
            .map(|f| f.power.into())
    }

    pub fn building_name(&self, owner: u32, kind: WireBuilding) -> &'static str {
        let power = self.faction_power(owner).unwrap_or(Power::Mafia);
        BuildingKind::from(kind).name(power)
    }

    fn push_log(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > LOG_LINES {
            self.log.pop_front();
        }
    }
}

/// Apply everything the server sent since last frame.
pub fn receive(net: Res<Net>, mut game: ResMut<Game>) {
    for event in net.drain() {
        match event {
            NetEvent::Msg(msg) => match *msg {
                ServerMsg::Welcome {
                    faction, galaxy, ..
                } => {
                    let home = galaxy.factions[faction as usize].home;
                    let home_system = galaxy.planets[home as usize].system;
                    game.me = Some(faction);
                    game.selected_system = Some(home_system);
                    game.selected_planet = Some(home);
                    game.galaxy = Some(galaxy);
                }
                ServerMsg::State { view, events } => {
                    for e in events {
                        game.push_log(format!("{}  {}", e.date, e.text));
                    }
                    game.view = Some(view);
                }
                ServerMsg::OrderAccepted { arrives_day, .. } => {
                    let today = game.view.as_ref().map(|v| v.day).unwrap_or(arrives_day);
                    let wait = arrives_day.saturating_sub(today);
                    game.notice = Some(if wait <= 1 {
                        "Order sent over your relay: it lands tomorrow.".to_string()
                    } else {
                        format!("Order sent by courier: it arrives in {wait} days.")
                    });
                }
                ServerMsg::OrderRejected { reason } => {
                    game.notice = Some(format!("Order refused: {reason}."));
                }
                ServerMsg::Clock {
                    days_per_second,
                    paused,
                } => {
                    game.speed = days_per_second;
                    game.paused = paused;
                }
                ServerMsg::Error { message } => {
                    game.notice = Some(format!("Server: {message}"));
                }
            },
            NetEvent::Disconnected(reason) => {
                game.disconnected = Some(reason);
            }
        }
        game.dirty = true;
    }
}
