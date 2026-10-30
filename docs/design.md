# Grandspace design notes

The living design document is kept outside the repo; this file records the
decisions the code depends on and what the prototype taught us.

## Decisions (30 Oct 2026)

| # | Decision | What it means |
| --- | --- | --- |
| 1 | Always hidden | The player is the power behind the throne and never holds official titles |
| 2 | Shared holdings | Planets are holdings any faction can build on; buildings raise the owner's influence |
| 3 | Control rule | A faction controls a planet when its influence exceeds all others' combined |
| 4 | Cluster bonuses | Leading a group of planets, a system or a region pays off |
| 5 | Clock | Tick-based; one tick is one day |
| 6 | Sandbox | Open-ended across hundreds of years; no win condition |
| 7 | Rivals | 2 to 4 factions per power, each spreading from its own power base |
| 8 | Races | Independent of factions; three to start (devout, transactional, industrial) |
| 9 | Warfare | Deferred; indirect only when added |
| 10 | Communication | Ansible relays are scarce buildings; couriers elsewhere; relay owners read traffic |
| 11 | Action economy | Each power spends its own currency: cash, faith, labour |
| 12 | Contested planets | The hot spots: best for smugglers, easiest to sway |
| 13 | Visibility | Exact influence only where you have holdings or agents |
| 14 | Exposure | Getting caught brings crackdowns and rivals ganging up |
| 15 | No external threat | Human politics only, for now |
| 16 | Tone | Expanse grit, Snow Crash satire in the writing |
| 17 | Galaxy | Procedural, with handcrafted key systems later |
| 18 | Open source | AGPL code; content licence to be decided |
| 19 | Thin client | Server-authoritative simulation from day one |
| 20 | Tech and look | Rust and Bevy; clean, light graphics |

## Architecture

```
            ┌────────────────────┐
            │  grandspace-sim    │  pure std, deterministic
            │  Sim::tick() = day │
            └─────────┬──────────┘
        ┌─────────────┼──────────────┐
        ▼             ▼              ▼
  headless runner   server ◄──── WebSocket JSON ────► Bevy client
                    (also hosts                        (thin: draws views,
                     single-player)                     sends orders)
```

- The server owns the only `Sim`. Clients get a `FactionView` that hides
  what their faction can't know (decision 13), so cheating clients learn
  nothing extra.
- Orders are intents with a travel time. That is a game rule (decision 10)
  and also what makes slow, asynchronous online play possible later.
- The simulation uses its own random number generator so a seed produces
  the same galaxy on every machine.

## What the headless runs taught us (0.1 balance pass)

Early versions snowballed: one faction controlled 129 of 144 planets by year
200. Three changes fixed it, and all of them fit the fiction:

1. **Holdings stay shared:** at most two buildings per faction per planet.
2. **Visibility scales with power:** heat drifts towards a level set by the
   size of your network, so past about 55 buildings you are exposed just by
   existing, and crackdowns trim you back.
3. **Bribes:** money can buy heat down, which gives the Mafia's cash a
   purpose and stops rich factions from hoarding.

The AI also had to learn to save up for good buildings instead of spamming
cheap outposts it couldn't afford to run.

Across 12 seeds, 200 years each, the three powers end with similar numbers
of controlled planets, and each power comes out ahead in some seeds. The
galaxy fills up by year 50 and then churns (around 50 control changes a
year); the late game is stable rather than dramatic. Characters,
succession and schemes (milestone 0.2) are the intended source of late-game
upheaval.

## Open questions

- Content licence (Creative Commons is the likely direction).
- Should relays form one network (as now) or need chains of relays?
- Mid- and late-game pressure beyond crackdowns: succession crises, schisms
  within a power, elections as scheduled set pieces.
- Browser client via WebAssembly.
