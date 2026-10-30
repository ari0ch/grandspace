//! The client's connection to a server.
//!
//! Uses `ewebsock`, which speaks WebSockets both natively and in the browser,
//! so desktop and web clients share this code. Bevy systems push
//! [`ClientMsg`]s and drain [`NetEvent`]s once per frame; on the web the
//! socket can't leave the main thread, so this is a non-send resource.

use std::cell::RefCell;

use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};
use grandspace_protocol::*;

pub enum NetEvent {
    Msg(Box<ServerMsg>),
    Disconnected(String),
}

pub struct Net {
    sender: Option<RefCell<WsSender>>,
    receiver: Option<WsReceiver>,
    faction: Option<u32>,
    /// Messages queued before the socket opened.
    pending: RefCell<Vec<ClientMsg>>,
    open: RefCell<bool>,
    failed: RefCell<Option<String>>,
}

impl Net {
    pub fn send(&self, msg: ClientMsg) {
        if *self.open.borrow() {
            if let Some(sender) = &self.sender {
                sender.borrow_mut().send(WsMessage::Text(encode(&msg)));
            }
        } else {
            self.pending.borrow_mut().push(msg);
        }
    }

    /// Everything that arrived since the last call.
    pub fn drain(&self) -> Vec<NetEvent> {
        let mut out = Vec::new();
        if let Some(reason) = self.failed.borrow_mut().take() {
            out.push(NetEvent::Disconnected(reason));
        }
        let Some(receiver) = &self.receiver else {
            return out;
        };
        while let Some(event) = receiver.try_recv() {
            match event {
                WsEvent::Opened => {
                    *self.open.borrow_mut() = true;
                    // Say hello first, then flush anything queued.
                    self.send(ClientMsg::Hello {
                        version: PROTOCOL_VERSION,
                        faction: self.faction,
                    });
                    let queued: Vec<ClientMsg> = self.pending.borrow_mut().drain(..).collect();
                    for msg in queued {
                        self.send(msg);
                    }
                }
                WsEvent::Message(WsMessage::Text(text)) => match decode_server(&text) {
                    Ok(msg) => out.push(NetEvent::Msg(Box::new(msg))),
                    Err(e) => out.push(NetEvent::Disconnected(format!(
                        "the server sent something unreadable: {e}"
                    ))),
                },
                WsEvent::Message(_) => {}
                WsEvent::Error(e) => {
                    *self.open.borrow_mut() = false;
                    out.push(NetEvent::Disconnected(format!("connection error: {e}")));
                }
                WsEvent::Closed => {
                    *self.open.borrow_mut() = false;
                    out.push(NetEvent::Disconnected(
                        "the server closed the connection".into(),
                    ));
                }
            }
        }
        out
    }
}

pub fn connect(url: String, faction: Option<u32>) -> Net {
    match ewebsock::connect(url.clone(), ewebsock::Options::default()) {
        Ok((sender, receiver)) => Net {
            sender: Some(RefCell::new(sender)),
            receiver: Some(receiver),
            faction,
            pending: RefCell::new(Vec::new()),
            open: RefCell::new(false),
            failed: RefCell::new(None),
        },
        Err(e) => Net {
            sender: None,
            receiver: None,
            faction,
            pending: RefCell::new(Vec::new()),
            open: RefCell::new(false),
            failed: RefCell::new(Some(format!("could not connect to {url}: {e}"))),
        },
    }
}
