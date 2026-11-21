//! JSON Schema → provider schema rewrite and limit checks. Pure `Value → Value`.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use super::{LlmSchemaError as E, Provider};

const OPENAI_FORMATS: &[&str] = &[
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "ipv4",
    "ipv6",
    "uuid",
];
const ANTHROPIC_FORMATS: &[&str] = &[
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];
const GEMINI_FORMATS: &[&str] = &["date-time", "date", "time"];

/// Keywords that change the accepted shape: an error, not a description note.
const STRUCTURAL: &[&str] = &[
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "dependentRequired",
    "dependentSchemas",
    "prefixItems",
    "patternProperties",
];

pub(super) fn convert(
    root_name: Option<&str>,
    root: &Value,
    defs: &[(&str, Value)],
    provider: Provider,
) -> Result<Value, E> {
    let own_defs = root.get("$defs").and_then(Value::as_object);
    let mut names: Vec<&str> = defs.iter().map(|(n, _)| *n).collect();
    names.extend(
        own_defs
            .into_iter()
            .flat_map(|d| d.keys().map(String::as_str)),
    );
    let cx = Cx {
        provider,
        root_name,
        names: &names,
    };

    let mut out_defs = Map::new();
    for (name, schema) in own_defs.into_iter().flatten() {
        out_defs.insert(name.clone(), cx.schema(schema, &format!("#/$defs/{name}"))?);
    }
    for (name, schema) in defs {
        if Some(*name) != root_name && !out_defs.contains_key(*name) {
            out_defs.insert(
                name.to_string(),
                cx.schema(schema, &format!("#/$defs/{name}"))?,
            );
        }
    }
    let mut body = root.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.remove("$defs");
    }
    let mut out = cx.schema(&body, "#")?;
    if let (Some(obj), false) = (out.as_object_mut(), out_defs.is_empty()) {
        obj.insert("$defs".into(), Value::Object(out_defs));
    }
    check(&out, provider)?;
    Ok(out)
}

struct Cx<'a> {
    provider: Provider,
    root_name: Option<&'a str>,
    names: &'a [&'a str],
}

