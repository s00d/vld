//! # vld-fake — random test data that passes `vld` schemas
//!
//! Generates values that satisfy a JSON Schema produced by `vld` (or written
//! by hand): formats, patterns, length / numeric / count bounds,
//! `multipleOf`, unions, tuples and `$ref`s. Field names (`email`, `city`,
//! `first_name`, …) pick realistic values when no format is set.
//!
//! ## Typed API
//!
//! ```rust
//! use vld::prelude::*;
//! use vld_fake::FakeData;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name:  String => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!         pub age:   i64    => vld::number().int().min(18).max(99),
//!     }
//! }
//!
//! vld_fake::impl_fake!(User);
//!
//! let user = User::fake(); // generated, then parsed through `User::parse_value`
//! assert!(user.name.chars().count() >= 2);
//! assert!(user.email.contains('@'));
//!
//! let users = User::fake_many(5);
//! assert_eq!(users.len(), 5);
//!
//! // Reproducible
//! assert_eq!(User::fake_seeded(42).name, User::fake_seeded(42).name);
//! ```
//!
//! ## Untyped API
//!
//! ```rust
//! let schema = serde_json::json!({
//!     "type": "object",
//!     "required": ["port"],
//!     "properties": { "port": { "type": "integer", "minimum": 1, "maximum": 65535 } }
//! });
//! let value = vld_fake::fake_value(&schema);
//! let port = value["port"].as_i64().unwrap();
//! assert!((1..=65535).contains(&port));
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/codegen/fake>
#![warn(missing_docs)]

mod constraints;
mod dict;
mod regex_gen;

#[doc(hidden)]
pub use vld as __vld;

use dict::*;
use rand::Rng;
use rand::RngExt;
use serde_json::{Map, Value, json};
use vld::prelude::VldParse;

trait RngCompat {
    fn gen_range<T, R>(&mut self, range: R) -> T
    where
        T: rand::distr::uniform::SampleUniform,
        R: rand::distr::uniform::SampleRange<T>;
    fn gen_bool(&mut self, p: f64) -> bool;
    fn generator<T>(&mut self) -> T
    where
        rand::distr::StandardUniform: rand::distr::Distribution<T>;
}

impl<R: rand::Rng + ?Sized> RngCompat for R {
    fn gen_range<T, RB>(&mut self, range: RB) -> T
    where
        T: rand::distr::uniform::SampleUniform,
        RB: rand::distr::uniform::SampleRange<T>,
    {
        self.random_range(range)
    }

    fn gen_bool(&mut self, p: f64) -> bool {
        self.random_bool(p)
    }

    fn generator<T>(&mut self) -> T
    where
        rand::distr::StandardUniform: rand::distr::Distribution<T>,
    {
        self.random()
    }
}

fn thread_rng_compat() -> rand::rngs::ThreadRng {
    rand::rng()
}

// ───────────────────────── public convenience API ──────────────────────────

/// Generate a single random [`Value`] conforming to the given JSON Schema.
pub fn fake_value(schema: &Value) -> Value {
    FakeGen::new().value(schema)
}

/// Generate a JSON string conforming to the given JSON Schema.
pub fn fake_json(schema: &Value) -> String {
    serde_json::to_string_pretty(&fake_value(schema)).expect("serialisation cannot fail")
}

/// Generate `count` random values from the same schema.
pub fn fake_many(schema: &Value, count: usize) -> Vec<Value> {
    let mut generator = FakeGen::new();
    (0..count).map(|_| generator.value(schema)).collect()
}

/// Generate a random value **and** parse it through `T::vld_parse_value`,
/// returning a fully validated typed instance.
///
/// Panics if the generated value does not pass validation (should not happen
/// unless the schema is ambiguous).
pub fn fake_parsed<T: VldParse>(schema: &Value) -> T {
    parse_or_panic(&fake_value(schema))
}

#[doc(hidden)]
pub fn parse_or_panic<T: VldParse>(val: &Value) -> T {
    T::vld_parse_value(val).unwrap_or_else(|e| {
        panic!(
            "vld_fake::fake_parsed: generated value failed validation.\nValue: {}\nError: {:?}",
            serde_json::to_string_pretty(val).unwrap_or_default(),
            e,
        )
    })
}

/// Same as [`fake_parsed`], but returns a `Result` instead of panicking.
pub fn try_fake_parsed<T: VldParse>(schema: &Value) -> Result<T, vld::error::VldError> {
    let val = fake_value(schema);
    T::vld_parse_value(&val)
}

/// Generate with a specific seed for reproducible output.
pub fn fake_value_seeded(schema: &Value, seed: u64) -> Value {
    use rand::SeedableRng;
    let rng = rand::rngs::StdRng::seed_from_u64(seed);
    FakeGen::with_rng(rng).value(schema)
}

// ──────────────────────── FakeGen (configurable) ───────────────────────────

/// Stateful fake-data generator backed by any [`rand::Rng`].
pub struct FakeGen<R: Rng> {
    rng: R,
    /// Recursion depth guard.
    depth: usize,
    /// Auto-incrementing counter for unique values.
    counter: u64,
    /// Named definitions used to resolve `$ref` (collected from the root
    /// schema's `$defs` / `definitions` / `components.schemas`).
    defs: Map<String, Value>,
}

const MAX_DEPTH: usize = 12;

impl FakeGen<rand::rngs::ThreadRng> {
    /// Create a generator using the thread-local RNG.
    pub fn new() -> Self {
        Self {
            rng: thread_rng_compat(),
            depth: 0,
            counter: 0,
            defs: Map::new(),
        }
    }
}

impl Default for FakeGen<rand::rngs::ThreadRng> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Rng> FakeGen<R> {
    /// Create a generator from any [`Rng`] implementation (e.g. a seeded `StdRng`).
    pub fn with_rng(rng: R) -> Self {
        Self {
            rng,
            depth: 0,
            counter: 0,
            defs: Map::new(),
        }
    }

