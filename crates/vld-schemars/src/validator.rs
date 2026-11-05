use serde_json::Value;
use vld::error::{IssueCode, PathSegment, ValidationIssue, VldError};

/// Max `$ref` hops on one value — guards self-referencing schemas like `{"$ref": "#"}`.
const MAX_REF_DEPTH: usize = 64;

/// Root schema (for `$ref` resolution) plus current `$ref` nesting depth.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub root: &'a Value,
    pub depth: usize,
}

pub(crate) fn validate_value_against_schema(
    schema: &Value,
    value: &Value,
    path: &[PathSegment],
    cx: Ctx,
) -> Result<(), VldError> {
    // Boolean schema: true = accept all, false = reject all
    if let Some(b) = schema.as_bool() {
        if b {
            return Ok(());
        } else {
            return Err(make_error(
                path,
                IssueCode::Custom {
                    code: "false_schema".into(),
                },
                "Value rejected by false schema",
                Some(value),
            ));
        }
    }

    let schema_obj = match schema.as_object() {
        Some(obj) => obj,
        None => return Ok(()),
    };

    let mut errors = VldError::new();

    // $ref (siblings still apply, as in draft 2019-09+)
    if let Some(reference) = schema_obj.get("$ref").and_then(|r| r.as_str()) {
        match resolve_ref(cx.root, reference) {
            Some(target) if cx.depth < MAX_REF_DEPTH => {
                let inner = Ctx {
                    depth: cx.depth + 1,
                    ..cx
                };
                if let Err(e) = validate_value_against_schema(target, value, path, inner) {
                    errors = errors.merge(e);
                }
            }
            Some(_) => errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "ref_depth".into(),
                },
                format!("$ref nesting too deep at {}", reference),
                Some(value),
            )),
            None => errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "invalid_ref".into(),
                },
                format!("Unresolvable $ref in schema: {}", reference),
                Some(value),
            )),
        }
    }

    // enum
    if let Some(enum_values) = schema_obj.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.contains(value) {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "enum".into(),
                },
                format!(
                    "Value must be one of: {}",
                    enum_values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some(value),
            ));
        }
    }

    // const
    if let Some(const_val) = schema_obj.get("const") {
        if value != const_val {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "const".into(),
                },
                format!("Value must be {}", const_val),
                Some(value),
            ));
        }
    }

    // type checking
    if let Some(type_val) = schema_obj.get("type") {
        let types: Vec<&str> = if let Some(s) = type_val.as_str() {
            vec![s]
        } else if let Some(arr) = type_val.as_array() {
            arr.iter().filter_map(|v| v.as_str()).collect()
        } else {
            vec![]
        };

        if !types.is_empty() && !type_matches(value, &types) {
            errors.issues.push(make_issue(
                path,
                IssueCode::InvalidType {
                    expected: types.join(" | "),
                    received: json_type_name(value).to_string(),
                },
                format!(
                    "Expected type {}, received {}",
                    types.join(" | "),
                    json_type_name(value)
                ),
                Some(value),
            ));
            return Err(errors);
        }
    }

    // String constraints
    if value.is_string() {
        validate_string(schema_obj, value, path, &mut errors);
    }

    // Number constraints
    if value.is_number() {
        validate_number(schema_obj, value, path, &mut errors);
    }

    // Array constraints
    if let Some(arr) = value.as_array() {
        validate_array(schema_obj, arr, path, &mut errors, cx);
    }

    // Object constraints
    if let Some(obj) = value.as_object() {
        validate_object(schema_obj, obj, path, &mut errors, cx);
    }

    // oneOf
    if let Some(one_of) = schema_obj.get("oneOf").and_then(|v| v.as_array()) {
        let matches: usize = one_of
            .iter()
            .filter(|s| validate_value_against_schema(s, value, path, cx).is_ok())
            .count();
        if matches != 1 {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "oneOf".into(),
                },
                format!(
                    "Value must match exactly one of the schemas (matched {})",
                    matches
                ),
                Some(value),
            ));
        }
    }

    // anyOf
    if let Some(any_of) = schema_obj.get("anyOf").and_then(|v| v.as_array()) {
        let matches = any_of
            .iter()
            .any(|s| validate_value_against_schema(s, value, path, cx).is_ok());
        if !matches {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "anyOf".into(),
                },
                "Value must match at least one of the schemas",
                Some(value),
            ));
        }
    }

    // allOf
    if let Some(all_of) = schema_obj.get("allOf").and_then(|v| v.as_array()) {
        for sub_schema in all_of {
            if let Err(e) = validate_value_against_schema(sub_schema, value, path, cx) {
                errors = errors.merge(e);
            }
        }
    }

    // not
    if let Some(not_schema) = schema_obj.get("not") {
        if validate_value_against_schema(not_schema, value, path, cx).is_ok() {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom { code: "not".into() },
                "Value must NOT match the schema",
                Some(value),
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_string(
    schema: &serde_json::Map<String, Value>,
    value: &Value,
    path: &[PathSegment],
    errors: &mut VldError,
) {
    let s = match value.as_str() {
        Some(s) => s,
        None => return,
    };

    let len = s.chars().count() as u64;
    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        if len < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    origin: vld::error::SizeOrigin::String,
                    minimum: (min as f64).into(),
                    inclusive: true,
                },
                format!("String must be at least {} characters", min),
                Some(value),
            ));
        }
    }

    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        if len > max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    origin: vld::error::SizeOrigin::String,
                    maximum: (max as f64).into(),
                    inclusive: true,
                },
                format!("String must be at most {} characters", max),
                Some(value),
            ));
        }
    }

    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        match regex_lite::Regex::new(pattern) {
            Ok(re) if re.is_match(s) => {}
            Ok(_) => errors.issues.push(make_issue(
                path,
                IssueCode::InvalidString {
                    validation: vld::error::StringValidation::Regex,
                },
                format!("String does not match pattern: {}", pattern),
                Some(value),
            )),
            Err(_) => errors.issues.push(invalid_pattern(path, pattern)),
        }
    }

    if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
        let valid = match format {
            // Same shape rule as vld: `local@domain`, no dot required (`a@b`).
            "email" => {
                matches!(s.rsplit_once('@'), Some((l, d)) if !l.is_empty() && !d.is_empty() && !s.contains(char::is_whitespace))
            }
            "uri" => is_uri(s),
            "url" => s.starts_with("http://") || s.starts_with("https://"),
            "url-strict" => s.starts_with("http://") || s.starts_with("https://"),
            "uuid" => s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "uuid-v1" | "uuid-v4" | "uuid-v7" => {
                s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
            }
            "phone-e164" => s.starts_with('+'),
            "semver-full" => s.contains('.'),
            "ipv4" => s.parse::<std::net::Ipv4Addr>().is_ok(),
            "ipv6" => s.parse::<std::net::Ipv6Addr>().is_ok(),
            "date" => {
                s.len() == 10 && s.chars().nth(4) == Some('-') && s.chars().nth(7) == Some('-')
            }
            "date-time" => s.contains('T') || s.contains('t'),
            "country-code" => s.len() == 2,
            "currency-code" => s.len() == 3,
            "locale" => s.len() == 2 || s.len() == 5,
            _ => true, // unknown formats pass
        };
        if !valid {
            errors.issues.push(make_issue(
                path,
                IssueCode::InvalidString {
                    validation: format_to_string_validation(format),
                },
                format!("Invalid format: expected {}", format),
                Some(value),
            ));
        }

        // formatMinimum / formatMaximum (inclusive) for date, date-time and time.
        for (key, too_small) in [("formatMinimum", true), ("formatMaximum", false)] {
            let Some(bound) = schema.get(key).and_then(Value::as_str) else {
                continue;
            };
            let Some(order) = cmp_temporal(format, s, bound) else {
                continue;
            };
            let bad = if too_small {
                order == std::cmp::Ordering::Less
            } else {
                order == std::cmp::Ordering::Greater
            };
            if bad {
                errors.issues.push(make_issue(
                    path,
                    IssueCode::Custom {
                        code: if too_small { "too_small" } else { "too_big" }.into(),
                    },
                    format!(
                        "Value must be {} {}",
                        if too_small { ">=" } else { "<=" },
                        bound
                    ),
                    Some(value),
                ));
            }
        }
    }
}

