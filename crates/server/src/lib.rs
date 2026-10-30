//! Authoritative Grandspace server.
//!
//! One task owns the [`Sim`] and advances it on a clock (decision 5: one
//! tick is one day). Each WebSocket connection claims a faction, sends
//! orders and receives only that faction's view. Factions nobody is playing
//! are run by the AI, so a galaxy is always alive.
//!
//! Single-player uses exactly the same path: [`spawn_local`] starts a server
//! on a loopback port in a background thread and the client connects to it
//! (decision 19: thin client from day one).

use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use grandspace_protocol::*;
use grandspace_sim::view::{FactionView, GalaxyInfo};
use grandspace_sim::{FactionId, Sim, SimConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

pub const MIN_SPEED: f32 = 0.25;
pub const MAX_SPEED: f32 = 64.0;
/// Recent events sent to a client when it joins.
const BACKLOG_EVENTS: usize = 30;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub sim: SimConfig,
    /// Simulated days per real second.
    pub days_per_second: f32,
    pub start_paused: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            sim: SimConfig::default(),
            days_per_second: 2.0,
            start_paused: false,
        }
    }
}

enum Command {
    Join {
        client: u64,
        faction: Option<u32>,
        tx: UnboundedSender<ServerMsg>,
    },
    Client {
        client: u64,
        msg: ClientMsg,
    },
    Leave {
        client: u64,
    },
}

/// Accept connections forever, running the galaxy alongside.
pub async fn serve(listener: TcpListener, config: ServerConfig) -> io::Result<()> {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    tokio::spawn(game_loop(config, cmd_rx));
    let mut next_client = 0u64;
    loop {
        let (stream, _peer) = listener.accept().await?;
        next_client += 1;
        tokio::spawn(handle_connection(stream, next_client, cmd_tx.clone()));
    }
}

/// A server running on a loopback port in a background thread, for
/// single-player.
pub struct LocalServer {
    pub addr: SocketAddr,
    _thread: std::thread::JoinHandle<()>,
}

impl LocalServer {
    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }
}

pub fn spawn_local(config: ServerConfig) -> io::Result<LocalServer> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let thread = std::thread::Builder::new()
        .name("grandspace-server".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to start the local server runtime");
            rt.block_on(async move {
                let listener =
                    TcpListener::from_std(listener).expect("failed to adopt the local listener");
                if let Err(e) = serve(listener, config).await {
                    eprintln!("local server stopped: {e}");
                }
            });
        })?;
    Ok(LocalServer {
        addr,
        _thread: thread,
    })
}

async fn handle_connection(stream: TcpStream, id: u64, cmd_tx: UnboundedSender<Command>) {
    let ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(_) => return,
    };
    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::text(encode(&msg))).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let mut joined = false;
    while let Some(Ok(frame)) = source.next().await {
        let text = match frame {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        match decode_client(&text) {
            Ok(ClientMsg::Hello { version, faction }) if !joined => {
                if version != PROTOCOL_VERSION {
                    let _ = tx.send(ServerMsg::Error {
                        message: format!(
                            "protocol version {version} not supported; server speaks {PROTOCOL_VERSION}"
                        ),
                    });
                    break;
                }
                joined = true;
                let _ = cmd_tx.send(Command::Join {
                    client: id,
                    faction,
                    tx: tx.clone(),
                });
            }
            Ok(msg) if joined => {
                let _ = cmd_tx.send(Command::Client { client: id, msg });
            }
            Ok(_) => {
                let _ = tx.send(ServerMsg::Error {
                    message: "send hello first".into(),
                });
            }
            Err(e) => {
                let _ = tx.send(ServerMsg::Error {
                    message: format!("bad message: {e}"),
                });
            }
        }
    }
    if joined {
        let _ = cmd_tx.send(Command::Leave { client: id });
    }
    drop(tx);
    let _ = writer.await;
}

struct Client {
    faction: FactionId,
    tx: UnboundedSender<ServerMsg>,
    last_seq: u64,
}

struct Game {
    sim: Sim,
    galaxy: WireGalaxy,
    clients: BTreeMap<u64, Client>,
    speed: f32,
    paused: bool,
}

fn period(speed: f32) -> Duration {
    Duration::from_secs_f32(1.0 / speed.clamp(MIN_SPEED, MAX_SPEED))
}

async fn game_loop(config: ServerConfig, mut rx: UnboundedReceiver<Command>) {
    let sim = Sim::new(config.sim.clone());
    let galaxy = WireGalaxy::from(&GalaxyInfo::from_sim(&sim));
    let mut game = Game {
        sim,
        galaxy,
        clients: BTreeMap::new(),
        speed: config.days_per_second.clamp(MIN_SPEED, MAX_SPEED),
        paused: config.start_paused,
    };
    let mut next_tick = Instant::now() + period(game.speed);
    loop {
        tokio::select! {
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { break };
                let was_paused = game.paused;
                game.handle(cmd);
                if was_paused && !game.paused {
                    next_tick = Instant::now() + period(game.speed);
                }
            }
            _ = tokio::time::sleep_until(next_tick), if !game.paused => {
                game.sim.tick();
                game.broadcast();
                next_tick += period(game.speed);
                let now = Instant::now();
                if next_tick < now {
                    // Fell behind (e.g. a slow machine): don't try to catch up.
                    next_tick = now + period(game.speed);
                }
            }
        }
    }
}

