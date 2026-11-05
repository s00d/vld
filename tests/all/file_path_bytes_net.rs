//! `bytes`, `duration`, `path`, `file`, `ip_network`, `socket_addr`,
//! `json_value`, `decimal`.

use crate::common::{codes, parse_err, tempdir};
use serde_json::json;
use std::time::Duration;
use vld::error::{FormatKind, IssueCode, SizeOrigin};
use vld::prelude::*;

// --- bytes -------------------------------------------------------------------

#[test]
fn bytes_array_mode() {
    let b = vld::bytes().min_len(2).max_len(4);
    assert_eq!(b.parse_value(&json!([1, 2, 3])).unwrap(), vec![1, 2, 3]);
    assert_issue!(
        parse_err(&b, json!([1])),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Bytes,
            ..
        }
    );
    assert_issue!(
        parse_err(&b, json!([1, 2, 3, 4, 5])),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Bytes,
            ..
        }
    );
    // A bad element is reported at its index.
    assert_issue!(
        vld::bytes().parse("[1, 300]"),
        "[1]",
        IssueCode::TooBig { origin: SizeOrigin::Number, maximum, .. } if maximum == &json!(255)
    );
    assert_issue!(
        vld::bytes().parse(r#"[1, "a"]"#),
        "[1]",
        IssueCode::InvalidType { .. }
    );
    assert_issue!(
        vld::bytes().parse(r#""AQID""#),
        "",
        IssueCode::InvalidType { .. }
    );
    assert_issue!(
        parse_err(&vld::bytes().len(2), json!([1])),
        "",
        IssueCode::InvalidLength {
            origin: SizeOrigin::Bytes,
            expected: 2
        }
    );
}

#[test]
fn bytes_string_modes() {
    let b = vld::bytes().base64().non_empty();
    assert_eq!(b.parse_value(&json!("AQID")).unwrap(), vec![1, 2, 3]);
    assert_issue!(
        parse_err(&b, json!("@@@")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Base64
        }
    );
    // Like `string().base64()`, an empty string is not accepted as Base64.
    assert_issue!(
        parse_err(&b, json!("")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Base64
        }
    );
    assert_issue!(
        parse_err(&vld::bytes().base64().min_len(2), json!("AA==")),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Bytes,
            ..
        }
    );

    let hex = vld::bytes().hex();
    assert_eq!(hex.parse_value(&json!("0a0B0c")).unwrap(), vec![10, 11, 12]);
    for bad in ["0a0", "zz", "é"] {
        assert_issue!(
            parse_err(&hex, json!(bad)),
            "",
            IssueCode::InvalidFormat {
                format: FormatKind::Hex
            }
        );
    }

    let url = vld::bytes().base64url();
    assert_eq!(url.parse_value(&json!("AQID")).unwrap(), vec![1, 2, 3]);
    assert_eq!(url.parse_value(&json!("-_8")).unwrap(), vec![0xfb, 0xff]);
    assert_issue!(
        parse_err(&url, json!("!!")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Base64Url
        }
    );
}

#[test]
fn bytes_base64url_rejects_standard_alphabet() {
    assert!(vld::bytes().base64url().parse_value(&json!("+/8")).is_err());
}

// --- duration -----------------------------------------------------------------

#[test]
fn duration_formats_and_bounds() {
    let d = vld::duration();
    let table = [
        (json!(3), Duration::from_secs(3)),
        (json!("PT5S"), Duration::from_secs(5)),
        (json!("10s"), Duration::from_secs(10)),
        (json!("250ms"), Duration::from_millis(250)),
        (json!(1.5), Duration::from_millis(1500)),
        (json!("1.5s"), Duration::from_millis(1500)),
        (json!("PT1.5S"), Duration::from_millis(1500)),
    ];
    for (input, want) in table {
        assert_eq!(d.parse_value(&input).unwrap(), want, "{input}");
    }
    assert_issue!(
        parse_err(&d, json!(-1)),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Duration,
            ..
        }
    );
    for bad in [json!("five"), json!(""), json!(null)] {
        assert_issue!(parse_err(&d, bad), "", IssueCode::InvalidType { .. });
    }
    let bounded = vld::duration().min_secs(1).max_secs(10);
    assert_issue!(
        parse_err(&bounded, json!("20s")),
        "",
        IssueCode::TooBig { origin: SizeOrigin::Duration, maximum, .. } if maximum == &json!(10.0)
    );
    assert_issue!(
        parse_err(&bounded, json!("500ms")),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Duration,
            ..
        }
    );
    let bounded = vld::duration()
        .min(Duration::from_millis(100))
        .max(Duration::from_millis(200));
    assert!(bounded.parse_value(&json!("150ms")).is_ok());
    assert!(bounded.parse_value(&json!("250ms")).is_err());
}

