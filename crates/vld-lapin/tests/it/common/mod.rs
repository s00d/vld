//! Helpers shared by every module of the `it` test binary.
#![allow(dead_code)]

pub mod broker;

#[cfg(feature = "lapin-2")]
pub use lapin2 as lapin;
#[cfg(feature = "lapin-3")]
pub use lapin3 as lapin;
#[cfg(feature = "lapin-4")]
pub use lapin4 as lapin;

#[cfg(feature = "lapin-4")]
pub use lapin::Confirmation;
#[cfg(not(feature = "lapin-4"))]
pub use lapin::publisher_confirm::Confirmation;

use broker::FakeBroker;
use vld_lapin::{LapinChannel, VldLapinError};

vld::schema! {
    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct Evt {
        pub event: String => vld::string().min(1),
        pub retries: i64 => vld::number().int().min(0).max(10),
    }
}

pub fn evt(event: &str, retries: i64) -> Evt {
    Evt {
        event: event.into(),
        retries,
    }
}

/// How a valid event is published: the validated JSON value (keys sorted).
pub fn payload(e: &Evt) -> Vec<u8> {
    format!(r#"{{"event":"{}","retries":{}}}"#, e.event, e.retries).into_bytes()
}

/// A broker plus a wrapped channel connected to it (the connection is kept
/// alive alongside).
pub struct Setup {
    pub broker: FakeBroker,
    pub conn: lapin::Connection,
    pub channel: LapinChannel,
}

pub async fn setup() -> Setup {
    let broker = FakeBroker::start();
    let conn = lapin::Connection::connect(&broker.uri, lapin::ConnectionProperties::default())
        .await
        .unwrap();
    let channel = LapinChannel::new(conn.create_channel().await.unwrap());
    Setup {
        broker,
        conn,
        channel,
    }
}

/// A round trip on the channel: once it returns, the broker has processed
/// every frame sent before it (settlements have no reply of their own).
pub async fn sync(channel: &LapinChannel) {
    channel
        .queue_declare("sync", Default::default(), Default::default())
        .await
        .unwrap();
}

/// Publish raw bytes to `queue` through the native channel (another
/// producer, no validation).
#[allow(clippy::useless_conversion)] // `&str` on lapin 2/3, `ShortString` on 4
pub async fn publish_raw(channel: &LapinChannel, queue: &str, body: &[u8]) {
    let _ = channel
        .basic_publish(
            "".into(),
            queue.into(),
            Default::default(),
            body,
            Default::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();
}

/// `(path, code)` of every issue of a `Validation` error (panics otherwise).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldLapinError>) -> Vec<(String, String)> {
    match r {
        Err(VldLapinError::Validation(e)) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs for comparing with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}