impl Game {
    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Join {
                client,
                faction,
                tx,
            } => self.join(client, faction, tx),
            Command::Leave { client } => {
                if let Some(c) = self.clients.remove(&client) {
                    let still_played = self.clients.values().any(|o| o.faction == c.faction);
                    if !still_played {
                        // The AI takes over again.
                        self.sim.set_human(c.faction, false);
                    }
                }
            }
            Command::Client { client, msg } => self.client_msg(client, msg),
        }
    }

    fn join(&mut self, client: u64, wanted: Option<u32>, tx: UnboundedSender<ServerMsg>) {
        let taken: Vec<FactionId> = self.clients.values().map(|c| c.faction).collect();
        let count = self.sim.factions.len() as u32;
        let faction = wanted
            .filter(|&w| w < count && !taken.contains(&FactionId(w)))
            .or_else(|| (0..count).find(|&i| !taken.contains(&FactionId(i))))
            .map(FactionId);
        let Some(faction) = faction else {
            let _ = tx.send(ServerMsg::Error {
                message: "every faction already has a player".into(),
            });
            return;
        };
        self.sim.set_human(faction, true);
        let _ = tx.send(ServerMsg::Welcome {
            version: PROTOCOL_VERSION,
            faction: faction.0,
            galaxy: self.galaxy.clone(),
        });
        let _ = tx.send(ServerMsg::Clock {
            days_per_second: self.speed,
            paused: self.paused,
        });
        let recent: Vec<WireEvent> = {
            let visible: Vec<_> = self
                .sim
                .events
                .iter()
                .filter(|e| e.visible_to(faction))
                .collect();
            let skip = visible.len().saturating_sub(BACKLOG_EVENTS);
            visible
                .into_iter()
                .skip(skip)
                .map(|e| describe_event(&self.sim, faction, e))
                .collect()
        };
        let _ = tx.send(ServerMsg::State {
            view: self.view_for(faction),
            events: recent,
        });
        let last_seq = self.sim.events.back().map(|e| e.seq).unwrap_or(0);
        self.clients.insert(
            client,
            Client {
                faction,
                tx,
                last_seq,
            },
        );
    }

    fn client_msg(&mut self, client: u64, msg: ClientMsg) {
        let Some(faction) = self.clients.get(&client).map(|c| c.faction) else {
            return;
        };
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::Order { action } => {
                let reply = match self.sim.issue(faction, action.into()) {
                    Ok(id) => {
                        let arrives_day = self
                            .sim
                            .orders
                            .iter()
                            .find(|o| o.id == id)
                            .map(|o| o.arrives)
                            .unwrap_or(self.sim.day);
                        ServerMsg::OrderAccepted { id, arrives_day }
                    }
                    Err(e) => ServerMsg::OrderRejected {
                        reason: e.to_string(),
                    },
                };
                self.send_to(client, reply);
                // Show the new balance and pending order straight away.
                let view = self.view_for(faction);
                self.send_to(
                    client,
                    ServerMsg::State {
                        view,
                        events: Vec::new(),
                    },
                );
            }
            ClientMsg::SetSpeed { days_per_second } => {
                if days_per_second.is_finite() {
                    self.speed = days_per_second.clamp(MIN_SPEED, MAX_SPEED);
                    self.broadcast_clock();
                }
            }
            ClientMsg::Pause => {
                self.paused = true;
                self.broadcast_clock();
            }
            ClientMsg::Resume => {
                self.paused = false;
                self.broadcast_clock();
            }
        }
    }

    fn view_for(&self, faction: FactionId) -> WireView {
        WireView::from_view(
            &FactionView::build(&self.sim, faction),
            self.sim.faction(faction).power,
        )
    }

    fn send_to(&mut self, client: u64, msg: ServerMsg) {
        let dead = match self.clients.get(&client) {
            Some(c) => c.tx.send(msg).is_err(),
            None => false,
        };
        if dead {
            self.handle(Command::Leave { client });
        }
    }

    fn broadcast(&mut self) {
        let latest = self.sim.events.back().map(|e| e.seq).unwrap_or(0);
        let mut dead = Vec::new();
        for (&id, client) in self.clients.iter_mut() {
            let events = self
                .sim
                .events_since(client.last_seq)
                .filter(|e| e.visible_to(client.faction))
                .map(|e| describe_event(&self.sim, client.faction, e))
                .collect();
            client.last_seq = latest;
            let view = WireView::from_view(
                &FactionView::build(&self.sim, client.faction),
                self.sim.faction(client.faction).power,
            );
            if client.tx.send(ServerMsg::State { view, events }).is_err() {
                dead.push(id);
            }
        }
        for id in dead {
            self.handle(Command::Leave { client: id });
        }
    }

    fn broadcast_clock(&mut self) {
        let msg = ServerMsg::Clock {
            days_per_second: self.speed,
            paused: self.paused,
        };
        let ids: Vec<u64> = self.clients.keys().copied().collect();
        for id in ids {
            self.send_to(id, msg.clone());
        }
    }
}
