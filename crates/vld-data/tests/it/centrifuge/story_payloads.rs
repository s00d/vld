//! Stories: chat messages decoded from publications, published into a
//! channel and sent as RPCs.

use crate::issues::{of, pairs};
use centrifuge::{Client, Publication, RpcResult};
use vld_data::centrifuge::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct ChatMessage {
        pub text: String => vld::string().min(1).max(20),
    }
}

fn chat(text: &str) -> ChatMessage {
    ChatMessage { text: text.into() }
}

#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldCentrifugeError>) -> Vec<(String, String)> {
    match r {
        Err(VldCentrifugeError::Validation(e)) => of(&e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn publication(data: &[u8]) -> Publication {
    let mut p = Publication::default();
    p.data = data.to_vec().into();
    p
}

fn rpc_result(data: &[u8]) -> RpcResult {
    let mut r = RpcResult::default();
    r.data = data.to_vec().into();
    r
}

/// Given publications and RPC results as the client delivers them,
/// when they are decoded,
/// then the payload `encode` produces decodes back, a JSON payload that
/// breaks the schema names its fields, and a non-JSON one is a
/// `Deserialization` error.
#[test]
fn received_payloads_are_validated() {
    let bytes = encode(&chat("hi")).unwrap();
    assert_eq!(&bytes[..], br#"{"text":"hi"}"#);
    assert_eq!(
        decode_publication::<ChatMessage>(&publication(&bytes)).unwrap(),
        chat("hi")
    );
    assert_eq!(
        decode_rpc::<ChatMessage>(&rpc_result(&bytes)).unwrap(),
        chat("hi")
    );
    assert_eq!(decode::<ChatMessage>(&bytes).unwrap(), chat("hi"));

    assert_eq!(
        issues(decode_publication::<ChatMessage>(&publication(
            br#"{"text":""}"#
        ))),
        pairs(&[(".text", "too_small")])
    );
    assert_eq!(
        issues(decode_rpc::<ChatMessage>(&rpc_result(br#"{"msg":"hi"}"#))),
        pairs(&[(".text", "missing_field")])
    );
    let err = decode_publication::<ChatMessage>(&publication(b"hi")).unwrap_err();
    assert!(
        matches!(err, VldCentrifugeError::Deserialization(_)),
        "{err:?}"
    );
    assert_eq!(
        issues(encode(&chat(&"x".repeat(21)))),
        pairs(&[(".text", "too_big")])
    );
}

/// Given a subscription that is not subscribed and a closed client,
/// when values are published or sent as RPCs,
/// then an invalid value is a validation error (the client is never
/// called), and a valid one reaches the client, which reports why it
/// cannot deliver as a `Centrifuge` error.
#[tokio::test]
async fn publish_and_rpc_validate_before_calling_the_client() {
    let client = Client::builder("ws://127.0.0.1:1/connection/websocket")
        .build()
        .unwrap();
    let sub = client.new_subscription("chat:lobby").build().unwrap();

    assert_eq!(
        issues(publish(&sub, &chat("")).await),
        pairs(&[(".text", "too_small")])
    );
    let err = publish(&sub, &chat("hi")).await.unwrap_err();
    assert!(
        matches!(
            err,
            VldCentrifugeError::Centrifuge(centrifuge::Error::SubscriptionUnsubscribed)
        ),
        "{err:?}"
    );

    client.close().await;
    assert_eq!(
        issues(client_publish(&client, "chat:lobby", &chat("")).await),
        pairs(&[(".text", "too_small")])
    );
    let err = client_publish(&client, "chat:lobby", &chat("hi"))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            VldCentrifugeError::Centrifuge(centrifuge::Error::ClientClosed)
        ),
        "{err:?}"
    );

    assert_eq!(
        issues(rpc::<_, ChatMessage>(&client, "echo", &chat("")).await),
        pairs(&[(".text", "too_small")])
    );
    let err = rpc::<_, ChatMessage>(&client, "echo", &chat("hi"))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            VldCentrifugeError::Centrifuge(centrifuge::Error::ClientClosed)
        ),
        "{err:?}"
    );
    assert!(std::error::Error::source(&err).is_some());
}
