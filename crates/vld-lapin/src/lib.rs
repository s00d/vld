//! # vld-lapin
//!
//! Lapin (RabbitMQ) integration for `vld`.
//!
//! ## Overview
//!
//! `vld-lapin` keeps one entrypoint macro:
//!
//! - `impl_to_lapin!(channel)`
//!
//! After rebinding, `channel` becomes a validating wrapper with:
//!
//! - publish helper: `publish`
//! - get/decode helpers: `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
//! - settle helpers: `ack_decode` (ack when valid), `nack_decode` / `reject_decode`
//!   (nack/reject when invalid, so poison messages are not left unacked)
//! - the same for `basic_get` messages: `ack_decode_get`, `nack_decode_get`, `reject_decode_get`
//!
//! All other native `lapin::Channel` methods are still available through deref.

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
    pub fn new(inner: lapin::Channel) -> Self {
        Self { inner }
    }

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

    pub fn decode_delivery<T>(
        &self,
        delivery: &lapin::message::Delivery,
    ) -> Result<T, VldLapinError>
    where
        T: vld::schema::VldParse,
    {
        decode(&delivery.data)
    }

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
    /// On a decode/validation error the delivery is left unsettled and the
    /// error is returned (use [`nack_decode`](Self::nack_decode) /
    /// [`reject_decode`](Self::reject_decode) to drop poison messages).
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
    Validation(vld::error::VldError),
    Serialization(String),
    Deserialization(String),
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

#[macro_export]
macro_rules! impl_to_lapin {
    ($channel:ident) => {
        let $channel = $crate::LapinChannel::new($channel);
    };
}

pub mod prelude {
    pub use crate::{impl_to_lapin, LapinChannel, VldLapinError};
    pub use vld::prelude::*;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    vld::schema! {
        #[derive(Debug)]
        struct Evt { event: String => vld::string().min(1) }
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
        let r: Result<Evt, _> = run(decode_or_settle(br#"{"event":""}"#, settle));
        assert!(matches!(r, Err(VldLapinError::Validation(_))));
        assert_eq!(settled.get(), 1);

        let r: Result<Evt, _> = run(decode_or_settle(b"not json", settle));
        assert!(matches!(r, Err(VldLapinError::Deserialization(_))));
        assert_eq!(settled.get(), 2);
    }

    #[test]
    fn valid_payload_is_not_settled() {
        let settled = Cell::new(false);
        let r: Result<Evt, _> = run(decode_or_settle(br#"{"event":"x"}"#, || {
            settled.set(true);
            async { Ok::<_, lapin::Error>(()) }
        }));
        assert_eq!(r.unwrap().event, "x");
        assert!(!settled.get());
    }
}
