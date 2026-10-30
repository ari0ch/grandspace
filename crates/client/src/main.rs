//! Grandspace thin client.
//!
//! ```text
//! grandspace-client                         # single-player: hosts a local server
//! grandspace-client --seed 42               # single-player with a chosen galaxy
//! grandspace-client --connect ws://HOST:7878 [--faction N]
//! ```
//!
//! The client only draws what the server sends and forwards orders back.
//! Single-player starts a server on a loopback port and connects to it like
//! any other client, so solo and online play share one code path.

mod map;
mod net;
mod palette;
mod state;
mod ui;

use bevy::prelude::*;
use grandspace_server::{spawn_local, ServerConfig};
use grandspace_sim::SimConfig;

struct Args {
    connect: Option<String>,
    faction: Option<u32>,
    seed: u64,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        connect: None,
        faction: None,
        seed: 1,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--connect" => a.connect = Some(value()?),
            "--faction" => {
                a.faction = Some(value()?.parse().map_err(|_| "bad --faction".to_string())?)
            }
            "--seed" => a.seed = value()?.parse().map_err(|_| "bad --seed".to_string())?,
            "-h" | "--help" => {
                return Err(
                    "grandspace-client [--connect ws://HOST:PORT] [--faction N] [--seed N]"
                        .to_string(),
                )
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(a)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };

    // Keep the local server alive for as long as the window is open.
    let mut _local = None;
    let url = match args.connect {
        Some(url) => url,
        None => {
            let server = spawn_local(ServerConfig {
                sim: SimConfig {
                    seed: args.seed,
                    ..SimConfig::default()
                },
                days_per_second: 4.0,
                start_paused: true,
            })
            .expect("could not start the local server");
            let url = server.url();
            _local = Some(server);
            url
        }
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Grandspace".into(),
                resolution: (1440, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(palette::BACKGROUND))
        .insert_resource(net::connect(url, args.faction))
        .insert_resource(state::Game {
            paused: true,
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
