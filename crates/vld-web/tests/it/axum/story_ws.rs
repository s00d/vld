//! Stories of an axum WebSocket endpoint validating messages with
//! `vld_web::axum::ws`.

use axum::Router;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::any;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message as ClientMessage;
use vld_web::axum::ws::{decode, encode, error_message};

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct Chat {
        pub room: String => vld::string().min(1),
        pub text: String => vld::string().min(1).max(10),
    }
}

/// `(path, code)` of every issue in an error body (checking its shape).
fn issues(text: &str) -> Vec<(String, String)> {
    let body: Value = serde_json::from_str(text).unwrap();
    assert_eq!(body["error"], "Validation failed");
    body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            assert!(i["message"].is_string(), "{i}");
            (
                i["path"].as_str().unwrap().to_string(),
                i["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// Given axum `Message`s,
/// when text, binary, control, invalid and malformed frames are decoded and
/// values encoded,
/// then the results match the tungstenite module's rules.
#[test]
fn axum_messages_decode_encode_and_answer() {
    let chat: Chat = decode(&Message::text(r#"{"room":"a","text":"hi"}"#))
        .unwrap()
        .unwrap();
    assert_eq!((chat.room.as_str(), chat.text.as_str()), ("a", "hi"));
    let chat: Chat = decode(&Message::binary(br#"{"room":"b","text":"yo"}"#.to_vec()))
        .unwrap()
        .unwrap();
    assert_eq!(chat.room, "b");
    for control in [
        Message::Ping(vec![1].into()),
        Message::Pong(Default::default()),
        Message::Close(None),
        Message::Close(Some(CloseFrame {
            code: 1000,
            reason: "bye".into(),
        })),
    ] {
        assert!(decode::<Chat>(&control).unwrap().is_none(), "{control:?}");
    }

    let err = decode::<Chat>(&Message::text(r#"{"room":""}"#)).unwrap_err();
    let reply = error_message(&err);
    assert!(matches!(reply, Message::Text(_)));
    assert_eq!(
        issues(reply.to_text().unwrap()),
        pairs(&[(".room", "too_small"), (".text", "missing_field")])
    );
    let err = decode::<Chat>(&Message::text("not json")).unwrap_err();
    assert_eq!(
        issues(error_message(&err).to_text().unwrap()),
        pairs(&[("", "parse_error")])
    );

    let frame = encode(&Chat {
        room: "a".into(),
        text: "hello".into(),
    })
    .unwrap();
    let sent: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
    assert_eq!(sent, json!({"room": "a", "text": "hello"}));
    let err = encode(&Chat {
        room: "a".into(),
        text: "far too long".into(),
    })
    .unwrap_err();
    assert_eq!(
        issues(error_message(&err).to_text().unwrap()),
        pairs(&[(".text", "too_big")])
    );
}

async fn chat(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle)
}

/// Echo each valid chat message as `room: text`, answer invalid ones with
/// the error frame.
async fn handle(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        let reply = match decode::<Chat>(&msg) {
            Ok(Some(chat)) => Message::text(format!("{}: {}", chat.room, chat.text)),
            Ok(None) => continue,
            Err(err) => error_message(&err),
        };
        if socket.send(reply).await.is_err() {
            break;
        }
    }
}

/// Given a chat endpoint served by axum on a local port,
/// when a client upgrades and sends a valid then an invalid message,
/// then it reads the echo and then the 422 error body, on the same socket.
#[tokio::test]
async fn a_chat_endpoint_answers_over_a_real_socket() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/ws", any(chat));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();

    client
        .send(ClientMessage::text(r#"{"room":"general","text":"hi"}"#))
        .await
        .unwrap();
    let reply = client.next().await.unwrap().unwrap();
    assert_eq!(reply.to_text().unwrap(), "general: hi");

    client
        .send(ClientMessage::text(r#"{"room":"general","text":""}"#))
        .await
        .unwrap();
    let reply = client.next().await.unwrap().unwrap();
    assert_eq!(
        issues(reply.to_text().unwrap()),
        pairs(&[(".text", "too_small")])
    );

    client.close(None).await.unwrap();
    server.abort();
}
