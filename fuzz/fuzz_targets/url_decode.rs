//! vld-http-common decoding: query strings, form bodies, cookies and
//! percent-decoding never panic; decoding is identity on text without `%`/`+`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = vld_http_common::parse_form_body(data);
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let decoded = vld_http_common::url_decode(s);
    let _ = vld_http_common::percent_decode(s);
    if !s.contains(['%', '+']) {
        assert_eq!(decoded, s);
    }
    let _ = vld_http_common::query_string_to_json(s);
    let _ = vld_http_common::cookies_to_json(s);
    let _ = vld_http_common::coerce_value(s);
});
