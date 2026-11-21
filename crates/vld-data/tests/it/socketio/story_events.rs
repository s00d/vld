//! Stories: payloads decoded by position, events reaching a handler only
//! when valid, emits and emit-with-ack validated both ways, and server
//! callbacks answered with validated values.

use super::server::FakeSocketIo;
use crate::issues::{of, pairs};
use serde_json::json;
use socketio_next::{Client, ClientBuilder, Event, Payload, TransportType};
use std::time::Duration;
use tokio::sync::mpsc;
use vld::schema::{VldParse, VldSchema};
use vld_data::socketio::prelude::*;

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
        pub sum: i64 => vld::number().int().min(0),
    }
}

/// All arguments of an event as positive ids (a schema over the argument
/// array, for `decode_args`).
#[derive(Debug, PartialEq)]
struct Ids(Vec<i64>);

impl VldParse for Ids {
    fn vld_parse_value(value: &serde_json::Value) -> Result<Self, vld::error::VldError> {
        vld::array(vld::number().int().min(1))
            .parse_value(value)
            .map(Ids)
    }
}

fn order(id: &str, qty: i64) -> Order {
    Order { id: id.into(), qty }
}

#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldSocketIoError>) -> Vec<(String, String)> {
    match r {
        Err(VldSocketIoError::Validation(e)) => of(&e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn builder(server: &FakeSocketIo) -> ClientBuilder {
    ClientBuilder::new(server.url.as_str())
        .transport_type(TransportType::Websocket)
        .reconnection(false)
        .no_proxy()
}

/// The ack id and the rest of an EVENT packet `2[/nsp,]<id>[…]`.
fn ack_id(packet: &str) -> (u64, &str) {
    let body = packet.strip_prefix('2').expect("an EVENT packet");
    let digits = body.find('[').unwrap();
    (body[..digits].parse().unwrap(), &body[digits..])
}

/// Given ack replies and event arguments as socketio-next hands them over,
/// when they are decoded by position or as one array,
/// then valid arguments come back typed, invalid ones as issues, a missing
/// or binary argument as `NoArgument`, a buffer inside an argument as its
/// placeholder object (which fails the schema).
#[test]
fn payloads_are_decoded_by_position() {
    let payload = Payload::Text(vec![
        json!({"id": "o-1", "qty": 2}),
        json!({"id": "", "qty": 0}),
    ]);
    assert_eq!(
        decode_payload_arg::<Order>(&payload, 0).unwrap(),
        order("o-1", 2)
    );
    assert_eq!(
        issues(decode_payload_arg::<Order>(&payload, 1)),
        pairs(&[(".id", "too_small"), (".qty", "too_small")])
    );
    assert!(matches!(
        decode_payload_arg::<Order>(&payload, 2),
        Err(VldSocketIoError::NoArgument(2))
    ));

    let ids = Payload::Text(vec![json!(1), json!(2), json!(3)]);
    assert_eq!(
        decode_payload_args::<Ids>(&ids).unwrap(),
        Ids(vec![1, 2, 3])
    );
    assert_eq!(
        issues(decode_payload_args::<Ids>(&Payload::Text(vec![
            json!(1),
            json!(0)
        ]))),
        pairs(&[("[1]", "too_small")])
    );

    let binary = Payload::Binary(vec![1, 2].into());
    assert!(matches!(
        decode_payload_arg::<Order>(&binary, 0),
        Err(VldSocketIoError::NoArgument(0))
    ));
    let mixed = Payload::Mixed {
        args: vec![Payload::placeholder(0)],
        attachments: vec![vec![1].into()],
    };
    assert!(matches!(
        decode_payload_arg::<Order>(&mixed, 0),
        Err(VldSocketIoError::Validation(_))
    ));
}

/// Given a client whose `order` handler is wrapped in `validated`, whose
/// `strict` handler uses `validated_or` to emit a `rejected` event, and a
/// raw `raw` handler,
/// when the server sends a valid order, an invalid one and another valid
/// one, then an invalid `strict` event, then `raw` events with several
/// arguments and with a binary attachment,
/// then the `order` handler sees only the two valid orders, the `strict`
/// callback reports the error back to the server, and the raw events decode
/// by position and as a whole; a buffer is read from the payload, not
/// validated.
#[tokio::test]
async fn handlers_run_only_with_valid_arguments() {
    let mut server = FakeSocketIo::start().await;
    let (orders, mut orders_rx) = mpsc::unbounded_channel();
    let (raw, mut raw_rx) = mpsc::unbounded_channel::<Event>();
    let _client = builder(&server)
        .on(
            "order",
            validated(move |o: Order, _client: Client| {
                let orders = orders.clone();
                async move { orders.send(o).unwrap() }
            }),
        )
        .on(
            "strict",
            validated_or(
                |_o: Order, _client: Client| async move { panic!("invalid order accepted") },
                |event: Event, client: Client, error: VldSocketIoError| async move {
                    let issues = match error {
                        VldSocketIoError::Validation(e) => e.issues.len(),
                        other => panic!("unexpected {other:?}"),
                    };
                    client
                        .emit("rejected", json!({"event": event.name(), "issues": issues}))
                        .unwrap();
                },
            ),
        )
        .on("raw", move |event: Event, _client: Client| {
            let raw = raw.clone();
            async move { raw.send(event).unwrap() }
        })
        .connect()
        .await
        .unwrap();

    server.send(r#"2["order",{"id":"o-1","qty":2}]"#);
    server.send(r#"2["order",{"id":"","qty":2}]"#);
    server.send(r#"2["order",{"id":"o-3","qty":1}]"#);
    assert_eq!(orders_rx.recv().await.unwrap(), order("o-1", 2));
    assert_eq!(orders_rx.recv().await.unwrap(), order("o-3", 1));

    server.send(r#"2["strict",{"id":"","qty":0}]"#);
    assert_eq!(
        server.recv().await,
        r#"2["rejected",{"event":"strict","issues":2}]"#
    );

    server.send(r#"2["raw",{"id":"o-4","qty":4},{"id":"x","qty":0}]"#);
    let event = raw_rx.recv().await.unwrap();
    assert_eq!(decode::<Order>(&event).unwrap(), order("o-4", 4));
    assert_eq!(
        issues(decode_arg::<Order>(&event, 1)),
        pairs(&[(".qty", "too_small")])
    );
    assert!(matches!(
        decode_arg::<Order>(&event, 2),
        Err(VldSocketIoError::NoArgument(2))
    ));
    assert_eq!(
        issues(decode_args::<Ids>(&event)),
        pairs(&[("[0]", "invalid_type"), ("[1]", "invalid_type")])
    );

    server.send(r#"51-["raw",{"id":"o-5","qty":1},{"_placeholder":true,"num":0}]"#);
    server.send_binary(&[7, 8]);
    let event = raw_rx.recv().await.unwrap();
    assert_eq!(decode::<Order>(&event).unwrap(), order("o-5", 1));
    assert!(matches!(
        decode_arg::<Order>(&event, 1),
        Err(VldSocketIoError::Validation(_))
    ));
    assert_eq!(&event.payload().binary()[0][..], &[7, 8]);
    assert_eq!(orders_rx.try_recv().ok(), None);
}

/// Given a connected client,
/// when it emits a valid order, an invalid one and another valid one, then
/// calls `emit_with_ack` with a valid request answered validly, an invalid
/// request, a request answered with an invalid sum, and one answered with
/// no arguments,
/// then the server receives only valid values, and each ack comes back as
/// the typed reply, a validation error or `NoArgument`.
#[tokio::test]
async fn emits_and_acks_are_validated_both_ways() {
    let mut server = FakeSocketIo::start().await;
    let client = builder(&server).connect().await.unwrap();

    emit(&client, "order", &order("o-1", 2)).unwrap();
    assert_eq!(
        issues(emit(&client, "order", &order("", 0))),
        pairs(&[(".id", "too_small"), (".qty", "too_small")])
    );
    emit(&client, "order", &order("o-2", 1)).unwrap();
    assert_eq!(server.recv().await, r#"2["order",{"id":"o-1","qty":2}]"#);
    assert_eq!(server.recv().await, r#"2["order",{"id":"o-2","qty":1}]"#);

    let timeout = Duration::from_secs(30);
    let call = |req: Add| {
        let client = client.clone();
        tokio::spawn(async move { emit_with_ack::<Add, Sum>(&client, "add", &req, timeout).await })
    };

    let ok = call(Add { a: 2, b: 3 });
    let packet = server.recv().await;
    let (id, args) = ack_id(&packet);
    assert_eq!(args, r#"["add",{"a":2,"b":3}]"#);
    server.send(&format!(r#"3{id}[{{"sum":5}}]"#));
    assert_eq!(ok.await.unwrap().unwrap(), Sum { sum: 5 });

    assert_eq!(
        issues(call(Add { a: 1, b: -1 }).await.unwrap()),
        pairs(&[(".b", "too_small")])
    );

    let bad_reply = call(Add { a: -3, b: 1 });
    let packet = server.recv().await;
    let (id, args) = ack_id(&packet);
    assert_eq!(args, r#"["add",{"a":-3,"b":1}]"#);
    server.send(&format!(r#"3{id}[{{"sum":-2}}]"#));
    assert_eq!(
        issues(bad_reply.await.unwrap()),
        pairs(&[(".sum", "too_small")])
    );

    let empty_reply = call(Add { a: 0, b: 0 });
    let packet = server.recv().await;
    let (id, _) = ack_id(&packet);
    server.send(&format!("3{id}[]"));
    assert!(matches!(
        empty_reply.await.unwrap(),
        Err(VldSocketIoError::NoArgument(0))
    ));
}

/// Given a client on namespace `/math` that answers `add` with `answer`,
/// `add-checked` with `answer_or` (acking an error value), and `manual` by
/// hand with `ack`,
/// when the server emits these events with callbacks (and `manual` once
/// without),
/// then valid requests are answered with the validated sum, an invalid
/// request or an invalid sum is acked with the error value, and `ack`
/// refuses an invalid value and an event without callback.
#[tokio::test]
async fn server_callbacks_are_answered_with_validated_values() {
    let mut server = FakeSocketIo::start().await;
    let (manual, mut manual_rx) = mpsc::unbounded_channel();
    let add = |a: Add, _client: Client| async move { Sum { sum: a.a + a.b } };
    let _client = builder(&server)
        .namespace("/math")
        .on("add", answer(add))
        .on(
            "add-checked",
            answer_or(
                add,
                |event: Event, _client: Client, error: VldSocketIoError| async move {
                    let kind = match error {
                        VldSocketIoError::Validation(_) => "validation",
                        _ => "other",
                    };
                    event.ack(json!({"error": kind})).unwrap();
                },
            ),
        )
        .on("manual", move |event: Event, _client: Client| {
            let manual = manual.clone();
            async move {
                let invalid = ack(&event, &Sum { sum: -1 });
                let valid = ack(&event, &Sum { sum: 1 });
                manual.send((invalid, valid)).unwrap();
            }
        })
        .connect()
        .await
        .unwrap();

    server.send(r#"2/math,5["add",{"a":2,"b":3}]"#);
    assert_eq!(server.recv().await, r#"3/math,5[{"sum":5}]"#);

    server.send(r#"2/math,6["add-checked",{"a":1,"b":-1}]"#);
    assert_eq!(server.recv().await, r#"3/math,6[{"error":"validation"}]"#);
    server.send(r#"2/math,7["add-checked",{"a":-5,"b":1}]"#);
    assert_eq!(server.recv().await, r#"3/math,7[{"error":"validation"}]"#);
    server.send(r#"2/math,8["add-checked",{"a":4,"b":4}]"#);
    assert_eq!(server.recv().await, r#"3/math,8[{"sum":8}]"#);

    server.send(r#"2/math,["manual"]"#);
    let (invalid, valid) = manual_rx.recv().await.unwrap();
    assert!(matches!(invalid, Err(VldSocketIoError::NoAck)));
    assert!(matches!(valid, Err(VldSocketIoError::NoAck)));

    server.send(r#"2/math,9["manual"]"#);
    let (invalid, valid) = manual_rx.recv().await.unwrap();
    assert_eq!(issues(invalid), pairs(&[(".sum", "too_small")]));
    valid.unwrap();
    assert_eq!(server.recv().await, r#"3/math,9[{"sum":1}]"#);
}
