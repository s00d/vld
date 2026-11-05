//! ntex 3 / ntex 4 test glue: ntex 4 renamed `TestRequest::set_json` /
//! `set_payload` to `json` / `payload`; tests keep the ntex 3 names.

#[cfg(feature = "ntex-4")]
pub trait TestRequestExt: Sized {
    fn set_json<T: serde::Serialize>(self, data: &T) -> Self;
    fn set_payload<B: Into<vld_ntex::ntex::util::Bytes>>(self, data: B) -> Self;
}

#[cfg(feature = "ntex-4")]
impl<St> TestRequestExt for vld_ntex::ntex::web::test::TestRequest<St> {
    fn set_json<T: serde::Serialize>(self, data: &T) -> Self {
        self.json(data)
    }

    fn set_payload<B: Into<vld_ntex::ntex::util::Bytes>>(self, data: B) -> Self {
        self.payload(data)
    }
}
