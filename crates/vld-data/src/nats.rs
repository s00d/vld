//! # `vld_data::nats` — NATS (async-nats) for `vld`
//!
//! Validate NATS payloads as JSON:
//!
//! - [`publish`] — validate a value, encode it as JSON and publish it;
//! - [`request`] — validate the request, send it, then parse and validate
//!   the reply;
//! - [`respond`] — answer a request message with a validated reply;
//! - [`decode`] — parse and validate a received [`Message`];
//! - [`encode`] / [`decode_bytes`] — the same on raw bytes.
//!
//! Published values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). Feature `nats` (async-nats 0.53, needs
//! Rust 1.89; vld-data enables none of its default features).
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use vld_data::nats::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct Add {
//!         pub a: i64 => vld::number().int(),
//!         pub b: i64 => vld::number().int(),
//!     }
//! }
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct Sum {
//!         pub sum: i64 => vld::number().int(),
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = async_nats::connect("127.0.0.1:4222").await?;
//!
//! // Service: answer valid requests.
//! let mut requests = client.subscribe("math.add").await?;
//! let service = client.clone();
//! tokio::spawn(async move {
//!     while let Some(msg) = requests.next().await {
//!         if let Ok(add) = decode::<Add>(&msg) {
//!             let _ = respond(&service, &msg, &Sum { sum: add.a + add.b }).await;
//!         }
//!     }
//! });
//!
//! // Caller: both directions are validated.
//! let sum: Sum = request(&client, "math.add", &Add { a: 2, b: 3 }).await?;
//! assert_eq!(sum.sum, 5);
//! # Ok(()) }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/nats>

use async_nats::{Client, Message, PublishError, RequestError, ToSubject};
use std::fmt;

pub use vld;

/// Validate `value` and encode it as a JSON payload.
pub fn encode<V>(value: &V) -> Result<bytes::Bytes, VldNatsError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    Ok(crate::codec::encode(value)?.into())
}

/// Parse a JSON payload and validate it against `T`.
pub fn decode_bytes<T>(payload: &[u8]) -> Result<T, VldNatsError>
where
    T: vld::schema::VldParse,
{
    Ok(crate::codec::decode(payload)?)
}

/// Parse and validate a received message's payload.
pub fn decode<T>(message: &Message) -> Result<T, VldNatsError>
where
    T: vld::schema::VldParse,
{
    decode_bytes(&message.payload)
}

/// Validate `value`, encode it as JSON and publish it to `subject`.
pub async fn publish<V>(
    client: &Client,
    subject: impl ToSubject,
    value: &V,
) -> Result<(), VldNatsError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    Ok(client.publish(subject, payload).await?)
}

/// Validate `req`, send it as a request to `subject`, then parse and
/// validate the reply as `Resp`.
pub async fn request<Req, Resp>(
    client: &Client,
    subject: impl ToSubject,
    req: &Req,
) -> Result<Resp, VldNatsError>
where
    Req: serde::Serialize + vld::schema::VldParse,
    Resp: vld::schema::VldParse,
{
    let payload = encode(req)?;
    let reply = client.request(subject, payload).await?;
    decode(&reply)
}

/// Validate `value` and publish it to the reply subject of `request`.
///
/// [`VldNatsError::NoReply`] when the message has no reply subject (it was
/// published, not sent with `request`).
pub async fn respond<V>(client: &Client, request: &Message, value: &V) -> Result<(), VldNatsError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    let reply = request.reply.clone().ok_or(VldNatsError::NoReply)?;
    Ok(client.publish(reply, payload).await?)
}

/// Error type of the nats module.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldNatsError {
    /// The payload failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The payload is not valid JSON.
    Deserialization(String),
    /// `Client::publish` failed.
    Publish(PublishError),
    /// `Client::request` failed (timeout, no responders, …).
    Request(RequestError),
    /// [`respond`] was given a message without a reply subject.
    NoReply,
}

impl fmt::Display for VldNatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldNatsError::Validation(e) => write!(f, "Validation error: {e}"),
            VldNatsError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldNatsError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldNatsError::Publish(e) => write!(f, "NATS publish error: {e}"),
            VldNatsError::Request(e) => write!(f, "NATS request error: {e}"),
            VldNatsError::NoReply => write!(f, "Message has no reply subject"),
        }
    }
}

impl std::error::Error for VldNatsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldNatsError::Validation(e) => Some(e),
            VldNatsError::Publish(e) => Some(e),
            VldNatsError::Request(e) => Some(e),
            VldNatsError::Serialization(_)
            | VldNatsError::Deserialization(_)
            | VldNatsError::NoReply => None,
        }
    }
}

impl From<vld::error::VldError> for VldNatsError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

impl From<PublishError> for VldNatsError {
    fn from(e: PublishError) -> Self {
        Self::Publish(e)
    }
}

impl From<RequestError> for VldNatsError {
    fn from(e: RequestError) -> Self {
        Self::Request(e)
    }
}

crate::codec::__vld_codec_from!(VldNatsError);

/// Common imports: the functions, `VldNatsError` and `vld::prelude`.
pub mod prelude {
    pub use super::{VldNatsError, decode, decode_bytes, encode, publish, request, respond};
    pub use vld::prelude::*;
}
