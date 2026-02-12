//! # vld-fake
//!
//! Generate fake / test data that satisfies a [`vld`](https://docs.rs/vld) JSON Schema.
//!
//! ## Quick start — typed API
//!
//! ```rust
//! use vld::prelude::*;
//! use vld_fake::FakeData;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name:  String      => vld::string().min(2).max(50),
//!         pub email: String      => vld::string().email(),
//!         pub age:   i64         => vld::number().int().min(18).max(99),
//!     }
//! }
//!
//! vld_fake::impl_fake!(User);
//!
//! let user = User::fake();
//! assert!(user.name.len() >= 2);
//! assert!(user.email.contains('@'));
//!
//! // Multiple
//! let users = User::fake_many(5);
//!
//! // Reproducible
//! let u1 = User::fake_seeded(42);
//! let u2 = User::fake_seeded(42);
//! assert_eq!(u1.name, u2.name);
//! ```
//!
//! ## Low-level (untyped) API
//!
//! ```rust
//! use vld::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name:  String      => vld::string().min(2).max(50),
//!         pub email: String      => vld::string().email(),
//!         pub age:   i64         => vld::number().int().min(18).max(99),
//!     }
//! }
//!
//! let schema = User::json_schema();
//! let value  = vld_fake::fake_value(&schema);
//! // value is a random serde_json::Value
//! ```

mod dict;

use dict::*;
use rand::Rng;
use serde_json::{json, Map, Value};
use vld::prelude::VldParse;

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
    let mut gen = FakeGen::new();
    (0..count).map(|_| gen.value(schema)).collect()
}