// --- path ----------------------------------------------------------------------

#[test]
fn path_shape_checks() {
    let rel = vld::path().relative();
    assert_eq!(
        rel.parse_value(&json!("src/lib.rs")).unwrap(),
        std::path::PathBuf::from("src/lib.rs")
    );
    assert_issue!(
        parse_err(&rel, json!("/tmp")),
        "",
        IssueCode::PathNotRelative
    );
    let abs = vld::path().absolute();
    assert!(abs.parse_value(&json!("/tmp")).is_ok());
    assert_issue!(
        parse_err(&abs, json!("tmp")),
        "",
        IssueCode::PathNotAbsolute
    );
    assert_issue!(
        parse_err(&vld::path(), json!(1)),
        "",
        IssueCode::InvalidType { .. }
    );
}

#[test]
fn path_filesystem_checks() {
    let dir = tempdir();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, b"x").unwrap();
    let missing = dir.path().join("missing");
    let s = |p: &std::path::Path| json!(p.to_string_lossy());

    assert!(vld::path().exists().parse_value(&s(&file)).is_ok());
    assert!(vld::path().exists().parse_value(&s(dir.path())).is_ok());
    let err = parse_err(&vld::path().exists(), s(&missing));
    assert_eq!(codes(&err), ["io_error"]);
    assert!(
        err.issues[0].message.starts_with("Path does not exist"),
        "{err}"
    );

    assert!(vld::path().file().parse_value(&s(&file)).is_ok());
    assert!(parse_err(&vld::path().file(), s(dir.path())).issues[0]
        .message
        .starts_with("Path is not a file"));
    assert!(vld::path().dir().parse_value(&s(dir.path())).is_ok());
    assert!(parse_err(&vld::path().dir(), s(&file)).issues[0]
        .message
        .starts_with("Path is not a directory"));
}

/// `within(base)` joins relative input onto the base and cannot be escaped
/// with `..`, absolute paths or (on unix) symlinks.
#[test]
fn path_within_base() {
    let base = tempdir();
    let outside = tempdir();
    let schema = vld::path().within(base.path());

    let ok = schema.parse_value(&json!("sub/new.txt")).unwrap();
    assert_eq!(ok, base.path().join("sub/new.txt"));
    for escape in [
        format!("{}/../etc/x", base.path().display()),
        "../../etc/passwd".to_string(),
        "sub/../../x".to_string(),
        outside.path().display().to_string(),
    ] {
        assert_issue!(
            parse_err(&schema, json!(escape)),
            "",
            IssueCode::PathOutsideBase { .. }
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), base.path().join("link")).unwrap();
        assert_issue!(
            parse_err(&schema, json!("link/x")),
            "",
            IssueCode::PathOutsideBase { .. }
        );
    }
}

// --- file ------------------------------------------------------------------------

/// A valid `w`×`h` 8-bit grayscale PNG.
#[cfg(feature = "file")]
fn png(w: u32, h: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    // Raw scanlines (filter byte + pixels) in one stored zlib block.
    let raw = vec![0u8; (h * (w + 1)) as usize];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    let len = raw.len() as u16;
    let mut idat = vec![0x78, 0x01, 0x01];
    idat.extend_from_slice(&len.to_le_bytes());
    idat.extend_from_slice(&(!len).to_le_bytes());
    idat.extend_from_slice(&raw);
    idat.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(feature = "file")]
mod file {
    use super::*;
    use vld::error::FileTypeKind;

    fn s(p: &std::path::Path) -> serde_json::Value {
        json!(p.to_string_lossy())
    }

