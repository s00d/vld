//! # vld-lapin — lapin (RabbitMQ) integration for `vld`
//!
//! Wrap a channel: `let channel = LapinChannel::new(channel);`
//!
//! The wrapper adds:
//!
//! - publish helper: `publish`
//! - get/decode helpers: `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
//! - settle helpers: `ack_or_nack_decode` (ack when valid, nack when invalid),
//!   `ack_decode` (ack when valid — an invalid "poison" message stays unacked),
//!   `nack_decode` / `reject_decode` (nack/reject when invalid, so poison
//!   messages are not left unacked)
//! - the same for `basic_get` messages: `ack_decode_get`, `nack_decode_get`, `reject_decode_get`
//!
//! All other native `lapin::Channel` methods are still available through deref.
//!
//! Published values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). Features: `lapin-2` (default), `lapin-3`,
//! `lapin-4` — exactly one, matching your `lapin` dependency.
//!
//! ```no_run
//! use vld_lapin::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct EventSchema {
//!         pub event: String => vld::string().min(1),
//!     }
//! }
//!
//! async fn roundtrip(channel: &LapinChannel) -> Result<(), VldLapinError> {
//!     let event = EventSchema { event: "user.created".into() };
//!     channel
//!         .publish("", "events", Default::default(), Default::default(), &event)
//!         .await?;
//!
//!     if let Some(message) = channel.basic_get("events", Default::default()).await? {
//!         // Ack when valid, nack when invalid: every message is settled.
//!         let parsed: EventSchema = channel
//!             .ack_or_nack_decode(&message, Default::default(), Default::default())
//!             .await?;
//!         println!("{}", parsed.event);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/lapin>
#![warn(missing_docs)]

use std::fmt;
use std::ops::{Deref, DerefMut};

#[cfg(any(
    all(feature = "lapin-2", feature = "lapin-3"),
    all(feature = "lapin-2", feature = "lapin-4"),
    all(feature = "lapin-3", feature = "lapin-4")
))]
compile_error!("Enable exactly one lapin version feature: `lapin-2`, `lapin-3`, or `lapin-4`.");
#[cfg(not(any(feature = "lapin-2", feature = "lapin-3", feature = "lapin-4")))]
compile_error!("Enable one lapin version feature: `lapin-2`, `lapin-3`, or `lapin-4`.");

#[cfg(feature = "lapin-2")]
use lapin2 as lapin;
#[cfg(feature = "lapin-3")]
use lapin3 as lapin;
#[cfg(feature = "lapin-4")]
use lapin4 as lapin;

#[cfg(feature = "lapin-2")]
type LapinConfirmation = lapin::publisher_confirm::Confirmation;
#[cfg(feature = "lapin-3")]
type LapinConfirmation = lapin::publisher_confirm::Confirmation;
#[cfg(feature = "lapin-4")]
type LapinConfirmation = lapin::Confirmation;

pub use vld;

/// Lapin channel wrapper with validate+JSON behavior.
pub struct LapinChannel {
    inner: lapin::Channel,
}

impl LapinChannel {
    /// Wrap a channel.
    pub fn new(inner: lapin::Channel) -> Self {
        Self { inner }
    }

    /// Unwrap the inner channel.
    pub fn into_inner(self) -> lapin::Channel {
        self.inner
    }

    fn encode_payload<V>(value: &V) -> Result<Vec<u8>, VldLapinError>
    where
        V: serde::Serialize + vld::schema::VldParse,
    {
        let json =
            serde_json::to_value(value).map_err(|e| VldLapinError::Serialization(e.to_string()))?;
        <V as vld::schema::VldParse>::vld_parse_value(&json).map_err(VldLapinError::Validation)?;
        serde_json::to_vec(&json).map_err(|e| VldLapinError::Serialization(e.to_string()))
    }

