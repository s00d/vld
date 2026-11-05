//! Shared HTTP conformance suite for the `vld` web adapters (test support).
//!
//! Enabled by the `conformance` feature; only the adapters' dev-dependencies
//! turn it on. Every adapter mounts the same routes, using the schemas below:
//!
//! | Route | Extractor | 200 body |
//! |---|---|---|
//! | `POST /json` | JSON [`User`] | `"{name}:{age}"` |
//! | `POST /form` | form [`User`] | `"{name}:{age}"` |
//! | `GET /query` | query [`Query`] | `"{tag joined by ,}\|{q}"` |
//! | `GET /path/{id}` | path [`PathId`] | `"id={id}"` |
//! | `GET /headers` | headers [`Headers`] | `"{x_tag}"` |
//! | `GET /cookie` | cookies [`Cookies`] | `"{sid}"` |
//!
//! Body limits stay at the adapter/framework defaults; the oversized cases
//! send [`OVERSIZED_LEN`] bytes, above every default.
//!
//! Each case is a function in [`cases`]; adapters list the cases they support
//! with [`conformance_suite!`](crate::conformance_suite), marking known
//! failures with `#[ignore = "BUG: ..."]`.

use std::future::Future;

/// Body size used by the oversized cases: above [`DEFAULT_BODY_LIMIT`](crate::DEFAULT_BODY_LIMIT)
/// and every framework default limit.
pub const OVERSIZED_LEN: usize = crate::DEFAULT_BODY_LIMIT + 4096;

vld::schema! {
    /// `POST /json` and `POST /form` body.
    #[derive(Debug, Clone)]
    pub struct User {
        pub name: String => vld::string().min(2),
        pub age: i64 => vld::number().int().min(0),
    }
}

vld::schema! {
    /// `GET /query` parameters.
    #[derive(Debug, Clone)]
    pub struct Query {
        pub tag: Vec<String> => vld::array(vld::string()),
        pub q: String => vld::string(),
    }
}

vld::schema! {
    /// `GET /path/{id}` parameters.
    #[derive(Debug, Clone)]
    pub struct PathId {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    /// `GET /headers` headers (`x-tag` → `x_tag`).
    #[derive(Debug, Clone)]
    pub struct Headers {
        pub x_tag: String => vld::string(),
    }
}

vld::schema! {
    /// `GET /cookie` cookies.
    #[derive(Debug, Clone)]
    pub struct Cookies {
        pub sid: String => vld::string(),
    }
}

impl User {
    /// The 200 body for `/json` and `/form`.
    pub fn reply(&self) -> String {
        format!("{}:{}", self.name, self.age)
    }
}

impl Query {
    /// The 200 body for `/query`.
    pub fn reply(&self) -> String {
        format!("{}|{}", self.tag.join(","), self.q)
    }
}

impl PathId {
    /// The 200 body for `/path/{id}`.
    pub fn reply(&self) -> String {
        format!("id={}", self.id)
    }
}

/// A framework-independent request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: &'static str,
    /// Path and query, e.g. `/query?q=1`.
    pub uri: String,
    /// Header name/value pairs; values may be non-UTF-8.
    pub headers: Vec<(&'static str, Vec<u8>)>,
    pub body: Vec<u8>,
}

/// A framework-independent response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("response body is not JSON ({e}): {}", self.text()))
    }
}

/// What a case expects back.
#[derive(Debug, Clone)]
pub enum Expect {
    /// 200 with exactly this text body.
    Ok(String),
    /// 422 `{"error":"Validation failed","issues":[...]}` whose issue paths
    /// are exactly these (in order).
    Invalid(Vec<&'static str>),
    /// 413 `{"error":"Payload too large"}`.
    TooLarge,
}

/// One conformance case.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: &'static str,
    pub request: Request,
    pub expect: Expect,
}

