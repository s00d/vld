//! Stories of an adapter author building a framework integration on
//! `vld_web::common`: the error bodies, the text sources, and a minimal
//! adapter checked by the shared suite.

use serde_json::{Value, json};
use vld::error::{IssueCode, PathSegment, VldError};
use vld_web::common::*;

vld::schema! {
    #[derive(Debug)]
    pub struct Item {
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Order {
        pub user: String => vld::string().min(2),
        pub items: Vec<Item> => vld::array(vld::nested(Item::parse_value)),
    }
}

/// What an adapter's JSON extractor does with the helpers.
fn extract_order(content_type: Option<&str>, body: &[u8]) -> Result<Order, VldHttpError> {
    require_json_content_type(content_type)?;
    if body.len() > VldBodyLimit(64).0 {
        return Err(VldHttpError::payload_too_large());
    }
    let value = parse_json_body(body)?;
    Ok(Order::parse_value(&value)?)
}

fn issues(body: &Value) -> Vec<(String, String)> {
    body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            assert!(i["message"].is_string(), "{i}");
            (
                i["path"].as_str().unwrap().to_string(),
                i["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// Given an adapter's JSON extractor built from the helpers,
/// when bodies arrive with every kind of failure,
/// then each failure becomes the documented status and body, and the bodies
/// themselves are valid against the published error schemas.
#[test]
fn every_failure_becomes_the_documented_body() {
    // Valid body.
    let order = extract_order(
        Some("application/json"),
        br#"{"user":"Al","items":[{"name":"pen"}]}"#,
    )
    .unwrap();
    assert_eq!(
        (order.user.as_str(), order.items[0].name.as_str()),
        ("Al", "pen")
    );

    // Wrong media type: 415 with the JSON message.
    let e = extract_order(Some("text/plain"), b"{}").unwrap_err();
    assert_eq!(e.status(), 415);
    assert_eq!(
        e.body(),
        json!({"error": "Unsupported Media Type", "message": UNSUPPORTED_MEDIA_TYPE_MESSAGE})
    );
    assert_eq!(e.body(), format_unsupported_media_type());
    assert_eq!(e.to_string(), "Unsupported Media Type");
    assert_eq!(e.error().issues[0].code, IssueCode::ParseError);
    let body = ErrorWithMessage::parse_value(&e.body()).unwrap();
    assert_eq!(body.message, UNSUPPORTED_MEDIA_TYPE_MESSAGE);
    let form = VldHttpError::unsupported_form_media_type();
    assert_eq!(
        (form.status(), form.body()["message"].clone()),
        (415, json!(UNSUPPORTED_FORM_MEDIA_TYPE_MESSAGE))
    );

    // Oversized: 413 `{"error": "Payload too large"}`.
    let e = extract_order(Some("application/json"), &[b' '; 65]).unwrap_err();
    assert_eq!(e.status(), 413);
    assert_eq!(e.body(), json!({"error": "Payload too large"}));
    assert_eq!(e.body(), format_payload_too_large());
    assert_eq!(e.to_string(), "Payload too large");
    assert_eq!(e.error().issues.len(), 1);
    assert_eq!(e.error().issues[0].message, "Payload too large");
    assert_eq!(
        ErrorBody::parse_value(&e.body()).unwrap().error,
        "Payload too large"
    );

    // Malformed JSON: 422 with one root parse error.
    let e = extract_order(Some("application/json"), b"{").unwrap_err();
    assert_eq!(e.status(), 422);
    let body = e.body();
    assert_eq!(body["error"], "Validation failed");
    assert_eq!(issues(&body), pairs(&[("", "parse_error")]));
    assert!(
        body["issues"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("Invalid JSON")
    );
    assert!(e.to_string().starts_with("Validation failed: "));

    // Schema errors: one issue per failure, `.field` and `[i]` paths.
    let e = extract_order(
        Some("application/json"),
        br#"{"user":"A","items":[{"name":"ok"},{"name":"x"}]}"#,
    )
    .unwrap_err();
    assert_eq!(e.status(), 422);
    let body = e.body();
    assert_eq!(
        issues(&body),
        pairs(&[(".user", "too_small"), (".items[1].name", "too_small")])
    );
    assert_eq!(body, format_vld_error(e.error()));
    let parsed = ValidationErrorBody::parse_value(&body).unwrap();
    assert_eq!(parsed.error, "Validation failed");
    assert_eq!(parsed.issues[1].path, ".items[1].name");
    assert_eq!(parsed.issues[1].message, e.error().issues[1].message);

    // The same issues as structs, and the error round-trips.
    let formatted = format_issues(e.error());
    assert_eq!(formatted.len(), 2);
    assert_eq!(formatted[0].code, "too_small");
    assert_eq!(formatted[0].message, e.error().issues[0].message);
    let raw = e.clone().into_error();
    assert_eq!(raw.issues.len(), 2);
    let again: VldHttpError = raw.into();
    assert_eq!((again.status(), again.body()), (422, body));

    // A hand-built error with a mixed path.
    let mut err = VldError::single(IssueCode::MissingField, "Required");
    err.issues[0].path = vec![
        PathSegment::Field("user".into()),
        PathSegment::Index(3),
        PathSegment::Field("age".into()),
    ];
    assert_eq!(
        format_vld_error(&err),
        json!({"error": "Validation failed", "issues": [
            {"path": ".user[3].age", "message": "Required", "code": "missing_field"}
        ]})
    );
    assert_eq!(
        format_issues(&VldError::single(IssueCode::MissingField, "m"))[0].path,
        ""
    );

    // Generic errors.
    assert_eq!(
        format_generic_error("Not Found"),
        json!({"error": "Not Found"})
    );
    assert_eq!(
        VldHttpError::parse("boom").error().issues[0].message,
        "boom"
    );
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string(),
        pub page: i64 => vld::number().int().min(1),
        pub tags: Vec<String> => vld::array(vld::string()),
        pub flag: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Meta {
        pub x_tag: String => vld::string(),
        pub x_num: i64 => vld::number().int(),
        pub x_bin: String => vld::string(),
        pub x_user: String => vld::string(),
    }
}

/// Given the text sources of a request (query, form body, headers, cookies,
/// path parameters),
/// when an adapter turns each into JSON and validates it with
/// `parse_coerced`,
/// then values are decoded, coerced and grouped as documented and a bad value
/// is a 422 issue at its field.
#[test]
fn text_sources_fill_a_typed_request() {
    // Query: numeric text fills a string field, one tag fills an array, a
    // bare key is "".
    let s: Search = parse_coerced(&query_string_to_json("q=123&page=2&tags=a&flag")).unwrap();
    assert_eq!(
        (s.q.as_str(), s.page, s.tags.clone(), s.flag.as_str()),
        ("123", 2, vec!["a".into()], "")
    );
    let e: VldHttpError =
        parse_coerced::<Search>(&query_string_to_json("q=x&page=abc&tags=a&flag="))
            .unwrap_err()
            .into();
    assert_eq!(issues(&e.body()), pairs(&[(".page", "invalid_type")]));
    assert!(parse_query_string("").is_empty());

    // Form body: same rules; non-UTF-8 and more than MAX_FORM_FIELDS fields
    // are 422 parse errors, before any expansion.
    assert_eq!(
        parse_form_body(b"a=1&b=x+y").unwrap(),
        json!({"a": 1, "b": "x y"})
    );
    let e = parse_form_body(b"a=\xFF").unwrap_err();
    assert_eq!(
        (e.status(), issues(&e.body())),
        (422, pairs(&[("", "parse_error")]))
    );
    let at_cap = "a=1&".repeat(MAX_FORM_FIELDS);
    assert!(parse_form_body(at_cap.as_bytes()).is_ok());
    let e = parse_form_body(format!("{at_cap}b").as_bytes()).unwrap_err();
    assert_eq!(
        (e.status(), issues(&e.body())),
        (422, pairs(&[("", "parse_error")]))
    );
    // The default body limit is axum's: 2 MiB.
    assert_eq!(DEFAULT_BODY_LIMIT, 2_097_152);
    let amplified = "a&".repeat(DEFAULT_BODY_LIMIT / 2);
    assert_eq!(
        parse_form_body(amplified.as_bytes()).unwrap_err().status(),
        422
    );

    // Headers: lowercased, `-` → `_`, repeats joined, lossy UTF-8, coerced;
    // names with `_` are skipped.
    let h: Vec<(&str, &[u8])> = vec![
        ("X-Tag", b"a"),
        ("x-tag", b"b"),
        ("X-Bin", b"caf\xC3\xA9 \xFF"),
        ("X-Num", b"42"),
        ("x_user", b"admin"),
        ("X-User", b"bob"),
    ];
    let value = headers_to_json(h);
    assert_eq!(
        value,
        json!({"x_tag": "a, b", "x_bin": "café \u{FFFD}", "x_num": 42, "x_user": "bob"})
    );
    let m: Meta = parse_coerced(&value).unwrap();
    assert_eq!(
        (m.x_tag.as_str(), m.x_num, m.x_user.as_str()),
        ("a, b", 42, "bob")
    );
    let only_underscore: Vec<(&str, &[u8])> = vec![("x_user", b"admin")];
    assert_eq!(headers_to_json(only_underscore), json!({}));

    // Cookies: unquoted, percent-decoded (`+` kept), coerced, first wins,
    // every header read.
    assert_eq!(
        cookies_to_json(r#" sid="abc" ; name=caf%C3%A9; plus=a+b;; n=7; bare; sid=evil"#),
        json!({"sid": "abc", "name": "café", "plus": "a+b", "n": 7, "bare": ""})
    );
    assert_eq!(cookies_to_json(""), json!({}));
    assert_eq!(cookies_to_json("q=\"x"), json!({"q": "\"x"}));
    let headers: [&[u8]; 3] = [b"a=1", b"b=2; a=3", "c=caf\u{e9}".as_bytes()];
    assert_eq!(
        cookie_headers_to_json(headers),
        json!({"a": 1, "b": 2, "c": "café"})
    );
    assert_eq!(cookie_headers_to_json([]), json!({}));
    assert_eq!(
        cookie_headers_to_json([&b"s=\xFF"[..]]),
        json!({"s": "\u{FFFD}"})
    );

    // Path parameters: already decoded, only coerced.
    assert_eq!(
        path_params_to_json([("id", "7"), ("slug", "007")]),
        json!({"id": 7, "slug": "007"})
    );
}

mod conformance_adapter {
    use super::*;
    use crate::conformance::{
        Case, Cookies, Expect, Headers, PathId, PathNone, PathStr, Query, Request, Response, User,
        cases,
    };
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(f: F) -> F::Output {
        let mut f = pin!(f);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// The body wrapper an adapter would export.
    struct Json<T>(pub T);
    impl<T> std::ops::Deref for Json<T> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.0
        }
    }
    impl<T> std::ops::DerefMut for Json<T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.0
        }
    }

    fn header<'a>(r: &'a Request, name: &str) -> impl Iterator<Item = &'a [u8]> {
        r.headers
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
    }

    fn body(
        r: &Request,
        check: fn(Option<&str>) -> Result<(), VldHttpError>,
    ) -> Result<&[u8], VldHttpError> {
        let ct = header(r, "content-type")
            .next()
            .and_then(|v| std::str::from_utf8(v).ok());
        check(ct)?;
        if r.body.len() > DEFAULT_BODY_LIMIT {
            return Err(VldHttpError::payload_too_large());
        }
        Ok(&r.body)
    }

    fn route(r: &Request) -> Result<String, VldHttpError> {
        let (path, query) = r.uri.split_once('?').unwrap_or((&r.uri, ""));
        let segment = |prefix: &str| path.strip_prefix(prefix).map(percent_decode);
        Ok(match (r.method, path) {
            ("POST", "/json") => {
                let mut user = Json(User::parse_value(&parse_json_body(body(
                    r,
                    require_json_content_type,
                )?)?)?);
                user.name.push('!');
                user.name.pop();
                user.reply()
            }
            ("POST", "/form") => {
                parse_coerced::<User>(&parse_form_body(body(r, require_form_content_type)?)?)?
                    .reply()
            }
            ("GET", "/query") => parse_coerced::<Query>(&query_string_to_json(query))?.reply(),
            ("GET", "/pnone") => parse_coerced::<PathNone>(&path_params_to_json([]))?.reply(),
            ("GET", "/headers") => {
                let pairs = r.headers.iter().map(|(n, v)| (*n, v.as_slice()));
                parse_coerced::<Headers>(&headers_to_json(pairs))?.x_tag
            }
            ("GET", "/cookie") => {
                parse_coerced::<Cookies>(&cookie_headers_to_json(header(r, "cookie")))?.sid
            }
            ("GET", _) if path.starts_with("/path/") => {
                let id = segment("/path/").unwrap();
                parse_coerced::<PathId>(&path_params_to_json([("id", id.as_str())]))?.reply()
            }
            ("GET", _) if path.starts_with("/pstr/") => {
                let s = segment("/pstr/").unwrap();
                parse_coerced::<PathStr>(&path_params_to_json([("s", s.as_str())]))?.s
            }
            _ => panic!("no route for {} {}", r.method, r.uri),
        })
    }

    async fn send(r: Request) -> Response {
        match route(&r) {
            Ok(text) => Response {
                status: 200,
                body: text.into_bytes(),
            },
            Err(e) => Response {
                status: e.status(),
                body: e.body().to_string().into_bytes(),
            },
        }
    }

    fn all_cases() -> Vec<Case> {
        vec![
            cases::json_valid(),
            cases::json_content_type_case_insensitive(),
            cases::json_content_type_bare_param(),
            cases::json_content_type_unterminated_quote(),
            cases::json_content_type_empty_suffix(),
            cases::json_content_type_json_suffix(),
            cases::json_content_type_json_suffix_params(),
            cases::json_content_type_missing(),
            cases::json_content_type_not_json(),
            cases::json_malformed(),
            cases::json_validation_error(),
            cases::json_oversized(),
            cases::form_valid(),
            cases::form_content_type_params(),
            cases::form_wrong_content_type(),
            cases::form_too_many_fields(),
            cases::form_validation_error(),
            cases::form_non_utf8(),
            cases::form_oversized(),
            cases::query_percent_decoding(),
            cases::query_repeated_keys(),
            cases::query_coercion_follows_schema(),
            cases::query_int_invalid(),
            cases::query_missing_field(),
            cases::path_valid(),
            cases::path_invalid(),
            cases::path_percent_decoding(),
            cases::path_no_double_decoding(),
            cases::path_invalid_utf8(),
            cases::path_no_params_optional(),
            cases::headers_valid(),
            cases::headers_underscore_ignored(),
            cases::headers_underscore_only(),
            cases::headers_missing(),
            cases::cookie_quoted_encoded(),
            cases::cookie_multiple_headers(),
            cases::cookie_duplicate_first_wins(),
            cases::cookie_non_ascii(),
            cases::cookie_numeric_value(),
            cases::cookie_missing(),
        ]
    }

    /// Every way to spoil a correct response: another status, a changed text
    /// body, a JSON body missing any key at the top or inside an issue, an
    /// issue less.
    fn spoiled(resp: &Response) -> Vec<Response> {
        let with = |status: u16, body: Vec<u8>| Response { status, body };
        let mut out = vec![with(resp.status + 1, resp.body.clone())];
        let Ok(Value::Object(top)) = serde_json::from_slice::<Value>(&resp.body) else {
            let mut text = resp.body.clone();
            text.push(b'!');
            out.push(with(resp.status, text));
            return out;
        };
        let json = |v: &Value| v.to_string().into_bytes();
        for key in top.keys() {
            let mut v = top.clone();
            v.remove(key);
            out.push(with(resp.status, json(&Value::Object(v))));
        }
        if let Some(Value::Array(list)) = top.get("issues") {
            for (i, issue) in list.iter().enumerate() {
                for key in issue.as_object().unwrap().keys() {
                    let mut v = top.clone();
                    v["issues"][i].as_object_mut().unwrap().remove(key);
                    out.push(with(resp.status, json(&Value::Object(v))));
                }
            }
            let mut v = top.clone();
            v["issues"].as_array_mut().unwrap().pop();
            out.push(with(resp.status, json(&Value::Object(v.clone()))));
            v["issues"] = json!("none");
            out.push(with(resp.status, json(&Value::Object(v))));
        }
        out
    }

    /// Given a minimal adapter written only with the helpers (plus a prefix
    /// mount point),
    /// when the whole conformance suite runs against it,
    /// then every case passes, and every spoiled response (wrong status,
    /// changed body, a missing key or issue) fails its case.
    #[test]
    fn a_minimal_adapter_passes_the_suite_and_spoiled_responses_fail_it() {
        let me = std::thread::current().name().map(str::to_string);
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().name().map(str::to_string) != me {
                prev(info);
            }
        }));

        let prefixed = cases::path_valid().with_prefix("/api");
        assert_eq!(prefixed.request.uri, "/api/path/42");
        block_on(prefixed.check(|mut r: Request| {
            r.uri = r.uri.strip_prefix("/api").unwrap().to_string();
            send(r)
        }));

        // The oversized cases send just over every default limit.
        let oversized = cases::json_oversized().request.body.len();
        assert!(oversized > DEFAULT_BODY_LIMIT && oversized < 2 * DEFAULT_BODY_LIMIT);

        for case in all_cases() {
            let name = case.name;
            let good = block_on(send(case.request.clone()));
            match (&case.expect, good.status) {
                (Expect::Ok(_), 200)
                | (Expect::Invalid(_), 422)
                | (Expect::TooLarge, 413)
                | (Expect::Unsupported, 415) => {}
                (e, s) => panic!("[{name}] {e:?} answered {s}"),
            }
            block_on(case.clone().check(|_| async { good.clone() }));
            for bad in spoiled(&good) {
                let c = case.clone();
                let shown = String::from_utf8_lossy(&bad.body).into_owned();
                let failed = std::panic::catch_unwind(move || {
                    block_on(c.check(|_| async move { bad }));
                })
                .is_err();
                assert!(failed, "[{name}] accepted a spoiled response: {shown}");
            }
        }
    }
}
