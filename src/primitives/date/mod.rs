// Backend priority when multiple date features are enabled (e.g. `--all-features`):
// `chrono` > `jiff` > `time`.
#[cfg(feature = "chrono")]
mod chrono;
#[cfg(all(feature = "jiff", not(feature = "chrono")))]
mod jiff;
#[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
mod time;

#[cfg(feature = "chrono")]
pub use chrono::{ZDate, ZDateTime};
#[cfg(all(feature = "jiff", not(feature = "chrono")))]
pub use jiff::{ZDate, ZDateTime};
#[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
pub use time::{ZDate, ZDateTime};