    #[test]
    fn size_extension_and_storage() {
        let dir = tempdir();
        let path = dir.path().join("pic.PNG");
        let content = png(2, 3);
        std::fs::write(&path, &content).unwrap();

        let schema = vld::file()
            .non_empty()
            .max_size(1024)
            .extensions(&[".png", "JPG"])
            .media_type("image/png");
        let f = schema.parse_value(&s(&path)).unwrap();
        assert_eq!(f.size(), content.len() as u64);
        assert_eq!(f.extension(), Some("png"));
        assert_eq!(f.media_type(), Some("image/png"));
        assert_eq!(f.storage(), FileStorage::InMemory);
        assert_eq!(f.bytes(), Some(content.as_slice()));
        assert_eq!(f.path(), path.as_path());
        assert_eq!(f.into_bytes(), Some(content.clone()));

        // `{"path": ...}` objects are accepted too.
        assert!(vld::file()
            .parse_value(&json!({"path": path.to_string_lossy()}))
            .is_ok());

        let f = vld::file()
            .store_path_only()
            .store_in_memory()
            .parse_path(&path)
            .unwrap();
        assert_eq!(f.storage(), FileStorage::InMemory);
    }

    #[test]
    fn rejections() {
        let dir = tempdir();
        let txt = dir.path().join("a.txt");
        std::fs::write(&txt, b"hello").unwrap();

        assert_issue!(
            parse_err(&vld::file().min_size(10), s(&txt)),
            "",
            IssueCode::TooSmall {
                origin: SizeOrigin::File,
                ..
            }
        );
        let err = vld::file().max_size(3).parse_path(&txt).unwrap_err();
        assert_issue!(&err, "", IssueCode::TooBig { origin: SizeOrigin::File, maximum, .. } if maximum == &json!(3));
        assert!(err.issues[0].message.contains("at most 3"), "{err}");
        assert_issue!(
            parse_err(&vld::file().extension("png"), s(&txt)),
            "",
            IssueCode::FileTypeNotAllowed { kind: FileTypeKind::Extension, actual, .. } if actual == "txt"
        );
        assert_issue!(
            parse_err(&vld::file().deny_extension(".TXT"), s(&txt)),
            "",
            IssueCode::FileTypeDenied {
                kind: FileTypeKind::Extension,
                ..
            }
        );
        // Plain text has no detectable media / magic type.
        assert_issue!(
            parse_err(&vld::file().media_type("text/plain"), s(&txt)),
            "",
            IssueCode::FileTypeUnknown {
                kind: FileTypeKind::MediaType
            }
        );
        assert_issue!(
            parse_err(&vld::file().allow_magic_type("png"), s(&txt)),
            "",
            IssueCode::FileTypeUnknown {
                kind: FileTypeKind::MagicType
            }
        );
        let missing = dir.path().join("missing.txt");
        assert!(parse_err(&vld::file(), s(&missing)).issues[0]
            .message
            .starts_with("File does not exist"));
        assert!(parse_err(&vld::file(), s(dir.path())).issues[0]
            .message
            .starts_with("Path is not a file"));
    }

    #[test]
    fn media_and_magic_allow_deny_lists() {
        let dir = tempdir();
        let path = dir.path().join("pic.bin");
        std::fs::write(&path, png(1, 1)).unwrap();

        assert!(vld::file()
            .media_types(&["image/*", "application/pdf"])
            .parse_path(&path)
            .is_ok());
        assert_issue!(
            vld::file().media_types(&["application/pdf"]).parse_path(&path),
            "",
            IssueCode::FileTypeNotAllowed { kind: FileTypeKind::MediaType, actual, .. } if actual == "image/png"
        );
        assert_issue!(
            vld::file().deny_media_type("image/*").parse_path(&path),
            "",
            IssueCode::FileTypeDenied {
                kind: FileTypeKind::MediaType,
                ..
            }
        );
        assert!(vld::file()
            .allow_magic_type("PNG")
            .parse_path(&path)
            .is_ok());
        assert_issue!(
            vld::file().allow_magic_type("jpg").parse_path(&path),
            "",
            IssueCode::FileTypeNotAllowed {
                kind: FileTypeKind::MagicType,
                ..
            }
        );
        assert_issue!(
            vld::file().deny_magic_type("png").parse_path(&path),
            "",
            IssueCode::FileTypeDenied { kind: FileTypeKind::MagicType, actual } if actual == "png"
        );
    }

