//! Stories of a chat server validating WebSocket messages with
//! `vld_web::tungstenite`.

use serde::Serialize;
use serde_json::{Value, json};
use tungstenite::Message;
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::coding::CloseCode;
use vld_web::tungstenite::prelude::*;

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct Chat {
        pub room: String => vld::string().min(1),
        pub text: String => vld::string().min(1).max(10),
    }
}

/// `(path, code)` of every issue in an error frame (checking its shape).
fn issues(frame: &Message) -> Vec<(String, String)> {
    let body: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
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

/// Given a chat server reading client frames,
/// when clients send text and binary JSON, invalid and malformed messages,
/// and control frames,
/// then valid messages decode, control frames are `None`, and every
/// failure answers with a text frame holding the HTTP 422 body.
#[test]
fn a_chat_server_validates_incoming_frames() {
    let chat: Chat = decode(&Message::text(r#"{"room":"general","text":"hi"}"#))
        .unwrap()
        .unwrap();
    assert_eq!((chat.room.as_str(), chat.text.as_str()), ("general", "hi"));
    let chat: Chat = decode(&Message::binary(br#"{"room":"r","text":"yo"}"#.to_vec()))
        .unwrap()
        .unwrap();
    assert_eq!(chat.room, "r");

    for control in [
        Message::Ping(vec![1].into()),
        Message::Pong(Default::default()),
        Message::Close(None),
        Message::Close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "bye".into(),
        })),
    ] {
        assert!(decode::<Chat>(&control).unwrap().is_none(), "{control:?}");
    }

    let err = decode::<Chat>(&Message::text(r#"{"room":"","text":"far too long"}"#)).unwrap_err();
    let reply = error_message(&err);
    assert!(reply.is_text());
    assert_eq!(
        issues(&reply),
        pairs(&[(".room", "too_small"), (".text", "too_big")])
    );

    let err = decode::<Chat>(&Message::text("{")).unwrap_err();
    assert_eq!(issues(&error_message(&err)), pairs(&[("", "parse_error")]));
    let err = decode::<Chat>(&Message::binary(vec![0xFF, 0xFE])).unwrap_err();
    assert_eq!(issues(&error_message(&err)), pairs(&[("", "parse_error")]));
}

/// Given the server sending messages to clients,
/// when it encodes a valid and an invalid value,
/// then the valid one is a JSON text frame and the invalid one is refused
/// with the issues the client would have reported.
#[test]
fn a_chat_server_validates_outgoing_messages() {
    let frame = encode(&Chat {
        room: "general".into(),
        text: "hello".into(),
    })
    .unwrap();
    assert!(frame.is_text());
    let sent: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
    assert_eq!(sent, json!({"room": "general", "text": "hello"}));

    let err = encode(&Chat {
        room: String::new(),
        text: "hello".into(),
    })
    .unwrap_err();
    assert_eq!(
        issues(&error_message(&err)),
        pairs(&[(".room", "too_small")])
    );
}
