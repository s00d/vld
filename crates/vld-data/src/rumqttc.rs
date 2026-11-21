//! # `vld_data::rumqttc` — MQTT (rumqttc) for `vld`
//!
//! Validate MQTT payloads as JSON:
//!
//! - [`publish`] / [`try_publish`] — validate a value, encode it as JSON and
//!   send it through an [`AsyncClient`];
//! - [`decode_publish`] — parse and validate an incoming
//!   [`Publish`] (from `Event::Incoming(Packet::Publish(p))`);
//! - [`encode`] / [`decode`] — the same on raw bytes.
//!
//! Published values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). Feature `rumqttc` (MQTT 3.1.1 client of
//! rumqttc 0.25; no TLS feature is enabled by vld-data).
//!
//! ```no_run
//! use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
//! use vld_data::rumqttc::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct Reading {
//!         pub sensor: String => vld::string().min(1),
//!         pub celsius: f64 => vld::number().min(-50.0).max(150.0),
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let (client, mut eventloop) = AsyncClient::new(MqttOptions::new("vld", "127.0.0.1", 1883), 10);
//! client.subscribe("sensors/#", QoS::AtLeastOnce).await?;
//! publish(&client, "sensors/1", QoS::AtLeastOnce, false,
//!     &Reading { sensor: "s1".into(), celsius: 21.5 }).await?;
//!
//! loop {
//!     if let Event::Incoming(Packet::Publish(p)) = eventloop.poll().await? {
//!         match decode_publish::<Reading>(&p) {
//!             Ok(reading) => println!("{} {}", reading.sensor, reading.celsius),
//!             Err(e) => eprintln!("bad payload on {}: {e}", p.topic),
//!         }
//!     }
//! }
//! # }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/mqtt>

use ::rumqttc::{AsyncClient, ClientError, Publish, QoS};
use std::fmt;

pub use vld;

/// Validate `value` and encode it as a JSON payload.
pub fn encode<V>(value: &V) -> Result<Vec<u8>, VldMqttError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    Ok(crate::codec::encode(value)?)
}

/// Parse a JSON payload and validate it against `T`.
pub fn decode<T>(payload: &[u8]) -> Result<T, VldMqttError>
where
    T: vld::schema::VldParse,
{
    Ok(crate::codec::decode(payload)?)
}

/// Parse and validate an incoming publish's payload.
pub fn decode_publish<T>(publish: &Publish) -> Result<T, VldMqttError>
where
    T: vld::schema::VldParse,
{
    decode(&publish.payload)
}

/// Validate `value`, encode it as JSON and queue it with
/// `AsyncClient::publish` (waits while the request channel is full).
pub async fn publish<V>(
    client: &AsyncClient,
    topic: impl Into<String>,
    qos: QoS,
    retain: bool,
    value: &V,
) -> Result<(), VldMqttError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    Ok(client.publish(topic, qos, retain, payload).await?)
}

/// Validate `value`, encode it as JSON and queue it with
/// `AsyncClient::try_publish` (fails when the request channel is full).
pub fn try_publish<V>(
    client: &AsyncClient,
    topic: impl Into<String>,
    qos: QoS,
    retain: bool,
    value: &V,
) -> Result<(), VldMqttError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    Ok(client.try_publish(topic, qos, retain, payload)?)
}

/// Error type of the rumqttc module.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldMqttError {
    /// The payload failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The payload is not valid JSON.
    Deserialization(String),
    /// The client could not queue the request (invalid topic, closed or full
    /// channel).
    Client(ClientError),
}

impl fmt::Display for VldMqttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldMqttError::Validation(e) => write!(f, "Validation error: {e}"),
            VldMqttError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldMqttError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldMqttError::Client(e) => write!(f, "MQTT client error: {e}"),
        }
    }
}

impl std::error::Error for VldMqttError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldMqttError::Validation(e) => Some(e),
            VldMqttError::Client(e) => Some(e),
            VldMqttError::Serialization(_) | VldMqttError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldMqttError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

impl From<ClientError> for VldMqttError {
    fn from(e: ClientError) -> Self {
        Self::Client(e)
    }
}

crate::codec::__vld_codec_from!(VldMqttError);

/// Common imports: the functions, `VldMqttError` and `vld::prelude`.
pub mod prelude {
    pub use super::{VldMqttError, decode, decode_publish, encode, publish, try_publish};
    pub use vld::prelude::*;
}