    #[test]
    fn path_only_storage_reads_lazily() {
        use std::io::Read;
        let dir = tempdir();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello path-only").unwrap();

        let f = vld::file().store_path_only().parse_path(&path).unwrap();
        assert_eq!(f.storage(), FileStorage::PathOnly);
        assert!(f.bytes().is_none());
        assert_eq!(f.read_bytes().unwrap(), b"hello path-only");
        let mut text = String::new();
        f.open().unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "hello path-only");
        assert_eq!(f.into_bytes(), None);
    }

    #[test]
    fn within_base() {
        let base = tempdir();
        let f = base.path().join("data.txt");
        std::fs::write(&f, b"0123456789").unwrap();
        let schema = vld::file().within(base.path());
        assert_issue!(
            parse_err(&schema, json!({"path": "/etc/passwd"})),
            "",
            IssueCode::PathOutsideBase { .. }
        );
        assert_issue!(
            parse_err(&schema, json!("../outside.txt")),
            "",
            IssueCode::PathOutsideBase { .. }
        );
        let ok = schema.parse_value(&json!("data.txt")).unwrap();
        assert_eq!(ok.path(), f.as_path());
        assert_eq!(ok.bytes(), Some(&b"0123456789"[..]));
    }

    #[cfg(feature = "file-advanced")]
    #[test]
    fn hashes() {
        let dir = tempdir();
        let path = dir.path().join("abc.txt");
        std::fs::write(&path, b"abc").unwrap();
        let ok = vld::file()
            .md5("900150983CD24FB0D6963F7D28E17F72")
            .sha256("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(ok.parse_path(&path).is_ok());
        assert_issue!(
            vld::file().sha256("00").parse_path(&path),
            "",
            IssueCode::ChecksumMismatch { algorithm } if algorithm == "SHA-256"
        );
        assert_issue!(
            vld::file().md5("00").parse_path(&path),
            "",
            IssueCode::ChecksumMismatch { algorithm } if algorithm == "MD5"
        );
    }

    #[cfg(feature = "file-advanced")]
    #[test]
    fn image_dimensions_and_exif() {
        let dir = tempdir();
        let path = dir.path().join("pic.png");
        std::fs::write(&path, png(4, 2)).unwrap();

        let ok = vld::file()
            .min_width(4)
            .max_width(4)
            .min_height(2)
            .max_height(2);
        assert!(ok.parse_path(&path).is_ok());
        let table = [
            (
                vld::file().min_width(5),
                SizeOrigin::ImageWidth,
                "too_small",
            ),
            (vld::file().max_width(3), SizeOrigin::ImageWidth, "too_big"),
            (
                vld::file().min_height(3),
                SizeOrigin::ImageHeight,
                "too_small",
            ),
            (
                vld::file().max_height(1),
                SizeOrigin::ImageHeight,
                "too_big",
            ),
        ];
        for (schema, origin, code) in table {
            let err = schema.parse_path(&path).unwrap_err();
            assert_eq!(codes(&err), [code]);
            match &err.issues[0].code {
                IssueCode::TooSmall { origin: o, .. } | IssueCode::TooBig { origin: o, .. } => {
                    assert_eq!(*o, origin)
                }
                other => panic!("{other:?}"),
            }
        }
        assert_issue!(
            vld::file().require_exif().parse_path(&path),
            "",
            IssueCode::MissingExif
        );

        let txt = dir.path().join("a.txt");
        std::fs::write(&txt, b"not an image").unwrap();
        assert_issue!(
            vld::file().min_width(1).parse_path(&txt),
            "",
            IssueCode::InvalidFormat {
                format: FormatKind::Image
            }
        );
    }
}

// --- network / json / decimal ---------------------------------------------------

