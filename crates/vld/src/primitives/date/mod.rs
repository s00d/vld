//! Date / datetime schemas.
//!
//! Each date library is an additive feature with its own module —
//! `vld::chrono`, `vld::jiff`, `vld::time` — exposing `date()`, `datetime()`,
//! `time()`, `ZDate`, `ZDateTime` and `ZTime`; any combination can be enabled at once. The
//! generic [`ZDate`] / [`ZDateTime`] here take the backend marker
//! (`ZDate<vld::chrono::Chrono>`).
//!
//! When **exactly one** backend is enabled, `vld::date()`, `vld::datetime()`,
//! `vld::time_of_day()` and `vld::prelude::{ZDate, ZDateTime, ZTime}` are shortcuts for it. They disappear
//! as soon as a second backend is enabled (e.g. by another crate through
//! feature unification), so libraries should use the backend modules.

mod common;

pub use common::{DateBackend, ZDate, ZDateTime, ZTime};

#[cfg(feature = "chrono")]
pub mod chrono;
#[cfg(feature = "jiff")]
pub mod jiff;
#[cfg(feature = "time")]
pub mod time;
