//! Stories: events validated and published, fetched with `basic_get`, decoded;
//! publisher confirms; failures as typed errors.

use super::common::broker::Settle;
use super::common::*;
use super::vld_lapin::prelude::*;
use std::error::Error as _;

/// Given an `events` queue declared through the wrapper,
/// when a valid event is published, fetched with `basic_get` and settled with
/// `ack_or_nack_decode`,
/// then the broker receives the validated JSON on the default exchange, the
/// queue counts it, every decode helper gives the event back, the delivery is
/// acked, and the queue is then empty.
#[tokio::test(flavor = "multi_thread")]
async fn event_is_published_fetched_and_acked() {
    let s = setup().await;
    let ch = &s.channel;
    let q = ch
        .queue_declare("events", Default::default(), Default::default())
        .await
        .unwrap();
    assert_eq!((q.name().as_str(), q.message_count()), ("events", 0));

    let e = evt("user.created", 1);
    let confirmation = ch
        .publish("", "events", Default::default(), Default::default(), &e)
        .await
        .unwrap();
    assert!(
        matches!(confirmation, Confirmation::NotRequested),
        "{confirmation:?}"
    );
    let q = ch
        .queue_declare("events", Default::default(), Default::default())
        .await
        .unwrap();
    assert_eq!(q.message_count(), 1);
    assert_eq!(
        s.broker.state().published,
        [(String::new(), "events".to_string(), payload(&e))]
    );

    let msg = ch
        .basic_get("events", Default::default())
        .await
        .unwrap()
        .expect("a message");
    assert_eq!(ch.decode_get::<Evt>(&msg).unwrap(), e);
    assert_eq!(ch.decode_delivery::<Evt>(&msg).unwrap(), e);
    assert_eq!(ch.decode_bytes::<Evt>(&msg.data).unwrap(), e);
    let got: Evt = ch
        .ack_or_nack_decode(&msg, Default::default(), Default::default())
        .await
        .unwrap();
    assert_eq!(got, e);
    sync(ch).await;
    assert_eq!(s.broker.state().settled, [Settle::Ack { tag: 1 }]);
    assert!(
        ch.basic_get("events", Default::default())
            .await
            .unwrap()
            .is_none()
    );
}

/// A value that refuses to serialize (but has a schema).
#[derive(Debug)]
struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}
impl vld::schema::VldParse for Unserializable {
    fn vld_parse_value(_: &serde_json::Value) -> Result<Self, vld::error::VldError> {
        Ok(Self)
    }
}

/// Given an invalid event (empty name, too many retries) and a value that
/// cannot be serialized,
/// when they are published,
/// then the first fails with both issues and the second with a
/// `Serialization` error, and the broker receives nothing.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_events_are_never_published() {
    let s = setup().await;
    let ch = &s.channel;
    ch.queue_declare("events", Default::default(), Default::default())
        .await
        .unwrap();
    assert_eq!(
        issues(
            ch.publish(
                "",
                "events",
                Default::default(),
                Default::default(),
                &evt("", 99)
            )
            .await
        ),
        pairs(&[(".event", "too_small"), (".retries", "too_big")])
    );
    let err = ch
        .publish(
            "",
            "events",
            Default::default(),
            Default::default(),
            &Unserializable,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, VldLapinError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Serialization error: nope");
    assert!(err.source().is_none());
    sync(ch).await;
    assert!(s.broker.state().published.is_empty());
}

/// Given a channel in publisher-confirm mode (native `confirm_select`),
/// when events are published while the broker acks, then while it nacks,
/// then `publish` returns the broker's confirmation each time.
#[tokio::test(flavor = "multi_thread")]
async fn publish_awaits_the_publisher_confirmation() {
    let s = setup().await;
    let ch = &s.channel;
    ch.confirm_select(Default::default()).await.unwrap();
    let e = evt("order.paid", 0);
    let c = ch
        .publish("", "orders", Default::default(), Default::default(), &e)
        .await
        .unwrap();
    assert!(matches!(c, Confirmation::Ack(None)), "{c:?}");
    s.broker.state().nack_publishes = true;
    let c = ch
        .publish("", "orders", Default::default(), Default::default(), &e)
        .await
        .unwrap();
    assert!(matches!(c, Confirmation::Nack(None)), "{c:?}");
    assert_eq!(s.broker.state().published.len(), 2);
}

/// Given payloads that are not JSON or break the schema,
/// when they are decoded,
/// then non-JSON is a `Deserialization` error and schema failures list every
/// bad path; each error's text names its kind and only a validation error has
/// a source.
#[tokio::test(flavor = "multi_thread")]
async fn decode_errors_are_typed() {
    let s = setup().await;
    let ch = &s.channel;
    let err = ch.decode_bytes::<Evt>(b"{").unwrap_err();
    assert!(matches!(err, VldLapinError::Deserialization(_)), "{err:?}");
    assert!(
        err.to_string().starts_with("Deserialization error: "),
        "{err}"
    );
    assert!(err.source().is_none());
    let bad = ch.decode_bytes::<Evt>(br#"{"event":"","retries":99}"#);
    assert_eq!(
        issues(bad),
        pairs(&[(".event", "too_small"), (".retries", "too_big")])
    );
    let err = ch.decode_bytes::<Evt>(br#"{"retries":1}"#).unwrap_err();
    assert!(err.to_string().starts_with("Validation error: "), "{err}");
    assert!(err.source().is_some());
    let err: VldLapinError =
        vld::error::VldError::single(vld::error::IssueCode::MissingField, "required").into();
    assert_eq!(issues::<()>(Err(err)), pairs(&[("", "missing_field")]));
}

/// Given a channel that the application has closed,
/// when the wrapper is used to declare, fetch and publish,
/// then each call fails with a `Lapin` error whose source is lapin's error;
/// `into_inner` gives the closed native channel back.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::useless_conversion)] // `&str` on lapin 2/3, `ShortString` on 4
async fn lapin_failures_are_typed() {
    let s = setup().await;
    let ch = s.channel;
    ch.close(200, "bye".into()).await.unwrap();
    let err = ch
        .basic_get("events", Default::default())
        .await
        .unwrap_err();
    assert!(matches!(err, VldLapinError::Lapin(_)), "{err:?}");
    assert!(err.to_string().starts_with("Lapin error: "), "{err}");
    assert!(err.source().is_some());
    let err = ch
        .queue_declare("events", Default::default(), Default::default())
        .await
        .unwrap_err();
    assert!(matches!(err, VldLapinError::Lapin(_)), "{err:?}");
    let err = ch
        .publish(
            "",
            "events",
            Default::default(),
            Default::default(),
            &evt("a", 0),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, VldLapinError::Lapin(_)), "{err:?}");
    let native = ch.into_inner();
    assert!(!native.status().connected());
    drop(s.conn);
}
