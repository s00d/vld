//! Regression tests for the 0.5.0 audit fixes.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vld_warp::prelude::*;
use warp::Filter;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Slug {
        pub slug: String => vld::string(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Pair {
        pub a: String => vld::string(),
        pub b: String => vld::string(),
    }
}

fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone
{
    let json = warp::post()
        .and(warp::path("json"))
        .and(warp::path::end())
        .and(vld_json_limit::<Named>(16))
        .map(|n: Named| n.name);
    let form = warp::post()
        .and(warp::path("form"))
        .and(warp::path::end())
        .and(vld_form_limit::<Named>(16))
        .map(|n: Named| n.name);
    let param = warp::get()
        .and(warp::path("p"))
        .and(vld_param::<Slug>("slug"))
        .and(warp::path::end())
        .map(|s: Slug| s.slug);
    let tail = warp::get()
        .and(warp::path("t"))
        .and(vld_path::<Pair>(&["a", "b"]))
        .map(|p: Pair| format!("{}|{}", p.a, p.b));
    let header = warp::get()
        .and(warp::path("h"))
        .and(warp::header::<String>("x-required"))
        .map(|h: String| h);
    let query = warp::get()
        .and(warp::path("q"))
        .and(warp::query::<std::collections::HashMap<String, u8>>())
        .map(|_| "ok");
    json.or(form)
        .or(param)
        .or(tail)
        .or(header)
        .or(query)
        .recover(handle_rejection)
}

#[tokio::test]
async fn json_over_limit_is_413() {
    let resp = warp::test::request()
        .method("POST")
        .path("/json")
        .header("content-type", "application/json")
        .body(r#"{"name":"a-very-long-name"}"#)
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 413);
    assert_eq!(resp.body(), r#"{"error":"Payload too large"}"#);
}

#[tokio::test]
async fn form_over_limit_is_413() {
    let resp = warp::test::request()
        .method("POST")
        .path("/form")
        .body("name=a-very-long-name")
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 413);
}

#[tokio::test]
async fn non_utf8_form_is_422() {
    let resp = warp::test::request()
        .method("POST")
        .path("/form")
        .body(b"name=\xFF\xFE".as_slice())
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn path_params_are_percent_decoded() {
    let resp = warp::test::request()
        .path("/p/caf%C3%A9%20au+lait")
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body(), "café au+lait");

    let resp = warp::test::request()
        .path("/t/a%2Fb/%41")
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body(), "a/b|A");
}

#[test]
fn validate_path_params_decodes() {
    let p: Slug = validate_path_params(&[("slug", "x%20y")]).unwrap();
    assert_eq!(p.slug, "x y");
}

#[tokio::test]
async fn known_rejections_map_to_their_status() {
    let r = routes();
    let resp = warp::test::request()
        .method("DELETE")
        .path("/json")
        .reply(&r)
        .await;
    assert_eq!(resp.status(), 405);

    let resp = warp::test::request().path("/h").reply(&r).await;
    assert_eq!(resp.status(), 400);

    let resp = warp::test::request()
        .path("/q?x=notanumber")
        .reply(&r)
        .await;
    assert_eq!(resp.status(), 400);

    let only_get = warp::path("only").map(|| "x").recover(handle_rejection);
    let resp = warp::test::request().path("/nope").reply(&only_get).await;
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn unknown_rejection_is_500() {
    #[derive(Debug)]
    struct Custom;
    impl warp::reject::Reject for Custom {}
    let route = warp::any()
        .and_then(|| async { Err::<String, _>(warp::reject::custom(Custom)) })
        .recover(handle_rejection);
    let resp = warp::test::request().reply(&route).await;
    assert_eq!(resp.status(), 500);
}

#[tokio::test]
async fn chunked_body_without_content_length_is_accepted() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(warp::serve(routes()).incoming(listener).run());

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let body = r#"{"name":"Al"}"#;
    let req = format!(
        "POST /json HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n\
         Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(resp.ends_with("Al"), "{resp}");
}