    /// Decode + validate a JSON payload.
    pub fn decode_bytes<T>(&self, payload: &[u8]) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode(payload)
    }

    /// `lapin::Channel::queue_declare` taking a `&str` queue name.
    #[allow(clippy::useless_conversion)]
    pub async fn queue_declare(
        &self,
        queue: &str,
        options: lapin::options::QueueDeclareOptions,
        arguments: lapin::types::FieldTable,
    ) -> Result<lapin::Queue, VldLapinError> {
        self.inner
            .queue_declare(queue.into(), options, arguments)
            .await
            .map_err(VldLapinError::Lapin)
    }

    /// `lapin::Channel::basic_get` taking a `&str` queue name (the message is
    /// not decoded; see [`decode_get`](Self::decode_get) and the `*_decode_get` helpers).
    #[allow(clippy::useless_conversion)]
    pub async fn basic_get(
        &self,
        queue: &str,
        options: lapin::options::BasicGetOptions,
    ) -> Result<Option<lapin::message::BasicGetMessage>, VldLapinError> {
        self.inner
            .basic_get(queue.into(), options)
            .await
            .map_err(VldLapinError::Lapin)
    }

    /// Validate `value`, encode it as JSON, publish it and await the publisher
    /// confirmation.
    #[allow(clippy::useless_conversion)]
    pub async fn publish<V>(
        &self,
        exchange: &str,
        routing_key: &str,
        options: lapin::options::BasicPublishOptions,
        properties: lapin::BasicProperties,
        value: &V,
    ) -> Result<LapinConfirmation, VldLapinError>
    where
        V: serde::Serialize + vld::schema::VldParse,
    {
        let payload = Self::encode_payload(value)?;

        let confirm = self
            .inner
            .basic_publish(
                exchange.into(),
                routing_key.into(),
                options,
                &payload,
                properties,
            )
            .await
            .map_err(VldLapinError::Lapin)?;
        confirm.await.map_err(VldLapinError::Lapin)
    }

    /// Decode + validate a delivery's payload (does not settle it).
    pub fn decode_delivery<T>(
        &self,
        delivery: &lapin::message::Delivery,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode(&delivery.data)
    }

    /// Decode + validate a `basic_get` message's payload (does not settle it).
    pub fn decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode(&message.data)
    }

    /// Decode + validate, then **ack on success**.
    ///
    /// On a decode/validation error the delivery is left **unsettled** and the
    /// error is returned. Such a poison message stays unacked: it holds a
    /// prefetch (`basic_qos`) slot and is redelivered — to fail again — once the
    /// channel closes. Use [`ack_or_nack_decode`](Self::ack_or_nack_decode) (or
    /// [`nack_decode`](Self::nack_decode) / [`reject_decode`](Self::reject_decode))
    /// to settle it.
    pub async fn ack_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicAckOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let parsed = self.decode_delivery(delivery)?;
        delivery.ack(options).await?;
        Ok(parsed)
    }

    /// Decode + validate, then **ack when valid and nack when invalid** — every
    /// delivery is settled.
    ///
    /// - valid payload: acked with `ack`, the value is returned;
    /// - invalid payload: nacked with `nack` (`requeue: false` drops it or
    ///   dead-letters it), then the decode/validation error is returned (a nack
    ///   failure is returned instead).
    ///
    /// Takes a `basic_get` message too (it derefs to a delivery).
    pub async fn ack_or_nack_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        ack: lapin::options::BasicAckOptions,
        nack: lapin::options::BasicNackOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        let parsed = decode_or_settle(&delivery.data, || delivery.nack(nack)).await?;
        delivery.ack(ack).await?;
        Ok(parsed)
    }

    /// Decode + validate, and **nack the delivery if it is invalid**.
    ///
    /// - invalid payload: the delivery is nacked with `options`, then the
    ///   decode/validation error is returned (a nack failure is returned instead);
    /// - valid payload: the value is returned and the delivery is left
    ///   unsettled — ack it after processing.
    pub async fn nack_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicNackOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode_or_settle(&delivery.data, || delivery.nack(options)).await
    }

    /// Decode + validate, and **reject the delivery if it is invalid**.
    ///
    /// Same contract as [`nack_decode`](Self::nack_decode), using `basic.reject`.
    pub async fn reject_decode<T>(
        &self,
        delivery: &lapin::message::Delivery,
        options: lapin::options::BasicRejectOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode_or_settle(&delivery.data, || delivery.reject(options)).await
    }

    /// [`ack_decode`](Self::ack_decode) for a `basic_get` message.
    pub async fn ack_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicAckOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.ack_decode(message, options).await
    }

    /// [`nack_decode`](Self::nack_decode) for a `basic_get` message.
    pub async fn nack_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicNackOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.nack_decode(message, options).await
    }

    /// [`reject_decode`](Self::reject_decode) for a `basic_get` message.
    pub async fn reject_decode_get<T>(
        &self,
        message: &lapin::message::BasicGetMessage,
        options: lapin::options::BasicRejectOptions,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        self.reject_decode(message, options).await
    }
}

fn decode<T: vld::schema::VldParse>(payload: &[u8]) -> Result<T, VldLapinError> {
    let value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| VldLapinError::Deserialization(e.to_string()))?;
    T::vld_parse_value(&value).map_err(VldLapinError::Validation)
}