/// Generate a random value **and** parse it through `T::vld_parse_value`,
/// returning a fully validated typed instance.
///
/// Panics if the generated value does not pass validation (should not happen
/// unless the schema is ambiguous).
pub fn fake_parsed<T: VldParse>(schema: &Value) -> T {
    let val = fake_value(schema);
    T::vld_parse_value(&val).unwrap_or_else(|e| {
        panic!(
            "vld_fake::fake_parsed: generated value failed validation.\nValue: {}\nError: {:?}",
            serde_json::to_string_pretty(&val).unwrap_or_default(),
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
}

const MAX_DEPTH: usize = 12;

impl FakeGen<rand::rngs::ThreadRng> {
    /// Create a generator using [`rand::thread_rng()`].
    pub fn new() -> Self {
        Self {
            rng: rand::thread_rng(),
            depth: 0,
            counter: 0,
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
        self.depth += 1;
        let result = self.dispatch(schema, hint);
        self.depth -= 1;
        result
    }

    fn dispatch(&mut self, schema: &Value, hint: Option<&str>) -> Value {
        let obj = match schema.as_object() {
            Some(o) => o,
            None => return Value::Null,
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
            let concrete: Vec<&Value> = variants
                .iter()
                .filter(|v| v.get("type").and_then(Value::as_str) != Some("null"))
                .collect();
            if !concrete.is_empty() {
                let idx = self.rng.gen_range(0..concrete.len());
                return self.value_with_hint(concrete[idx], hint);
            }
            if !variants.is_empty() {
                let idx = self.rng.gen_range(0..variants.len());
                return self.value_with_hint(&variants[idx], hint);
            }
            return Value::Null;
        }

        // ---- allOf ----
        if let Some(all) = obj.get("allOf").and_then(Value::as_array) {
            return self.merge_all_of(all);
        }

        // ---- type-based dispatch ----
        let ty = obj.get("type").and_then(Value::as_str).unwrap_or("string");

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
        let format = obj.get("format").and_then(Value::as_str);

        // 1) Explicit JSON Schema format
        if let Some(fmt) = format {
            return Value::String(self.gen_string_format(fmt, obj));
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

        // Check length constraints
        if let Some(mut s) = result {
            let min = obj.get("minLength").and_then(Value::as_u64).unwrap_or(0) as usize;
            let max = obj
                .get("maxLength")
                .and_then(Value::as_u64)
                .map(|v| v as usize);

            // Pad if too short
            while s.len() < min {
                s.push('x');
            }
            // Truncate if too long
            if let Some(mx) = max {
                if s.len() > mx {
                    s.truncate(mx);
                }
            }
            Some(Value::String(s))
        } else {
            None
        }
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

        // Pad if too short
        while s.len() < min_len {
            s.push('a');
        }

        // Truncate if too long
        if s.len() > max_len {
            s.truncate(max_len);
        }

        Value::String(s)
    }

    // ── format-specific generators ──────────────────────────────────────

    fn gen_string_format(&mut self, fmt: &str, obj: &Map<String, Value>) -> String {
        match fmt {
            "email" => self.gen_email(),
            "uuid" => self.gen_uuid(),
            "uri" | "url" => self.gen_url(),
            "ipv4" => self.gen_ipv4(),
            "ipv6" => self.gen_ipv6(),
            "hostname" => self.gen_hostname(),
            "date" | "iso-date" => self.gen_date(),
            "time" | "iso-time" => self.gen_time(),
            "date-time" | "iso-datetime" => self.gen_datetime(),
            "base64" => self.gen_base64(),
            "cuid2" => self.gen_cuid2(),
            "ulid" => self.gen_ulid(),
            "nanoid" => self.gen_nanoid(),
            "emoji" => self.gen_emoji(),
            "phone" => self.gen_phone(),
            "credit-card" => self.gen_credit_card(),
            "mac-address" | "mac" => self.gen_mac_address(),
            "color" | "hex-color" => self.gen_hex_color(),
            "semver" => {
                let major = self.rng.gen_range(0u8..10);
                let minor = self.rng.gen_range(0u8..30);
                let patch = self.rng.gen_range(0u16..100);
                format!("{major}.{minor}.{patch}")
            }
            "slug" => self.gen_slug(),
            _ => {
                // Unknown — readable string
                let min = obj.get("minLength").and_then(Value::as_u64).unwrap_or(1) as usize;
                let max = obj
                    .get("maxLength")
                    .and_then(Value::as_u64)
                    .unwrap_or(min as u64 + 20) as usize;
                let target = self.rng.gen_range(min..=max.max(min));
                let mut s = String::new();
                while s.len() < target {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    let idx = self.rng.gen_range(0..WORDS.len());
                    s.push_str(WORDS[idx]);
                }
                s.truncate(max.max(min));
                while s.len() < min {
                    s.push('a');
                }
                s
            }
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
        let hex = |n: usize, rng: &mut R| -> String {
            (0..n)
                .map(|_| HEX[rng.gen_range(0..HEX.len())] as char)
                .collect()
        };
        format!(
            "{}-{}-4{}-{}{}-{}",
            hex(8, &mut self.rng),
            hex(4, &mut self.rng),
            hex(3, &mut self.rng),
            HEX_89AB[self.rng.gen_range(0..4)] as char,
            hex(3, &mut self.rng),
            hex(12, &mut self.rng),
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
        let bytes: Vec<u8> = (0..byte_len).map(|_| self.rng.gen()).collect();
        base64_encode(&bytes)
    }

    fn gen_cuid2(&mut self) -> String {
        let first = LOWER_ALPHA[self.rng.gen_range(0..LOWER_ALPHA.len())] as char;
        let rest: String = (0..23)
            .map(|_| LOWER_DIGIT[self.rng.gen_range(0..LOWER_DIGIT.len())] as char)
            .collect();
        format!("{first}{rest}")
    }

    fn gen_ulid(&mut self) -> String {
        (0..26)
            .map(|_| CROCKFORD[self.rng.gen_range(0..CROCKFORD.len())] as char)
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
        let mut s: String = (0..len)
            .map(|_| PASSWORD_ALPHA[self.rng.gen_range(0..PASSWORD_ALPHA.len())] as char)
            .collect();
        // Guarantee at least one of each class
        let positions: Vec<usize> = (0..s.len()).collect();
        if s.len() >= 4 {
            let bytes = unsafe { s.as_bytes_mut() };
            bytes[positions[0]] = b'A' + self.rng.gen_range(0..26);
            bytes[positions[1]] = b'a' + self.rng.gen_range(0..26);
            bytes[positions[2]] = b'0' + self.rng.gen_range(0..10);
            let specials = b"!@#$%&*";
            bytes[positions[3]] = specials[self.rng.gen_range(0..specials.len())];
        }
        s
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
        let min = self.num_as_i64(obj, "minimum", "exclusiveMinimum", 1);
        let max = self.num_as_i64_max(obj, "maximum", "exclusiveMaximum", 1);

        let (min, max) = if min > max { (max, min) } else { (min, max) };

        // Heuristic defaults based on field name
        let (min, max) = if let Some(h) = hint {
            self.adjust_int_range_by_hint(h, min, max, obj)
        } else {
            (min, max)
        };

        let mut val = self.rng.gen_range(min..=max);

        // multipleOf
        if let Some(m) = obj
            .get("multipleOf")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        {
            if m > 0 {
                val = (val / m) * m;
                if val < min {
                    val += m;
                }
            }
        }

        json!(val)
    }

    fn adjust_int_range_by_hint(
        &self,
        hint: &str,
        schema_min: i64,
        schema_max: i64,
        obj: &Map<String, Value>,
    ) -> (i64, i64) {
        let h = hint.to_ascii_lowercase();
        let has_min = obj.contains_key("minimum") || obj.contains_key("exclusiveMinimum");
        let has_max = obj.contains_key("maximum") || obj.contains_key("exclusiveMaximum");

        if has_min && has_max {
            return (schema_min, schema_max);
        }

        match h.as_str() {
            "age" if !has_max => (schema_min, schema_max.min(99)),
            "port" if !has_max => (schema_min.max(1024), schema_max.min(65535)),
            "year" if !has_min => (2000.max(schema_min), schema_max.min(2030)),
            "quantity" | "qty" | "count" if !has_max => (schema_min, schema_max.min(100)),
            "rating" | "score" if !has_max => (schema_min, schema_max.min(10)),
            _ => (schema_min, schema_max),
        }
    }

    fn num_as_i64(&self, obj: &Map<String, Value>, key: &str, ex_key: &str, offset: i64) -> i64 {
        if let Some(v) = obj
            .get(key)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        {
            return v;
        }
        if let Some(v) = obj
            .get(ex_key)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        {
            return v + offset;
        }
        0
    }

    fn num_as_i64_max(
        &self,
        obj: &Map<String, Value>,
        key: &str,
        ex_key: &str,
        offset: i64,
    ) -> i64 {
        if let Some(v) = obj
            .get(key)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        {
            return v;
        }
        if let Some(v) = obj
            .get(ex_key)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        {
            return v - offset;
        }
        1000
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
            if !has_min && !has_max {
                if let Some(val) = self.gen_number_by_hint(h) {
                    return json!(val);
                }
            }
        }

        let min = obj
            .get("minimum")
            .and_then(Value::as_f64)
            .or_else(|| {
                obj.get("exclusiveMinimum")
                    .and_then(Value::as_f64)
                    .map(|v| v + 0.01)
            })
            .unwrap_or(0.0);
        let max = obj
            .get("maximum")
            .and_then(Value::as_f64)
            .or_else(|| {
                obj.get("exclusiveMaximum")
                    .and_then(Value::as_f64)
                    .map(|v| v - 0.01)
            })
            .unwrap_or(min + 1000.0);

        let max = if max < min { min + 1.0 } else { max };

        let val = min + self.rng.gen::<f64>() * (max - min);

        // Round to 2 decimal places for nicer output
        let val = (val * 100.0).round() / 100.0;

        json!(val)
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
        Value::Bool(self.rng.gen())
    }

    // ═══════════════════════════════════════════════════════════════════════
    //  ARRAY
    // ═══════════════════════════════════════════════════════════════════════

    fn gen_array(&mut self, obj: &Map<String, Value>, _hint: Option<&str>) -> Value {
        let min_items = obj.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
        let max_items = obj
            .get("maxItems")
            .and_then(Value::as_u64)
            .unwrap_or(min_items.max(1) as u64 + 3) as usize;
        let max_items = max_items.max(min_items);

        let count = if min_items == max_items {
            min_items
        } else {
            self.rng.gen_range(min_items..=max_items)
        };

        // Tuple-style (prefixItems)
        if let Some(prefix) = obj.get("prefixItems").and_then(Value::as_array) {
            let mut arr: Vec<Value> = prefix.iter().map(|s| self.value(s)).collect();
            if count > arr.len() {
                if let Some(items) = obj.get("items") {
                    for _ in arr.len()..count {
                        arr.push(self.value(items));
                    }
                }
            }
            return Value::Array(arr);
        }

        // uniqueItems
        let unique = obj
            .get("uniqueItems")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let items_schema = obj
            .get("items")
            .cloned()
            .unwrap_or(json!({"type": "string"}));

        if unique {
            let mut arr = Vec::new();
            let mut attempts = 0;
            while arr.len() < count && attempts < count * 10 {
                let v = self.value(&items_schema);
                if !arr.contains(&v) {
                    arr.push(v);
                }
                attempts += 1;
            }
            Value::Array(arr)
        } else {
            let arr: Vec<Value> = (0..count).map(|_| self.value(&items_schema)).collect();
            Value::Array(arr)
        }
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
        if let Some(additional) = obj.get("additionalProperties") {
            if additional.is_object() {
                let extra_count = self.rng.gen_range(0..3usize);
                for _ in 0..extra_count {
                    let key = format!("{}_{}", self.pick(WORDS), self.next_id());
                    result.insert(key, self.value(additional));
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

    // ═══════════════════════════════════════════════════════════════════════
    //  allOf merge
    // ═══════════════════════════════════════════════════════════════════════

    fn merge_all_of(&mut self, schemas: &[Value]) -> Value {
        let mut merged = Map::new();
        let mut required: Vec<String> = Vec::new();

        for s in schemas {
            if let Some(obj) = s.as_object() {
                if let Some(props) = obj.get("properties").and_then(Value::as_object) {
                    for (k, v) in props {
                        merged
                            .entry("properties")
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .unwrap()
                            .insert(k.clone(), v.clone());
                    }
                }
                if let Some(req) = obj.get("required").and_then(Value::as_array) {
                    for r in req {
                        if let Some(s) = r.as_str() {
                            if !required.contains(&s.to_string()) {
                                required.push(s.to_string());
                            }
                        }
                    }
                }
                if let Some(ty) = obj.get("type") {
                    merged.insert("type".into(), ty.clone());
                }
            }
        }

        if !required.is_empty() {
            merged.insert(
                "required".into(),
                Value::Array(required.into_iter().map(Value::String).collect()),
            );
        }

        self.value(&Value::Object(merged))
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

const LOWER_ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
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
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
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
/// ```rust,ignore
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
/// ```rust,ignore
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
                let schema = <$ty>::json_schema();
                $crate::fake_parsed::<$ty>(&schema)
            }

            fn fake_many(count: usize) -> Vec<Self> {
                let schema = <$ty>::json_schema();
                let mut gen = $crate::FakeGen::new();
                (0..count)
                    .map(|_| {
                        let val = gen.value(&schema);
                        <$ty as ::vld::prelude::VldParse>::vld_parse_value(&val)
                            .expect("vld_fake: generated value failed validation")
                    })
                    .collect()
            }

            fn fake_seeded(seed: u64) -> Self {
                let schema = <$ty>::json_schema();
                let val = $crate::fake_value_seeded(&schema, seed);
                <$ty as ::vld::prelude::VldParse>::vld_parse_value(&val)
                    .expect("vld_fake: generated value failed validation")
            }

            fn try_fake() -> Result<Self, ::vld::error::VldError> {
                let schema = <$ty>::json_schema();
                $crate::try_fake_parsed::<$ty>(&schema)
            }
        }
    };
}

// ──────────────────── prelude ────────────────────────────────────────────

pub mod prelude {
    pub use crate::{
        fake_json, fake_many, fake_parsed, fake_value, fake_value_seeded, impl_fake,
        try_fake_parsed, FakeData, FakeGen,
    };
}
