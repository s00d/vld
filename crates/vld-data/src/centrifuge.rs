//! # `vld_data::centrifuge` — Centrifugo client (centrifuge-client) for `vld`
//!
//! Validate Centrifugo payloads as JSON:
//!
//! - [`publish`] — validate a value and publish it into a subscription's
//!   channel; [`client_publish`] — the same through the client, by channel
//!   name;
//! - [`rpc`] — validate the request, call an RPC method, then parse and
//!   validate the result;
//! - [`decode_publication`] / [`decode_rpc`] — parse and validate a received
//!   [`Publication`] (`PublicationEvent::publication`) or [`RpcResult`];
//! - [`encode`] / [`decode`] — the same on raw bytes.
//!
//! Published values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). Feature `centrifuge` (crate
//! `centrifuge-client` 0.1, library name `centrifuge`; vld-data turns off its
//! default features and enables `ring` as the rustls crypto provider, which
//! the crate needs to compile). **`centrifuge-client` is 0.1: its API may
//! change in a minor release.**
//!
//! ```no_run
//! use vld_data::centrifuge::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct ChatMessage {
//!         pub text: String => vld::string().min(1).max(500),
//!     }
//! }
//!
//! # async fn run(client: centrifuge::Client, sub: centrifuge::Subscription,
//! #              publication: centrifuge::Publication) -> Result<(), VldCentrifugeError> {
//! publish(&sub, &ChatMessage { text: "hi".into() }).await?;
//! client_publish(&client, "chat:lobby", &ChatMessage { text: "hello".into() }).await?;
//!
//! // From a `PublicationEvent`:
//! let msg: ChatMessage = decode_publication(&publication)?;
//! # let _ = msg; Ok(()) }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/centrifugo>

use ::centrifuge::{Client, Publication, RpcResult, Subscription};
use std::fmt;

pub use vld;

/// Validate `value` and encode it as a JSON payload.
pub fn encode<V>(value: &V) -> Result<Vec<u8>, VldCentrifugeError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    Ok(crate::codec::encode(value)?)
}

/// Parse a JSON payload and validate it against `T`.
pub fn decode<T>(payload: &[u8]) -> Result<T, VldCentrifugeError>
where
    T: vld::schema::VldParse,
{
    Ok(crate::codec::decode(payload)?)
}

/// Parse and validate a publication's payload.
pub fn decode_publication<T>(publication: &Publication) -> Result<T, VldCentrifugeError>
where
    T: vld::schema::VldParse,
{
    decode(&publication.data)
}

/// Parse and validate an RPC result's payload.
pub fn decode_rpc<T>(result: &RpcResult) -> Result<T, VldCentrifugeError>
where
    T: vld::schema::VldParse,
{
    decode(&result.data)
}

/// Validate `value` and publish it into the subscription's channel.
pub async fn publish<V>(subscription: &Subscription, value: &V) -> Result<(), VldCentrifugeError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    Ok(subscription.publish(payload).await?)
}

/// Validate `value` and publish it into `channel` through the client.
pub async fn client_publish<V>(
    client: &Client,
    channel: &str,
    value: &V,
) -> Result<(), VldCentrifugeError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let payload = encode(value)?;
    Ok(client.publish(channel, payload).await?)
}

/// Validate `req`, call RPC `method`, then parse and validate the result as
/// `Resp`.
pub async fn rpc<Req, Resp>(
    client: &Client,
    method: &str,
    req: &Req,
) -> Result<Resp, VldCentrifugeError>
where
    Req: serde::Serialize + vld::schema::VldParse,
    Resp: vld::schema::VldParse,
{
    let payload = encode(req)?;
    let result = client.rpc(method, payload).await?;
    decode_rpc(&result)
}

/// Error type of the centrifuge module.
#[derive(Debug)]
#[non_exhaustive]
pub enum VldCentrifugeError {
    /// The payload failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to JSON.
    Serialization(String),
    /// The payload is not valid JSON.
    Deserialization(String),
    /// The client call failed (disconnected, timeout, server error, …).
    Centrifuge(::centrifuge::Error),
}

impl fmt::Display for VldCentrifugeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldCentrifugeError::Validation(e) => write!(f, "Validation error: {e}"),
            VldCentrifugeError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldCentrifugeError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldCentrifugeError::Centrifuge(e) => write!(f, "Centrifuge error: {e}"),
        }
    }
}

impl std::error::Error for VldCentrifugeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldCentrifugeError::Validation(e) => Some(e),
            VldCentrifugeError::Centrifuge(e) => Some(e),
            VldCentrifugeError::Serialization(_) | VldCentrifugeError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldCentrifugeError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

impl From<::centrifuge::Error> for VldCentrifugeError {
    fn from(e: ::centrifuge::Error) -> Self {
        Self::Centrifuge(e)
    }
}

crate::codec::__vld_codec_from!(VldCentrifugeError);

/// Common imports: the functions, `VldCentrifugeError` and `vld::prelude`.
pub mod prelude {
    pub use super::{
        VldCentrifugeError, client_publish, decode, decode_publication, decode_rpc, encode,
        publish, rpc,
    };
    pub use vld::prelude::*;
}