impl Case {
    /// Prefix the request path (e.g. a framework mount point).
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.request.uri = format!("{prefix}{}", self.request.uri);
        self
    }

    /// Send the request through `send` and assert the response.
    pub async fn check<F, Fut>(self, send: F)
    where
        F: FnOnce(Request) -> Fut,
        Fut: Future<Output = Response>,
    {
        let name = self.name;
        let resp = send(self.request).await;
        match self.expect {
            Expect::Ok(text) => {
                assert_eq!(resp.status, 200, "[{name}] body: {}", resp.text());
                assert_eq!(resp.text(), text, "[{name}]");
            }
            Expect::Invalid(paths) => {
                assert_eq!(resp.status, 422, "[{name}] body: {}", resp.text());
                let body = resp.json();
                assert_eq!(body["error"], "Validation failed", "[{name}] {body}");
                let got: Vec<&str> = body["issues"]
                    .as_array()
                    .unwrap_or_else(|| panic!("[{name}] no issues array: {body}"))
                    .iter()
                    .map(|i| {
                        assert!(
                            i["message"].is_string(),
                            "[{name}] issue without message: {i}"
                        );
                        i["path"].as_str().unwrap_or("<no path>")
                    })
                    .collect();
                assert_eq!(got, paths, "[{name}] issue paths: {body}");
            }
            Expect::TooLarge => {
                assert_eq!(resp.status, 413, "[{name}] body: {}", resp.text());
                assert_eq!(
                    resp.json(),
                    serde_json::json!({"error": "Payload too large"}),
                    "[{name}]"
                );
            }
        }
    }
}

