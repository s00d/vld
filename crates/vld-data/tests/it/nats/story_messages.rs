//! Stories: orders published and received, a request / reply service, and
//! messages from other publishers checked on receive.

use super::server::FakeNats;
use crate::issues::{of, pairs};
use futures_util::StreamExt;
use vld_data::nats::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct Order {
        pub id: String => vld::string().min(1),
        pub qty: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct Add {
        pub a: i64 => vld::number().int(),
        pub b: i64 => vld::number().int().min(0),
    }
}

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct Sum {
        pub sum: i64 => vld::number().int(),
    }
}

fn order(id: &str, qty: i64) -> Order {
    Order { id: id.into(), qty }
}

#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldNatsError>) -> Vec<(String, String)> {
    match r {
        Err(VldNatsError::Validation(e)) => of(&e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn message(payload: &[u8], reply: Option<&str>) -> async_nats::Message {
    async_nats::Message {
        subject: "orders".into(),
        reply: reply.map(Into::into),
        payload: payload.to_vec().into(),
        headers: None,
        status: None,
        description: None,
        length: payload.len(),
    }
}

/// Given a client subscribed to `orders`,
/// when a valid order, then an invalid one, then another valid one are
/// published,
/// then the subscriber receives the two valid orders in order and decodes
/// them; the invalid one is a validation error and never reaches the server.
#[tokio::test]
async fn orders_are_validated_before_publish() {
    let server = FakeNats::start();
    let client = async_nats::connect(&server.url).await.unwrap();
    let mut orders = client.subscribe("orders").await.unwrap();

    publish(&client, "orders", &order("o-1", 2)).await.unwrap();
    assert_eq!(
        issues(publish(&client, "orders", &order("", 0)).await),
        pairs(&[(".id", "too_small"), (".qty", "too_small")])
    );
    publish(&client, "orders", &order("o-2", 1)).await.unwrap();

    let first = orders.next().await.unwrap();
    assert_eq!(&first.payload[..], br#"{"id":"o-1","qty":2}"#);
    assert_eq!(decode::<Order>(&first).unwrap(), order("o-1", 2));
    let second = orders.next().await.unwrap();
    assert_eq!(decode::<Order>(&second).unwrap(), order("o-2", 1));

    let subjects: Vec<_> = server.published().into_iter().map(|p| p.0).collect();
    assert_eq!(subjects, ["orders", "orders"]);
}

/// Given a `math.add` service that answers with `respond`,
/// when a caller sends `request`s,
/// then a valid request gets the validated sum, an invalid request fails
/// before it is sent, and a reply that breaks the response schema is a
/// validation error for the caller.
#[tokio::test]
async fn requests_and_replies_are_validated_both_ways() {
    let server = FakeNats::start();
    let client = async_nats::connect(&server.url).await.unwrap();

    let mut calls = client.subscribe("math.add").await.unwrap();
    let service = client.clone();
    tokio::spawn(async move {
        while let Some(msg) = calls.next().await {
            let add: Add = decode(&msg).unwrap();
            if add.a == 0 {
                // A broken service: the reply does not match `Sum`.
                let reply = msg.reply.clone().unwrap();
                service
                    .publish(reply, r#"{"sum":"x"}"#.into())
                    .await
                    .unwrap();
            } else {
                respond(&service, &msg, &Sum { sum: add.a + add.b })
                    .await
                    .unwrap();
            }
        }
    });

    let sum: Sum = request(&client, "math.add", &Add { a: 2, b: 3 })
        .await
        .unwrap();
    assert_eq!(sum, Sum { sum: 5 });

    assert_eq!(
        issues(request::<_, Sum>(&client, "math.add", &Add { a: 1, b: -1 }).await),
        pairs(&[(".b", "too_small")])
    );
    assert_eq!(
        issues(request::<_, Sum>(&client, "math.add", &Add { a: 0, b: 1 }).await),
        pairs(&[(".sum", "invalid_type")])
    );

    let requests = server
        .published()
        .into_iter()
        .filter(|p| p.0 == "math.add")
        .count();
    assert_eq!(requests, 2);
}

/// Given messages from other publishers,
/// when they are decoded or answered,
/// then a payload that is not JSON is a `Deserialization` error, a JSON one
/// that breaks the schema names its fields, and `respond` to a message
/// without a reply subject is `NoReply` (checked after the value).
#[tokio::test]
async fn foreign_messages_are_checked_on_receive() {
    let err = decode::<Order>(&message(b"o-1 x2", None)).unwrap_err();
    assert!(matches!(err, VldNatsError::Deserialization(_)), "{err:?}");
    assert_eq!(
        issues(decode::<Order>(&message(br#"{"id":"o-1","qty":0}"#, None))),
        pairs(&[(".qty", "too_small")])
    );
    assert_eq!(
        decode_bytes::<Order>(&encode(&order("o-3", 3)).unwrap()).unwrap(),
        order("o-3", 3)
    );

    let server = FakeNats::start();
    let client = async_nats::connect(&server.url).await.unwrap();
    let published = message(br#"{"id":"o-1","qty":1}"#, None);
    let err = respond(&client, &published, &Sum { sum: 1 })
        .await
        .unwrap_err();
    assert!(matches!(err, VldNatsError::NoReply), "{err:?}");
    assert!(server.published().is_empty());
}
