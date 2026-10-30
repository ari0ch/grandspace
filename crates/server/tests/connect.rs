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

/// Minimal HTTP GET over a raw socket, so tests need no HTTP client crate.
async fn http_get(addr: std::net::SocketAddr, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut out = String::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_string(&mut out))
        .await
        .expect("http response")
        .unwrap();
    out
}

#[tokio::test]
async fn serves_health_check_and_web_client() {
    let dir = std::env::temp_dir().join(format!("grandspace-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<title>grandspace-test-page</title>",
    )
    .unwrap();
    let server = spawn_local(ServerConfig {
        start_paused: true,
        web_dir: Some(dir.clone()),
        ..ServerConfig::default()
    })
    .unwrap();

    let health = http_get(server.addr, "/healthz").await;
    assert!(health.starts_with("HTTP/1.1 200"), "{health}");
    assert!(health.ends_with("ok"));

    let index = http_get(server.addr, "/").await;
    assert!(index.contains("grandspace-test-page"), "{index}");
    // Unknown paths fall back to the client page.
    let other = http_get(server.addr, "/some/deep/link").await;
    assert!(other.contains("grandspace-test-page"), "{other}");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn locked_clock_refuses_pause() {
    let server = spawn_local(ServerConfig {
        locked_clock: true,
        ..ServerConfig::default()
    })
    .unwrap();
    let (mut ws, _) = tokio_tungstenite::connect_async(server.url())
        .await
        .unwrap();
    ws.send(Message::text(encode(&ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        faction: None,
    })))
    .await
    .unwrap();
    assert!(matches!(next_msg(&mut ws).await, ServerMsg::Welcome { .. }));
    assert!(matches!(
        next_msg(&mut ws).await,
        ServerMsg::Clock { locked: true, .. }
    ));
    ws.send(Message::text(encode(&ClientMsg::Pause)))
        .await
        .unwrap();
    // State updates keep arriving; the refusal is among them.
    let mut refused = false;
    for _ in 0..20 {
        if let ServerMsg::Error { message } = next_msg(&mut ws).await {
            assert!(message.contains("clock"));
            refused = true;
            break;
        }
    }
    assert!(refused, "pause should be refused when the clock is locked");
}
