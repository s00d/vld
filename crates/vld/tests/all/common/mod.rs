//! Helpers shared by every module of the `all` test binary.
#![allow(dead_code)] // each feature combination uses a different subset

use std::fmt::Debug;
use vld::error::{ValidationIssue, VldError};
use vld::schema::VldSchema;

/// Parse `input` and return the error; panics (showing the value) on success.
#[track_caller]
pub fn parse_err<S>(schema: &S, input: serde_json::Value) -> VldError
where
    S: VldSchema,
    S::Output: Debug,
{
    match schema.parse_value(&input) {
        Ok(v) => panic!("expected an error for {input}, got Ok({v:?})"),
        Err(e) => e,
    }
}

/// `.a.b[0]` — the issue path as one string (`""` for the root).
pub fn path_of(issue: &ValidationIssue) -> String {
    issue.path.iter().map(ToString::to_string).collect()
}

/// Every issue path, in order.
pub fn paths(err: &VldError) -> Vec<String> {
    err.issues.iter().map(path_of).collect()
}

/// Every issue code key (`"too_small"`, ...), in order.
pub fn codes(err: &VldError) -> Vec<String> {
    err.issues
        .iter()
        .map(|i| i.code.key().to_string())
        .collect()
}

/// The first issue code of a failed `Result` (panics on `Ok`).
#[track_caller]
pub fn code<T: Debug>(r: Result<T, VldError>) -> vld::error::IssueCode {
    IntoErr::into_err(r).issues[0].code.clone()
}

/// A serde enum shared by the adapter and keyed-record tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    User,
}

/// A fresh temporary directory, removed on drop.
pub fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}

/// Accepts a `VldError` or a `Result<T, VldError>` (which must be `Err`).
pub trait IntoErr {
    fn into_err(self) -> VldError;
}

impl IntoErr for VldError {
    fn into_err(self) -> VldError {
        self
    }
}

impl IntoErr for &VldError {
    fn into_err(self) -> VldError {
        self.clone()
    }
}

impl<T: Debug> IntoErr for Result<T, VldError> {
    #[track_caller]
    fn into_err(self) -> VldError {
        match self {
            Ok(v) => panic!("expected an error, got Ok({v:?})"),
            Err(e) => e,
        }
    }
}

/// Assert that an error (or failed `Result`) has an issue at `path` whose code
/// matches `pattern`:
///
/// ```ignore
/// assert_issue!(schema.parse_value(&json!(1)), ".name", IssueCode::InvalidType { .. });
/// assert_issue!(err, "", IssueCode::TooSmall { minimum, .. } if minimum == &json!(3));
/// ```
#[macro_export]
macro_rules! assert_issue {
    ($res:expr, $path:expr, $code:pat $(if $guard:expr)? $(,)?) => {{
        let err = $crate::common::IntoErr::into_err($res);
        let path: &str = $path;
        assert!(
            err.issues.iter().any(|i| $crate::common::path_of(i) == path
                && matches!(&i.code, $code $(if $guard)?)),
            "no issue at {:?} matching `{}`; issues: {:#?}",
            path,
            stringify!($code),
            err.issues
        );
    }};
}
