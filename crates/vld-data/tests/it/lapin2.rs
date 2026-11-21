//! `vld_data::lapin2`: the shared lapin tests (`lapin/`) compiled against
//! lapin 2, connected to the in-memory AMQP broker (`lapin/broker.rs`).

use ::lapin2 as lapin;
use vld_data::lapin2 as vld_lapin;

/// What differs between the majors in the tests.
mod compat {
    use super::vld_lapin::VldLapinError;

    pub use super::lapin::publisher_confirm::Confirmation;

    /// lapin 2 fails a second settle of a delivery (`Lapin`).
    pub fn check_settled_twice<T: std::fmt::Debug>(
        nacked: Result<T, VldLapinError>,
        acked: Result<T, VldLapinError>,
    ) {
        assert!(matches!(nacked, Err(VldLapinError::Lapin(_))), "{nacked:?}");
        assert!(matches!(acked, Err(VldLapinError::Lapin(_))), "{acked:?}");
    }
}

#[allow(clippy::duplicate_mod)]
#[path = "lapin/common.rs"]
mod common;
#[allow(clippy::duplicate_mod)]
#[path = "lapin/model_queue.rs"]
mod model_queue;
#[allow(clippy::duplicate_mod)]
#[path = "lapin/story_publish.rs"]
mod story_publish;
#[allow(clippy::duplicate_mod)]
#[path = "lapin/story_settle.rs"]
mod story_settle;
