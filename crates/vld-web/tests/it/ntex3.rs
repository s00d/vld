//! `vld_web::ntex3`: the shared ntex tests (`ntex/`) compiled against ntex 3.

use ::ntex3 as ntex;
use vld_web::ntex3 as vld_ntex;

/// Registers app state (`VldBodyLimit`) on the app.
macro_rules! with_state {
    ($app:expr, $cfg:expr) => {
        $app.state($cfg)
    };
}

/// What differs between the majors in the tests.
mod compat {
    use super::ntex;
    use super::vld_ntex::VldNtexError;
    use ntex::http::StatusCode;
    use ntex::web::DefaultError;
    use ntex::web::error::WebResponseError;

    /// ntex 3 has `TestRequest::set_payload` itself.
    #[allow(dead_code)]
    pub trait TestRequestExt {}

    /// ntex 3 middleware and loggers read the status through
    /// `WebResponseError::status_code`.
    pub fn check_status_code(e: &VldNtexError, too_large: &VldNtexError) {
        assert_eq!(
            WebResponseError::<DefaultError>::status_code(e),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            WebResponseError::<DefaultError>::status_code(too_large),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }
}

#[allow(clippy::duplicate_mod)]
#[path = "ntex/conformance.rs"]
mod conformance;
#[allow(clippy::duplicate_mod)]
#[path = "ntex/story_api.rs"]
mod story_api;
