//! Standalone Grandspace server.
//!
//! ```text
//! grandspace-server --addr 0.0.0.0:7878 --seed 42 --speed 2
//! ```
//!
//! Clients connect with `grandspace-client --connect ws://HOST:7878`.

use std::process::ExitCode;

use grandspace_server::{serve, ServerConfig};
use grandspace_sim::{GalaxyConfig, SimConfig};

fn usage() -> &'static str {
    "grandspace-server [--addr HOST:PORT] [--seed N] [--speed DAYS_PER_SECOND] [--paused] \
     [--systems N] [--regions N] [--factions-per-power N]"
}

fn parse() -> Result<(String, ServerConfig), String> {
    let mut addr = "127.0.0.1:7878".to_string();
    let mut cfg = ServerConfig::default();
    let mut galaxy = GalaxyConfig::default();
    let mut sim = SimConfig::default();
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--paused" => {
                cfg.start_paused = true;
                continue;
            }
            "-h" | "--help" => return Err(usage().to_string()),
            _ => {}
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{}", usage()))?;
        let bad = || format!("bad value for {flag}: {value}");
        match flag.as_str() {
            "--addr" => addr = value.clone(),
            "--seed" => sim.seed = value.parse().map_err(|_| bad())?,
            "--speed" => cfg.days_per_second = value.parse().map_err(|_| bad())?,
            "--systems" => galaxy.systems = value.parse().map_err(|_| bad())?,
            "--regions" => galaxy.regions = value.parse().map_err(|_| bad())?,
            "--factions-per-power" => sim.factions_per_power = value.parse().map_err(|_| bad())?,
            _ => return Err(format!("unknown flag {flag}\n{}", usage())),
        }
    }
    sim.galaxy = galaxy;
    cfg.sim = sim;
    Ok((addr, cfg))
}

#[tokio::main]
async fn main() -> ExitCode {
    let (addr, config) = match parse() {
        Ok(x) => x,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Grandspace server on ws://{addr} | seed {} | {} days/second{}",
        config.sim.seed,
        config.days_per_second,
        if config.start_paused { " (paused)" } else { "" }
    );
    if let Err(e) = serve(listener, config).await {
        eprintln!("server stopped: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