/// Order of two `date` / `date-time` / `time` strings; `None` if either does
/// not parse (or for other formats). A date-time without an offset is taken
/// as UTC.
fn cmp_temporal(format: &str, a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let key = |s: &str| -> Option<(i64, u32)> {
        match format {
            "date" => Some((days(s)? * 86_400, 0)),
            "time" => time_of_day(s),
            "date-time" => {
                let (date, rest) = s.split_at_checked(10)?;
                let rest = rest.strip_prefix(['T', 't', ' '])?;
                let (time, offset) = match rest.strip_suffix(['Z', 'z']) {
                    Some(t) => (t, 0),
                    None => match rest
                        .len()
                        .checked_sub(6)
                        .and_then(|i| rest.split_at_checked(i))
                    {
                        Some((t, o))
                            if o.is_ascii()
                                && o.starts_with(['+', '-'])
                                && o.as_bytes()[3] == b':' =>
                        {
                            let h: i64 = o[1..3].parse().ok()?;
                            let m: i64 = o[4..].parse().ok()?;
                            let sign = if o.starts_with('-') { -1 } else { 1 };
                            (t, sign * (h * 3600 + m * 60))
                        }
                        _ => (rest, 0),
                    },
                };
                let (secs, nanos) = time_of_day(time)?;
                Some((days(date)? * 86_400 + secs - offset, nanos))
            }
            _ => None,
        }
    };
    Some(key(a)?.cmp(&key(b)?))
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date.
fn days(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !s.is_ascii() {
        return None;
    }
    let y: i64 = s[..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..].parse().ok()?;
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// `HH:MM[:SS[.frac]]` → (seconds of day, nanoseconds).
fn time_of_day(s: &str) -> Option<(i64, u32)> {
    let mut parts = s.splitn(3, ':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let (sec, nanos) = match parts.next() {
        None => (0, 0),
        Some(rest) => {
            let (sec, frac) = rest.split_once('.').unwrap_or((rest, ""));
            let digits: String = frac.chars().chain(std::iter::repeat('0')).take(9).collect();
            (sec.parse().ok()?, digits.parse().ok()?)
        }
    };
    Some((h * 3600 + m * 60 + sec, nanos))
}

fn validate_number(
    schema: &serde_json::Map<String, Value>,
    value: &Value,
    path: &[PathSegment],
    errors: &mut VldError,
) {
    use std::cmp::Ordering::{Greater, Less};
    if !value.is_number() {
        return;
    }
    // (keyword, failing orderings of value vs bound, too small?, inclusive, op)
    let checks: [(&str, &[std::cmp::Ordering], bool, bool, &str); 4] = [
        ("minimum", &[Less], true, true, ">="),
        ("maximum", &[Greater], false, true, "<="),
        (
            "exclusiveMinimum",
            &[Less, std::cmp::Ordering::Equal],
            true,
            false,
            ">",
        ),
        (
            "exclusiveMaximum",
            &[Greater, std::cmp::Ordering::Equal],
            false,
            false,
            "<",
        ),
    ];
    for (key, fails, small, inclusive, op) in checks {
        let Some(bound) = schema.get(key).filter(|b| b.is_number()) else {
            continue;
        };
        if !cmp_numbers(value, bound).is_some_and(|o| fails.contains(&o)) {
            continue;
        }
        let b = bound.as_f64().unwrap_or_default();
        let code = if small {
            IssueCode::TooSmall {
                origin: vld::error::SizeOrigin::Number,
                minimum: b.into(),
                inclusive,
            }
        } else {
            IssueCode::TooBig {
                origin: vld::error::SizeOrigin::Number,
                maximum: b.into(),
                inclusive,
            }
        };
        errors.issues.push(make_issue(
            path,
            code,
            format!("Value must be {} {}", op, bound),
            Some(value),
        ));
    }

    if let Some(multiple) = schema.get("multipleOf").filter(|m| m.is_number()) {
        let ok = match (as_int(value), as_int(multiple)) {
            (Some(n), Some(m)) if m > 0 => n % m == 0,
            _ => match (value.as_f64(), multiple.as_f64()) {
                (Some(n), Some(m)) if m > 0.0 => is_multiple_of(n, m),
                _ => true,
            },
        };
        if !ok {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "multipleOf".into(),
                },
                format!("Value must be a multiple of {}", multiple),
                Some(value),
            ));
        }
    }
}