impl Cx<'_> {
    fn reference(&self, r: &Value) -> Result<Value, E> {
        let r = r.as_str().unwrap_or_default();
        let name = r
            .strip_prefix("#/components/schemas/")
            .or_else(|| r.strip_prefix("#/$defs/"));
        match name {
            Some(n) if Some(n) == self.root_name => Ok("#".into()),
            Some(n) if self.names.contains(&n) => Ok(format!("#/$defs/{n}").into()),
            None if r == "#" => Ok(r.into()),
            _ => Err(E::UnresolvedRef {
                reference: r.to_string(),
            }),
        }
    }

    fn schema(&self, v: &Value, path: &str) -> Result<Value, E> {
        let Some(obj) = v.as_object() else {
            return Ok(v.clone());
        };
        let llm = self.provider != Provider::Generic;
        let mut out = Map::new();
        for (k, val) in obj {
            let at = |seg: &str| format!("{path}/{k}{seg}");
            let val = match k.as_str() {
                "discriminator" => continue,
                "$ref" => self.reference(val)?,
                "properties" | "$defs" | "patternProperties" | "dependentSchemas" => {
                    match val.as_object() {
                        Some(map) => {
                            let mut m = Map::new();
                            for (n, s) in map {
                                m.insert(n.clone(), self.schema(s, &at(&format!("/{n}")))?);
                            }
                            Value::Object(m)
                        }
                        None => val.clone(),
                    }
                }
                "items"
                | "additionalProperties"
                | "not"
                | "contains"
                | "propertyNames"
                | "if"
                | "then"
                | "else"
                | "unevaluatedProperties"
                | "unevaluatedItems" => self.schema(val, &at(""))?,
                "anyOf" | "oneOf" | "allOf" | "prefixItems" => match val.as_array() {
                    Some(items) => Value::Array(
                        items
                            .iter()
                            .enumerate()
                            .map(|(i, s)| self.schema(s, &at(&format!("/{i}"))))
                            .collect::<Result<_, _>>()?,
                    ),
                    None => val.clone(),
                },
                _ => val.clone(),
            };
            let key = if llm && k == "oneOf" && !obj.contains_key("anyOf") {
                "anyOf"
            } else {
                k
            };
            out.insert(key.to_string(), val);
        }
        if llm && let Some(c) = out.remove("const") {
            out.entry("enum").or_insert_with(|| json!([c]));
        }
        collapse_nullable(&mut out);
        if llm {
            self.restrict(&mut out, path)?;
        }
        Ok(Value::Object(out))
    }

    /// Provider rules for one schema node whose children are already converted.
    fn restrict(&self, out: &mut Map<String, Value>, path: &str) -> Result<(), E> {
        let p = self.provider;
        for k in STRUCTURAL {
            let ok = match *k {
                "allOf" => p == Provider::Anthropic,
                "prefixItems" => p == Provider::Gemini,
                _ => false,
            };
            if !ok && out.contains_key(*k) {
                return Err(E::Unsupported {
                    path: path.to_string(),
                    keyword: k.to_string(),
                });
            }
        }

        let strict = matches!(p, Provider::OpenAiStrict | Provider::Anthropic);
        if strict && is_object(out) {
            match out.get("additionalProperties") {
                // `true` is vld's default for objects that strip unknown keys.
                None | Some(Value::Bool(_)) => {}
                Some(_) => {
                    return Err(E::Unsupported {
                        path: path.to_string(),
                        keyword: "additionalProperties".into(),
                    });
                }
            }
            out.insert("additionalProperties".into(), Value::Bool(false));
        }
        if p == Provider::OpenAiStrict {
            require_all(out);
        }

        let mut notes = Vec::new();
        let keys: Vec<String> = out.keys().cloned().collect();
        for k in keys {
            if !allowed(p, &k, &out[&k]) {
                let v = out.remove(&k).expect("key listed above");
                notes.push(note(&k, &v));
            }
        }
        if !notes.is_empty() {
            let text = notes.join("; ");
            let d = match out.get("description").and_then(Value::as_str) {
                Some(d) if !d.is_empty() => format!("{d} ({text})"),
                _ => text,
            };
            out.insert("description".into(), d.into());
        }

        let typed = ["type", "anyOf", "allOf", "$ref", "enum"]
            .iter()
            .any(|k| out.contains_key(*k));
        if !typed && matches!(p, Provider::OpenAiStrict | Provider::Gemini) {
            return Err(E::Untyped {
                path: path.to_string(),
            });
        }
        Ok(())
    }
}

/// OpenAI strict: every property in `required`; the optional ones accept `null`.
fn require_all(out: &mut Map<String, Value>) {
    let required: HashSet<String> = out
        .get("required")
        .and_then(Value::as_array)
        .map(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let Some(Value::Object(props)) = out.get_mut("properties") else {
        return;
    };
    let mut all = Vec::with_capacity(props.len());
    for (k, v) in props.iter_mut() {
        if !required.contains(k) {
            *v = nullable(std::mem::take(v));
        }
        all.push(Value::String(k.clone()));
    }
    out.insert("required".into(), Value::Array(all));
}

fn is_object(out: &Map<String, Value>) -> bool {
    match out.get("type") {
        Some(Value::String(t)) => t == "object",
        Some(Value::Array(ts)) => ts.iter().any(|t| t == "object"),
        _ => out.contains_key("properties"),
    }
}

fn is_null_schema(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == 1 && o.get("type").is_some_and(|t| t == "null"))
}

/// Add `null` to a typed schema's `type` (and `enum`).
fn add_null(o: &mut Map<String, Value>) {
    match o.get_mut("type") {
        Some(Value::String(t)) if t != "null" => {
            let t = std::mem::take(t);
            o.insert("type".into(), json!([t, "null"]));
        }
        Some(Value::Array(ts)) if !ts.iter().any(|t| t == "null") => ts.push("null".into()),
        _ => {}
    }
    if let Some(Value::Array(e)) = o.get_mut("enum")
        && !e.contains(&Value::Null)
    {
        e.push(Value::Null);
    }
}

