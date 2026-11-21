//! The lapin integration, written once against `super::lapin` and compiled
//! by `lapin2.rs` / `lapin3.rs` / `lapin4.rs`; what differs between the
//! majors is in their `compat` modules.

use super::compat::LapinConfirmation;
use super::lapin;
use std::fmt;
use std::ops::{Deref, DerefMut};

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
        settled(delivery.ack(options).await?)?;
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
        settled(delivery.ack(ack).await?)?;
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

/// What an ack / nack / reject returns: `()` on lapin 2; on lapin 3/4 a
/// `bool` that is `false` when nothing was sent (the delivery was already
/// settled, or its channel is closed).
trait Settle {
    fn sent(&self) -> bool;
}

impl Settle for () {
    fn sent(&self) -> bool {
        true
    }
}

impl Settle for bool {
    fn sent(&self) -> bool {
        *self
    }
}

/// [`VldLapinError::NotSettled`] when the settle call sent nothing.
fn settled(r: impl Settle) -> Result<(), VldLapinError> {
    if r.sent() {
        Ok(())
    } else {
        Err(VldLapinError::NotSettled)
    }
}

/// Decode `payload`; if that fails, run `settle` (nack/reject) before returning
/// the error (a settle failure is returned instead).
async fn decode_or_settle<T, R, F, Fut>(payload: &[u8], settle: F) -> Result<T, VldLapinError>
where
    T: vld::schema::VldParse,
    R: Settle,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<R, lapin::Error>>,
{
    match decode(payload) {
        Ok(v) => Ok(v),
        Err(e) => {
            settled(settle().await?)?;
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

/// Error type of the lapin wrapper.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldLapinError {
    /// The payload failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The payload is not valid JSON.
    Deserialization(String),
    /// A lapin operation (publish, ack, nack, …) failed.
    Lapin(lapin::Error),
    /// An ack / nack / reject sent nothing: the delivery was already settled
    /// or its channel is closed (lapin 3/4 report this as `Ok(false)`).
    NotSettled,
}

impl fmt::Display for VldLapinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldLapinError::Validation(e) => write!(f, "Validation error: {e}"),
            VldLapinError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldLapinError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldLapinError::Lapin(e) => write!(f, "Lapin error: {e}"),
            VldLapinError::NotSettled => {
                write!(f, "Delivery not settled: already settled or channel closed")
            }
        }
    }
}

impl std::error::Error for VldLapinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldLapinError::Validation(e) => Some(e),
            VldLapinError::Lapin(e) => Some(e),
            VldLapinError::Serialization(_)
            | VldLapinError::Deserialization(_)
            | VldLapinError::NotSettled => None,
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
    pub use super::{LapinChannel, VldLapinError};
    pub use vld::prelude::*;
}
