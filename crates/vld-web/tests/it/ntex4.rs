//! `vld_web::ntex4`: the shared ntex tests (`ntex/`) compiled against ntex 4.

use ::ntex4 as ntex;
use vld_web::ntex4 as vld_ntex;

/// Registers app state (`VldBodyLimit`) on the app.
macro_rules! with_state {
    ($app:expr, $cfg:expr) => {
        $app.with_config(ntex::web::WebAppConfig::new().set_state($cfg))
    };
}

/// What differs between the majors in the tests.
mod compat {
    use super::ntex;
    use super::vld_ntex::VldNtexError;

    /// ntex 4 renamed `TestRequest::set_payload` to `payload`; the tests keep
    /// the ntex 3 name.
    pub trait TestRequestExt: Sized {
        fn set_payload<B: Into<ntex::util::Bytes>>(self, data: B) -> Self;
    }

    impl<St> TestRequestExt for ntex::web::test::TestRequest<St> {
        fn set_payload<B: Into<ntex::util::Bytes>>(self, data: B) -> Self {
            self.payload(data)
        }
    }

    /// ntex 4's `WebResponseError` has no `status_code`; nothing to check.
    pub fn check_status_code(_: &VldNtexError, _: &VldNtexError) {}
}

#[allow(clippy::duplicate_mod)]
#[path = "ntex/conformance.rs"]
mod conformance;
#[allow(clippy::duplicate_mod)]
#[path = "ntex/story_api.rs"]
mod story_api;