/// An integer JSON number, exactly (`i64` / `u64` range).
fn as_int(v: &Value) -> Option<i128> {
    v.as_i64()
        .map(i128::from)
        .or_else(|| v.as_u64().map(i128::from))
}

/// Exact for two integers (no `f64` rounding of large `u64` / `i64`), `f64` otherwise.
fn cmp_numbers(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (as_int(a), as_int(b)) {
        (Some(x), Some(y)) => Some(x.cmp(&y)),
        _ => a.as_f64()?.partial_cmp(&b.as_f64()?),
    }
}

fn validate_array(
    schema: &serde_json::Map<String, Value>,
    arr: &[Value],
    path: &[PathSegment],
    errors: &mut VldError,
    cx: Ctx,
) {
    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        if (arr.len() as u64) < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    origin: vld::error::SizeOrigin::Array,
                    minimum: (min as f64).into(),
                    inclusive: true,
                },
                format!("Array must have at least {} items", min),
                Some(&Value::Number(serde_json::Number::from(arr.len()))),
            ));
        }
    }

    if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
        if (arr.len() as u64) > max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    origin: vld::error::SizeOrigin::Array,
                    maximum: (max as f64).into(),
                    inclusive: true,
                },
                format!("Array must have at most {} items", max),
                Some(&Value::Number(serde_json::Number::from(arr.len()))),
            ));
        }
    }

    // Tuple items: `prefixItems` (2020-12) or array-form `items` (draft-07).
    let (prefix, rest): (&[Value], Option<&Value>) =
        match (schema.get("prefixItems"), schema.get("items")) {
            (Some(Value::Array(p)), items) => (p, items),
            (_, Some(Value::Array(p))) => (p, schema.get("additionalItems")),
            (_, items) => (&[], items),
        };
    for (i, item) in arr.iter().enumerate() {
        let Some(item_schema) = prefix.get(i).or(rest) else {
            break;
        };
        let mut item_path = path.to_vec();
        item_path.push(PathSegment::Index(i));
        if let Err(e) = validate_value_against_schema(item_schema, item, &item_path, cx) {
            *errors = std::mem::take(errors).merge(e);
        }
    }

    if let Some(true) = schema.get("uniqueItems").and_then(|v| v.as_bool()) {
        let mut seen = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            if seen.contains(&item) {
                let mut item_path = path.to_vec();
                item_path.push(PathSegment::Index(i));
                errors.issues.push(make_issue(
                    &item_path,
                    IssueCode::Custom {
                        code: "uniqueItems".into(),
                    },
                    "Array items must be unique",
                    Some(item),
                ));
                break;
            }
            seen.push(item);
        }
    }

    // contains / minContains (default 1) / maxContains
    if let Some(contains) = schema.get("contains") {
        let count = arr
            .iter()
            .filter(|item| validate_value_against_schema(contains, item, path, cx).is_ok())
            .count() as u64;
        let min = schema
            .get("minContains")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let max = schema.get("maxContains").and_then(Value::as_u64);
        if count < min || max.is_some_and(|m| count > m) {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "contains".into(),
                },
                match max {
                    Some(m) => format!(
                        "Array must contain {min}..={m} items matching {contains} (found {count})"
                    ),
                    None => format!(
                        "Array must contain at least {min} items matching {contains} (found {count})"
                    ),
                },
                None,
            ));
        }
    }
}