/// `v` accepting `null` too: a type array when `v` has a `type`, else `anyOf`.
fn nullable(v: Value) -> Value {
    match v {
        Value::Object(mut o) if o.contains_key("type") && !o.contains_key("$ref") => {
            add_null(&mut o);
            Value::Object(o)
        }
        Value::Object(o)
            if o.get("anyOf")
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(is_null_schema)) =>
        {
            Value::Object(o)
        }
        other => json!({ "anyOf": [other, { "type": "null" }] }),
    }
}

/// `{"anyOf": [X, {"type": "null"}]}` with a typed `X` → `X` with `type: [t, "null"]`.
fn collapse_nullable(out: &mut Map<String, Value>) {
    let Some(Value::Array(items)) = out.get("anyOf") else {
        return;
    };
    if items.len() != 2 {
        return;
    }
    let Some(null_at) = items.iter().position(is_null_schema) else {
        return;
    };
    let Some(other) = items[1 - null_at].as_object() else {
        return;
    };
    if !matches!(other.get("type"), Some(Value::String(_))) || other.contains_key("$ref") {
        return;
    }
    let mut merged = other.clone();
    add_null(&mut merged);
    out.remove("anyOf");
    for (k, v) in merged {
        out.entry(k).or_insert(v);
    }
}

fn allowed(p: Provider, key: &str, val: &Value) -> bool {
    use Provider::*;
    match key {
        "type"
        | "properties"
        | "required"
        | "additionalProperties"
        | "items"
        | "anyOf"
        | "enum"
        | "$ref"
        | "$defs"
        | "description"
        | "title" => true,
        "allOf" => p == Anthropic,
        "prefixItems" => p == Gemini,
        "pattern" => match p {
            OpenAiStrict => true,
            Anthropic => val.as_str().is_some_and(|re| !has_lookaround(re)),
            _ => false,
        },
        "format" => {
            let formats = match p {
                OpenAiStrict => OPENAI_FORMATS,
                Anthropic => ANTHROPIC_FORMATS,
                _ => GEMINI_FORMATS,
            };
            val.as_str().is_some_and(|f| formats.contains(&f))
        }
        "minimum" | "maximum" | "maxItems" => matches!(p, OpenAiStrict | Gemini),
        "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf" => p == OpenAiStrict,
        "minItems" => p != Anthropic || val.as_u64().is_some_and(|n| n <= 1),
        _ => false,
    }
}

/// Lookaround or a backreference (unsupported by Anthropic's regex engine).
fn has_lookaround(re: &str) -> bool {
    ["(?=", "(?!", "(?<=", "(?<!", "\\k<"]
        .iter()
        .any(|s| re.contains(s))
        || re
            .as_bytes()
            .windows(2)
            .any(|w| w[0] == b'\\' && w[1].is_ascii_digit() && w[1] != b'0')
}

/// A dropped keyword as description text: `min length 2`, `pattern ^a+$`.
fn note(key: &str, val: &Value) -> String {
    let label = match key {
        "minLength" => "min length",
        "maxLength" => "max length",
        "exclusiveMinimum" => "greater than",
        "exclusiveMaximum" => "less than",
        "multipleOf" => "multiple of",
        "minItems" => "min items",
        "maxItems" => "max items",
        "uniqueItems" => "unique items",
        "minProperties" => "min properties",
        "maxProperties" => "max properties",
        k => k.strip_prefix("x-").unwrap_or(k),
    };
    match val {
        Value::Bool(true) => label.to_string(),
        Value::String(s) => format!("{label} {s}"),
        other => format!("{label} {other}"),
    }
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Stats {
    properties: usize,
    enum_values: usize,
    chars: usize,
    optional: usize,
    unions: usize,
}

fn is_union(v: &Value) -> bool {
    v.get("anyOf").is_some()
        || v.get("type")
            .and_then(Value::as_array)
            .is_some_and(|t| t.len() > 1)
}

fn stats(v: &Value, s: &mut Stats) {
    let Some(o) = v.as_object() else {
        return;
    };
    if let Some(props) = o.get("properties").and_then(Value::as_object) {
        let required: Vec<&str> = o
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        s.properties += props.len();
        for (k, p) in props {
            s.chars += k.chars().count();
            s.optional += usize::from(!required.contains(&k.as_str()));
            s.unions += usize::from(is_union(p));
            stats(p, s);
        }
    }
    if let Some(e) = o.get("enum").and_then(Value::as_array) {
        s.enum_values += e.len();
        s.chars += e
            .iter()
            .filter_map(Value::as_str)
            .map(|x| x.chars().count())
            .sum::<usize>();
    }
    for k in ["items", "additionalProperties"] {
        if let Some(c) = o.get(k) {
            stats(c, s);
        }
    }
    for k in ["anyOf", "allOf", "prefixItems"] {
        for c in o.get(k).and_then(Value::as_array).into_iter().flatten() {
            stats(c, s);
        }
    }
    for c in o
        .get("$defs")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        stats(c.1, s);
    }
}