#[cfg(feature = "net")]
#[test]
fn ip_network_families() {
    let any = vld::ip_network();
    assert_eq!(
        any.parse_value(&json!("10.0.0.0/24")).unwrap().to_string(),
        "10.0.0.0/24"
    );
    assert!(any.parse_value(&json!("2001:db8::/32")).is_ok());
    assert_issue!(
        parse_err(&any, json!("10.0.0.0/33")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::IpNetwork
        }
    );

    let v4 = vld::ip_network().ipv4_only();
    assert!(v4.parse_value(&json!("10.0.0.0/24")).is_ok());
    assert_issue!(
        parse_err(&v4, json!("2001:db8::/32")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Ipv4Network
        }
    );
    let v6 = vld::ip_network().ipv6_only();
    assert!(v6.parse_value(&json!("2001:db8::/32")).is_ok());
    assert_issue!(
        parse_err(&v6, json!("10.0.0.0/24")),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Ipv6Network
        }
    );
}

#[test]
fn socket_addr_ports() {
    let s = vld::socket_addr().min_port(1024).max_port(9000);
    assert_eq!(
        s.parse_value(&json!("127.0.0.1:8080")).unwrap().port(),
        8080
    );
    assert!(s.parse_value(&json!("[::1]:8080")).is_ok());
    assert_issue!(
        parse_err(&s, json!("127.0.0.1:80")),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Port,
            ..
        }
    );
    assert_issue!(
        parse_err(&s, json!("127.0.0.1:9001")),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Port,
            ..
        }
    );
    let err = parse_err(&vld::socket_addr(), json!("example.com:80"));
    assert_issue!(
        &err,
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::SocketAddr
        }
    );
    assert!(
        err.issues[0].message.contains("hostnames are not resolved"),
        "{err}"
    );
}

#[test]
fn json_value_shape() {
    let s = vld::json_value().object().require_key("id").max_depth(3);
    assert!(s.parse_value(&json!({"id": 1, "x": {"a": 1}})).is_ok());
    assert_issue!(
        parse_err(&s, json!({"x": 1})),
        ".id",
        IssueCode::MissingField
    );
    assert_issue!(
        parse_err(&s, json!({"id": 1, "x": {"a": {"b": {"c": 1}}}})),
        "",
        IssueCode::JsonTooDeep { max_depth: 3 }
    );
    assert_issue!(parse_err(&s, json!([1])), "", IssueCode::InvalidType { .. });
    assert!(vld::json_value().array().parse_value(&json!([])).is_ok());
    assert_issue!(
        parse_err(&vld::json_value().array(), json!({})),
        "",
        IssueCode::InvalidType { .. }
    );
    let e = parse_err(&vld::json_value().object().type_error("obj!"), json!(1));
    assert_eq!(e.issues[0].message, "obj!");
}

#[cfg(feature = "decimal")]
#[test]
fn decimal_bounds_and_checks() {
    let d = vld::decimal().min("1.10").max("2.20");
    assert_eq!(d.parse_value(&json!("1.50")).unwrap().to_string(), "1.50");
    assert_issue!(
        parse_err(&d, json!("0.99")),
        "",
        IssueCode::TooSmall { origin: SizeOrigin::Decimal, minimum, .. } if minimum == &json!("1.10")
    );
    assert_eq!(
        vld::decimal().parse("1e-7").unwrap().to_string(),
        "0.0000001"
    );
    assert!(vld::decimal().parse("1.5e20").is_ok());
    assert_issue!(
        vld::decimal().parse(r#""abc""#),
        "",
        IssueCode::InvalidFormat {
            format: FormatKind::Decimal
        }
    );

    let s = vld::decimal().gt("1").lt("2").multiple_of("0.25");
    assert!(s.parse(r#""1.5""#).is_ok());
    assert_eq!(codes(&s.parse(r#""1""#).unwrap_err()), ["too_small"]);
    assert_eq!(codes(&s.parse(r#""2""#).unwrap_err()), ["too_big"]);
    assert_eq!(
        codes(&s.parse(r#""1.3""#).unwrap_err()),
        ["not_multiple_of"]
    );
    for (schema, bad) in [
        (vld::decimal().positive(), "0"),
        (vld::decimal().negative(), "0"),
        (vld::decimal().non_negative(), "-0.01"),
        (vld::decimal().non_positive(), "0.01"),
    ] {
        assert!(schema.parse_value(&json!(bad)).is_err(), "{bad}");
    }
}
