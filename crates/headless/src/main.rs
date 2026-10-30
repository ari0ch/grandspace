//! Headless runner: lets AI factions play for centuries in seconds and prints
//! a history, so we can see whether the influence rules produce an
//! interesting galaxy before polishing any UI.
//!
//! ```text
//! grandspace-headless --seed 42 --years 200 --report-every 25
//! ```

use std::process::ExitCode;
use std::time::Instant;

use grandspace_sim::balance::DAYS_PER_YEAR;
use grandspace_sim::*;

struct Args {
    seed: u64,
    years: u32,
    report_every: u32,
    factions_per_power: usize,
    systems: usize,
    regions: usize,
    chronicle: bool,
}

fn usage() -> &'static str {
    "grandspace-headless [--seed N] [--years N] [--report-every N] \
     [--factions-per-power N] [--systems N] [--regions N] [--no-chronicle]"
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        seed: 1,
        years: 200,
        report_every: 25,
        factions_per_power: 3,
        systems: 40,
        regions: 5,
        chronicle: true,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "--no-chronicle" {
            a.chronicle = false;
            continue;
        }
        if flag == "-h" || flag == "--help" {
            return Err(usage().to_string());
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{}", usage()))?;
        let num = |v: &str| {
            v.parse::<u64>()
                .map_err(|_| format!("bad number for {flag}: {v}"))
        };
        match flag.as_str() {
            "--seed" => a.seed = num(&value)?,
            "--years" => a.years = num(&value)? as u32,
            "--report-every" => a.report_every = (num(&value)? as u32).max(1),
            "--factions-per-power" => a.factions_per_power = (num(&value)? as usize).clamp(1, 6),
            "--systems" => a.systems = (num(&value)? as usize).max(4),
            "--regions" => a.regions = (num(&value)? as usize).max(1),
            _ => return Err(format!("unknown flag {flag}\n{}", usage())),
        }
    }
    Ok(a)
}

fn faction_label(sim: &Sim, f: FactionId) -> String {
    let fac = sim.faction(f);
    format!("{} ({})", fac.name, fac.power)
}

fn report(sim: &Sim) {
    let controlled = sim.controlled_counts();
    let buildings = sim.buildings_per_faction();
    let totals = sim.influence_totals();
    let contested = sim
        .control
        .iter()
        .filter(|c| **c == ControlState::Contested)
        .count();
    let unclaimed = sim
        .control
        .iter()
        .filter(|c| **c == ControlState::Unclaimed)
        .count();

    println!("\n=== {} ===", sim.date());
    println!(
        "{:<34} {:>5} {:>6} {:>8} {:>8} {:>6} {:>5} {:>4} {:>4}",
        "faction", "ctrl", "bldgs", "infl", "funds", "net/d", "heat", "sys", "reg"
    );
    let mut order: Vec<usize> = (0..sim.factions.len()).collect();
    order.sort_by(|&a, &b| totals[b].total_cmp(&totals[a]));
    for i in order {
        let f = FactionId(i as u32);
        let sys = sim.system_leaders.iter().filter(|l| **l == Some(f)).count();
        let reg = sim.region_leaders.iter().filter(|l| **l == Some(f)).count();
        println!(
            "{:<34} {:>5} {:>6} {:>8.0} {:>8.0} {:>6.1} {:>5.0} {:>4} {:>4}",
            faction_label(sim, f),
            controlled[i],
            buildings[i],
            totals[i],
            sim.factions[i].funds,
            sim.net_income(f),
            sim.factions[i].heat,
            sys,
            reg
        );
    }
    let mut by_power = [0usize; 3];
    for (i, c) in controlled.iter().enumerate() {
        let p = Power::ALL
            .iter()
            .position(|&x| x == sim.factions[i].power)
            .unwrap();
        by_power[p] += c;
    }
    println!(
        "planets: {} controlled (Mafia {}, Church {}, Unions {}), {} contested, {} unclaimed",
        controlled.iter().sum::<usize>(),
        by_power[0],
        by_power[1],
        by_power[2],
        contested,
        unclaimed
    );
}

fn chronicle_line(sim: &Sim, e: &Event) -> Option<String> {
    let name = |f: FactionId| sim.faction(f).name.clone();
    let planet = |p: PlanetId| sim.galaxy.planet(p).name.clone();
    let date = GameDate::from_day(e.day);
    let text = match &e.kind {
        EventKind::RegionLeaderChanged { region, from, to } => {
            let r = &sim.galaxy.regions[region.idx()].name;
            match (from, to) {
                (_, Some(t)) => format!("{} now dominates the {r}", name(*t)),
                (Some(f), None) => format!("{} loses its grip on the {r}", name(*f)),
                (None, None) => return None,
            }
        }
        EventKind::Crackdown {
            faction,
            region,
            buildings_lost,
        } => format!(
            "Government crackdown on {} in the {}: {} holdings raided",
            name(*faction),
            sim.galaxy.regions[region.idx()].name,
            buildings_lost
        ),
        EventKind::Destroyed {
            by: Some(by),
            victim,
            planet: p,
            kind,
        } if *kind == BuildingKind::Relay => format!(
            "{} sabotages {}'s ansible relay at {}",
            name(*by),
            name(*victim),
            planet(*p)
        ),
        _ => return None,
    };
    Some(format!("  {date}  {text}"))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    let config = SimConfig {
        seed: args.seed,
        galaxy: GalaxyConfig {
            systems: args.systems,
            regions: args.regions,
            ..GalaxyConfig::default()
        },
        factions_per_power: args.factions_per_power,
    };
    let mut sim = Sim::new(config);
    println!(
        "Grandspace headless | seed {} | {} systems, {} planets, {} regions, {} factions",
        args.seed,
        sim.galaxy.systems.len(),
        sim.galaxy.planets.len(),
        sim.galaxy.regions.len(),
        sim.factions.len()
    );
    for f in &sim.factions {
        let home = sim.galaxy.planet(f.home);
        println!(
            "  {:<32} {:<7} home {} ({}, {})",
            f.name,
            f.power.name(),
            home.name,
            home.race,
            home.race.temperament()
        );
    }

    let started = Instant::now();
    let mut last_seq = 0;
    let mut since_report = 0;
    report(&sim);
    for year in 1..=args.years {
        sim.run_days(DAYS_PER_YEAR);
        since_report += 1;
        if args.chronicle {
            let lines: Vec<String> = sim
                .events_since(last_seq)
                .filter_map(|e| chronicle_line(&sim, e))
                .collect();
            for l in lines {
                println!("{l}");
            }
        }
        last_seq = sim.events.back().map(|e| e.seq).unwrap_or(last_seq);
        if since_report == args.report_every || year == args.years {
            report(&sim);
            since_report = 0;
        }
    }
    let s = &sim.stats;
    println!(
        "\n{} years in {:.2}s | built {} | sabotage {} ok / {} failed | crackdowns {} | \
         interceptions {} | control changes {} | abandoned {}",
        args.years,
        started.elapsed().as_secs_f64(),
        s.buildings_built,
        s.sabotages,
        s.sabotages_failed,
        s.crackdowns,
        s.interceptions,
        s.control_changes,
        s.abandoned
    );
    ExitCode::SUCCESS
}