fn validate_object(
    schema: &serde_json::Map<String, Value>,
    obj: &serde_json::Map<String, Value>,
    path: &[PathSegment],
    errors: &mut VldError,
    cx: Ctx,
) {
    // required
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for req in required {
            if let Some(field_name) = req.as_str() {
                if !obj.contains_key(field_name) {
                    let mut field_path = path.to_vec();
                    field_path.push(PathSegment::Field(field_name.to_string()));
                    errors.issues.push(make_issue(
                        &field_path,
                        IssueCode::MissingField,
                        format!("Required field '{}' is missing", field_name),
                        None,
                    ));
                }
            }
        }
    }

    // properties / patternProperties / additionalProperties
    let properties = schema.get("properties").and_then(|v| v.as_object());
    let mut patterns = Vec::new();
    for (p, sub) in schema
        .get("patternProperties")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
    {
        match regex_lite::Regex::new(p) {
            Ok(re) => patterns.push((re, sub)),
            Err(_) => errors.issues.push(invalid_pattern(path, p)),
        }
    }
    let additional = schema.get("additionalProperties");

    for (key, prop_value) in obj {
        let mut prop_path = path.to_vec();
        prop_path.push(PathSegment::Field(key.clone()));
        let mut subs: Vec<&Value> = properties.and_then(|p| p.get(key)).into_iter().collect();
        subs.extend(
            patterns
                .iter()
                .filter(|(re, _)| re.is_match(key))
                .map(|(_, sub)| *sub),
        );
        if subs.is_empty() {
            match additional {
                Some(Value::Bool(false)) => {
                    errors.issues.push(make_issue(
                        &prop_path,
                        IssueCode::UnrecognizedField,
                        format!("Unrecognized field '{}'", key),
                        Some(prop_value),
                    ));
                }
                Some(sub @ Value::Object(_)) => subs.push(sub),
                _ => {}
            }
        }
        for sub in subs {
            if let Err(e) = validate_value_against_schema(sub, prop_value, &prop_path, cx) {
                *errors = std::mem::take(errors).merge(e);
            }
        }
    }

    // propertyNames: every key is a string valid against the schema.
    if let Some(names) = schema.get("propertyNames") {
        for key in obj.keys() {
            let mut key_path = path.to_vec();
            key_path.push(PathSegment::Field(key.clone()));
            let k = Value::String(key.clone());
            if let Err(e) = validate_value_against_schema(names, &k, &key_path, cx) {
                *errors = std::mem::take(errors).merge(e);
            }
        }
    }

    // minProperties / maxProperties
    if let Some(min) = schema.get("minProperties").and_then(|v| v.as_u64()) {
        if (obj.len() as u64) < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    origin: vld::error::SizeOrigin::Object,
                    minimum: (min as f64).into(),
                    inclusive: true,
                },
                format!("Object must have at least {} properties", min),
                None,
            ));
        }
    }

    if let Some(max) = schema.get("maxProperties").and_then(|v| v.as_u64()) {
        if (obj.len() as u64) > max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    origin: vld::error::SizeOrigin::Object,
                    maximum: (max as f64).into(),
                    inclusive: true,
                },
                format!("Object must have at most {} properties", max),
                None,
            ));
        }
    }
}