    fn next_id(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.rng.gen_range(0..items.len())]
    }

    // ────────────────────── main dispatcher ─────────────────────────────

    /// Generate one random [`Value`] conforming to `schema`.
    pub fn value(&mut self, schema: &Value) -> Value {
        self.value_with_hint(schema, None)
    }

    /// Generate one random [`Value`], optionally using a field-name hint for
    /// smarter generation.
    pub fn value_with_hint(&mut self, schema: &Value, hint: Option<&str>) -> Value {
        if self.depth > MAX_DEPTH {
            return Value::Null;
        }
        if self.depth == 0 {
            self.collect_defs(schema);
        }
        self.depth += 1;
        let result = self.dispatch(schema, hint);
        self.depth -= 1;
        result
    }

    /// Register definitions embedded in a root schema so `$ref`s resolve.
    fn collect_defs(&mut self, schema: &Value) {
        for defs in [
            schema.get("$defs"),
            schema.get("definitions"),
            schema.pointer("/components/schemas"),
        ]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        {
            for (k, v) in defs {
                self.defs.insert(k.clone(), v.clone());
            }
        }
    }

    /// Resolve a local `$ref` (`#/components/schemas/X`, `#/$defs/X`,
    /// `#/definitions/X`) against the collected definitions.
    fn resolve_ref(&self, r: &str) -> Option<Value> {
        let name = ["#/components/schemas/", "#/$defs/", "#/definitions/"]
            .iter()
            .find_map(|p| r.strip_prefix(p))?;
        self.defs.get(name).cloned()
    }

    fn dispatch(&mut self, schema: &Value, hint: Option<&str>) -> Value {
        let obj = match schema.as_object() {
            Some(o) => o,
            None => return Value::Null,
        };

        // ---- $ref ----
        if let Some(r) = obj.get("$ref").and_then(Value::as_str) {
            return match self.resolve_ref(r) {
                Some(target) => self.value_with_hint(&target, hint),
                None => Value::Null,
            };
        }

        // ---- allOf ----
        // Flatten into one schema (strictest bounds, LCM of `multipleOf`,
        // extra patterns / formats as fragments) and generate from that.
        let merged;
        let obj = match obj.get("allOf").and_then(Value::as_array) {
            Some(all) if all.iter().any(|s| !is_fragment(s)) || !obj.contains_key("type") => {
                let mut rest = obj.clone();
                rest.remove("allOf");
                let mut parts = vec![Value::Object(rest)];
                for s in all {
                    let resolved = s
                        .get("$ref")
                        .and_then(Value::as_str)
                        .and_then(|r| self.resolve_ref(r));
                    parts.push(resolved.unwrap_or_else(|| s.clone()));
                }
                merged = constraints::merge(&parts);
                &merged
            }
            // Only `pattern` / `format` fragments next to `type`: already
            // the flat form the generators read.
            _ => obj,
        };

        // ---- const / enum ----
        if let Some(c) = obj.get("const") {
            return c.clone();
        }
        if let Some(en) = obj.get("enum").and_then(Value::as_array) {
            if en.is_empty() {
                return Value::Null;
            }
            let idx = self.rng.gen_range(0..en.len());
            return en[idx].clone();
        }

        // ---- oneOf / anyOf ----
        if let Some(variants) = obj
            .get("oneOf")
            .or_else(|| obj.get("anyOf"))
            .and_then(Value::as_array)
        {
            let mut concrete: Vec<&Value> = variants
                .iter()
                .filter(|v| v.get("type").and_then(Value::as_str) != Some("null"))
                .collect();
            // A bare `{"type":"string"}` branch usually stands for a textual
            // form validated beyond JSON Schema (e.g. `vld::duration()`'s
            // `"PT15S"`): prefer the other branches when there are any.
            if concrete.iter().any(|v| !is_bare_string(v)) {
                concrete.retain(|v| !is_bare_string(v));
            }
            if !concrete.is_empty() {
                let idx = self.rng.gen_range(0..concrete.len());
                // Sibling keywords (`type`, `formatMinimum`, `x-past`, … next to
                // e.g. a naive-datetime `anyOf`) apply to every branch.
                let mut rest = obj.clone();
                for k in ["oneOf", "anyOf", "description", "default", "title"] {
                    rest.remove(k);
                }
                if rest.is_empty() {
                    return self.value_with_hint(concrete[idx], hint);
                }
                let merged = constraints::merge(&[Value::Object(rest), concrete[idx].clone()]);
                return self.value_with_hint(&Value::Object(merged), hint);
            }
            if !variants.is_empty() {
                let idx = self.rng.gen_range(0..variants.len());
                return self.value_with_hint(&variants[idx], hint);
            }
            return Value::Null;
        }

        // ---- type-based dispatch ----
        // `type` may be an array (`["string", "null"]`): use the first non-null.
        let ty = match obj.get("type") {
            Some(Value::Array(types)) => types
                .iter()
                .filter_map(Value::as_str)
                .find(|t| *t != "null")
                .unwrap_or("null"),
            Some(t) => t.as_str().unwrap_or("string"),
            None => "string",
        };

        match ty {
            "string" => self.gen_string(obj, hint),
            "integer" => self.gen_integer(obj, hint),
            "number" => self.gen_number(obj, hint),
            "boolean" => self.gen_boolean(),
            "array" => self.gen_array(obj, hint),
            "object" => self.gen_object(obj, hint),
            "null" => Value::Null,
            _ => Value::Null,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  STRING
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_string(&mut self, obj: &Map<String, Value>, hint: Option<&str>) -> Value {
        let (min, max) = len_bounds(obj);

        // 0) `pattern` (plus further patterns in `allOf`) — generate a match
        let patterns: Vec<&str> = obj
            .get("pattern")
            .into_iter()
            .chain(
                obj.get("allOf")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|s| s.get("pattern")),
            )
            .filter_map(Value::as_str)
            .collect();
        // 1) Formats (`format` plus further ones in `allOf`): generate and
        //    test against every constraint with vld's own checks.
        let mut formats = constraints::all_of_values(obj, "format");
        if formats.is_empty() {
            // Date bounds on a pattern-only string (vld's naive datetime branch).
            formats = constraints::temporal_formats(obj);
        }
        if !formats.is_empty() {
            return Value::String(self.gen_formatted(obj, &formats, &patterns));
        }

        if !patterns.is_empty() {
            if let Some(s) = self.gen_patterned(&patterns, min, max) {
                return Value::String(s);
            }
        }

        // 2) Heuristic: infer content from field name
        if let Some(h) = hint {
            if let Some(v) = self.gen_string_by_hint(h, obj) {
                return v;
            }
        }

        // 3) Fallback: readable word-based text
        self.gen_readable_string(obj)
    }

    /// A string valid for every format, pattern, length and date bound of
    /// `obj`: candidates come from each format's generator (in turn), from a
    /// length-fitted variant, and from the patterns; literal prefixes /
    /// suffixes / substrings are overlaid. Best effort (the last candidate)
    /// when nothing fits within the attempts, e.g. `uuid().max(10)`.
    fn gen_formatted(
        &mut self,
        obj: &Map<String, Value>,
        formats: &[String],
        patterns: &[&str],
    ) -> String {
        let check = constraints::StrCheck::new(obj);
        let (min, max) = len_bounds(obj);
        let (mut head, mut tail, mut mids) = (String::new(), String::new(), Vec::new());
        let mut free = Vec::new();
        for p in patterns {
            match regex_gen::literal(p) {
                Some(regex_gen::Literal::Prefix(h)) if h.len() > head.len() => head = h,
                Some(regex_gen::Literal::Suffix(t)) if t.len() > tail.len() => tail = t,
                Some(regex_gen::Literal::Contains(c)) => mids.push(c),
                Some(_) => {}
                None => free.push(*p),
            }
        }
        let fixed = head.chars().count()
            + tail.chars().count()
            + mids.iter().map(|m| m.chars().count()).sum::<usize>();
        let mut last = String::new();
        // ponytail: generate-and-test; unsatisfiable combinations end in a
        // best-effort value after the attempts.
        for i in 0..90 {
            let fmt = formats[i % formats.len()].as_str();
            let mut bases = Vec::new();
            let base = self.gen_string_format(fmt, obj);
            bases.push(base);
            if min.is_some() || max.is_some() || fixed > 0 {
                let n = self.pick_len(min, max, 4, 40);
                for target in [n, n.saturating_sub(fixed)] {
                    if let Some(s) = self.fit_format(fmt, target) {
                        bases.push(s);
                    }
                }
            }
            if !free.is_empty() {
                if let Some(s) = self.gen_patterned(patterns, min, max) {
                    bases.push(s);
                }
            }
            for b in bases {
                for c in self.overlays(&b, &head, &tail, &mids) {
                    if check.ok(&c) {
                        return c;
                    }
                    last = c;
                }
            }
        }
        last
    }

    /// `s` itself plus variants carrying the literal prefix / suffix /
    /// substrings, both overwriting and inserting (a naive datetime also
    /// without its `Z`).
    fn overlays(&mut self, s: &str, head: &str, tail: &str, mids: &[String]) -> Vec<String> {
        let mut out = vec![s.to_string()];
        if let Some(naive) = s.strip_suffix('Z') {
            out.push(naive.to_string());
        }
        if head.is_empty() && tail.is_empty() && mids.is_empty() {
            return out;
        }
        let chars: Vec<char> = s.chars().collect();
        let (h, t) = (head.chars().count(), tail.chars().count());
        let mid_of = |from: usize, to: usize| -> String {
            chars
                .get(from.min(chars.len())..to.max(from).min(chars.len()))
                .map(|c| c.iter().collect())
                .unwrap_or_default()
        };
        let with_mids = |body: String, rng: &mut R| -> String {
            let mut body: Vec<char> = body.chars().collect();
            for m in mids {
                let at = rng.gen_range(0..=body.len());
                body.splice(at..at, m.chars());
            }
            body.into_iter().collect()
        };
        let inserted = with_mids(s.to_string(), &mut self.rng);
        out.push(format!("{head}{inserted}{tail}"));
        let body = mid_of(h, chars.len().saturating_sub(t));
        let overwritten = with_mids(body.clone(), &mut self.rng);
        out.push(format!("{head}{overwritten}{tail}"));
        // Substrings written over the body: keeps the length (fixed-size formats).
        let mut body: Vec<char> = body.chars().collect();
        for m in mids {
            let m: Vec<char> = m.chars().collect();
            if m.len() <= body.len() {
                let at = self.rng.gen_range(0..=body.len() - m.len());
                body.splice(at..at + m.len(), m);
            }
        }
        out.push(format!(
            "{head}{}{tail}",
            body.into_iter().collect::<String>()
        ));
        // Inside an email / URL the local part / path takes literals best.
        if let Some((local, domain)) = s.split_once('@') {
            out.push(format!("{head}{local}{}@{domain}", mids.concat()));
        }
        out
    }

    /// A value of `fmt` with exactly `n` characters, for formats whose
    /// length can vary freely; `None` otherwise (or when `n` cannot fit).
    fn fit_format(&mut self, fmt: &str, n: usize) -> Option<String> {
        let letters = |g: &mut Self, k: usize| g.gen_chars(LOWER_ALPHA, k);
        Some(match fmt {
            "uri" | "url" | "url-strict" if n >= 12 => {
                // `https://` + host + `.io`
                format!("https://{}.io", letters(self, n - 11))
            }
            "uri" | "url" if n >= 8 => format!("http://{}", letters(self, n - 7)),
            "ipv4" if (7..=15).contains(&n) => {
                // Split `n - 3` digits over four octets of 1..=3 digits.
                let mut widths = [1usize; 4];
                let mut left = n - 7;
                while left > 0 {
                    let i = self.rng.gen_range(0..4);
                    if widths[i] < 3 {
                        widths[i] += 1;
                        left -= 1;
                    }
                }
                widths
                    .iter()
                    .map(|&w| match w {
                        1 => self.rng.gen_range(1u16..=9).to_string(),
                        2 => self.rng.gen_range(10u16..=99).to_string(),
                        _ => self.rng.gen_range(100u16..=255).to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            }
            "slug" if n >= 1 => {
                let mut s = letters(self, n);
                if n >= 3 {
                    let at = self.rng.gen_range(1..n - 1);
                    s.replace_range(at..at + 1, "-");
                }
                s
            }
            "hostname" if (1..=253).contains(&n) => {
                // Labels of at most 63 chars joined by dots.
                let mut parts = Vec::new();
                let mut left = n;
                while left > 0 {
                    let take = left.min(63);
                    let take = if left - take == 1 { take - 1 } else { take };
                    parts.push(letters(self, take));
                    left -= take;
                    left = left.saturating_sub(1);
                }
                parts.join(".")
            }
            "email" if n >= 6 => format!("{}@ex.io", letters(self, n - 6)),
            "nanoid" if n >= 1 => self.gen_chars(NANOID_ALPHA, n),
            "cuid2" if (2..=32).contains(&n) => {
                format!("{}{}", letters(self, 1), self.gen_chars(LOWER_DIGIT, n - 1))
            }
            "semver" | "semver-full" if n >= 5 => {
                let v = format!(
                    "{}.{}.{}",
                    self.rng.gen_range(0..10),
                    self.rng.gen_range(0..10),
                    self.rng.gen_range(0..10)
                );
                match n - 5 {
                    0 => v,
                    1 => format!("{v}0"),
                    k => format!("{v}-{}", letters(self, k - 1)),
                }
            }
            "json" if n >= 2 => format!("\"{}\"", letters(self, n - 2)),
            "emoji" if n >= 1 => "😀".repeat(n),
            "phone" | "phone-e164" if (9..=16).contains(&n) => {
                format!("+1{}", self.gen_chars(b"0123456789", n - 2))
            }
            "byte" if n >= 4 && n % 4 == 0 => {
                let bytes: Vec<u8> = (0..n / 4 * 3).map(|_| self.rng.generator()).collect();
                base64_encode(&bytes)
            }
            _ => return None,
        })
    }

    /// A `date` / `date-time` / `time` within `formatMinimum` /
    /// `formatMaximum` and `x-past` / `x-future`.
    fn gen_temporal(&mut self, fmt: &str, obj: &Map<String, Value>) -> String {
        use constraints::DAY;
        let (lo, hi) = constraints::temporal_bounds(fmt, obj);
        // Round inward: a bound with a fraction of a second.
        let lo = lo.map(|(s, n)| s + i64::from(n > 0));
        let hi = hi.map(|(s, _)| s);
        let span = if fmt == "time" {
            DAY - 1
        } else {
            30 * 365 * DAY
        };
        let (dlo, dhi) = if fmt == "time" {
            (0, DAY - 1)
        } else {
            // 2000-01-01 .. 2030-12-31
            (10_957 * DAY, 22_279 * DAY)
        };
        let (lo, hi) = match (lo, hi) {
            (Some(l), Some(h)) => (l, h),
            (Some(l), None) => (l, if fmt == "time" { DAY - 1 } else { l + span }),
            (None, Some(h)) => (if fmt == "time" { 0 } else { h - span }, h),
            (None, None) => (dlo, dhi),
        };
        let t = if lo <= hi {
            self.rng.gen_range(lo..=hi)
        } else {
            lo
        };
        match fmt {
            "time" => constraints::hms(t.clamp(0, DAY - 1)),
            "date" => {
                // A whole day inside the bounds.
                let (dl, dh) = (
                    lo.div_euclid(DAY) + i64::from(lo.rem_euclid(DAY) > 0),
                    hi.div_euclid(DAY),
                );
                let d = if dl <= dh {
                    self.rng.gen_range(dl..=dh)
                } else {
                    dl
                };
                constraints::civil(d)
            }
            _ => format!(
                "{}T{}Z",
                constraints::civil(t.div_euclid(DAY)),
                constraints::hms(t.rem_euclid(DAY))
            ),
        }
    }

    /// A string matching every pattern within the length bounds (best
    /// effort: the last candidate when nothing fits; `None` when a pattern
    /// does not parse).
    fn gen_patterned(
        &mut self,
        patterns: &[&str],
        min: Option<usize>,
        max: Option<usize>,
    ) -> Option<String> {
        // Only literals (`starts_with` / `ends_with` / `contains`): compose
        // prefix + contained parts + filler + suffix.
        let lits: Option<Vec<regex_gen::Literal>> =
            patterns.iter().map(|p| regex_gen::literal(p)).collect();
        if let Some(lits) = lits {
            let (mut head, mut mid, mut tail) = (String::new(), String::new(), String::new());
            for l in lits {
                match l {
                    regex_gen::Literal::Prefix(p) if p.len() > head.len() => head = p,
                    regex_gen::Literal::Suffix(s) if s.len() > tail.len() => tail = s,
                    regex_gen::Literal::Contains(c) => mid.push_str(&c),
                    _ => {}
                }
            }
            let fixed = head.chars().count() + mid.chars().count() + tail.chars().count();
            let n = self.pick_len(min, max, fixed + 3, fixed + 10);
            let filler = self.gen_chars(LOWER_ALPHA, n.saturating_sub(fixed));
            return Some(format!("{head}{mid}{filler}{tail}"));
        }

        let regexes: Vec<vld::regex_lite::Regex> = patterns
            .iter()
            .filter_map(|p| vld::regex_lite::Regex::new(p).ok())
            .collect();
        // Generate from the non-literal patterns; literal prefixes / suffixes
        // are also tried overlaid on each candidate.
        let (mut head, mut tail) = (String::new(), String::new());
        let mut bases = Vec::new();
        for p in patterns {
            match regex_gen::literal(p) {
                Some(regex_gen::Literal::Prefix(h)) => head = h,
                Some(regex_gen::Literal::Suffix(t)) => tail = t,
                _ => bases.push(*p),
            }
        }
        let overlay = |s: &str| -> String {
            let chars: Vec<char> = s.chars().collect();
            let (h, t) = (head.chars().count(), tail.chars().count());
            let mid: String = chars
                .get(h.min(chars.len())..chars.len().saturating_sub(t).max(h.min(chars.len())))
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            format!("{head}{mid}{tail}")
        };
        let mut last = None;
        // ponytail: generate-and-test; rare intersections of complex
        // patterns can miss, then the last candidate is returned.
        for i in 0..50 {
            let s = regex_gen::generate(&mut self.rng, bases[i % bases.len()])?;
            for c in [overlay(&s), s] {
                if len_ok(&c, min, max) && regexes.iter().all(|r| r.is_match(&c)) {
                    return Some(c);
                }
                last = Some(c);
            }
        }
        last
    }

    /// Try to generate a realistic value based on the JSON property name.
    fn gen_string_by_hint(&mut self, hint: &str, obj: &Map<String, Value>) -> Option<Value> {
        let h = hint.to_ascii_lowercase();
        let h = h.as_str();

        // Exact or suffix/contains matches
        let result: Option<String> = match h {
            // ── names ─────────────────────────────────────────
            "first_name" | "firstname" | "given_name" | "givenname" => {
                Some(self.pick(FIRST_NAMES).to_string())
            }
            "last_name" | "lastname" | "surname" | "family_name" | "familyname" => {
                Some(self.pick(LAST_NAMES).to_string())
            }
            "name" | "full_name" | "fullname" | "display_name" | "displayname" | "user_name"
            | "author" | "author_name" => {
                let first = self.pick(FIRST_NAMES);
                let last = self.pick(LAST_NAMES);
                Some(format!("{first} {last}"))
            }
            "username" | "login" | "handle" | "nick" | "nickname" => {
                let first = self.pick(FIRST_NAMES).to_lowercase();
                let n: u16 = self.rng.gen_range(1..999);
                Some(format!("{first}{n}"))
            }

            // ── contact ───────────────────────────────────────
            "email" | "email_address" | "emailaddress" | "mail" => Some(self.gen_email()),
            "phone" | "phone_number" | "phonenumber" | "tel" | "telephone" | "mobile" | "cell" => {
                Some(self.gen_phone())
            }

            // ── address ───────────────────────────────────────
            "city" | "town" => Some(self.pick(CITIES).to_string()),
            "country" => Some(self.pick(COUNTRIES).to_string()),
            "state" | "province" | "region" => Some(self.pick(STATES).to_string()),
            "street" | "street_address" | "address_line" | "address_line1" | "address"
            | "address1" | "line1" => Some(self.gen_street_address()),
            "zip" | "zipcode" | "zip_code" | "postal" | "postal_code" | "postalcode" => {
                Some(self.gen_zip())
            }
            "latitude" | "lat" => {
                let v: f64 = self.rng.gen_range(-90.0..=90.0);
                Some(format!("{:.6}", v))
            }
            "longitude" | "lng" | "lon" => {
                let v: f64 = self.rng.gen_range(-180.0..=180.0);
                Some(format!("{:.6}", v))
            }

            // ── company / org ─────────────────────────────────
            "company" | "company_name" | "companyname" | "organization" | "organisation"
            | "org" | "employer" => Some(self.pick(COMPANIES).to_string()),
            "department" | "team" | "division" => Some(self.pick(DEPARTMENTS).to_string()),
            "job_title" | "jobtitle" | "position" | "role" | "title" | "occupation" => {
                Some(self.pick(JOB_TITLES).to_string())
            }

            // ── internet ──────────────────────────────────────
            "url" | "website" | "homepage" | "link" | "site" | "uri" | "href" => {
                Some(self.gen_url())
            }
            "domain" | "domain_name" | "domainname" | "host" | "hostname" => {
                Some(self.gen_hostname())
            }
            "ip" | "ip_address" | "ipaddress" | "ipv4" => Some(self.gen_ipv4()),
            "ipv6" => Some(self.gen_ipv6()),
            "user_agent" | "useragent" | "ua" => Some(self.gen_user_agent()),
            "mac" | "mac_address" | "macaddress" => Some(self.gen_mac_address()),

            // ── identifiers ───────────────────────────────────
            "id" | "uid" | "uuid" | "guid" => Some(self.gen_uuid()),
            "slug" => Some(self.gen_slug()),
            "token" | "api_key" | "apikey" | "secret" | "access_token" | "refresh_token" => {
                Some(self.gen_token(32))
            }
            "password" | "pass" | "pwd" | "secret_key" => Some(self.gen_password()),

            // ── text ──────────────────────────────────────────
            "description" | "desc" | "bio" | "about" | "summary" | "overview" => {
                Some(self.gen_sentence_range(8, 20))
            }
            "comment" | "note" | "notes" | "message" | "body" | "content" | "text" => {
                Some(self.gen_sentence_range(5, 15))
            }

            // ── product ───────────────────────────────────────
            "product" | "product_name" | "productname" | "item" | "item_name" => {
                let adj = self.pick(ADJECTIVES);
                let noun = self.pick(PRODUCT_NOUNS);
                Some(format!("{adj} {noun}"))
            }
            "brand" => Some(self.pick(COMPANIES).to_string()),
            "sku" | "product_code" | "productcode" | "code" | "barcode" => {
                let prefix: String = (0..3)
                    .map(|_| self.rng.gen_range(b'A'..=b'Z') as char)
                    .collect();
                let num: u32 = self.rng.gen_range(10000..99999);
                Some(format!("{prefix}-{num}"))
            }
            "category" | "genre" | "type" | "kind" | "group" => {
                Some(self.pick(CATEGORIES).to_string())
            }
            "tag" | "label" => Some(self.pick(TAGS).to_string()),

            // ── color ─────────────────────────────────────────
            "color" | "colour" => Some(self.pick(COLORS).to_string()),
            "hex_color" | "hexcolor" | "color_hex" => Some(format!(
                "#{:02x}{:02x}{:02x}",
                self.rng.gen_range(0u8..=255),
                self.rng.gen_range(0u8..=255),
                self.rng.gen_range(0u8..=255),
            )),

            // ── misc ──────────────────────────────────────────
            "currency" | "currency_code" => Some(self.pick(CURRENCIES).to_string()),
            "locale" | "lang" | "language" => Some(self.pick(LOCALES).to_string()),
            "timezone" | "tz" | "time_zone" => Some(self.pick(TIMEZONES).to_string()),
            "mime" | "mime_type" | "mimetype" | "content_type" | "contenttype" => {
                Some(self.pick(MIME_TYPES).to_string())
            }
            "file_name" | "filename" => {
                let word = self.pick(WORDS).to_lowercase();
                let ext = self.pick(FILE_EXTENSIONS);
                Some(format!("{word}.{ext}"))
            }
            "extension" | "ext" | "file_ext" | "file_extension" => {
                Some(self.pick(FILE_EXTENSIONS).to_string())
            }
            "version" | "semver" => {
                let major = self.rng.gen_range(0u8..10);
                let minor = self.rng.gen_range(0u8..30);
                let patch = self.rng.gen_range(0u16..100);
                Some(format!("{major}.{minor}.{patch}"))
            }
            "credit_card" | "creditcard" | "card_number" | "cardnumber" | "cc" => {
                Some(self.gen_credit_card())
            }
            "isbn" => Some(self.gen_isbn()),
            "ssn" => {
                let a: u16 = self.rng.gen_range(100..999);
                let b: u8 = self.rng.gen_range(10..99);
                let c: u16 = self.rng.gen_range(1000..9999);
                Some(format!("{a}-{b}-{c}"))
            }

            _ => None,
        };

        let (min, max) = len_bounds(obj);
        result.map(|s| Value::String(fit_len(s, min.unwrap_or(0), max, 'x')))
    }

    /// Generate a readable, word-based string that respects `minLength`/`maxLength`.
    fn gen_readable_string(&mut self, obj: &Map<String, Value>) -> Value {
        let min_len = obj.get("minLength").and_then(Value::as_u64).unwrap_or(1) as usize;
        let max_len = obj
            .get("maxLength")
            .and_then(Value::as_u64)
            .unwrap_or(min_len.max(1) as u64 + 30) as usize;
        let max_len = max_len.max(min_len);

        // Build word-by-word up to the target range
        let target = if min_len == max_len {
            min_len
        } else {
            self.rng.gen_range(min_len..=max_len)
        };

        let mut s = String::new();
        let capitalize_first = true;

        loop {
            if s.len() >= target {
                break;
            }
            let word = self.pick(WORDS);
            if !s.is_empty() {
                // Check if adding " word" would exceed max
                if s.len() + 1 + word.len() > max_len {
                    // Try to fill remaining with a short word or chars
                    let remaining = max_len - s.len();
                    if remaining > 1 {
                        s.push(' ');
                        let filler: String = WORDS
                            .iter()
                            .filter(|w| w.len() < remaining)
                            .take(1)
                            .map(|w| w.to_string())
                            .next()
                            .unwrap_or_else(|| {
                                (0..remaining - 1)
                                    .map(|_| {
                                        LOWER_ALPHA[self.rng.gen_range(0..LOWER_ALPHA.len())]
                                            as char
                                    })
                                    .collect()
                            });
                        s.push_str(&filler);
                    }
                    break;
                }
                s.push(' ');
            }
            s.push_str(word);
        }

        // Capitalize first letter
        if capitalize_first && !s.is_empty() {
            let mut chars = s.chars();
            s = chars.next().unwrap().to_uppercase().chain(chars).collect();
        }

        Value::String(fit_len(s, min_len, Some(max_len), 'a'))
    }

    // ── format-specific generators ──────────────────────────────────────

    fn gen_string_format(&mut self, fmt: &str, obj: &Map<String, Value>) -> String {
        let (min, max) = len_bounds(obj);

        // Free-length character-class formats: build exactly `n` chars.
        let charset: Option<&[u8]> = match fmt {
            "hex" => Some(HEX),
            "alpha" => Some(ALPHA),
            "alphanumeric" => Some(TOKEN_ALPHA),
            "lowercase" => Some(LOWER_ALPHA),
            "uppercase" => Some(UPPER_ALPHA),
            _ => None,
        };
        if let Some(cs) = charset {
            let n = self.pick_len(min, max, 8, 24);
            return self.gen_chars(cs, n);
        }

        // Byte-string formats: `minLength`/`maxLength` count *encoded* characters
        // (as emitted by `vld::bytes()`), so pick a byte count whose encoding fits.
        if let Some(enc) = fmt.strip_prefix("byte") {
            let encoded_len = |n: usize| match enc {
                "-hex" => n * 2,
                "-base64url" => (n * 4).div_ceil(3),
                _ => n.div_ceil(3) * 4,
            };
            let fits: Vec<usize> = (1..=64)
                .filter(|_| min.is_some() || max.is_some())
                .filter(|&n| {
                    let l = encoded_len(n);
                    // vld's hex `maxLength` reserves 2 chars for an optional `0x`
                    // prefix, so an unprefixed value must leave that room.
                    let slack = if enc == "-hex" { 2 } else { 0 };
                    min.is_none_or(|m| l >= m) && max.is_none_or(|m| l + slack <= m)
                })
                .collect();
            let n = if fits.is_empty() {
                self.pick_len(None, None, 8, 32)
            } else {
                fits[self.rng.gen_range(0..fits.len())]
            };
            let bytes: Vec<u8> = (0..n).map(|_| self.rng.generator()).collect();
            return match enc {
                "-hex" => bytes.iter().map(|b| format!("{b:02x}")).collect(),
                "-base64url" => base64url(&bytes),
                _ => base64_encode(&bytes),
            };
        }

        if fmt == "decimal" {
            return self.gen_decimal(obj);
        }

        if matches!(fmt, "date" | "date-time" | "time") {
            return self.gen_temporal(fmt, obj);
        }
        // vld's offset-free variants: `partial-time` (time of day) and
        // `naive-date-time` (no `Z` / offset).
        if fmt == "partial-time" {
            return self.gen_temporal("time", obj);
        }
        if fmt == "naive-date-time" {
            let s = self.gen_temporal("date-time", obj);
            return s.strip_suffix('Z').map(str::to_string).unwrap_or(s);
        }

        // Unpadded Base64URL text: `n` chars encode `3n/4` bytes, `n % 4 != 1`.
        if fmt == "base64url" {
            let mut n = self.pick_len(min, max, 8, 32);
            if n % 4 == 1 {
                n = if max.is_none_or(|m| n < m) {
                    n + 1
                } else {
                    n - 1
                };
            }
            let bytes: Vec<u8> = (0..n * 3 / 4).map(|_| self.rng.generator()).collect();
            return base64url(&bytes);
        }

        // Structured formats: regenerate until the length fits.
        let mut last = None;
        for _ in 0..20 {
            let Some(s) = self.gen_format_value(fmt) else {
                break;
            };
            if len_ok(&s, min, max) {
                return s;
            }
            last = Some(s);
        }
        match last {
            // `email` can be padded in the local part without breaking it.
            Some(s) if fmt == "email" && min.is_some_and(|m| s.chars().count() < m) => {
                let pad = min.unwrap() - s.chars().count();
                s.replacen('@', &format!("{}@", "x".repeat(pad)), 1)
            }
            // Too long: shortest form `<local>@ex.io`.
            Some(_) if fmt == "email" && max.is_some_and(|m| m >= 7) => {
                let local = self.gen_chars(LOWER_ALPHA, (max.unwrap() - 6).min(8));
                format!("{local}@ex.io")
            }
            Some(s) => s,
            // Unknown format — readable string within the length bounds.
            None => match self.gen_readable_string(obj) {
                Value::String(s) => s,
                _ => unreachable!(),
            },
        }
    }

    /// Generate a value for a known string `format`; `None` if unknown.
    fn gen_format_value(&mut self, fmt: &str) -> Option<String> {
        Some(match fmt {
            "email" => self.gen_email(),
            "uuid" | "uuid-v4" => self.gen_uuid(),
            "uuid-v1" => self.gen_uuid_v(b'1'),
            "uuid-v7" => self.gen_uuid_v(b'7'),
            "uri" | "url" | "url-strict" => self.gen_url(),
            "ipv4" => self.gen_ipv4(),
            "ipv6" => self.gen_ipv6(),
            "ip" => {
                if self.rng.gen_bool(0.5) {
                    self.gen_ipv4()
                } else {
                    self.gen_ipv6()
                }
            }
            "cidr" => {
                let prefix = self.rng.gen_range(8u8..=32);
                format!("{}/{prefix}", self.gen_ipv4())
            }
            "socket-addr" => {
                let port = self.rng.gen_range(1024u16..=65535);
                format!("{}:{port}", self.gen_ipv4())
            }
            "hostname" => self.gen_hostname(),
            "date" | "iso-date" => self.gen_date(),
            "time" | "iso-time" => self.gen_time(),
            "date-time" | "iso-datetime" => self.gen_datetime(),
            "base64" => self.gen_base64(),
            "json" => match self.rng.gen_range(0..4) {
                0 => self.rng.gen_range(0..1000).to_string(),
                1 => json!([self.rng.gen_range(0..100), self.pick(WORDS)]).to_string(),
                2 => json!(self.pick(WORDS)).to_string(),
                _ => {
                    let mut m = Map::new();
                    m.insert(
                        self.pick(WORDS).to_lowercase(),
                        json!(self.rng.gen_range(0..100)),
                    );
                    Value::Object(m).to_string()
                }
            },
            "cuid2" => self.gen_cuid2(),
            "ulid" => self.gen_ulid(),
            "nanoid" => self.gen_nanoid(),
            "emoji" => self.gen_emoji(),
            "phone" => self.gen_phone(),
            "phone-e164" => {
                // US numbers with real area codes: valid per libphonenumber.
                let area = self.pick(&[201u16, 212, 213, 305, 312, 415, 617, 646, 702, 718]);
                let exchange = self.rng.gen_range(200u16..=999);
                let line = self.rng.gen_range(1000u16..=9999);
                format!("+1{area}{exchange}{line}")
            }
            "credit-card" => self.gen_credit_card(),
            "mac-address" | "mac" => self.gen_mac_address(),
            "color" | "hex-color" => self.gen_hex_color(),
            "semver" => self.gen_semver(),
            "semver-full" => {
                let v = self.gen_semver();
                match self.rng.gen_range(0..3) {
                    0 => format!("{v}-alpha.{}", self.rng.gen_range(1u8..10)),
                    1 => format!("{v}+build.{}", self.rng.gen_range(1u16..1000)),
                    _ => v,
                }
            }
            "slug" => self.gen_slug(),
            "currency-code" => self.pick(CURRENCIES).to_string(),
            "country-code" => self.pick(LOCALES)[3..].to_string(),
            "locale" => {
                let l = self.pick(LOCALES);
                if self.rng.gen_bool(0.3) {
                    l[..2].to_string()
                } else {
                    l.to_string()
                }
            }
            "cron" => {
                let minute = self.rng.gen_range(0u8..60);
                let hour = self.rng.gen_range(0u8..24);
                match self.rng.gen_range(0..3) {
                    0 => format!("{minute} {hour} * * *"),
                    1 => format!("*/{} * * * *", self.rng.gen_range(1u8..30)),
                    _ => format!("{minute} {hour} * * 1-5"),
                }
            }
            "jwt" => {
                let header = base64url(br#"{"alg":"HS256","typ":"JWT"}"#);
                let payload = base64url(format!(r#"{{"sub":"{}"}}"#, self.next_id()).as_bytes());
                let sig = self.gen_chars(B64URL_ALPHA, 43);
                format!("{header}.{payload}.{sig}")
            }
            "path" => {
                let dir = self.pick(WORDS).to_lowercase();
                let file = self.pick(WORDS).to_lowercase();
                let ext = self.pick(FILE_EXTENSIONS);
                format!("/var/data/{dir}/{file}.{ext}")
            }
            _ => return None,
        })
    }

    /// Pick a length in `[min, max]`, defaulting to `[dmin, dmax]` around
    /// whichever bound is missing.
    fn pick_len(
        &mut self,
        min: Option<usize>,
        max: Option<usize>,
        dmin: usize,
        dmax: usize,
    ) -> usize {
        let lo = min.unwrap_or(dmin.min(max.unwrap_or(dmin)));
        let hi = max.unwrap_or(lo.max(dmax)).max(lo);
        self.rng.gen_range(lo..=hi)
    }

    fn gen_chars(&mut self, charset: &[u8], n: usize) -> String {
        (0..n)
            .map(|_| charset[self.rng.gen_range(0..charset.len())] as char)
            .collect()
    }

    fn gen_semver(&mut self) -> String {
        let major = self.rng.gen_range(0u8..10);
        let minor = self.rng.gen_range(0u8..30);
        let patch = self.rng.gen_range(0u16..100);
        format!("{major}.{minor}.{patch}")
    }

    /// Decimal string honouring vld's `x-minDecimal` / `x-maxDecimal`.
    fn gen_decimal(&mut self, obj: &Map<String, Value>) -> String {
        let bound = |k: &str| {
            obj.get(k).and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or(v.as_f64())
            })
        };
        let lo = bound("x-minDecimal");
        let hi = bound("x-maxDecimal");
        let (lo, hi) = match (lo, hi) {
            (Some(l), Some(h)) => (l, h.max(l)),
            (Some(l), None) => (l, l + 1000.0),
            (None, Some(h)) => (h - 1000.0, h),
            (None, None) => (0.0, 1000.0),
        };
        let v = lo + self.rng.generator::<f64>() * (hi - lo);
        let s = format!("{v:.2}");
        if s.parse::<f64>().is_ok_and(|r| (lo..=hi).contains(&r)) {
            s
        } else {
            v.to_string()
        }
    }

    fn gen_email(&mut self) -> String {
        let first = self.pick(FIRST_NAMES).to_lowercase();
        let last = self.pick(LAST_NAMES).to_lowercase();
        let n: u16 = self.rng.gen_range(1..99);
        let domain = self.pick(EMAIL_DOMAINS);
        // Vary the pattern
        match self.rng.gen_range(0..4) {
            0 => format!("{first}.{last}@{domain}"),
            1 => format!("{first}{last}{n}@{domain}"),
            2 => format!("{first}_{last}@{domain}"),
            _ => format!("{}_{n}@{domain}", &first[..1.min(first.len())]),
        }
    }

    fn gen_uuid(&mut self) -> String {
        self.gen_uuid_v(b'4')
    }

    /// Random UUID with the given version nibble and RFC 4122 variant.
    fn gen_uuid_v(&mut self, version: u8) -> String {
        format!(
            "{}-{}-{}{}-{}{}-{}",
            self.gen_chars(HEX, 8),
            self.gen_chars(HEX, 4),
            version as char,
            self.gen_chars(HEX, 3),
            HEX_89AB[self.rng.gen_range(0..4)] as char,
            self.gen_chars(HEX, 3),
            self.gen_chars(HEX, 12),
        )
    }

    fn gen_url(&mut self) -> String {
        let tld = self.pick(TLDS);
        let word = self.pick(WORDS).to_lowercase();
        let path_word = self.pick(WORDS).to_lowercase();
        let scheme = if self.rng.gen_bool(0.9) {
            "https"
        } else {
            "http"
        };
        match self.rng.gen_range(0..4) {
            0 => format!("{scheme}://www.{word}.{tld}/{path_word}"),
            1 => format!("{scheme}://{word}.{tld}"),
            2 => {
                let slug = self.gen_slug();
                format!("{scheme}://{word}.{tld}/blog/{slug}")
            }
            _ => {
                let id: u32 = self.rng.gen_range(1..99999);
                format!("{scheme}://{word}.{tld}/item/{id}")
            }
        }
    }

    fn gen_ipv4(&mut self) -> String {
        let a = self.rng.gen_range(1u8..=254);
        let b = self.rng.gen_range(0u8..=255);
        let c = self.rng.gen_range(0u8..=255);
        let d = self.rng.gen_range(1u8..=254);
        format!("{a}.{b}.{c}.{d}")
    }

    fn gen_ipv6(&mut self) -> String {
        let groups: Vec<String> = (0..8)
            .map(|_| format!("{:04x}", self.rng.gen_range(0u16..=0xffff)))
            .collect();
        groups.join(":")
    }

    fn gen_hostname(&mut self) -> String {
        let sub = self.pick(WORDS).to_lowercase();
        let tld = self.pick(TLDS);
        let word = self.pick(WORDS).to_lowercase();
        format!("{sub}.{word}.{tld}")
    }

    fn gen_date(&mut self) -> String {
        let y = self.rng.gen_range(2000..=2030);
        let m = self.rng.gen_range(1..=12);
        let d = self.rng.gen_range(1..=days_in_month(m));
        format!("{y:04}-{m:02}-{d:02}")
    }

    fn gen_time(&mut self) -> String {
        let h = self.rng.gen_range(0..24);
        let m = self.rng.gen_range(0..60);
        let s = self.rng.gen_range(0..60);
        format!("{h:02}:{m:02}:{s:02}")
    }

    fn gen_datetime(&mut self) -> String {
        let date = self.gen_date();
        let time = self.gen_time();
        format!("{date}T{time}Z")
    }

    fn gen_base64(&mut self) -> String {
        let byte_len = self.rng.gen_range(8..32);
        let bytes: Vec<u8> = (0..byte_len).map(|_| self.rng.generator()).collect();
        base64_encode(&bytes)
    }

    fn gen_cuid2(&mut self) -> String {
        let first = LOWER_ALPHA[self.rng.gen_range(0..LOWER_ALPHA.len())] as char;
        let rest: String = (0..23)
            .map(|_| LOWER_DIGIT[self.rng.gen_range(0..LOWER_DIGIT.len())] as char)
            .collect();
        format!("{first}{rest}")
    }

    /// 26 Crockford chars; the first is `0`-`7` (a ULID is 128 bits).
    fn gen_ulid(&mut self) -> String {
        (0..26)
            .map(|i| {
                let n = if i == 0 { 8 } else { CROCKFORD.len() };
                CROCKFORD[self.rng.gen_range(0..n)] as char
            })
            .collect()
    }

    fn gen_nanoid(&mut self) -> String {
        let len = self.rng.gen_range(10..22);
        (0..len)
            .map(|_| NANOID_ALPHA[self.rng.gen_range(0..NANOID_ALPHA.len())] as char)
            .collect()
    }

    fn gen_emoji(&mut self) -> String {
        self.pick(EMOJIS).to_string()
    }

    fn gen_phone(&mut self) -> String {
        let area: u16 = self.rng.gen_range(200..999);
        let prefix: u16 = self.rng.gen_range(200..999);
        let line: u16 = self.rng.gen_range(1000..9999);
        match self.rng.gen_range(0..3) {
            0 => format!("+1 ({area}) {prefix}-{line}"),
            1 => format!("+44 {area} {prefix} {line}"),
            _ => format!("+7 {area} {prefix}-{line}"),
        }
    }

    fn gen_credit_card(&mut self) -> String {
        // Luhn-valid 16-digit number (Visa-like starting with 4)
        let mut digits = vec![4u8];
        for _ in 1..15 {
            digits.push(self.rng.gen_range(0..10));
        }
        // compute Luhn check digit
        let check = luhn_check_digit(&digits);
        digits.push(check);
        let s: String = digits.iter().map(|d| (b'0' + d) as char).collect();
        format!("{}-{}-{}-{}", &s[0..4], &s[4..8], &s[8..12], &s[12..16])
    }

    fn gen_isbn(&mut self) -> String {
        let mut digits: Vec<u8> = (0..12).map(|_| self.rng.gen_range(0..10)).collect();
        digits[0] = 9;
        digits[1] = 7;
        digits[2] = if self.rng.gen_bool(0.5) { 8 } else { 9 };
        let sum: u8 = digits
            .iter()
            .enumerate()
            .map(|(i, &d)| if i % 2 == 0 { d } else { d * 3 })
            .sum();
        let check = (10 - (sum % 10)) % 10;
        digits.push(check);
        let s: String = digits.iter().map(|d| (b'0' + d) as char).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &s[0..3],
            &s[3..4],
            &s[4..8],
            &s[8..12],
            &s[12..13]
        )
    }

    fn gen_mac_address(&mut self) -> String {
        let octets: Vec<String> = (0..6)
            .map(|_| format!("{:02X}", self.rng.gen_range(0u8..=255)))
            .collect();
        octets.join(":")
    }

    fn gen_hex_color(&mut self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.rng.gen_range(0u8..=255),
            self.rng.gen_range(0u8..=255),
            self.rng.gen_range(0u8..=255),
        )
    }

    fn gen_user_agent(&mut self) -> String {
        let browsers = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_2) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
            "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
        ];
        self.pick(&browsers).to_string()
    }

    fn gen_slug(&mut self) -> String {
        let count = self.rng.gen_range(2..5);
        let words: Vec<String> = (0..count)
            .map(|_| self.pick(WORDS).to_lowercase())
            .collect();
        words.join("-")
    }

    fn gen_token(&mut self, len: usize) -> String {
        (0..len)
            .map(|_| TOKEN_ALPHA[self.rng.gen_range(0..TOKEN_ALPHA.len())] as char)
            .collect()
    }

    fn gen_password(&mut self) -> String {
        let len = self.rng.gen_range(10..18);
        let mut bytes: Vec<u8> = (0..len)
            .map(|_| PASSWORD_ALPHA[self.rng.gen_range(0..PASSWORD_ALPHA.len())])
            .collect();
        // Guarantee at least one of each class.
        bytes[0] = b'A' + self.rng.gen_range(0..26);
        bytes[1] = b'a' + self.rng.gen_range(0..26);
        bytes[2] = b'0' + self.rng.gen_range(0..10);
        let specials = b"!@#$%&*";
        bytes[3] = specials[self.rng.gen_range(0..specials.len())];
        bytes.into_iter().map(char::from).collect()
    }

    fn gen_street_address(&mut self) -> String {
        let num: u16 = self.rng.gen_range(1..9999);
        let street = self.pick(STREET_NAMES);
        let suffix = self.pick(STREET_SUFFIXES);
        format!("{num} {street} {suffix}")
    }

    fn gen_zip(&mut self) -> String {
        let n: u32 = self.rng.gen_range(10000..99999);
        format!("{n}")
    }

    fn gen_sentence_range(&mut self, min_words: usize, max_words: usize) -> String {
        let count = self.rng.gen_range(min_words..=max_words);
        let mut words: Vec<String> = (0..count)
            .map(|_| self.pick(LOREM_WORDS).to_string())
            .collect();
        if let Some(first) = words.first_mut() {
            let mut chars = first.chars();
            *first = chars.next().unwrap().to_uppercase().chain(chars).collect();
        }
        let mut sentence = words.join(" ");
        sentence.push('.');
        sentence
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  INTEGER
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_integer(&mut self, obj: &Map<String, Value>, hint: Option<&str>) -> Value {
        // i128 covers the whole i64 and u64 ranges exactly.
        let int = |k: &str, round_up: bool| -> Option<i128> {
            let v = obj.get(k)?;
            v.as_i64()
                .map(i128::from)
                .or_else(|| v.as_u64().map(i128::from))
                .or_else(|| {
                    let f = v.as_f64()?;
                    Some((if round_up { f.ceil() } else { f.floor() }) as i128)
                })
        };
        // Smallest / largest integer satisfying the (exclusive) bounds.
        let lo = int("minimum", true).or_else(|| int("exclusiveMinimum", false).map(|v| v + 1));
        let hi = int("maximum", false).or_else(|| int("exclusiveMaximum", true).map(|v| v - 1));

        // A realistic range for known field names, when the schema leaves
        // one side open and the two ranges overlap.
        let hinted = hint
            .and_then(int_hint_range)
            .filter(|_| lo.is_none() || hi.is_none())
            .map(|(hl, hh)| (i128::from(hl), i128::from(hh)))
            .map(|(hl, hh)| (lo.map_or(hl, |l| l.max(hl)), hi.map_or(hh, |h| h.min(hh))))
            .filter(|(l, h)| l <= h);

        let (lo, hi) = match (hinted, lo, hi) {
            (Some(r), _, _) => r,
            (None, Some(l), Some(h)) => (l, h),
            (None, Some(l), None) => (l, l.max(0).saturating_add(1000)),
            (None, None, Some(h)) => (if h >= 0 { 0 } else { h.saturating_sub(1000) }, h),
            (None, None, None) => (0, 1000),
        };
        if lo > hi {
            // Unsatisfiable range — best effort, never panic.
            return int_json(lo);
        }

        let m = obj
            .get("multipleOf")
            .and_then(Value::as_f64)
            .filter(|m| *m >= 1.0 && m.fract() == 0.0)
            .map(|m| m as i128);
        if let Some(m) = m {
            // k * m within [lo, hi]
            let k_lo = lo.div_euclid(m) + i128::from(lo.rem_euclid(m) != 0);
            let k_hi = hi.div_euclid(m);
            if k_lo <= k_hi {
                return int_json(self.rng.gen_range(k_lo..=k_hi) * m);
            }
            return int_json(k_lo.saturating_mul(m));
        }

        int_json(self.rng.gen_range(lo..=hi))
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  NUMBER (float)
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_number(&mut self, obj: &Map<String, Value>, hint: Option<&str>) -> Value {
        if obj.get("type").and_then(Value::as_str) == Some("integer") {
            return self.gen_integer(obj, hint);
        }

        // Heuristic by hint — override range for known fields
        if let Some(h) = hint {
            let has_min = obj.contains_key("minimum") || obj.contains_key("exclusiveMinimum");
            let has_max = obj.contains_key("maximum") || obj.contains_key("exclusiveMaximum");
            if !has_min && !has_max && !obj.contains_key("multipleOf") {
                if let Some(val) = self.gen_number_by_hint(h) {
                    return json!(val);
                }
            }
        }

        let f = |k: &str| obj.get(k).and_then(Value::as_f64);
        let (ex_lo, ex_hi) = (f("exclusiveMinimum"), f("exclusiveMaximum"));
        let lo = f("minimum").or(ex_lo);
        let hi = f("maximum").or(ex_hi);
        let (lo, hi) = match (lo, hi) {
            (Some(l), Some(h)) => (l, h),
            (Some(l), None) => (l, l.max(0.0) + 1000.0),
            // `negative()` (`exclusiveMaximum: 0`) needs room below 0.
            (None, Some(h)) => (if h > 0.0 { 0.0 } else { h - 1000.0 }, h),
            (None, None) => (0.0, 1000.0),
        };
        let in_range = |v: f64| {
            v >= lo
                && v <= hi
                && ex_lo.is_none_or(|e| v > e)
                && ex_hi.is_none_or(|e| v < e)
                && f("minimum").is_none_or(|m| v >= m)
                && f("maximum").is_none_or(|m| v <= m)
        };

        // multipleOf: pick k * m inside the range.
        if let Some(m) = f("multipleOf").filter(|m| *m > 0.0) {
            let k_lo = (lo / m).ceil() as i64;
            let k_hi = (hi / m).floor() as i64;
            // Decimals of `m` (e.g. 0.05 -> 2) to strip float noise from k * m.
            let decimals = format!("{m}").split('.').nth(1).map_or(0, str::len) as i32;
            let p = 10f64.powi(decimals);
            for _ in 0..20 {
                if k_lo > k_hi {
                    break;
                }
                let k = self.rng.gen_range(k_lo..=k_hi) as f64;
                for v in [(k * m * p).round() / p, k * m] {
                    // Same check vld uses (`n % m` ~ 0).
                    if in_range(v) && (v % m).abs() <= f64::EPSILON {
                        return json!(v);
                    }
                }
            }
            // Zero is a multiple of everything.
            if in_range(0.0) {
                return json!(0.0);
            }
        }

        if lo > hi {
            // Unsatisfiable range — best effort, never panic.
            return json!(lo);
        }
        for _ in 0..20 {
            let val = lo + self.rng.generator::<f64>() * (hi - lo);
            // Prefer 2 decimals for readability, if that stays in range.
            let rounded = (val * 100.0).round() / 100.0;
            if in_range(rounded) {
                return json!(rounded);
            }
            if in_range(val) {
                return json!(val);
            }
        }
        json!((lo + hi) / 2.0)
    }

    /// Generate a realistic float based on field name when no constraints given.
    fn gen_number_by_hint(&mut self, hint: &str) -> Option<f64> {
        let h = hint.to_ascii_lowercase();
        match h.as_str() {
            "latitude" | "lat" => {
                let v: f64 = self.rng.gen_range(-90.0..=90.0);
                Some((v * 1_000_000.0).round() / 1_000_000.0)
            }
            "longitude" | "lng" | "lon" => {
                let v: f64 = self.rng.gen_range(-180.0..=180.0);
                Some((v * 1_000_000.0).round() / 1_000_000.0)
            }
            "altitude" | "elevation" | "alt" => {
                let v: f64 = self.rng.gen_range(0.0..=8848.0);
                Some((v * 100.0).round() / 100.0)
            }
            "price" | "cost" | "amount" | "total" | "subtotal" | "fee" | "balance" | "payment" => {
                let v: f64 = self.rng.gen_range(0.01..=9999.99);
                Some((v * 100.0).round() / 100.0)
            }
            "tax" | "discount" | "vat" => {
                let v: f64 = self.rng.gen_range(0.0..=30.0);
                Some((v * 100.0).round() / 100.0)
            }
            "rating" | "score" => {
                let v: f64 = self.rng.gen_range(1.0..=5.0);
                Some((v * 10.0).round() / 10.0)
            }
            "weight" | "mass" => {
                let v: f64 = self.rng.gen_range(0.1..=1000.0);
                Some((v * 100.0).round() / 100.0)
            }
            "temperature" | "temp" => {
                let v: f64 = self.rng.gen_range(-40.0..=50.0);
                Some((v * 10.0).round() / 10.0)
            }
            "percentage" | "percent" | "progress" => {
                let v: f64 = self.rng.gen_range(0.0..=100.0);
                Some((v * 100.0).round() / 100.0)
            }
            "speed" | "velocity" => {
                let v: f64 = self.rng.gen_range(0.0..=300.0);
                Some((v * 100.0).round() / 100.0)
            }
            "distance" | "radius" => {
                let v: f64 = self.rng.gen_range(0.1..=10000.0);
                Some((v * 100.0).round() / 100.0)
            }
            "area" => {
                let v: f64 = self.rng.gen_range(1.0..=100000.0);
                Some((v * 100.0).round() / 100.0)
            }
            _ => None,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  BOOLEAN
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_boolean(&mut self) -> Value {
        Value::Bool(self.rng.generator())
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  ARRAY
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_array(&mut self, obj: &Map<String, Value>, _hint: Option<&str>) -> Value {
        let n = |k: &str| obj.get(k).and_then(Value::as_u64).map(|v| v as usize);
        // Sets: `x-minUniqueItems` / `x-maxUniqueItems` count distinct items.
        let (umin, umax) = (n("x-minUniqueItems"), n("x-maxUniqueItems"));
        let min_items = n("minItems").unwrap_or(0).max(umin.unwrap_or(0));
        let max_items = n("maxItems")
            .into_iter()
            .chain(umax)
            .min()
            .unwrap_or(min_items.max(1) + 3)
            .max(min_items);

        let mut count = if min_items == max_items {
            min_items
        } else {
            self.rng.gen_range(min_items..=max_items)
        };

        // Tuple-style (prefixItems)
        if let Some(prefix) = obj.get("prefixItems").and_then(Value::as_array) {
            let mut arr: Vec<Value> = prefix.iter().map(|s| self.value(s)).collect();
            if count > arr.len() {
                if let Some(items) = obj.get("items").filter(|i| i.is_object()) {
                    for _ in arr.len()..count {
                        arr.push(self.value(items));
                    }
                }
            }
            return Value::Array(arr);
        }

        let items_schema = obj
            .get("items")
            .cloned()
            .unwrap_or(json!({"type": "string"}));

        // `contains` (+ `minContains` default 1 / `maxContains`): place that
        // many matching items; other items must not match a `const`.
        let contains = obj.get("contains");
        let contains_const = contains.and_then(|c| c.get("const"));
        let need = contains.map(|_| {
            let lo = n("minContains").unwrap_or(1);
            let hi = n("maxContains").unwrap_or(lo.max(1) + 1).max(lo);
            let hi = hi.min(max_items.max(lo));
            self.rng.gen_range(lo..=hi.max(lo))
        });
        if let Some(need) = need {
            count = count.max(need).min(max_items.max(need));
        }

        // Distinct items: `uniqueItems`, sets, and map entries (`[key, value]`
        // tuples, compared by key).
        let unique = obj.get("uniqueItems").and_then(Value::as_bool) == Some(true)
            || umin.is_some()
            || umax.is_some();
        let pair_keys = items_schema
            .get("prefixItems")
            .and_then(Value::as_array)
            .is_some_and(|p| p.len() == 2);
        let key_of = |v: &Value| -> Value {
            if pair_keys {
                v.get(0).cloned().unwrap_or(Value::Null)
            } else {
                v.clone()
            }
        };
        let distinct = unique || pair_keys;

        let mut arr: Vec<Value> = Vec::new();
        if let (Some(need), Some(c)) = (need, contains) {
            for _ in 0..need {
                let v = match contains_const {
                    Some(k) => k.clone(),
                    None => self.value(c),
                };
                arr.push(v);
            }
            if distinct && arr.len() > 1 {
                // A repeated const cannot be unique: keep one.
                arr.truncate(1);
            }
        }
        let mut attempts = 0;
        while arr.len() < count && attempts < count * 20 + 20 {
            attempts += 1;
            let v = self.value(&items_schema);
            if contains_const.is_some_and(|k| constraints::json_eq(k, &v)) {
                continue;
            }
            if distinct
                && arr
                    .iter()
                    .any(|a| constraints::json_eq(&key_of(a), &key_of(&v)))
            {
                continue;
            }
            arr.push(v);
        }
        // Spread the `contains` items.
        if need.is_some() {
            for i in (1..arr.len()).rev() {
                let j = self.rng.gen_range(0..=i);
                arr.swap(i, j);
            }
        }
        Value::Array(arr)
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  OBJECT
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_object(&mut self, obj: &Map<String, Value>, hint: Option<&str>) -> Value {
        let has_properties = obj
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|p| !p.is_empty());
        let has_required = obj
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|r| !r.is_empty());

        // If the object schema has no properties/required, try to generate a
        // template based on the field-name hint.
        if !has_properties && !has_required {
            if let Some(h) = hint {
                if let Some(tmpl) = self.gen_object_template(h) {
                    return tmpl;
                }
            }
        }

        let mut result = Map::new();

        let required: Vec<String> = obj
            .get("required")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        if let Some(props) = obj.get("properties").and_then(Value::as_object) {
            for (key, prop_schema) in props {
                if required.contains(key) || self.rng.gen_bool(0.7) {
                    result.insert(key.clone(), self.value_with_hint(prop_schema, Some(key)));
                }
            }
        }

        // additionalProperties with a schema
        // within minProperties / maxProperties, keys from `propertyNames`.
        if let Some(additional) = obj.get("additionalProperties") {
            if additional.is_object() {
                let count = |k: &str| obj.get(k).and_then(Value::as_u64).map(|v| v as usize);
                let lo = count("minProperties")
                    .unwrap_or(0)
                    .saturating_sub(result.len());
                let hi = count("maxProperties")
                    .map_or(lo.max(2), |m| m.saturating_sub(result.len()))
                    .max(lo);
                let extra_count = self.rng.gen_range(lo..=hi);
                let names = obj.get("propertyNames");
                let mut attempts = 0;
                let target = result.len() + extra_count;
                while result.len() < target && attempts < extra_count * 20 + 20 {
                    attempts += 1;
                    let key = match names.map(|n| self.value(n)) {
                        Some(Value::String(k)) => k,
                        _ => format!("{}_{}", self.pick(WORDS), self.next_id()),
                    };
                    if !result.contains_key(&key) {
                        let v = self.value(additional);
                        result.insert(key, v);
                    }
                }
            }
        }

        Value::Object(result)
    }

    /// Generate a pre-built object based on a field-name hint when the schema
    /// declares `{"type":"object"}` without any `properties`.
    fn gen_object_template(&mut self, hint: &str) -> Option<Value> {
        let h = hint.to_ascii_lowercase();
        match h.as_str() {
            // ── address ───────────────────────────────────────────────
            "address" | "billing_address" | "billingaddress" | "shipping_address"
            | "shippingaddress" | "home_address" | "work_address" | "mailing_address" => {
                Some(self.template_address())
            }

            // ── location / geo ────────────────────────────────────────
            "location" | "geo" | "geolocation" | "coordinates" | "coords" | "position" | "gps" => {
                Some(self.template_geo())
            }

            "place" | "venue" | "point_of_interest" | "poi" => Some(self.template_place()),

            // ── person ────────────────────────────────────────────────
            "person" | "user" | "author" | "owner" | "creator" | "contact" | "sender"
            | "recipient" | "assignee" | "reviewer" | "member" | "employee" | "customer"
            | "client" | "patient" => Some(self.template_person()),
            "profile" | "user_profile" | "account" => Some(self.template_profile()),

            // ── company / org ─────────────────────────────────────────
            "company" | "organization" | "organisation" | "employer" | "business" => {
                Some(self.template_company())
            }

            // ── product ───────────────────────────────────────────────
            "product" | "item" | "goods" | "merchandise" => Some(self.template_product()),

            // ── money / payment ───────────────────────────────────────
            "price" | "money" | "amount" | "cost" | "payment" | "transaction" => {
                Some(self.template_money())
            }

            // ── date range / period ───────────────────────────────────
            "period" | "date_range" | "daterange" | "time_range" | "timerange" | "duration"
            | "schedule" | "availability" => Some(self.template_date_range()),

            // ── config / settings / metadata ──────────────────────────
            "config" | "configuration" | "settings" | "options" | "preferences" => {
                Some(self.template_config())
            }
            "metadata" | "meta" | "extra" | "attributes" | "props" | "info" => {
                Some(self.template_metadata())
            }

            // ── image / media ─────────────────────────────────────────
            "image" | "photo" | "picture" | "avatar" | "thumbnail" | "media" => {
                Some(self.template_image())
            }

            // ── dimensions / size ─────────────────────────────────────
            "dimensions" | "size" | "resolution" => Some(self.template_dimensions()),

            _ => None,
        }
    }

    // ── template builders ─────────────────────────────────────────────────

    fn template_address(&mut self) -> Value {
        let num: u16 = self.rng.gen_range(1..9999);
        let street = self.pick(STREET_NAMES);
        let suffix = self.pick(STREET_SUFFIXES);
        let city = self.pick(CITIES);
        let state = self.pick(STATES);
        let country = self.pick(COUNTRIES);
        let zip: u32 = self.rng.gen_range(10000..99999);
        let lat: f64 = self.rng.gen_range(-90.0..=90.0);
        let lng: f64 = self.rng.gen_range(-180.0..=180.0);

        json!({
            "street": format!("{num} {street} {suffix}"),
            "city": city,
            "state": state,
            "country": country,
            "zip": format!("{zip}"),
            "latitude": (lat * 1_000_000.0).round() / 1_000_000.0,
            "longitude": (lng * 1_000_000.0).round() / 1_000_000.0,
        })
    }

    fn template_geo(&mut self) -> Value {
        let lat: f64 = self.rng.gen_range(-90.0..=90.0);
        let lng: f64 = self.rng.gen_range(-180.0..=180.0);
        let alt: f64 = self.rng.gen_range(0.0..=8848.0);
        let accuracy: f64 = self.rng.gen_range(1.0..=100.0);

        json!({
            "latitude": (lat * 1_000_000.0).round() / 1_000_000.0,
            "longitude": (lng * 1_000_000.0).round() / 1_000_000.0,
            "altitude": (alt * 100.0).round() / 100.0,
            "accuracy": (accuracy * 100.0).round() / 100.0,
        })
    }

    fn template_place(&mut self) -> Value {
        let city = self.pick(CITIES);
        let country = self.pick(COUNTRIES);
        let lat: f64 = self.rng.gen_range(-90.0..=90.0);
        let lng: f64 = self.rng.gen_range(-180.0..=180.0);
        let category = self.pick(PLACE_CATEGORIES);
        let adj = self.pick(ADJECTIVES);
        let noun = self.pick(PLACE_NOUNS);

        json!({
            "name": format!("{adj} {noun}"),
            "category": category,
            "city": city,
            "country": country,
            "latitude": (lat * 1_000_000.0).round() / 1_000_000.0,
            "longitude": (lng * 1_000_000.0).round() / 1_000_000.0,
        })
    }

    fn template_person(&mut self) -> Value {
        let first = self.pick(FIRST_NAMES);
        let last = self.pick(LAST_NAMES);
        let email = self.gen_email();
        let phone = self.gen_phone();

        json!({
            "first_name": first,
            "last_name": last,
            "email": email,
            "phone": phone,
        })
    }

    fn template_profile(&mut self) -> Value {
        let first = self.pick(FIRST_NAMES);
        let last = self.pick(LAST_NAMES);
        let email = self.gen_email();
        let phone = self.gen_phone();
        let username = format!("{}{}", first.to_lowercase(), self.rng.gen_range(1u16..999));
        let job = self.pick(JOB_TITLES);
        let company = self.pick(COMPANIES);
        let city = self.pick(CITIES);
        let bio = self.gen_sentence_range(8, 16);
        let avatar_id: u32 = self.rng.gen_range(1..999);

        json!({
            "username": username,
            "first_name": first,
            "last_name": last,
            "email": email,
            "phone": phone,
            "job_title": job,
            "company": company,
            "city": city,
            "bio": bio,
            "avatar_url": format!("https://i.pravatar.cc/300?u={avatar_id}"),
        })
    }

    fn template_company(&mut self) -> Value {
        let name = self.pick(COMPANIES);
        let department = self.pick(DEPARTMENTS);
        let industry = self.pick(INDUSTRIES);
        let employees: u32 = self.rng.gen_range(10..50000);
        let founded: u16 = self.rng.gen_range(1950..2024);
        let city = self.pick(CITIES);
        let country = self.pick(COUNTRIES);
        let website = self.gen_url();

        json!({
            "name": name,
            "industry": industry,
            "department": department,
            "employees": employees,
            "founded": founded,
            "city": city,
            "country": country,
            "website": website,
        })
    }

    fn template_product(&mut self) -> Value {
        let adj = self.pick(ADJECTIVES);
        let noun = self.pick(PRODUCT_NOUNS);
        let price: f64 = self.rng.gen_range(0.99..9999.99);
        let category = self.pick(CATEGORIES);
        let sku_prefix: String = (0..3)
            .map(|_| self.rng.gen_range(b'A'..=b'Z') as char)
            .collect();
        let sku_num: u32 = self.rng.gen_range(10000..99999);
        let rating: f64 = self.rng.gen_range(1.0..5.0);

        json!({
            "name": format!("{adj} {noun}"),
            "sku": format!("{sku_prefix}-{sku_num}"),
            "price": (price * 100.0).round() / 100.0,
            "currency": self.pick(CURRENCIES),
            "category": category,
            "rating": (rating * 10.0).round() / 10.0,
            "in_stock": self.rng.gen_bool(0.8),
        })
    }

    fn template_money(&mut self) -> Value {
        let amount: f64 = self.rng.gen_range(0.01..99999.99);
        let currency = self.pick(CURRENCIES);

        json!({
            "amount": (amount * 100.0).round() / 100.0,
            "currency": currency,
        })
    }

    fn template_date_range(&mut self) -> Value {
        let start = self.gen_datetime();
        let end = self.gen_datetime();

        json!({
            "start": start,
            "end": end,
        })
    }

    fn template_config(&mut self) -> Value {
        let env = self.pick(&["development", "staging", "production", "test"]);
        let port: u16 = self.rng.gen_range(3000..9999);
        let host = self.gen_hostname();

        json!({
            "environment": env,
            "host": host,
            "port": port,
            "debug": self.rng.gen_bool(0.3),
            "log_level": self.pick(&["debug", "info", "warn", "error"]),
        })
    }

    fn template_metadata(&mut self) -> Value {
        let created = self.gen_datetime();
        let updated = self.gen_datetime();
        let version = format!(
            "{}.{}.{}",
            self.rng.gen_range(0u8..10),
            self.rng.gen_range(0u8..30),
            self.rng.gen_range(0u16..100)
        );

        json!({
            "created_at": created,
            "updated_at": updated,
            "version": version,
            "source": self.pick(&["api", "web", "mobile", "import", "sync"]),
        })
    }

    fn template_image(&mut self) -> Value {
        let w: u16 = self.rng.gen_range(100..4000);
        let h: u16 = self.rng.gen_range(100..4000);
        let ext = self.pick(&["jpg", "png", "webp", "gif"]);
        let id: u32 = self.rng.gen_range(1..99999);

        json!({
            "url": format!("https://picsum.photos/{w}/{h}?random={id}"),
            "width": w,
            "height": h,
            "format": ext,
            "size_bytes": self.rng.gen_range(10_000u32..10_000_000),
        })
    }

    fn template_dimensions(&mut self) -> Value {
        let w: f64 = self.rng.gen_range(1.0..1000.0);
        let h: f64 = self.rng.gen_range(1.0..1000.0);
        let d: f64 = self.rng.gen_range(1.0..500.0);
        let unit = self.pick(&["mm", "cm", "in", "px", "m"]);

        json!({
            "width": (w * 100.0).round() / 100.0,
            "height": (h * 100.0).round() / 100.0,
            "depth": (d * 100.0).round() / 100.0,
            "unit": unit,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn days_in_month(m: u32) -> u32 {
    match m {
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn luhn_check_digit(digits: &[u8]) -> u8 {
    let mut sum: u16 = 0;
    for (i, &d) in digits.iter().rev().enumerate() {
        let mut v = d as u16;
        if i % 2 == 0 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    ((10 - (sum % 10)) % 10) as u8
}

/// Realistic integer range for a field name (applied within the schema range).
fn int_hint_range(hint: &str) -> Option<(i64, i64)> {
    Some(match hint.to_ascii_lowercase().as_str() {
        "age" => (0, 99),
        "port" => (1024, 65535),
        "year" => (2000, 2030),
        "quantity" | "qty" | "count" => (0, 100),
        "rating" | "score" => (0, 10),
        _ => return None,
    })
}

/// `(minLength, maxLength)` of a string schema.
/// JSON number for an integer (i64 / u64 exactly, beyond that as float).
fn int_json(v: i128) -> Value {
    i64::try_from(v)
        .map(Value::from)
        .or_else(|_| u64::try_from(v).map(Value::from))
        .unwrap_or_else(|_| json!(v as f64))
}

/// An `allOf` member that only adds a `pattern` / `format` (what vld emits
/// next to `type` for a second regex or format).
fn is_fragment(schema: &Value) -> bool {
    schema
        .as_object()
        .is_some_and(|o| o.keys().all(|k| k == "pattern" || k == "format"))
}

/// `{"type":"string"}` without any constraint that shapes the value.
fn is_bare_string(schema: &Value) -> bool {
    let Some(o) = schema.as_object() else {
        return false;
    };
    o.get("type").and_then(Value::as_str) == Some("string")
        && [
            "format",
            "pattern",
            "enum",
            "const",
            "minLength",
            "maxLength",
            "allOf",
        ]
        .iter()
        .all(|k| !o.contains_key(*k))
}

fn len_bounds(obj: &Map<String, Value>) -> (Option<usize>, Option<usize>) {
    let get = |k| obj.get(k).and_then(Value::as_u64).map(|v| v as usize);
    (get("minLength"), get("maxLength"))
}

/// Whether `s` (counted in chars, like vld) satisfies the length bounds.
fn len_ok(s: &str, min: Option<usize>, max: Option<usize>) -> bool {
    let n = s.chars().count();
    min.is_none_or(|m| n >= m) && max.is_none_or(|m| n <= m)
}

/// Pad with `pad` / truncate `s` so its char count lies in `[min, max]`.
fn fit_len(mut s: String, min: usize, max: Option<usize>, pad: char) -> String {
    let n = s.chars().count();
    if n < min {
        s.extend(std::iter::repeat_n(pad, min - n));
    }
    if let Some(mx) = max {
        if let Some((idx, _)) = s.char_indices().nth(mx) {
            s.truncate(idx);
        }
    }
    s
}

fn base64url(data: &[u8]) -> String {
    base64_encode(data)
        .trim_end_matches('=')
        .replace('+', "-")
        .replace('/', "_")
}

const LOWER_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER_ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const B64URL_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
const LOWER_DIGIT: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const HEX: &[u8] = b"0123456789abcdef";
const HEX_89AB: &[u8] = b"89ab";
const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const NANOID_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
const TOKEN_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const PASSWORD_ALPHA: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%&*";

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════════
//  FakeData trait — typed API
// ═══════════════════════════════════════════════════════════════════════════

/// Trait for types that can generate fake instances of themselves.
///
/// Implement it with the [`impl_fake!`] macro on any `vld::schema!` struct:
///
/// ```rust
/// # vld::schema! {
/// #     #[derive(Debug)]
/// #     pub struct User { pub name: String => vld::string().min(2) }
/// # }
/// use vld_fake::FakeData;
///
/// vld_fake::impl_fake!(User);
///
/// let user = User::fake();
/// println!("{}", user.name);
/// ```
pub trait FakeData: Sized {
    /// Generate one random, fully validated instance.
    fn fake() -> Self;

    /// Generate `count` random instances.
    fn fake_many(count: usize) -> Vec<Self>;

    /// Generate a reproducible instance from a seed.
    fn fake_seeded(seed: u64) -> Self;

    /// Try to generate — returns `Err` if the generated value somehow fails
    /// validation (should not happen for well-defined schemas).
    fn try_fake() -> Result<Self, vld::error::VldError>;
}

/// Implement [`FakeData`] for a `vld::schema!` struct.
///
/// The struct must have `json_schema()` (requires `openapi` feature on `vld`)
/// and implement `VldParse` (which `schema!` provides automatically).
///
/// # Example
///
/// ```rust
/// use vld_fake::FakeData;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct User {
///         pub name:  String => vld::string().min(2).max(50),
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// vld_fake::impl_fake!(User);
///
/// let user = User::fake();
/// println!("{}", user.name);   // typed access!
///
/// let users = User::fake_many(10);
/// let same  = User::fake_seeded(42);
/// ```
#[macro_export]
macro_rules! impl_fake {
    ($ty:ty) => {
        impl $crate::FakeData for $ty {
            fn fake() -> Self {
                $crate::fake_parsed::<$ty>(&$crate::__fake_schema!($ty))
            }

            fn fake_many(count: usize) -> Vec<Self> {
                let schema = $crate::__fake_schema!($ty);
                (0..count)
                    .map(|_| $crate::fake_parsed::<$ty>(&schema))
                    .collect()
            }

            fn fake_seeded(seed: u64) -> Self {
                let schema = $crate::__fake_schema!($ty);
                $crate::parse_or_panic(&$crate::fake_value_seeded(&schema, seed))
            }

            fn try_fake() -> Result<Self, $crate::__vld::error::VldError> {
                $crate::try_fake_parsed::<$ty>(&$crate::__fake_schema!($ty))
            }
        }
    };
}

/// `json_schema()` of `$ty` with its nested schemas embedded under `$defs`,
/// so the `$ref`s emitted by `vld::nested!` resolve.
#[doc(hidden)]
#[macro_export]
macro_rules! __fake_schema {
    ($ty:ty) => {{
        #[allow(unused_imports)]
        use $crate::__VldNestedSchemasFallback as _;
        $crate::with_nested_defs(<$ty>::json_schema(), <$ty>::__vld_nested_schemas())
    }};
}

/// Embed `nested` schemas into `schema` under `$defs`.
#[doc(hidden)]
pub fn with_nested_defs(mut schema: Value, nested: Vec<vld::openapi::NestedSchemaEntry>) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        if !nested.is_empty() {
            let defs = nested
                .into_iter()
                .map(|(name, f)| (name.to_string(), f()))
                .collect();
            obj.insert("$defs".into(), Value::Object(defs));
        }
    }
    schema
}

/// Fallback so `impl_fake!` compiles for types without `__vld_nested_schemas`.
/// An inherent `__vld_nested_schemas` (from `vld::schema!`) always takes
/// precedence over this trait method.
#[doc(hidden)]
pub trait __VldNestedSchemasFallback {
    fn __vld_nested_schemas() -> Vec<vld::openapi::NestedSchemaEntry> {
        Vec::new()
    }
}

impl<T: ?Sized> __VldNestedSchemasFallback for T {}

// ──────────────────── prelude ────────────────────────────────────────────

/// Common imports: the fake-data traits, functions and `impl_fake!`.
pub mod prelude {
    pub use crate::{
        FakeData, FakeGen, fake_json, fake_many, fake_parsed, fake_value, fake_value_seeded,
        impl_fake, try_fake_parsed,
    };
}

#[cfg(test)]
mod offset_free_formats {
    use vld::schema::VldSchema;

    /// `vld::*::time()` / naive `datetime()` emit `partial-time` /
    /// `naive-date-time` next to their patterns; values must be real times.
    #[test]
    fn partial_time_and_naive_datetime_are_valid() {
        let time = serde_json::json!({
            "type": "string", "format": "partial-time",
            "pattern": r"^([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$"
        });
        let naive = serde_json::json!({
            "type": "string",
            "anyOf": [
                {"format": "date-time"},
                {"format": "naive-date-time",
                 "pattern": r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])[Tt]([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$"}
            ]
        });
        let t = vld::string().iso_time();
        let dt = vld::string().iso_datetime();
        for seed in 0..500 {
            let v = super::fake_value_seeded(&time, seed);
            assert!(t.parse_value(&v).is_ok(), "time {v}");
            let v = super::fake_value_seeded(&naive, seed);
            assert!(dt.parse_value(&v).is_ok(), "datetime {v}");
        }
    }
}
