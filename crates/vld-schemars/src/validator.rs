use serde_json::Value;
use vld::error::{IssueCode, PathSegment, ValidationIssue, VldError};

pub(crate) fn validate_value_against_schema(
    schema: &Value,
    value: &Value,
    path: &[PathSegment],
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
        validate_array(schema_obj, arr, path, &mut errors);
    }

    // Object constraints
    if let Some(obj) = value.as_object() {
        validate_object(schema_obj, obj, path, &mut errors);
    }

    // oneOf
    if let Some(one_of) = schema_obj.get("oneOf").and_then(|v| v.as_array()) {
        let matches: usize = one_of
            .iter()
            .filter(|s| validate_value_against_schema(s, value, path).is_ok())
            .count();
        if matches != 1 {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "oneOf".into(),
                },
                format!("Value must match exactly one of the schemas (matched {})", matches),
                Some(value),
            ));
        }
    }

    // anyOf
    if let Some(any_of) = schema_obj.get("anyOf").and_then(|v| v.as_array()) {
        let matches = any_of
            .iter()
            .any(|s| validate_value_against_schema(s, value, path).is_ok());
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
            if let Err(e) = validate_value_against_schema(sub_schema, value, path) {
                errors = errors.merge(e);
            }
        }
    }

    // not
    if let Some(not_schema) = schema_obj.get("not") {
        if validate_value_against_schema(not_schema, value, path).is_ok() {
            errors.issues.push(make_issue(
                path,
                IssueCode::Custom {
                    code: "not".into(),
                },
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

    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        if (s.len() as u64) < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    minimum: min as f64,
                    inclusive: true,
                },
                format!("String must be at least {} characters", min),
                Some(value),
            ));
        }
    }

    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        if (s.len() as u64) > max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    maximum: max as f64,
                    inclusive: true,
                },
                format!("String must be at most {} characters", max),
                Some(value),
            ));
        }
    }

    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        if let Ok(re) = regex_lite::Regex::new(pattern) {
            if !re.is_match(s) {
                errors.issues.push(make_issue(
                    path,
                    IssueCode::InvalidString {
                        validation: vld::error::StringValidation::Regex,
                    },
                    format!("String does not match pattern: {}", pattern),
                    Some(value),
                ));
            }
        }
    }

    if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
        let valid = match format {
            "email" => s.contains('@') && s.contains('.'),
            "uri" | "url" => s.starts_with("http://") || s.starts_with("https://"),
            "uuid" => {
                s.len() == 36
                    && s.chars()
                        .all(|c| c.is_ascii_hexdigit() || c == '-')
            }
            "ipv4" => s.parse::<std::net::Ipv4Addr>().is_ok(),
            "ipv6" => s.parse::<std::net::Ipv6Addr>().is_ok(),
            "date" => s.len() == 10 && s.chars().nth(4) == Some('-') && s.chars().nth(7) == Some('-'),
            "date-time" => s.contains('T') || s.contains('t'),
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
    }
}

fn validate_number(
    schema: &serde_json::Map<String, Value>,
    value: &Value,
    path: &[PathSegment],
    errors: &mut VldError,
) {
    let n = match value.as_f64() {
        Some(n) => n,
        None => return,
    };

    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        if n < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    minimum: min,
                    inclusive: true,
                },
                format!("Value must be >= {}", min),
                Some(value),
            ));
        }
    }

    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        if n > max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    maximum: max,
                    inclusive: true,
                },
                format!("Value must be <= {}", max),
                Some(value),
            ));
        }
    }

    if let Some(ex_min) = schema.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
        if n <= ex_min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    minimum: ex_min,
                    inclusive: false,
                },
                format!("Value must be > {}", ex_min),
                Some(value),
            ));
        }
    }

    if let Some(ex_max) = schema.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
        if n >= ex_max {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooBig {
                    maximum: ex_max,
                    inclusive: false,
                },
                format!("Value must be < {}", ex_max),
                Some(value),
            ));
        }
    }

    if let Some(multiple) = schema.get("multipleOf").and_then(|v| v.as_f64()) {
        if multiple != 0.0 && (n % multiple).abs() > f64::EPSILON {
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

fn validate_array(
    schema: &serde_json::Map<String, Value>,
    arr: &[Value],
    path: &[PathSegment],
    errors: &mut VldError,
) {
    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        if (arr.len() as u64) < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    minimum: min as f64,
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
                    maximum: max as f64,
                    inclusive: true,
                },
                format!("Array must have at most {} items", max),
                Some(&Value::Number(serde_json::Number::from(arr.len()))),
            ));
        }
    }

    if let Some(items_schema) = schema.get("items") {
        for (i, item) in arr.iter().enumerate() {
            let mut item_path = path.to_vec();
            item_path.push(PathSegment::Index(i));
            if let Err(e) = validate_value_against_schema(items_schema, item, &item_path) {
                *errors = std::mem::take(errors).merge(e);
            }
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
}

fn validate_object(
    schema: &serde_json::Map<String, Value>,
    obj: &serde_json::Map<String, Value>,
    path: &[PathSegment],
    errors: &mut VldError,
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

    // properties
    if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
        for (prop_name, prop_schema) in properties {
            if let Some(prop_value) = obj.get(prop_name) {
                let mut prop_path = path.to_vec();
                prop_path.push(PathSegment::Field(prop_name.clone()));
                if let Err(e) =
                    validate_value_against_schema(prop_schema, prop_value, &prop_path)
                {
                    *errors = std::mem::take(errors).merge(e);
                }
            }
        }
    }

    // minProperties / maxProperties
    if let Some(min) = schema.get("minProperties").and_then(|v| v.as_u64()) {
        if (obj.len() as u64) < min {
            errors.issues.push(make_issue(
                path,
                IssueCode::TooSmall {
                    minimum: min as f64,
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
                    maximum: max as f64,
                    inclusive: true,
                },
                format!("Object must have at most {} properties", max),
                None,
            ));
        }
    }
}

// ========================= Helpers ===========================================

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
    value
        .as_f64()
        .map(|n| n.fract() == 0.0)
        .unwrap_or(false)
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
        "uuid" => vld::error::StringValidation::Uuid,
        "ipv4" => vld::error::StringValidation::Ipv4,
        "ipv6" => vld::error::StringValidation::Ipv6,
        "date" => vld::error::StringValidation::IsoDate,
        "date-time" => vld::error::StringValidation::IsoDatetime,
        "time" => vld::error::StringValidation::IsoTime,
        "hostname" => vld::error::StringValidation::Hostname,
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
