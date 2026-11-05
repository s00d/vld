//! Date / datetime schemas.
//!
//! Each date library is an additive feature with its own module —
//! `vld::chrono`, `vld::jiff`, `vld::time` — exposing `date()`, `datetime()`,
//! `ZDate` and `ZDateTime`; any combination can be enabled at once. The
//! generic [`ZDate`] / [`ZDateTime`] here take the backend marker
//! (`ZDate<vld::chrono::Chrono>`).
//!
//! When **exactly one** backend is enabled, `vld::date()`, `vld::datetime()`
//! and `vld::prelude::{ZDate, ZDateTime}` are shortcuts for it. They disappear
//! as soon as a second backend is enabled (e.g. by another crate through
//! feature unification), so libraries should use the backend modules.

mod common;

pub use common::{DateBackend, ZDate, ZDateTime};

#[cfg(feature = "chrono")]
pub mod chrono;
#[cfg(feature = "jiff")]
pub mod jiff;
#[cfg(feature = "time")]
pub mod time;
