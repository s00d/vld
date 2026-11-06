//! Shared HTTP conformance suite (see `vld_http_common::conformance`):
//! the extractors through `WebService`, and the JSON cases through
//! `ValidateJsonLayer`.

use rama::http::body::util::BodyExt;
use rama::http::service::web::WebService;
use rama::http::{Body, HeaderValue, Method};
use rama::Service;
use vld_http_common::conformance::{
    Cookies, Headers, PathId, PathNone, PathStr, Query, Request, Response, User,
};
use vld_rama::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

fn to_rama(r: Request) -> rama::http::Request {
    let mut req = rama::http::Request::builder()
        .method(Method::from_bytes(r.method.as_bytes()).unwrap())
        .uri(format!("http://example.com{}", r.uri));
    for (k, v) in r.headers {
        req = req.header(k, HeaderValue::from_bytes(&v).unwrap());
    }
    req.body(Body::from(r.body)).unwrap()
}

async fn from_rama(resp: rama::http::Response) -> Response {
    let status = resp.status().as_u16();
    Response {
        status,
        body: resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    }
}

mod extractors {
    use super::*;

    async fn send(r: Request) -> Response {
        let svc = WebService::default()
            .with_post("/json", async |VldJson(u): VldJson<User>| u.reply())
            .with_post("/form", async |VldForm(u): VldForm<User>| u.reply())
            .with_get("/query", async |VldQuery(q): VldQuery<Query>| q.reply())
            .with_get("/path/{id}", async |VldPath(p): VldPath<PathId>| p.reply())
            .with_get("/pstr/{s}", async |VldPath(p): VldPath<PathStr>| p.s)
            .with_get("/pnone", async |VldPath(p): VldPath<PathNone>| p.reply())
            .with_get("/headers", async |VldHeaders(h): VldHeaders<Headers>| {
                h.x_tag
            })
            .with_get("/cookie", async |VldCookie(c): VldCookie<Cookies>| c.sid);
        from_rama(svc.serve(to_rama(r)).await.unwrap()).await
    }

    vld_http_common::conformance_suite! {
        runtime = tokio::test,
        send = send,
        cases = [
            json_valid,
            json_content_type_case_insensitive,
            json_content_type_bare_param,
            json_content_type_unterminated_quote,
            json_content_type_empty_suffix,
            json_content_type_json_suffix,
            json_content_type_json_suffix_params,
            json_content_type_missing,
            json_content_type_not_json,
            json_malformed,
            json_validation_error,
            json_oversized,
            form_valid,
            form_content_type_params,
            form_wrong_content_type,
            form_too_many_fields,
            form_validation_error,
            form_non_utf8,
            form_oversized,
            query_percent_decoding,
            query_repeated_keys,
            query_coercion_follows_schema,
            query_int_invalid,
            query_missing_field,
            path_valid,
            path_invalid,
            path_percent_decoding,
            path_no_double_decoding,
            path_invalid_utf8,
            path_no_params_optional,
            headers_valid,
            headers_underscore_ignored,
            headers_underscore_only,
            headers_missing,
            cookie_quoted_encoded,
            cookie_multiple_headers,
            cookie_duplicate_first_wins,
            cookie_non_ascii,
            cookie_numeric_value,
            cookie_missing,
        ],
    }
}

mod layer {
    use super::*;
    use rama::Layer;
    use vld_rama::{validated, ValidateJsonLayer};

    #[derive(Clone)]
    struct Inner;

    impl Service<rama::http::Request> for Inner {
        type Output = rama::http::Response;
        type Error = std::convert::Infallible;

        async fn serve(&self, req: rama::http::Request) -> Result<Self::Output, Self::Error> {
            let reply =
                validated::<User>(&req).map_or_else(|| "not validated".into(), |u| u.reply());
            Ok(rama::http::Response::new(Body::from(reply)))
        }
    }

    async fn send(r: Request) -> Response {
        let svc = ValidateJsonLayer::<User>::new().layer(Inner);
        from_rama(svc.serve(to_rama(r)).await.unwrap()).await
    }

    vld_http_common::conformance_suite! {
        runtime = tokio::test,
        send = send,
        cases = [
            json_valid,
            json_content_type_case_insensitive,
            json_content_type_bare_param,
            json_content_type_unterminated_quote,
            json_content_type_json_suffix,
            json_content_type_json_suffix_params,
            json_malformed,
            json_validation_error,
            json_oversized,
        ],
    }
}
