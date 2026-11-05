//! Live RabbitMQ tests: run when `AMQP_URL` is set (CI starts a rabbitmq
//! service), otherwise print a note and pass. Each test uses its own
//! exclusive queue, deleted with the connection.
#![allow(clippy::useless_conversion)] // `.into()` bridges &str / ShortString across lapin 2-4

#[cfg(feature = "lapin-2")]
use lapin2 as lapin;
#[cfg(feature = "lapin-3")]
use lapin3 as lapin;
#[cfg(feature = "lapin-4")]
use lapin4 as lapin;

use lapin::options::{
    BasicAckOptions, BasicGetOptions, BasicNackOptions, BasicPublishOptions, BasicRejectOptions,
    ConfirmSelectOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Connection, ConnectionProperties};
use vld_lapin::{LapinChannel, VldLapinError};

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
        pub retries: i64 => vld::number().int().min(0).max(10),
    }
}

const INVALID: &[u8] = br#"{"event":"","retries":99}"#;

async fn connect() -> Option<Connection> {
    let url = match std::env::var("AMQP_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => {
            eprintln!("note: AMQP_URL not set, skipping live rabbitmq test");
            return None;
        }
    };
    Some(
        Connection::connect(&url, ConnectionProperties::default())
            .await
            .expect("connect to AMQP_URL"),
    )
}

/// A confirming channel and a fresh exclusive queue on `conn`.
async fn channel(conn: &Connection, queue: Option<&str>) -> (LapinChannel, String) {
    let ch = LapinChannel::new(conn.create_channel().await.unwrap());
    ch.confirm_select(ConfirmSelectOptions::default())
        .await
        .unwrap();
    let name = match queue {
        Some(q) => q.to_string(),
        None => {
            let opts = QueueDeclareOptions {
                exclusive: true,
                ..Default::default()
            };
            let q = ch.queue_declare("", opts, FieldTable::default()).await;
            q.unwrap().name().to_string()
        }
    };
    (ch, name)
}

/// Publish bytes as-is (no validation) and wait for the broker confirm.
async fn publish_raw(ch: &LapinChannel, queue: &str, payload: &[u8]) {
    ch.basic_publish(
        "".into(),
        queue.into(),
        BasicPublishOptions::default(),
        payload,
        BasicProperties::default(),
    )
    .await
    .unwrap()
    .await
    .unwrap();
}

async fn get(ch: &LapinChannel, queue: &str) -> Option<lapin::message::BasicGetMessage> {
    ch.basic_get(queue, BasicGetOptions::default())
        .await
        .unwrap()
}