// ========================= Helpers ===========================================

fn invalid_pattern(path: &[PathSegment], pattern: &str) -> ValidationIssue {
    make_issue(
        path,
        IssueCode::Custom {
            code: "invalid_pattern".into(),
        },
        format!("Invalid pattern in schema: {}", pattern),
        None,
    )
}

/// Resolve a local `$ref` (`#` or `#/json/pointer`) against the root schema.
fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    if pointer.is_empty() {
        return Some(root);
    }
    // schemars percent-encodes definition names inside refs.
    root.pointer(&percent_decode(pointer))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// RFC 3986 URI: `scheme ":" rest`, scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ).
fn is_uri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !rest.is_empty()
        && !s.contains(char::is_whitespace)
}

/// `n` is a multiple of `m`, tolerant to float rounding (e.g. 0.3 / 0.1).
fn is_multiple_of(n: f64, m: f64) -> bool {
    let q = n / m;
    (q - q.round()).abs() <= 1e-9 * q.abs().max(1.0)
}

fn type_matches(value: &Value, types: &[&str]) -> bool {
    types.iter().any(|t| match *t {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64() || (value.is_f64() && is_whole(value)),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => false,
    })
}

fn is_whole(value: &Value) -> bool {
    value.as_f64().map(|n| n.fract() == 0.0).unwrap_or(false)
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn format_to_string_validation(format: &str) -> vld::error::StringValidation {
    match format {
        "email" => vld::error::StringValidation::Email,
        "uri" | "url" => vld::error::StringValidation::Url,
        "url-strict" => vld::error::StringValidation::UrlStrict,
        "uuid" => vld::error::StringValidation::Uuid,
        "uuid-v1" => vld::error::StringValidation::UuidV1,
        "uuid-v4" => vld::error::StringValidation::UuidV4,
        "uuid-v7" => vld::error::StringValidation::UuidV7,
        "phone-e164" => vld::error::StringValidation::PhoneE164,
        "semver-full" => vld::error::StringValidation::SemverFull,
        "ipv4" => vld::error::StringValidation::Ipv4,
        "ipv6" => vld::error::StringValidation::Ipv6,
        "date" => vld::error::StringValidation::IsoDate,
        "date-time" => vld::error::StringValidation::IsoDatetime,
        "time" => vld::error::StringValidation::IsoTime,
        "hostname" => vld::error::StringValidation::Hostname,
        "country-code" => vld::error::StringValidation::CountryCode,
        "currency-code" => vld::error::StringValidation::Currency,
        "locale" => vld::error::StringValidation::Locale,
        _ => vld::error::StringValidation::Regex,
    }
}

fn make_issue(
    path: &[PathSegment],
    code: IssueCode,
    message: impl Into<String>,
    received: Option<&Value>,
) -> ValidationIssue {
    ValidationIssue {
        code,
        message: message.into(),
        path: path.to_vec(),
        received: received.cloned(),
    }
}

fn make_error(
    path: &[PathSegment],
    code: IssueCode,
    message: impl Into<String>,
    received: Option<&Value>,
) -> VldError {
    VldError {
        issues: vec![make_issue(path, code, message, received)],
    }
}
