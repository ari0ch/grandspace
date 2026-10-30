# Grandspace

A grand strategy game across many star systems where you never rule
anything officially. You lead a branch of one of three hidden powers (the
**Mafia**, the **Church** or the **Unions**) and compete for influence over
planets whose governors, premiers and emperors take the credit and the blame.

Think Crusader Kings' politics, played across a galaxy, from the shadows,
with the grit of The Expanse and a streak of Snow Crash satire.

**Status:** prototype 0.1, the influence sandbox. Characters, dynasties and
schemes come in 0.2.

## Play

```sh
cargo run --release -p grandspace-client            # single-player
cargo run --release -p grandspace-client -- --seed 42
```

Single-player starts a local server in the background and connects to it,
exactly as an online client would. The game starts paused.

- **Left-click** a star to select its system; pick a planet in the sidebar.
- **Right-drag** or **WASD** to pan, **scroll** to zoom, **Space** to pause.
- Build a foothold (Safehouse / Mission / Union Office) next to your
  holdings, then add influence and income buildings, run campaigns, sabotage
  rivals, place agents and bribe officials when the heat is on.

To play over a network:

```sh
cargo run --release -p grandspace-server -- --addr 0.0.0.0:7878 --seed 42
cargo run --release -p grandspace-client -- --connect ws://HOST:7878
```

Every faction nobody is playing is run by the AI.

### Watch the AI play for 200 years

```sh
cargo run --release -p grandspace-headless -- --seed 42 --years 200 --report-every 25
```

This prints a chronicle (region takeovers, crackdowns, sabotaged relays)
and a table every 25 years. It is the fastest way to see whether a rule
change makes the galaxy more interesting.

## How the prototype plays

| Rule | In the game |
| --- | --- |
| Holdings are shared | Every planet has a few building slots any faction can use, at most 2 per faction (more on your power base) |
| Control | You control a planet when your influence exceeds every other faction's **combined** |
| Cluster bonuses | Leading a system boosts your influence and income there; leading a region pays a daily stipend |
| Currencies | Mafia earn cash, the Church faith, the Unions labour |
| Ansible relays | Orders reach systems with your relay the next day; elsewhere couriers take 4 days per lane. Rival relays may read your orders, and can foil sabotage aimed at them |
| Intel | Exact numbers where you have holdings or agents; ±35% estimates elsewhere |
| Heat | Grows with the size of your network and with risky acts. Past 60 you are exposed and government crackdowns raid your holdings |
| Races | The devout Ostrani favour the Church, the transactional Merrow the Mafia, the industrial Haldane the Unions |

All tuning numbers live in [`crates/sim/src/balance.rs`](crates/sim/src/balance.rs).

## Layout

| Crate | What it is |
| --- | --- |
| `crates/sim` | The game: a deterministic, dependency-free simulation. One `tick()` is one day |
| `crates/headless` | Runs all-AI galaxies for centuries and prints their history |
| `crates/protocol` | JSON messages between server and clients |
| `crates/server` | Authoritative WebSocket server; also hosts single-player locally |
| `crates/client` | Thin Bevy 0.19 client: draws what the server sends, sends orders back |

The simulation never depends on the engine, so the same code runs in a
local single-player host, a dedicated server or the headless runner. See
[`docs/design.md`](docs/design.md) for the design decisions behind it.

## Licence

Code: [AGPL-3.0](LICENSE). Content (art, writing, setting): to be decided;
see [`CONTENT-LICENSE.md`](CONTENT-LICENSE.md).
