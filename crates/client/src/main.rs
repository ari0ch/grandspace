//! Grandspace thin client, for desktop and the browser.
//!
//! Desktop:
//!
//! ```text
//! grandspace-client                              # single-player: hosts a local server
//! grandspace-client --seed 42                    # single-player with a chosen galaxy
//! grandspace-client --connect wss://HOST/ws [--faction N]
//! ```
//!
//! Browser: the page connects to the server it was loaded from (`/ws`).
//! Add `?faction=N` to ask for a faction, or `?server=wss://HOST/ws` to
//! connect elsewhere.
//!
//! The client only draws what the server sends and forwards orders back.
//! Desktop single-player starts a server on a loopback port and connects to
//! it like any other client, so solo and online play share one code path.

mod map;
mod net;
mod palette;
mod state;
mod ui;

use bevy::prelude::*;

/// Where to connect, and which faction to ask for.
struct Target {
    url: String,
    faction: Option<u32>,
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use grandspace_server::{spawn_local, LocalServer, ServerConfig};
    use grandspace_sim::SimConfig;

    use super::Target;

    pub fn target() -> (Target, Option<LocalServer>) {
        let mut connect = None;
        let mut faction = None;
        let mut seed = 1;
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            let mut value = || match it.next() {
                Some(v) => v,
                None => fail(&format!("{flag} needs a value")),
            };
            match flag.as_str() {
                "--connect" => connect = Some(value()),
                "--faction" => {
                    faction = Some(value().parse().unwrap_or_else(|_| fail("bad --faction")))
                }
                "--seed" => seed = value().parse().unwrap_or_else(|_| fail("bad --seed")),
                "-h" | "--help" => {
                    fail("grandspace-client [--connect ws://HOST:PORT/ws] [--faction N] [--seed N]")
                }
                other => fail(&format!("unknown flag {other}")),
            }
        }
        match connect {
            Some(url) => (Target { url, faction }, None),
            None => {
                let server = spawn_local(ServerConfig {
                    sim: SimConfig {
                        seed,
                        ..SimConfig::default()
                    },
                    days_per_second: 4.0,
                    start_paused: true,
                    ..ServerConfig::default()
                })
                .unwrap_or_else(|e| fail(&format!("could not start the local server: {e}")));
                (
                    Target {
                        url: server.url(),
                        faction,
                    },
                    Some(server),
                )
            }
        }
    }

    fn fail(msg: &str) -> ! {
        eprintln!("{msg}");
        std::process::exit(2);
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::Target;

    /// Read the server address and faction from the page URL.
    pub fn target() -> Target {
        let location = web_sys::window().expect("no browser window").location();
        let host = location.host().unwrap_or_default();
        let secure = location.protocol().unwrap_or_default() == "https:";
        let search = location.search().unwrap_or_default();
        let mut url = format!("{}://{host}/ws", if secure { "wss" } else { "ws" });
        let mut faction = None;
        for pair in search.trim_start_matches('?').split('&') {
            match pair.split_once('=') {
                Some(("server", v)) if !v.is_empty() => url = v.to_string(),
                Some(("faction", v)) => faction = v.parse().ok(),
                _ => {}
            }
        }
        Target { url, faction }
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let (target, _local_server) = desktop::target();
    #[cfg(target_arch = "wasm32")]
    let target = web::target();

    // Until the server says otherwise: a local galaxy starts paused, a
    // shared online one keeps running.
    let paused = cfg!(not(target_arch = "wasm32"));

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Grandspace".into(),
                resolution: (1440, 900).into(),
                // In the browser, draw into the page's canvas and fill it.
                canvas: Some("#grandspace".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(palette::BACKGROUND))
        .insert_non_send(net::connect(target.url, target.faction))
        .insert_resource(state::Game {
            paused,
            dirty: true,
            ..default()
        })
        .insert_resource(map::CameraZoom(1.15))
        .add_systems(Startup, (map::spawn_camera, ui::spawn_panels))
        .add_systems(
            Update,
            (
                state::receive,
                map::spawn_labels,
                map::camera_controls,
                map::click_select,
                ui::handle_buttons,
                ui::button_hover,
                ui::keyboard,
                ui::rebuild,
                map::draw_map,
            )
                .chain(),
        )
        .run();
}