/// Decode `payload`; if that fails, run `settle` (nack/reject) before returning the error.
/// (`settle` output is `()` on lapin 2 and `bool` on lapin 3/4, hence `R`.)
async fn decode_or_settle<T, R, F, Fut>(payload: &[u8], settle: F) -> Result<T, VldLapinError>
where
    T: vld::schema::VldParse,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<R, lapin::Error>>,
{
    match decode(payload) {
        Ok(v) => Ok(v),
        Err(e) => {
            settle().await?;
            Err(e)
        }
    }
}

impl Deref for LapinChannel {
    type Target = lapin::Channel;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LapinChannel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Error type for `vld-lapin`.
#[derive(Debug)]
pub enum VldLapinError {
    /// The payload failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The payload is not valid JSON.
    Deserialization(String),
    /// A lapin operation (publish, ack, nack, …) failed.
    Lapin(lapin::Error),
}

impl fmt::Display for VldLapinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldLapinError::Validation(e) => write!(f, "Validation error: {e}"),
            VldLapinError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldLapinError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldLapinError::Lapin(e) => write!(f, "Lapin error: {e}"),
        }
    }
}

impl std::error::Error for VldLapinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldLapinError::Validation(e) => Some(e),
            VldLapinError::Lapin(e) => Some(e),
            VldLapinError::Serialization(_) | VldLapinError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldLapinError {
    fn from(value: vld::error::VldError) -> Self {
        Self::Validation(value)
    }
}

impl From<lapin::Error> for VldLapinError {
    fn from(value: lapin::Error) -> Self {
        Self::Lapin(value)
    }
}

/// Common imports: `LapinChannel`, `VldLapinError` and `vld::prelude`.
pub mod prelude {
    pub use crate::{LapinChannel, VldLapinError};
    pub use vld::prelude::*;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    vld::schema! {
        #[derive(Debug, serde::Serialize)]
        struct Evt {
            event: String => vld::string().min(1),
            retries: i64 => vld::number().int().min(0).max(10),
        }
    }

    fn paths(err: &VldLapinError) -> Vec<String> {
        match err {
            VldLapinError::Validation(e) => e
                .issues
                .iter()
                .map(|i| i.path.iter().map(|s| s.to_string()).collect())
                .collect(),
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn publish_payload_is_validated_json() {
        let evt = Evt {
            event: "signup".into(),
            retries: 1,
        };
        let bytes = LapinChannel::encode_payload(&evt).unwrap();
        let back: Evt = decode(&bytes).unwrap();
        assert_eq!((back.event.as_str(), back.retries), ("signup", 1));

        // Invalid values never become a payload, so nothing is published.
        let bad = Evt {
            event: String::new(),
            retries: 99,
        };
        let err = LapinChannel::encode_payload(&bad).unwrap_err();
        assert_eq!(paths(&err), [".event", ".retries"]);
    }

    #[test]
    fn decode_reports_json_and_schema_errors() {
        assert!(matches!(
            decode::<Evt>(b"{"),
            Err(VldLapinError::Deserialization(_))
        ));
        let err = decode::<Evt>(br#"{"event":"","retries":99}"#).unwrap_err();
        assert_eq!(paths(&err), [".event", ".retries"]);
    }

    #[test]
    fn settle_failure_is_returned_instead_of_the_decode_error() {
        #[cfg(feature = "lapin-2")]
        let failure = || lapin::Error::InvalidChannel(7);
        #[cfg(not(feature = "lapin-2"))]
        let failure = || lapin::Error::from(lapin::ErrorKind::InvalidChannel(7));
        let r: Result<Evt, _> = run(decode_or_settle(b"not json", || async move {
            Err::<(), _>(failure())
        }));
        assert!(matches!(r, Err(VldLapinError::Lapin(_))));
    }

    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn invalid_payload_is_settled_then_error_returned() {
        let settled = Cell::new(0);
        let settle = || {
            settled.set(settled.get() + 1);
            async { Ok::<_, lapin::Error>(()) }
        };
        let r: Result<Evt, _> = run(decode_or_settle(br#"{"event":"","retries":0}"#, settle));
        assert!(matches!(r, Err(VldLapinError::Validation(_))));
        assert_eq!(settled.get(), 1);

        let r: Result<Evt, _> = run(decode_or_settle(b"not json", settle));
        assert!(matches!(r, Err(VldLapinError::Deserialization(_))));
        assert_eq!(settled.get(), 2);
    }

    #[test]
    fn valid_payload_is_not_settled() {
        let settled = Cell::new(false);
        let r: Result<Evt, _> = run(decode_or_settle(br#"{"event":"x","retries":0}"#, || {
            settled.set(true);
            async { Ok::<_, lapin::Error>(()) }
        }));
        assert_eq!(r.unwrap().event, "x");
        assert!(!settled.get());
    }
}
