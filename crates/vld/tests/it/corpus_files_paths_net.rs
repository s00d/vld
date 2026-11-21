//! Corpus and stories: binary and system values — `bytes` (array / Base64 / hex / Base64url),
//! `duration`, `path`, `file` (features `file`, `file-advanced`), `ip_network` (`net`),
//! `socket_addr`, `json_value`, `decimal` (`decimal`).

use crate::common::*;
use serde_json::json;
use std::time::Duration;
use vld::error::{FormatKind, IssueCode, SizeOrigin};
use vld::prelude::*;

fn root(code: &str) -> Vec<(String, String)> {
    vec![(String::new(), code.to_string())]
}

/// The first issue code (panics on success).
#[track_caller]
fn first<T: std::fmt::Debug>(r: Result<T, VldError>) -> IssueCode {
    r.unwrap_err().issues[0].code.clone()
}

/// A path as the JSON string a request would carry.
fn s(p: &std::path::Path) -> serde_json::Value {
    json!(p.to_string_lossy())
}

/// Given `bytes()` in array mode and in each string encoding
/// When byte arrays, out-of-range / non-numeric elements and encoded strings are parsed
/// Then arrays become `Vec<u8>` with length bounds reported as `bytes` sizes, a bad element is
/// reported at its index (300 is `too_big` with maximum 255), a string in array mode is a type
/// error, and each encoding decodes its own alphabet and rejects anything else (including an empty
/// Base64 string and the standard alphabet under Base64url) as `invalid_format` of that encoding;
/// `type_error` replaces the type message.
#[test]
fn bytes_in_every_encoding() {
    let b = vld::bytes().min_len(2).max_len(4);
    assert_eq!(b.parse_value(&json!([1, 2, 3])).unwrap(), vec![1, 2, 3]);
    assert!(matches!(
        first(b.parse_value(&json!([1]))),
        IssueCode::TooSmall {
            origin: SizeOrigin::Bytes,
            ..
        }
    ));
    assert!(matches!(
        first(b.parse_value(&json!([1, 2, 3, 4, 5]))),
        IssueCode::TooBig {
            origin: SizeOrigin::Bytes,
            ..
        }
    ));
    let err = vld::bytes().parse("[1, 300]").unwrap_err();
    assert_eq!(issues(&err), [("[1]".into(), "too_big".into())]);
    assert!(matches!(
        &err.issues[0].code,
        IssueCode::TooBig { origin: SizeOrigin::Number, maximum, .. } if maximum == &json!(255)
    ));
    assert_eq!(
        issues(&vld::bytes().parse(r#"[1, "a"]"#).unwrap_err()),
        [("[1]".into(), "invalid_type".into())]
    );
    assert_eq!(
        issues(&vld::bytes().parse(r#""AQID""#).unwrap_err()),
        root("invalid_type")
    );
    assert_eq!(
        first(vld::bytes().len(2).parse_value(&json!([1]))),
        IssueCode::InvalidLength {
            origin: SizeOrigin::Bytes,
            expected: 2
        }
    );

    let fmt = |format| IssueCode::InvalidFormat { format };
    let b64 = vld::bytes().base64().non_empty();
    assert_eq!(b64.parse_value(&json!("AQID")).unwrap(), vec![1, 2, 3]);
    assert_eq!(
        first(b64.parse_value(&json!("@@@"))),
        fmt(FormatKind::Base64)
    );
    assert_eq!(first(b64.parse_value(&json!(""))), fmt(FormatKind::Base64));
    assert!(matches!(
        first(vld::bytes().base64().min_len(2).parse_value(&json!("AA=="))),
        IssueCode::TooSmall {
            origin: SizeOrigin::Bytes,
            ..
        }
    ));

    let hex = vld::bytes().hex();
    assert_eq!(hex.parse_value(&json!("0a0B0c")).unwrap(), vec![10, 11, 12]);
    for bad in ["0a0", "zz", "é"] {
        assert_eq!(
            first(hex.parse_value(&json!(bad))),
            fmt(FormatKind::Hex),
            "{bad}"
        );
    }

    let url = vld::bytes().base64url();
    assert_eq!(url.parse_value(&json!("AQID")).unwrap(), vec![1, 2, 3]);
    assert_eq!(url.parse_value(&json!("-_8")).unwrap(), vec![0xfb, 0xff]);
    for bad in ["!!", "+/8"] {
        assert_eq!(
            first(url.parse_value(&json!(bad))),
            fmt(FormatKind::Base64Url),
            "{bad}"
        );
    }

    let err = vld::bytes()
        .base64()
        .type_error("binary!")
        .parse_value(&json!(true))
        .unwrap_err();
    assert_eq!(err.issues[0].message, "binary!");
}

/// Given `duration()`, bare and bounded
/// When seconds (integer, fractional), `"10s"` / `"250ms"` / `"1.5s"` and ISO 8601 `PT…S` strings
/// are parsed, then malformed text, signs, exponents, non-finite numbers and other JSON types
/// Then valid forms become `Duration`s, negative numbers are `too_small` (minimum 0, inclusive),
/// every malformed form is `invalid_type`, and `min`/`max` (`Duration` or seconds) are inclusive
/// `duration` bounds; `type_error` replaces the type message.
#[test]
fn duration_corpus() {
    let d = vld::duration();
    let ok = [
        (json!(3), Duration::from_secs(3)),
        (json!(0), Duration::ZERO),
        (json!(1.5), Duration::from_millis(1500)),
        (json!("PT5S"), Duration::from_secs(5)),
        (json!("PT1.5S"), Duration::from_millis(1500)),
        (json!("PT0.25S"), Duration::from_millis(250)),
        (json!("10s"), Duration::from_secs(10)),
        (json!("1.5s"), Duration::from_millis(1500)),
        (json!("250ms"), Duration::from_millis(250)),
    ];
    for (input, want) in ok {
        assert_eq!(d.parse_value(&input).unwrap(), want, "{input}");
    }
    assert_eq!(
        first(d.parse("-1")),
        IssueCode::TooSmall {
            origin: SizeOrigin::Duration,
            minimum: json!(0),
            inclusive: true
        }
    );
    assert_eq!(issues(&d.parse("-0.5").unwrap_err()), root("too_small"));
    for bad in [
        r#""five""#,
        r#""""#,
        "null",
        "true",
        "1e300",
        r#""-1s""#,
        r#""+1s""#,
        r#""+250ms""#,
        r#""1e3s""#,
        r#""infs""#,
        r#""NaNs""#,
        r#""1.2.3s""#,
        r#"".s""#,
        r#""s""#,
        r#""ms""#,
        r#""1.5ms""#,
        r#""PTS""#,
    ] {
        assert_eq!(
            issues(&d.parse(bad).unwrap_err()),
            root("invalid_type"),
            "{bad}"
        );
    }

    let secs = vld::duration().min_secs(1).max_secs(10);
    assert!(matches!(
        first(secs.parse(r#""20s""#)),
        IssueCode::TooBig { origin: SizeOrigin::Duration, maximum, .. } if maximum == json!(10.0)
    ));
    assert!(matches!(
        first(secs.parse(r#""500ms""#)),
        IssueCode::TooSmall {
            origin: SizeOrigin::Duration,
            ..
        }
    ));
    let ms = vld::duration()
        .min(Duration::from_millis(100))
        .max(Duration::from_millis(200));
    assert!(ms.parse(r#""150ms""#).is_ok());
    assert!(ms.parse(r#""250ms""#).is_err());
    let mixed = vld::duration().min(Duration::from_millis(500)).max_secs(2);
    assert!(mixed.parse("0.5").is_ok());
    assert!(mixed.parse("0.499").is_err());
    assert!(mixed.parse("2.001").is_err());
    let err = vld::duration()
        .type_error("timeout?")
        .parse("null")
        .unwrap_err();
    assert_eq!(err.issues[0].message, "timeout?");
}

/// An absolute path on the host OS (`/tmp` is relative on Windows).
const ABS: &str = if cfg!(windows) { r"C:\tmp" } else { "/tmp" };

/// Given a config with path settings: relative and absolute shapes, `exists` / `file` / `dir`
/// against a temp directory, and an upload directory confined with `within(base)`
/// When paths of each kind (and of the wrong kind) are parsed
/// Then right shapes become `PathBuf`s; each failure has its own code (`path_not_relative`,
/// `path_not_absolute`, `path_not_found`, `path_not_file`, `path_not_dir`) and a non-string is a
/// type error (`type_error` replaces its message); under `within` a relative path is joined onto the base, and `..`, an outside
/// absolute path and (on unix) a symlink out of the base are `path_outside_base`.
#[test]
fn config_paths_are_checked_against_the_filesystem() {
    let rel = vld::path().relative();
    assert_eq!(
        rel.parse_value(&json!("src/lib.rs")).unwrap(),
        std::path::PathBuf::from("src/lib.rs")
    );
    assert_eq!(
        first(rel.parse_value(&json!(ABS))),
        IssueCode::PathNotRelative
    );
    let abs = vld::path().absolute();
    assert!(abs.parse_value(&json!(ABS)).is_ok());
    assert_eq!(
        first(abs.parse_value(&json!("tmp"))),
        IssueCode::PathNotAbsolute
    );
    assert_eq!(
        issues(&vld::path().parse_value(&json!(1)).unwrap_err()),
        root("invalid_type")
    );
    let err = vld::path()
        .type_error("path!")
        .parse_value(&json!(1))
        .unwrap_err();
    assert_eq!(err.issues[0].message, "path!");

    let dir = tempdir();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, b"x").unwrap();
    let missing = dir.path().join("missing");
    assert!(vld::path().exists().parse_value(&s(&file)).is_ok());
    assert!(vld::path().exists().parse_value(&s(dir.path())).is_ok());
    assert_eq!(
        issues(&vld::path().exists().parse_value(&s(&missing)).unwrap_err()),
        root("path_not_found")
    );
    assert!(vld::path().file().parse_value(&s(&file)).is_ok());
    assert_eq!(
        first(vld::path().file().parse_value(&s(dir.path()))),
        IssueCode::PathNotFile
    );
    assert!(vld::path().dir().parse_value(&s(dir.path())).is_ok());
    assert_eq!(
        first(vld::path().dir().parse_value(&s(&file))),
        IssueCode::PathNotDir
    );

    let base = tempdir();
    let outside = tempdir();
    let uploads = vld::path().within(base.path());
    assert_eq!(
        uploads.parse_value(&json!("sub/new.txt")).unwrap(),
        base.path().join("sub/new.txt")
    );
    for escape in [
        format!("{}/../etc/x", base.path().display()),
        "../../etc/passwd".to_string(),
        "sub/../../x".to_string(),
        outside.path().display().to_string(),
    ] {
        assert!(
            matches!(
                first(uploads.parse_value(&json!(escape))),
                IssueCode::PathOutsideBase { .. }
            ),
            "{escape}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), base.path().join("link")).unwrap();
        assert!(matches!(
            first(uploads.parse_value(&json!("link/x"))),
            IssueCode::PathOutsideBase { .. }
        ));
    }
}

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

/// Given an avatar upload endpoint: a non-empty PNG under 1 KiB with an allowed extension, and a
/// text-file endpoint confined to a base directory
/// When a real PNG (named `.PNG`, then `.bin`), a text file, a missing path and a directory are
/// submitted, as a path string or a `{"path": …}` object, with in-memory or path-only storage
/// Then the PNG is accepted with its size, lower-cased extension, media type, bytes and path; each
/// rule that fails reports its own code (`too_small` / `too_big` file sizes, inclusive; an empty
/// file under `non_empty`; extension / media / magic type not allowed, denied or unknown — plain
/// text has no detectable type), a directory is `path_not_file`, a missing file `path_not_found`,
/// an unreadable file `io_error`, a non-path a type error (`type_error` replaces its message);
/// path-only storage reads the bytes lazily, detects no media type unless a content rule needs
/// it, and still enforces every content rule; and `within` rejects absolute and `..` paths while
/// reading files inside the base.
#[cfg(feature = "file")]
#[test]
fn uploaded_file_is_checked_and_stored() {
    use vld::error::FileTypeKind;

    let dir = tempdir();
    let pic = dir.path().join("pic.PNG");
    let content = png(2, 3);
    std::fs::write(&pic, &content).unwrap();

    let avatar = vld::file()
        .non_empty()
        .max_size(1024)
        .extensions(&[".png", "JPG"])
        .media_type("image/png");
    let f = avatar.parse_value(&s(&pic)).unwrap();
    assert_eq!(f.size(), content.len() as u64);
    assert_eq!(f.extension(), Some("png"));
    assert_eq!(f.media_type(), Some("image/png"));
    assert_eq!(f.storage(), FileStorage::InMemory);
    assert_eq!(f.bytes(), Some(content.as_slice()));
    assert_eq!(f.path(), pic.as_path());
    assert_eq!(f.into_bytes(), Some(content.clone()));
    let empty = dir.path().join("empty.png");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(
        first(avatar.parse_path(&empty)),
        IssueCode::TooSmall {
            origin: SizeOrigin::File,
            minimum: json!(1),
            inclusive: true
        }
    );
    assert!(
        vld::file()
            .parse_value(&json!({"path": pic.to_string_lossy()}))
            .is_ok()
    );
    // The last storage call wins.
    let f = vld::file()
        .store_path_only()
        .store_in_memory()
        .parse_path(&pic)
        .unwrap();
    assert_eq!(f.storage(), FileStorage::InMemory);

    // Media and magic types come from the content, not the name.
    let bin = dir.path().join("pic.bin");
    std::fs::write(&bin, png(1, 1)).unwrap();
    assert!(
        vld::file()
            .media_types(&["image/*", "application/pdf"])
            .parse_path(&bin)
            .is_ok()
    );
    assert!(matches!(
        first(vld::file().media_types(&["application/pdf"]).parse_path(&bin)),
        IssueCode::FileTypeNotAllowed { kind: FileTypeKind::MediaType, actual, .. } if actual == "image/png"
    ));
    assert!(matches!(
        first(vld::file().deny_media_type("image/*").parse_path(&bin)),
        IssueCode::FileTypeDenied {
            kind: FileTypeKind::MediaType,
            ..
        }
    ));
    assert!(vld::file().allow_magic_type("PNG").parse_path(&bin).is_ok());
    assert!(matches!(
        first(vld::file().allow_magic_type("jpg").parse_path(&bin)),
        IssueCode::FileTypeNotAllowed {
            kind: FileTypeKind::MagicType,
            ..
        }
    ));
    assert_eq!(
        first(vld::file().deny_magic_type("png").parse_path(&bin)),
        IssueCode::FileTypeDenied {
            kind: FileTypeKind::MagicType,
            actual: "png".into()
        }
    );

    // Path-only storage reads the content only for a rule that needs it, and every such rule
    // still applies.
    let lazy = || vld::file().store_path_only();
    assert_eq!(lazy().parse_path(&bin).unwrap().media_type(), None);
    assert_eq!(
        lazy()
            .media_type("image/png")
            .parse_path(&bin)
            .unwrap()
            .media_type(),
        Some("image/png")
    );
    assert!(matches!(
        first(lazy().deny_media_type("image/*").parse_path(&bin)),
        IssueCode::FileTypeDenied {
            kind: FileTypeKind::MediaType,
            ..
        }
    ));
    assert!(matches!(
        first(lazy().allow_magic_type("jpg").parse_path(&bin)),
        IssueCode::FileTypeNotAllowed {
            kind: FileTypeKind::MagicType,
            ..
        }
    ));
    assert!(matches!(
        first(lazy().deny_magic_type("png").parse_path(&bin)),
        IssueCode::FileTypeDenied {
            kind: FileTypeKind::MagicType,
            ..
        }
    ));

    let txt = dir.path().join("a.txt");
    std::fs::write(&txt, b"hello").unwrap();
    assert_eq!(
        first(avatar.parse_path(&txt)),
        IssueCode::FileTypeNotAllowed {
            kind: FileTypeKind::Extension,
            actual: "txt".into(),
            allowed: vec!["png".into(), "jpg".into()]
        }
    );
    // Size bounds are inclusive; the storage call keeps the other rules.
    assert!(vld::file().min_size(5).max_size(5).parse_path(&txt).is_ok());
    assert!(matches!(
        first(vld::file().max_size(3).store_in_memory().parse_path(&txt)),
        IssueCode::TooBig {
            origin: SizeOrigin::File,
            ..
        }
    ));
    let err = vld::file()
        .type_error("upload!")
        .parse_value(&json!(1))
        .unwrap_err();
    assert_eq!(err.issues[0].message, "upload!");
    // A file that exists but cannot be opened is an `io_error` (skipped where the permission bits
    // do not stop the reader, e.g. root).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = dir.path().join("locked.txt");
        std::fs::write(&locked, b"x").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&locked).is_err() {
            assert_eq!(
                issues(&vld::file().parse_path(&locked).unwrap_err()),
                root("io_error")
            );
        }
    }
    assert!(matches!(
        first(vld::file().min_size(10).parse_value(&s(&txt))),
        IssueCode::TooSmall {
            origin: SizeOrigin::File,
            ..
        }
    ));
    assert!(matches!(
        first(vld::file().max_size(3).parse_path(&txt)),
        IssueCode::TooBig { origin: SizeOrigin::File, maximum, .. } if maximum == json!(3)
    ));
    assert!(matches!(
        first(vld::file().extension("png").parse_value(&s(&txt))),
        IssueCode::FileTypeNotAllowed { kind: FileTypeKind::Extension, actual, .. } if actual == "txt"
    ));
    assert!(matches!(
        first(vld::file().deny_extension(".TXT").parse_value(&s(&txt))),
        IssueCode::FileTypeDenied {
            kind: FileTypeKind::Extension,
            ..
        }
    ));
    assert_eq!(
        first(vld::file().media_type("text/plain").parse_value(&s(&txt))),
        IssueCode::FileTypeUnknown {
            kind: FileTypeKind::MediaType
        }
    );
    assert_eq!(
        first(vld::file().allow_magic_type("png").parse_value(&s(&txt))),
        IssueCode::FileTypeUnknown {
            kind: FileTypeKind::MagicType
        }
    );
    assert_eq!(
        issues(
            &vld::file()
                .parse_value(&s(&dir.path().join("missing.txt")))
                .unwrap_err()
        ),
        root("path_not_found")
    );
    assert_eq!(
        issues(&vld::file().parse_value(&s(dir.path())).unwrap_err()),
        root("path_not_file")
    );

    {
        use std::io::Read;
        let f = vld::file().store_path_only().parse_path(&txt).unwrap();
        assert_eq!(f.storage(), FileStorage::PathOnly);
        assert!(f.bytes().is_none());
        assert_eq!(f.read_bytes().unwrap(), b"hello");
        let mut text = String::new();
        f.open().unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(f.into_bytes(), None);
    }

    let base = tempdir();
    let data = base.path().join("data.txt");
    std::fs::write(&data, b"0123456789").unwrap();
    let confined = vld::file().within(base.path());
    for escape in [json!({"path": "/etc/passwd"}), json!("../outside.txt")] {
        assert!(
            matches!(
                first(confined.parse_value(&escape)),
                IssueCode::PathOutsideBase { .. }
            ),
            "{escape}"
        );
    }
    let ok = confined.parse_value(&json!("data.txt")).unwrap();
    assert_eq!(ok.path(), data.as_path());
    assert_eq!(ok.bytes(), Some(&b"0123456789"[..]));
}

/// Given a download check with expected MD5 / SHA-256 digests and an image endpoint with exact
/// dimensions and required EXIF (feature `file-advanced`)
/// When a file with known content and a 4×2 PNG are submitted, then wrong digests, out-of-range
/// dimensions and a text file in place of an image
/// Then matching digests (case-insensitive hex) pass and a wrong one is `checksum_mismatch`
/// naming the algorithm; each dimension bound reports `too_small` / `too_big` with an
/// `image_width` / `image_height` origin; a PNG without EXIF is `missing_exif`; and a non-image is
/// `invalid_format` (image).
#[cfg(feature = "file-advanced")]
#[test]
fn uploaded_file_checksums_and_image_dimensions() {
    let dir = tempdir();
    let abc = dir.path().join("abc.txt");
    std::fs::write(&abc, b"abc").unwrap();
    let digests = vld::file()
        .md5("900150983CD24FB0D6963F7D28E17F72")
        .sha256("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert!(digests.parse_path(&abc).is_ok());
    assert_eq!(
        first(vld::file().sha256("00").parse_path(&abc)),
        IssueCode::ChecksumMismatch {
            algorithm: "SHA-256".into()
        }
    );
    assert_eq!(
        first(vld::file().md5("00").parse_path(&abc)),
        IssueCode::ChecksumMismatch {
            algorithm: "MD5".into()
        }
    );
    // Digests are checked with path-only storage too.
    assert_eq!(
        first(vld::file().store_path_only().sha256("00").parse_path(&abc)),
        IssueCode::ChecksumMismatch {
            algorithm: "SHA-256".into()
        }
    );

    let pic = dir.path().join("pic.png");
    std::fs::write(&pic, png(4, 2)).unwrap();
    let exact = vld::file()
        .min_width(4)
        .max_width(4)
        .min_height(2)
        .max_height(2);
    assert!(exact.parse_path(&pic).is_ok());
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
        let err = schema.parse_path(&pic).unwrap_err();
        assert_eq!(issues(&err), root(code));
        match &err.issues[0].code {
            IssueCode::TooSmall { origin: o, .. } | IssueCode::TooBig { origin: o, .. } => {
                assert_eq!(*o, origin)
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        first(vld::file().require_exif().parse_path(&pic)),
        IssueCode::MissingExif
    );

    let txt = dir.path().join("a.txt");
    std::fs::write(&txt, b"not an image").unwrap();
    assert_eq!(
        first(vld::file().min_width(1).parse_path(&txt)),
        IssueCode::InvalidFormat {
            format: FormatKind::Image
        }
    );
}

/// Given network settings: an allowed subnet (`ip_network`, any family / IPv4 only / IPv6 only,
/// feature `net`) and a listen address with a port range (`socket_addr`)
/// When networks and addresses of each family, out-of-range ports and a hostname are parsed
/// Then networks round-trip and the wrong family or a bad prefix is `invalid_format` of that
/// family; addresses (IPv4 and bracketed IPv6) are accepted inside the port range and `too_small`
/// / `too_big` with a `port` origin outside it; a hostname is `invalid_format` (socket address)
/// and its message says hostnames are not resolved; `type_error` replaces each type message.
#[test]
fn network_settings() {
    #[cfg(feature = "net")]
    {
        let fmt = |format| IssueCode::InvalidFormat { format };
        let any = vld::ip_network();
        assert_eq!(
            any.parse_value(&json!("10.0.0.0/24")).unwrap().to_string(),
            "10.0.0.0/24"
        );
        assert!(any.parse_value(&json!("2001:db8::/32")).is_ok());
        assert_eq!(
            first(any.parse_value(&json!("10.0.0.0/33"))),
            fmt(FormatKind::IpNetwork)
        );
        let v4 = vld::ip_network().ipv4_only();
        assert!(v4.parse_value(&json!("10.0.0.0/24")).is_ok());
        assert_eq!(
            first(v4.parse_value(&json!("2001:db8::/32"))),
            fmt(FormatKind::Ipv4Network)
        );
        let v6 = vld::ip_network().ipv6_only();
        assert!(v6.parse_value(&json!("2001:db8::/32")).is_ok());
        assert_eq!(
            first(v6.parse_value(&json!("10.0.0.0/24"))),
            fmt(FormatKind::Ipv6Network)
        );
        let err = vld::ip_network()
            .type_error("cidr!")
            .parse_value(&json!(1))
            .unwrap_err();
        assert_eq!(err.issues[0].message, "cidr!");
    }

    let listen = vld::socket_addr().min_port(1024).max_port(9000);
    assert_eq!(
        listen.parse_value(&json!("127.0.0.1:8080")).unwrap().port(),
        8080
    );
    assert!(listen.parse_value(&json!("[::1]:8080")).is_ok());
    assert!(matches!(
        first(listen.parse_value(&json!("127.0.0.1:80"))),
        IssueCode::TooSmall {
            origin: SizeOrigin::Port,
            ..
        }
    ));
    assert!(matches!(
        first(listen.parse_value(&json!("127.0.0.1:9001"))),
        IssueCode::TooBig {
            origin: SizeOrigin::Port,
            ..
        }
    ));
    let err = vld::socket_addr()
        .parse_value(&json!("example.com:80"))
        .unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidFormat {
            format: FormatKind::SocketAddr
        }
    );
    assert!(
        err.issues[0].message.contains("hostnames are not resolved"),
        "{err}"
    );
    let err = vld::socket_addr()
        .type_error("addr!")
        .parse_value(&json!(80))
        .unwrap_err();
    assert_eq!(err.issues[0].message, "addr!");
}

/// Given a webhook payload accepted as raw JSON: an object that must have `id` and nest at most 3
/// levels, and an array variant
/// When payloads with and without `id`, too deep, and of the wrong container type are parsed
/// Then a good payload passes, a missing key is `missing_field` at `.id`, depth over 3 is
/// `json_too_deep` (max 3), the wrong container is a type error, and `type_error` replaces its
/// message.
#[test]
fn raw_json_payload_shape() {
    let s = vld::json_value().object().require_key("id").max_depth(3);
    assert!(s.parse_value(&json!({"id": 1, "x": {"a": 1}})).is_ok());
    assert_eq!(
        issues(&s.parse_value(&json!({"x": 1})).unwrap_err()),
        [(".id".into(), "missing_field".into())]
    );
    assert_eq!(
        first(s.parse_value(&json!({"id": 1, "x": {"a": {"b": {"c": 1}}}}))),
        IssueCode::JsonTooDeep { max_depth: 3 }
    );
    assert_eq!(
        issues(&s.parse_value(&json!([1])).unwrap_err()),
        root("invalid_type")
    );
    assert!(vld::json_value().array().parse_value(&json!([])).is_ok());
    assert_eq!(
        issues(
            &vld::json_value()
                .array()
                .parse_value(&json!({}))
                .unwrap_err()
        ),
        root("invalid_type")
    );
    let e = vld::json_value()
        .object()
        .type_error("obj!")
        .parse_value(&json!(1))
        .unwrap_err();
    assert_eq!(e.issues[0].message, "obj!");
}

/// Given decimal prices (feature `decimal`): an inclusive range, exclusive bounds with a step, and
/// sign checks
/// When decimal strings, exponent numbers and junk are parsed
/// Then values keep their scale (`"1.50"`), exponents expand (`1e-7` → `0.0000001`), junk is
/// `invalid_format` (decimal), bounds report `too_small` / `too_big` with a `decimal` origin and
/// the bound as written, a value off the step is `not_multiple_of`, and each sign check rejects
/// its edge; `type_error` replaces the type message.
#[cfg(feature = "decimal")]
#[test]
fn decimal_prices() {
    let d = vld::decimal().min("1.10").max("2.20");
    assert_eq!(d.parse_value(&json!("1.50")).unwrap().to_string(), "1.50");
    assert!(matches!(
        first(d.parse_value(&json!("0.99"))),
        IssueCode::TooSmall { origin: SizeOrigin::Decimal, minimum, .. } if minimum == json!("1.10")
    ));
    assert_eq!(
        vld::decimal().parse("1e-7").unwrap().to_string(),
        "0.0000001"
    );
    assert!(vld::decimal().parse("1.5e20").is_ok());
    assert_eq!(
        first(vld::decimal().parse(r#""abc""#)),
        IssueCode::InvalidFormat {
            format: FormatKind::Decimal
        }
    );

    let step = vld::decimal().gt("1").lt("2").multiple_of("0.25");
    assert!(step.parse(r#""1.5""#).is_ok());
    assert_eq!(
        issues(&step.parse(r#""1""#).unwrap_err()),
        root("too_small")
    );
    assert_eq!(issues(&step.parse(r#""2""#).unwrap_err()), root("too_big"));
    assert_eq!(
        issues(&step.parse(r#""1.3""#).unwrap_err()),
        root("not_multiple_of")
    );
    for (schema, bad) in [
        (vld::decimal().positive(), "0"),
        (vld::decimal().negative(), "0"),
        (vld::decimal().non_negative(), "-0.01"),
        (vld::decimal().non_positive(), "0.01"),
    ] {
        assert!(schema.parse_value(&json!(bad)).is_err(), "{bad}");
    }
    let err = vld::decimal()
        .type_error("price!")
        .parse("true")
        .unwrap_err();
    assert_eq!(err.issues[0].message, "price!");
}