/// The schema a local `$ref` points to (`#` or `#/$defs/<name>`).
fn target<'a>(root: &'a Value, r: &str) -> Option<&'a Value> {
    if r == "#" {
        return Some(root);
    }
    root.get("$defs")?.get(r.strip_prefix("#/$defs/")?)
}

/// Every `$ref` under `v`, not descending into `$defs`.
fn refs<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::Object(o) => {
            for (k, c) in o {
                match (k.as_str(), c) {
                    ("$defs", _) => {}
                    ("$ref", Value::String(r)) => out.push(r),
                    _ => refs(c, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|c| refs(c, out)),
        _ => {}
    }
}

/// A `$ref` reachable from itself.
fn cycle<'a>(
    root: &'a Value,
    node: &'a str,
    stack: &mut Vec<&'a str>,
    done: &mut HashSet<&'a str>,
) -> Option<String> {
    stack.push(node);
    let mut found = Vec::new();
    refs(target(root, node)?, &mut found);
    for r in found {
        if stack.contains(&r) {
            return Some(r.strip_prefix("#/$defs/").unwrap_or(r).to_string());
        }
        if !done.contains(r)
            && let Some(c) = cycle(root, r, stack, done)
        {
            return Some(c);
        }
    }
    stack.pop();
    done.insert(node);
    None
}

/// Object nesting depth, following `$ref`s (a cycle adds nothing more).
fn depth<'a>(root: &'a Value, v: &'a Value, stack: &mut Vec<&'a str>) -> usize {
    let Some(o) = v.as_object() else {
        return 0;
    };
    if let Some(r) = o.get("$ref").and_then(Value::as_str) {
        if stack.contains(&r) {
            return 0;
        }
        let Some(t) = target(root, r) else {
            return 0;
        };
        stack.push(r);
        let d = depth(root, t, stack);
        stack.pop();
        return d;
    }
    let mut children: Vec<&Value> = Vec::new();
    children.extend(
        o.get("properties")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|m| m.values()),
    );
    children.extend(
        ["items", "additionalProperties"]
            .iter()
            .filter_map(|k| o.get(*k)),
    );
    for k in ["anyOf", "allOf", "prefixItems"] {
        children.extend(o.get(k).and_then(Value::as_array).into_iter().flatten());
    }
    let inner = children
        .into_iter()
        .map(|c| depth(root, c, stack))
        .max()
        .unwrap_or(0);
    inner + usize::from(o.contains_key("properties"))
}

fn limit(what: &'static str, count: usize, max: usize) -> Result<(), E> {
    if count > max {
        return Err(E::TooMany { what, count, max });
    }
    Ok(())
}

fn check(out: &Value, p: Provider) -> Result<(), E> {
    if matches!(p, Provider::OpenAiStrict | Provider::Anthropic)
        && out.get("type").and_then(Value::as_str) != Some("object")
    {
        return Err(E::RootNotObject);
    }
    let mut s = Stats::default();
    stats(out, &mut s);
    match p {
        Provider::OpenAiStrict => {
            limit("properties", s.properties, 5000)?;
            limit("nesting depth", depth(out, out, &mut vec!["#"]), 10)?;
            limit("enum values", s.enum_values, 1000)?;
            limit("name and enum characters", s.chars, 120_000)?;
        }
        Provider::Anthropic => {
            if let Some(name) = cycle(out, "#", &mut Vec::new(), &mut HashSet::new()) {
                return Err(E::Recursive { name });
            }
            limit("optional parameters", s.optional, 24)?;
            limit("union parameters", s.unions, 16)?;
        }
        _ => {}
    }
    Ok(())
}