fn issue_paths(err: &VldLapinError) -> Vec<String> {
    match err {
        VldLapinError::Validation(e) => e
            .issues
            .iter()
            .map(|i| i.path.iter().map(|s| s.to_string()).collect())
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_validates_and_ack_decode_round_trips() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;

    let evt = EventSchema {
        event: "signup".into(),
        retries: 1,
    };
    ch.publish(
        "",
        &q,
        BasicPublishOptions::default(),
        BasicProperties::default(),
        &evt,
    )
    .await
    .unwrap();
    let msg = get(&ch, &q).await.expect("message");
    let got: EventSchema = ch
        .ack_decode_get(&msg, BasicAckOptions::default())
        .await
        .unwrap();
    assert_eq!((got.event.as_str(), got.retries), ("signup", 1));
    assert!(get(&ch, &q).await.is_none(), "acked message is gone");

    // An invalid value is rejected before anything is sent.
    let bad = EventSchema {
        event: String::new(),
        retries: 1,
    };
    let err = ch
        .publish(
            "",
            &q,
            BasicPublishOptions::default(),
            BasicProperties::default(),
            &bad,
        )
        .await
        .unwrap_err();
    assert_eq!(issue_paths(&err), [".event"]);
    assert!(get(&ch, &q).await.is_none(), "nothing was published");
}

#[tokio::test(flavor = "multi_thread")]
async fn nack_decode_drops_invalid_and_leaves_valid_unsettled() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;
    let drop_it = BasicNackOptions {
        requeue: false,
        ..Default::default()
    };

    publish_raw(&ch, &q, INVALID).await;
    let msg = get(&ch, &q).await.expect("message");
    let err = ch
        .nack_decode_get::<EventSchema>(&msg, drop_it)
        .await
        .unwrap_err();
    assert_eq!(issue_paths(&err), [".event", ".retries"]);
    assert!(get(&ch, &q).await.is_none(), "nacked without requeue");

    // Valid: returned, not settled — the caller acks after processing.
    publish_raw(&ch, &q, br#"{"event":"ok","retries":0}"#).await;
    let msg = get(&ch, &q).await.expect("message");
    let got: EventSchema = ch.nack_decode_get(&msg, drop_it).await.unwrap();
    assert_eq!(got.event, "ok");
    msg.ack(BasicAckOptions::default()).await.unwrap();
    assert!(get(&ch, &q).await.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn nack_decode_with_requeue_redelivers() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;

    publish_raw(&ch, &q, INVALID).await;
    let msg = get(&ch, &q).await.expect("message");
    let requeue = BasicNackOptions {
        requeue: true,
        ..Default::default()
    };
    assert!(ch
        .nack_decode_get::<EventSchema>(&msg, requeue)
        .await
        .is_err());
    let again = get(&ch, &q).await.expect("requeued");
    assert!(again.delivery.redelivered);
    assert_eq!(again.delivery.data, INVALID);
}

#[tokio::test(flavor = "multi_thread")]
async fn reject_decode_drops_malformed_json() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;

    publish_raw(&ch, &q, b"not json").await;
    let msg = get(&ch, &q).await.expect("message");
    let err = ch
        .reject_decode_get::<EventSchema>(&msg, BasicRejectOptions { requeue: false })
        .await
        .unwrap_err();
    assert!(matches!(err, VldLapinError::Deserialization(_)), "{err:?}");
    assert!(get(&ch, &q).await.is_none(), "rejected without requeue");
}

/// `ack_or_nack_decode` settles both ways: valid acked, invalid nacked.
#[tokio::test(flavor = "multi_thread")]
async fn ack_or_nack_decode_settles_every_delivery() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;
    let drop_it = BasicNackOptions {
        requeue: false,
        ..Default::default()
    };

    publish_raw(&ch, &q, INVALID).await;
    let msg = get(&ch, &q).await.expect("message");
    let err = ch
        .ack_or_nack_decode::<EventSchema>(&msg, BasicAckOptions::default(), drop_it)
        .await
        .unwrap_err();
    assert_eq!(issue_paths(&err), [".event", ".retries"]);

    publish_raw(&ch, &q, br#"{"event":"ok","retries":1}"#).await;
    let msg = get(&ch, &q).await.expect("message");
    let got: EventSchema = ch
        .ack_or_nack_decode(&msg, BasicAckOptions::default(), drop_it)
        .await
        .unwrap();
    assert_eq!(got.retries, 1);

    // Both settled: closing the channel redelivers nothing.
    ch.close(200, "bye".into()).await.unwrap();
    let (ch2, _) = channel(&conn, Some(&q)).await;
    assert!(get(&ch2, &q).await.is_none());
}

/// `ack_decode` only acks valid messages: an invalid one stays unacked and is
/// redelivered once its channel closes.
#[tokio::test(flavor = "multi_thread")]
async fn ack_decode_leaves_invalid_unsettled() {
    let Some(conn) = connect().await else { return };
    let (ch, q) = channel(&conn, None).await;

    publish_raw(&ch, &q, INVALID).await;
    let msg = get(&ch, &q).await.expect("message");
    assert!(ch
        .ack_decode_get::<EventSchema>(&msg, BasicAckOptions::default())
        .await
        .is_err());
    ch.close(200, "bye".into()).await.unwrap();

    let (ch2, _) = channel(&conn, Some(&q)).await;
    let again = get(&ch2, &q)
        .await
        .expect("redelivered after channel close");
    assert!(again.delivery.redelivered);
}
