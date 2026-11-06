use super::{IssueCode, PathSegment, ValidationIssue, VldError, truncate_value};

/// Fluent builder for constructing a single [`ValidationIssue`].
///
/// Obtained via [`VldError::issue()`]. Push it into a `VldError` with
/// [`.finish()`](IssueBuilder::finish).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let mut errors = VldError::new();
/// errors
///     .issue(IssueCode::Custom { code: "password_weak".into() })
///     .message("Password is too weak")
///     .path_field("password")
///     .received(&serde_json::json!("123"))
///     .finish();
/// assert_eq!(errors.issues.len(), 1);
/// assert_eq!(errors.issues[0].message, "Password is too weak");
/// ```
pub struct IssueBuilder<'a> {
    errors: &'a mut VldError,
    code: IssueCode,
    message: Option<String>,
    path: Vec<PathSegment>,
    received: Option<serde_json::Value>,
}

impl<'a> IssueBuilder<'a> {
    /// Set the error message.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Add a field path segment.
    pub fn path_field(mut self, name: impl Into<String>) -> Self {
        self.path.push(PathSegment::Field(name.into()));
        self
    }

    /// Add an index path segment.
    pub fn path_index(mut self, idx: usize) -> Self {
        self.path.push(PathSegment::Index(idx));
        self
    }

    /// Attach the received value.
    pub fn received(mut self, value: &serde_json::Value) -> Self {
        self.received = Some(truncate_value(value));
        self
    }

    /// Finish building and push the issue into the parent `VldError`.
    ///
    /// Without [`message`](Self::message) the default message for the code is used.
    pub fn finish(self) {
        let mut issue = ValidationIssue::with_override(self.code, self.message.as_ref());
        issue.path = self.path;
        issue.received = self.received;
        self.errors.issues.push(issue);
    }
}

impl VldError {
    /// Start building a new issue with the fluent API.
    ///
    /// Call `.message(...)`, `.path_field(...)`, `.received(...)` etc., then
    /// `.finish()` to push the issue.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let mut errors = VldError::new();
    /// errors
    ///     .issue(IssueCode::Custom { code: "my_check".into() })
    ///     .message("something went wrong")
    ///     .path_field("field_name")
    ///     .finish();
    /// ```
    pub fn issue(&mut self, code: IssueCode) -> IssueBuilder<'_> {
        IssueBuilder {
            errors: self,
            code,
            message: None,
            path: vec![],
            received: None,
        }
    }
}
