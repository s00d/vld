use vld::prelude::*;

#[test]
fn multiple_errors() {
    let schema = vld::string().super_refine(|s, errors| {
        if s.len() < 3 {
            errors.push(
                IssueCode::Custom {
                    code: "too_short".into(),
                },
                "Too short",
            );
        }
        if !s.contains('@') {
            errors.push(
                IssueCode::Custom {
                    code: "no_at".into(),
                },
                "Missing @",
            );
        }
    });
    let err = schema.parse(r#""hi""#).unwrap_err();
    assert_eq!(err.issues.len(), 2);
    assert!(err.issues.iter().any(|i| i.message == "Too short"));
    assert!(err.issues.iter().any(|i| i.message == "Missing @"));
}

#[test]
fn passes_when_valid() {
    let schema = vld::string().super_refine(|s, errors| {
        if s.is_empty() {
            errors.push(
                IssueCode::Custom {
                    code: "empty".into(),
                },
                "Empty",
            );
        }
    });
    assert_eq!(schema.parse(r#""hello""#).unwrap(), "hello");
}

#[test]
fn no_errors_pushed() {
    let schema = vld::number().int().super_refine(|_n, _errors| {
        // no-op
    });
    assert_eq!(schema.parse("42").unwrap(), 42);
}