fn req(method: &'static str, uri: &str) -> Request {
    Request {
        method,
        uri: uri.to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

fn post(uri: &str, content_type: &str, body: impl Into<Vec<u8>>) -> Request {
    let mut r = req("POST", uri);
    r.headers
        .push(("content-type", content_type.as_bytes().to_vec()));
    r.body = body.into();
    r
}

fn case(name: &'static str, request: Request, expect: Expect) -> Case {
    Case {
        name,
        request,
        expect,
    }
}

fn ok(text: &str) -> Expect {
    Expect::Ok(text.to_string())
}

const FORM: &str = "application/x-www-form-urlencoded";

/// Every conformance case, one function each.
pub mod cases {
    use super::*;

    pub fn json_valid() -> Case {
        case(
            "json_valid",
            post("/json", "application/json", r#"{"name":"Alice","age":25}"#),
            ok("Alice:25"),
        )
    }

    /// Media types are case-insensitive and may carry parameters.
    pub fn json_content_type_case_insensitive() -> Case {
        case(
            "json_content_type_case_insensitive",
            post(
                "/json",
                "Application/JSON; charset=utf-8",
                r#"{"name":"Alice","age":25}"#,
            ),
            ok("Alice:25"),
        )
    }

    /// Malformed JSON is a 422 with the validation shape (one root issue).
    pub fn json_malformed() -> Case {
        case(
            "json_malformed",
            post("/json", "application/json", "{"),
            Expect::Invalid(vec![""]),
        )
    }

    pub fn json_validation_error() -> Case {
        case(
            "json_validation_error",
            post("/json", "application/json", r#"{"name":"A","age":-1}"#),
            Expect::Invalid(vec![".name", ".age"]),
        )
    }

    pub fn json_oversized() -> Case {
        let pad = "x".repeat(OVERSIZED_LEN);
        case(
            "json_oversized",
            post(
                "/json",
                "application/json",
                format!(r#"{{"name":"{pad}","age":1}}"#),
            ),
            Expect::TooLarge,
        )
    }

    pub fn form_valid() -> Case {
        case(
            "form_valid",
            post("/form", FORM, "name=Al+ice&age=25"),
            ok("Al ice:25"),
        )
    }

    pub fn form_validation_error() -> Case {
        case(
            "form_validation_error",
            post("/form", FORM, "name=A&age=x"),
            Expect::Invalid(vec![".name", ".age"]),
        )
    }

    /// A form body that is not UTF-8 is a 422 parse error.
    pub fn form_non_utf8() -> Case {
        case(
            "form_non_utf8",
            post("/form", FORM, b"name=\xff\xfe&age=1".to_vec()),
            Expect::Invalid(vec![""]),
        )
    }

    pub fn form_oversized() -> Case {
        let pad = "x".repeat(OVERSIZED_LEN);
        case(
            "form_oversized",
            post("/form", FORM, format!("name={pad}&age=1")),
            Expect::TooLarge,
        )
    }

    /// `%C3%A9` decodes to `é`, `+` to a space.
    pub fn query_percent_decoding() -> Case {
        case(
            "query_percent_decoding",
            req("GET", "/query?tag=caf%C3%A9&tag=a+b&q=caf%C3%A9"),
            ok("café,a b|café"),
        )
    }

    /// Repeated keys become an array; non-canonical numbers stay strings.
    pub fn query_repeated_keys() -> Case {
        case(
            "query_repeated_keys",
            req("GET", "/query?tag=a&tag=b&tag=c&q=01234"),
            ok("a,b,c|01234"),
        )
    }

    pub fn query_missing_field() -> Case {
        case(
            "query_missing_field",
            req("GET", "/query?tag=a&tag=b"),
            Expect::Invalid(vec![".q"]),
        )
    }

    pub fn path_valid() -> Case {
        case("path_valid", req("GET", "/path/42"), ok("id=42"))
    }

    pub fn path_invalid() -> Case {
        case(
            "path_invalid",
            req("GET", "/path/0"),
            Expect::Invalid(vec![".id"]),
        )
    }

    /// Header names map `x-tag` → `x_tag`; values may be non-ASCII UTF-8.
    pub fn headers_valid() -> Case {
        let mut r = req("GET", "/headers");
        r.headers.push(("x-tag", "café".as_bytes().to_vec()));
        case("headers_valid", r, ok("café"))
    }

    pub fn headers_missing() -> Case {
        case(
            "headers_missing",
            req("GET", "/headers"),
            Expect::Invalid(vec![".x_tag"]),
        )
    }

    /// Quoted cookie values are unquoted and percent-decoded.
    pub fn cookie_quoted_encoded() -> Case {
        let mut r = req("GET", "/cookie");
        r.headers
            .push(("cookie", b"other=1; sid=\"abc%20def\"".to_vec()));
        case("cookie_quoted_encoded", r, ok("abc def"))
    }

    pub fn cookie_missing() -> Case {
        let mut r = req("GET", "/cookie");
        r.headers.push(("cookie", b"other=1".to_vec()));
        case("cookie_missing", r, Expect::Invalid(vec![".sid"]))
    }
}

/// Generate one test per listed case.
///
/// ```ignore
/// vld_http_common::conformance_suite! {
///     runtime = tokio::test,
///     send = send,
///     cases = [json_valid, #[ignore = "BUG: ..."] form_non_utf8],
/// }
/// ```
///
/// `send` is an `async fn(conformance::Request) -> conformance::Response`.
/// An optional `prefix = "/api",` prefixes every request path.
#[macro_export]
macro_rules! conformance_suite {
    (
        runtime = $rt:meta,
        send = $send:path,
        cases = [ $( $(#[$m:meta])* $case:ident ),* $(,)? ] $(,)?
    ) => {
        $crate::conformance_suite! {
            runtime = $rt,
            send = $send,
            prefix = "",
            cases = [ $( $(#[$m])* $case ),* ],
        }
    };
    (
        runtime = $rt:meta,
        send = $send:path,
        prefix = $prefix:expr,
        cases = [ $( $(#[$m:meta])* $case:ident ),* $(,)? ] $(,)?
    ) => {
        $(
            $(#[$m])*
            #[$rt]
            async fn $case() {
                $crate::conformance::cases::$case()
                    .with_prefix($prefix)
                    .check($send)
                    .await;
            }
        )*
    };
}
