//! Stories: readings published through the client and decoded from
//! incoming publish packets.

use crate::issues::{of, pairs};
use rumqttc::{AsyncClient, Publish, QoS, Request};
use vld_data::rumqttc::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct Reading {
        pub sensor: String => vld::string().min(1),
        pub celsius: f64 => vld::number().min(-50.0).max(150.0),
    }
}

fn reading(sensor: &str, celsius: f64) -> Reading {
    Reading {
        sensor: sensor.into(),
        celsius,
    }
}

#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldMqttError>) -> Vec<(String, String)> {
    match r {
        Err(VldMqttError::Validation(e)) => of(&e),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[track_caller]
fn sent(rx: &flume::Receiver<Request>) -> Publish {
    match rx.try_recv() {
        Ok(Request::Publish(p)) => p,
        other => panic!("expected a publish request, got {other:?}"),
    }
}

/// Given a client and a valid reading,
/// when it is sent with `publish` and `try_publish`,
/// then the event loop receives a publish with the topic, QoS, retain flag
/// and the JSON payload, which a subscriber decodes back with
/// `decode_publish`.
#[tokio::test]
async fn valid_readings_are_published_as_json() {
    let (tx, rx) = flume::bounded(4);
    let client = AsyncClient::from_senders(tx);

    publish(
        &client,
        "sensors/1",
        QoS::AtLeastOnce,
        true,
        &reading("s1", 21.5),
    )
    .await
    .unwrap();
    let p = sent(&rx);
    assert_eq!(
        (p.topic.as_str(), p.qos, p.retain),
        ("sensors/1", QoS::AtLeastOnce, true)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&p.payload).unwrap(),
        serde_json::json!({"sensor": "s1", "celsius": 21.5})
    );
    assert_eq!(decode_publish::<Reading>(&p).unwrap(), reading("s1", 21.5));

    try_publish(
        &client,
        "sensors/2",
        QoS::AtMostOnce,
        false,
        &reading("s2", -3.0),
    )
    .unwrap();
    let p = sent(&rx);
    assert_eq!((p.topic.as_str(), p.retain), ("sensors/2", false));
    assert_eq!(decode_publish::<Reading>(&p).unwrap(), reading("s2", -3.0));
}

/// Given invalid readings, a bad topic and a full request channel,
/// when they are published,
/// then an invalid reading is a validation error and nothing is queued,
/// while topic and channel failures are `Client` errors.
#[tokio::test]
async fn invalid_readings_never_reach_the_event_loop() {
    let (tx, rx) = flume::bounded(1);
    let client = AsyncClient::from_senders(tx);

    assert_eq!(
        issues(
            publish(
                &client,
                "sensors/1",
                QoS::AtLeastOnce,
                false,
                &reading("", 200.0)
            )
            .await
        ),
        pairs(&[(".sensor", "too_small"), (".celsius", "too_big")])
    );
    assert_eq!(
        issues(try_publish(
            &client,
            "sensors/1",
            QoS::AtLeastOnce,
            false,
            &reading("s", -60.0)
        )),
        pairs(&[(".celsius", "too_small")])
    );
    assert!(rx.is_empty());

    let err = publish(
        &client,
        "sensors/#/x",
        QoS::AtLeastOnce,
        false,
        &reading("s", 1.0),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, VldMqttError::Client(_)), "{err:?}");

    try_publish(&client, "a", QoS::AtMostOnce, false, &reading("s", 1.0)).unwrap();
    let err = try_publish(&client, "b", QoS::AtMostOnce, false, &reading("s", 2.0)).unwrap_err();
    assert!(matches!(err, VldMqttError::Client(_)), "{err:?}");
    assert!(std::error::Error::source(&err).is_some());
}

/// Given incoming publish packets from other clients,
/// when they are decoded,
/// then a payload that is not JSON is a `Deserialization` error and a JSON
/// payload that breaks the schema names its fields; `encode` / `decode`
/// work on raw bytes the same way.
#[test]
fn foreign_payloads_are_checked_on_receive() {
    let p = Publish::new("sensors/9", QoS::AtLeastOnce, "21.5 C");
    let err = decode_publish::<Reading>(&p).unwrap_err();
    assert!(matches!(err, VldMqttError::Deserialization(_)), "{err:?}");

    let p = Publish::new("sensors/9", QoS::AtLeastOnce, r#"{"sensor":"s9"}"#);
    assert_eq!(
        issues(decode_publish::<Reading>(&p)),
        pairs(&[(".celsius", "missing_field")])
    );

    let bytes = encode(&reading("s3", 0.0)).unwrap();
    assert_eq!(decode::<Reading>(&bytes).unwrap(), reading("s3", 0.0));
    assert_eq!(
        issues(encode(&reading("", 0.0))),
        pairs(&[(".sensor", "too_small")])
    );
}
