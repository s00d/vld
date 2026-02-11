use vld::prelude::*;

#[test]
fn email_msg() {
    let s = vld::string().email_msg("bad email!!!");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad email!!!"));
}

#[test]
fn uuid_msg() {
    let s = vld::string().uuid_msg("not a uuid");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("not a uuid"));
}

#[test]
fn url_msg() {
    let s = vld::string().url_msg("invalid url");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("invalid url"));
}

#[test]
fn ipv4_msg() {
    let s = vld::string().ipv4_msg("bad ipv4");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad ipv4"));
}

#[test]
fn ipv6_msg() {
    let s = vld::string().ipv6_msg("bad ipv6");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad ipv6"));
}

#[test]
fn base64_msg() {
    let s = vld::string().base64_msg("bad b64");
    let err = s.parse(r#""!!""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad b64"));
}

#[test]
fn iso_date_msg() {
    let s = vld::string().iso_date_msg("bad date");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad date"));
}

#[test]
fn iso_time_msg() {
    let s = vld::string().iso_time_msg("bad time");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad time"));
}

#[test]
fn iso_datetime_msg() {
    let s = vld::string().iso_datetime_msg("bad dt");
    let err = s.parse(r#""nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad dt"));
}

#[test]
fn hostname_msg() {
    let s = vld::string().hostname_msg("bad host");
    let err = s.parse(r#""-nope""#).unwrap_err();
    assert!(err.issues[0].message.contains("bad host"));
}

#[test]
fn non_empty_msg() {
    let s = vld::string().non_empty_msg("cannot be blank");
    let err = s.parse(r#""""#).unwrap_err();
    assert!(err.issues[0].message.contains("cannot be blank"));
}

#[test]
fn len_msg() {
    let s = vld::string().len_msg(5, "must be 5 chars");
    let err = s.parse(r#""hi""#).unwrap_err();
    assert!(err.issues[0].message.contains("must be 5 chars"));
}
