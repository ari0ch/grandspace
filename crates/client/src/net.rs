//! The client's connection to a server, on its own thread.
//!
//! Bevy systems never touch the socket: they push [`ClientMsg`]s into a
//! channel and drain [`NetEvent`]s once per frame.

use std::sync::mpsc;
use std::sync::Mutex;

use bevy::prelude::Resource;
use futures_util::{SinkExt, StreamExt};
use grandspace_protocol::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

pub enum NetEvent {
    Msg(Box<ServerMsg>),
    Disconnected(String),
}

#[derive(Resource)]
pub struct Net {
    tx: UnboundedSender<ClientMsg>,
    rx: Mutex<mpsc::Receiver<NetEvent>>,
}

impl Net {
    pub fn send(&self, msg: ClientMsg) {
        // If the connection is gone the UI already shows it; nothing to do.
        let _ = self.tx.send(msg);
    }

    /// Everything that arrived since the last call.
    pub fn drain(&self) -> Vec<NetEvent> {
        let rx = self.rx.lock().expect("net receiver poisoned");
        rx.try_iter().collect()
    }
}

pub fn connect(url: String, faction: Option<u32>) -> Net {
    let (out_tx, mut out_rx) = unbounded_channel::<ClientMsg>();
    let (in_tx, in_rx) = mpsc::channel::<NetEvent>();
    std::thread::Builder::new()
        .name("grandspace-net".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to start the network runtime");
            rt.block_on(async move {
                let ws = match tokio_tungstenite::connect_async(url.as_str()).await {
                    Ok((ws, _)) => ws,
                    Err(e) => {
                        let _ = in_tx.send(NetEvent::Disconnected(format!(
                            "could not connect to {url}: {e}"
                        )));
                        return;
                    }
                };
                let (mut sink, mut source) = ws.split();
                let hello = ClientMsg::Hello {
                    version: PROTOCOL_VERSION,
                    faction,
                };
                if sink.send(Message::text(encode(&hello))).await.is_err() {
                    let _ = in_tx.send(NetEvent::Disconnected("handshake failed".into()));
                    return;
                }
                let reason = loop {
                    tokio::select! {
                        out = out_rx.recv() => match out {
                            Some(msg) => {
                                if sink.send(Message::text(encode(&msg))).await.is_err() {
                                    break "lost connection to the server".to_string();
                                }
                            }
                            None => break "client closed".to_string(),
                        },
                        inc = source.next() => match inc {
                            Some(Ok(Message::Text(text))) => match decode_server(&text) {
                                Ok(msg) => {
                                    if in_tx.send(NetEvent::Msg(Box::new(msg))).is_err() {
                                        break "client closed".to_string();
                                    }
                                }
                                Err(e) => break format!("server sent something unreadable: {e}"),
                            },
                            Some(Ok(Message::Close(_))) | None => {
                                break "the server closed the connection".to_string()
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => break format!("connection error: {e}"),
                        },
                    }
                };
                let _ = in_tx.send(NetEvent::Disconnected(reason));
            });
        })
        .expect("failed to start the network thread");
    Net {
        tx: out_tx,
        rx: Mutex::new(in_rx),
    }
}
