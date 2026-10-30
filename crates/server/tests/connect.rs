//! End-to-end: start a local server, connect over a real WebSocket, join,
//! receive state and place an order.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use grandspace_protocol::*;
use grandspace_server::{spawn_local, ServerConfig};
use tokio_tungstenite::tungstenite::Message;

async fn next_msg<S>(ws: &mut S) -> ServerMsg
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("server went quiet")
            .expect("connection closed")
            .expect("websocket error");
        if let Message::Text(t) = frame {
            return decode_server(&t).expect("valid server message");
        }
    }
}

#[tokio::test]
async fn join_receive_state_and_order() {
    let server = spawn_local(ServerConfig {
        start_paused: true,
        ..ServerConfig::default()
    })
    .expect("server starts");
    let (mut ws, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .expect("connects");

    ws.send(Message::text(encode(&ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        faction: Some(2),
    })))
    .await
    .unwrap();

    let (faction, home) = match next_msg(&mut ws).await {
        ServerMsg::Welcome {
            faction, galaxy, ..
        } => (faction, galaxy.factions[faction as usize].home),
        other => panic!("expected welcome, got {other:?}"),
    };
    assert_eq!(faction, 2);
    assert!(matches!(
        next_msg(&mut ws).await,
        ServerMsg::Clock { paused: true, .. }
    ));
    let funds_before = match next_msg(&mut ws).await {
        ServerMsg::State { view, .. } => {
            assert_eq!(view.me, 2);
            view.funds
        }
        other => panic!("expected state, got {other:?}"),
    };

    ws.send(Message::text(encode(&ClientMsg::Order {
        action: WireAction::Campaign { planet: home },
    })))
    .await
    .unwrap();
    assert!(matches!(
        next_msg(&mut ws).await,
        ServerMsg::OrderAccepted { .. }
    ));
    match next_msg(&mut ws).await {
        ServerMsg::State { view, .. } => {
            assert!(view.funds < funds_before, "the order was paid for");
            assert_eq!(view.orders.len(), 1);
        }
        other => panic!("expected state, got {other:?}"),
    }

    // A second player can't take the same faction.
    let (mut ws2, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    ws2.send(Message::text(encode(&ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        faction: Some(2),
    })))
    .await
    .unwrap();
    match next_msg(&mut ws2).await {
        ServerMsg::Welcome { faction, .. } => assert_ne!(faction, 2),
        other => panic!("expected welcome, got {other:?}"),
    }
}

#[tokio::test]
async fn bad_version_is_refused() {
    let server = spawn_local(ServerConfig {
        start_paused: true,
        ..ServerConfig::default()
    })
    .unwrap();
    let (mut ws, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    ws.send(Message::text(encode(&ClientMsg::Hello {
        version: 999,
        faction: None,
    })))
    .await
    .unwrap();
    assert!(matches!(next_msg(&mut ws).await, ServerMsg::Error { .. }));
}
