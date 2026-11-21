//! Geographic schemas (feature `geo`): [`ZGeoPoint`] and [`ZGeoJson`].

use serde_json::{Map, Value};

use crate::error::{IssueCode, PathSegment, SizeOrigin, VldError};
use crate::primitives::ZNumber;
use crate::schema::VldSchema;

/// A WGS 84 point: the output of [`vld::geo_point()`](crate::geo_point).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GeoPoint {
    /// Latitude in degrees, `-90..=90`.
    pub lat: f64,
    /// Longitude in degrees, `-180..=180`.
    pub lng: f64,
}

fn field(key: &str) -> PathSegment {
    PathSegment::Field(key.to_string())
}

fn issue(code: IssueCode, value: &Value) -> VldError {
    VldError::from_code_with_value(code, value)
}

fn missing(key: &str) -> VldError {
    VldError::from_code(IssueCode::MissingField).with_prefix(field(key))
}

fn wrong_type(expected: &str, value: &Value) -> VldError {
    crate::primitives::type_error(&None, expected, value)
}

/// Schema for a point given as `{"lat": .., "lng": ..}` (`"lon"` is accepted
/// for `"lng"`) or as a GeoJSON-order pair `[lng, lat]` — longitude
/// **first**, as in GeoJSON positions. Created via
/// [`vld::geo_point()`](crate::geo_point).
///
/// Latitude must be within `-90..=90` and longitude within `-180..=180`
/// (`too_small` / `too_big`, origin `number`, at `.lat` / `.lng` or `[1]` /
/// `[0]`). A pair must have exactly 2 items (`invalid_length`, origin
/// `tuple`; no altitude); other values are `invalid_type` (`geo_point`).
///
/// ```
/// use vld::prelude::*;
///
/// let p = vld::geo_point().parse(r#"{"lat": 52.52, "lng": 13.405}"#).unwrap();
/// assert_eq!((p.lat, p.lng), (52.52, 13.405));
/// let q = vld::geo_point().parse("[13.405, 52.52]").unwrap(); // [lng, lat]
/// assert_eq!(p, q);
/// let err = vld::geo_point().parse("[52.52, 213.4]").unwrap_err();
/// assert_eq!(err.issues[0].path_string(), "[1]");
/// ```
#[derive(Clone, Default)]
pub struct ZGeoPoint {
    custom_type_error: Option<String>,
}

impl ZGeoPoint {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom error message for type mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }
}

/// Parse `value` with `schema`, issues under `seg`.
fn coord(schema: ZNumber, value: &Value, seg: PathSegment) -> Result<f64, VldError> {
    schema.parse_value(value).map_err(|e| e.with_prefix(seg))
}

fn both(lat: Result<f64, VldError>, lng: Result<f64, VldError>) -> Result<GeoPoint, VldError> {
    match (lat, lng) {
        (Ok(lat), Ok(lng)) => Ok(GeoPoint { lat, lng }),
        (a, b) => Err(a
            .err()
            .unwrap_or_default()
            .merge(b.err().unwrap_or_default())),
    }
}

impl VldSchema for ZGeoPoint {
    type Output = GeoPoint;

    fn parse_value(&self, value: &Value) -> Result<GeoPoint, VldError> {
        match value {
            Value::Object(o) => {
                let lat = match o.get("lat") {
                    Some(v) => coord(crate::latitude(), v, field("lat")),
                    None => Err(missing("lat")),
                };
                let lng = match o
                    .get("lng")
                    .map(|v| (v, "lng"))
                    .or(o.get("lon").map(|v| (v, "lon")))
                {
                    Some((v, key)) => coord(crate::longitude(), v, field(key)),
                    None => Err(missing("lng")),
                };
                both(lat, lng)
            }
            Value::Array(a) if a.len() == 2 => {
                let lng = coord(crate::longitude(), &a[0], PathSegment::Index(0));
                let lat = coord(crate::latitude(), &a[1], PathSegment::Index(1));
                both(lat, lng)
            }
            Value::Array(_) => Err(issue(
                IssueCode::InvalidLength {
                    origin: SizeOrigin::Tuple,
                    expected: 2,
                },
                value,
            )),
            _ => Err(crate::primitives::type_error(
                &self.custom_type_error,
                "geo_point",
                value,
            )),
        }
    }
}

/// `{"anyOf": [{"type": "object", "properties": {"lat": .., "lng": ..},
/// "required": ["lat", "lng"]}, {"type": "array", "prefixItems": [lng, lat],
/// "items": false, "minItems": 2, "maxItems": 2}]}` (the `"lon"` alias is not
/// in the schema).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZGeoPoint {
    fn json_schema(&self) -> Value {
        let (lat, lng) = (
            crate::latitude().json_schema(),
            crate::longitude().json_schema(),
        );
        serde_json::json!({
            "anyOf": [
                {
                    "type": "object",
                    "properties": {"lat": lat, "lng": lng},
                    "required": ["lat", "lng"],
                },
                {
                    "type": "array",
                    "prefixItems": [lng, lat],
                    "items": false,
                    "minItems": 2,
                    "maxItems": 2,
                },
            ]
        })
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZGeoPoint {}

/// The seven RFC 7946 geometry types.
const GEOMETRIES: &[&str] = &[
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

/// Schema for a GeoJSON object (RFC 7946), output as the input
/// `serde_json::Value`. Created via [`vld::geojson()`](crate::geojson).
///
/// Accepts any geometry by default; [`types`](Self::types) narrows or
/// widens the accepted top-level `type` (`"Feature"`, `"FeatureCollection"`).
/// Every issue is reported, at its exact path:
///
/// | Rule | Issue |
/// |---|---|
/// | an object with a known `type` | `invalid_type` (`object`) / `missing_field` / `invalid_enum_value` at `.type` |
/// | position: an array of ≥ 2 numbers (`[lng, lat]`, optional altitude; ranges are not checked) | `invalid_type` / `too_small` (origin `array`) |
/// | `Point`: a position; `MultiPoint`: positions | — |
/// | `LineString`: ≥ 2 positions; `MultiLineString`: arrays of them | `too_small` (`array`, 2) |
/// | `Polygon`: linear rings of ≥ 4 positions, last = first; `MultiPolygon`: arrays of them | `too_small` (`array`, 4), `ring_not_closed` at the ring |
/// | `GeometryCollection`: `geometries`, an array of geometries | per item |
/// | `Feature`: `geometry` (a geometry or `null`) and `properties` (an object or `null`), both required | `missing_field`, `invalid_type` |
/// | `FeatureCollection`: `features`, an array of Features | per item |
/// | optional `bbox`: 4 or 6 numbers | `too_small` / `too_big` / `invalid_length` (`array`) |
///
/// Ring winding order and coordinate ranges are not checked (RFC 7946 asks
/// parsers not to reject either); an empty `coordinates` array is accepted
/// only where its type allows zero items (not for `Point` / `LineString`).
/// Foreign members (`id`, `title`, …) pass through unchecked.
///
/// ```
/// use vld::prelude::*;
///
/// let ok = vld::geojson().parse(r#"{"type": "LineString", "coordinates": [[0, 0], [1, 1]]}"#);
/// assert!(ok.is_ok());
/// let err = vld::geojson()
///     .parse(r#"{"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1]]]}"#)
///     .unwrap_err();
/// assert_eq!(err.issues[0].path_string(), ".coordinates[0]");
/// assert_eq!(err.issues[0].code.key(), "ring_not_closed");
/// ```
#[derive(Clone)]
pub struct ZGeoJson {
    types: Vec<String>,
}

impl ZGeoJson {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            types: GEOMETRIES.iter().map(|t| t.to_string()).collect(),
        }
    }

    /// Accept only these top-level types: any of the seven geometry types,
    /// `"Feature"` and `"FeatureCollection"`.
    ///
    /// # Panics
    /// On an unknown type name or an empty list.
    pub fn types(mut self, types: &[&str]) -> Self {
        assert!(
            !types.is_empty(),
            "geojson().types() needs at least one type"
        );
        for t in types {
            assert!(
                GEOMETRIES.contains(t) || *t == "Feature" || *t == "FeatureCollection",
                "Unknown GeoJSON type: {}",
                t
            );
        }
        self.types = types.iter().map(|t| t.to_string()).collect();
        self
    }
}

impl Default for ZGeoJson {
    fn default() -> Self {
        Self::new()
    }
}

/// A position: an array of at least two numbers.
fn position(v: &Value) -> VldError {
    let Some(a) = v.as_array() else {
        return wrong_type("array", v);
    };
    let mut err = VldError::new();
    for (i, x) in a.iter().enumerate().filter(|(_, x)| !x.is_number()) {
        err = err.merge(wrong_type("number", x).with_prefix(PathSegment::Index(i)));
    }
    if a.len() < 2 {
        err = err.merge(too_small(2, v));
    }
    err
}

fn too_small(min: usize, v: &Value) -> VldError {
    issue(
        IssueCode::TooSmall {
            origin: SizeOrigin::Array,
            minimum: min.into(),
            inclusive: true,
        },
        v,
    )
}

/// An array whose items all pass `item` (issues under their index), with at
/// least `min` items.
fn array_of(v: &Value, min: usize, item: impl Fn(&Value) -> VldError) -> VldError {
    let Some(a) = v.as_array() else {
        return wrong_type("array", v);
    };
    let mut err = VldError::new();
    for (i, x) in a.iter().enumerate() {
        err = err.merge(item(x).with_prefix(PathSegment::Index(i)));
    }
    if a.len() < min {
        err = err.merge(too_small(min, v));
    }
    err
}

/// A linear ring: ≥ 4 positions, the last equal to the first.
fn ring(v: &Value) -> VldError {
    let err = array_of(v, 4, position);
    let closed = match v.as_array().map(Vec::as_slice) {
        Some([first, .., last]) => crate::primitives::json_eq(first, last),
        _ => true,
    };
    if err.is_empty() && !closed {
        return issue(IssueCode::RingNotClosed, v);
    }
    err
}

fn coordinates(ty: &str, v: &Value) -> VldError {
    let line = |v: &Value| array_of(v, 2, position);
    let polygon = |v: &Value| array_of(v, 0, ring);
    match ty {
        "Point" => position(v),
        "MultiPoint" => array_of(v, 0, position),
        "LineString" => line(v),
        "MultiLineString" => array_of(v, 0, line),
        "Polygon" => polygon(v),
        _ => array_of(v, 0, polygon),
    }
}

/// `bbox`: 4 or 6 numbers.
fn bbox(v: &Value) -> VldError {
    let Some(a) = v.as_array() else {
        return wrong_type("array", v);
    };
    let mut err = VldError::new();
    for (i, x) in a.iter().enumerate().filter(|(_, x)| !x.is_number()) {
        err = err.merge(wrong_type("number", x).with_prefix(PathSegment::Index(i)));
    }
    let code = match a.len() {
        4 | 6 => return err,
        0..4 => IssueCode::TooSmall {
            origin: SizeOrigin::Array,
            minimum: 4.into(),
            inclusive: true,
        },
        5 => IssueCode::InvalidLength {
            origin: SizeOrigin::Array,
            expected: 6,
        },
        _ => IssueCode::TooBig {
            origin: SizeOrigin::Array,
            maximum: 6.into(),
            inclusive: true,
        },
    };
    err.merge(issue(code, v))
}

/// Member `key` of `obj` checked by `check`, issues under `.key`.
fn member(obj: &Map<String, Value>, key: &str, check: impl Fn(&Value) -> VldError) -> VldError {
    match obj.get(key) {
        Some(v) => check(v).with_prefix(field(key)),
        None => missing(key),
    }
}

/// A GeoJSON object whose `type` is one of `allowed`.
fn object(v: &Value, allowed: &[&str]) -> VldError {
    let Some(obj) = v.as_object() else {
        return wrong_type("object", v);
    };
    let ty = match obj.get("type") {
        None => return missing("type"),
        Some(t) => match t.as_str() {
            Some(s) if allowed.contains(&s) => s,
            Some(_) => {
                let options = allowed.iter().map(|s| s.to_string()).collect();
                return issue(IssueCode::InvalidEnumValue { options }, t)
                    .with_prefix(field("type"));
            }
            None => return wrong_type("string", t).with_prefix(field("type")),
        },
    };
    let mut err = match ty {
        "GeometryCollection" => member(obj, "geometries", |g| {
            array_of(g, 0, |x| object(x, GEOMETRIES))
        }),
        "Feature" => member(obj, "geometry", |g| match g {
            Value::Null => VldError::new(),
            g => object(g, GEOMETRIES),
        })
        .merge(member(obj, "properties", |p| match p {
            Value::Null | Value::Object(_) => VldError::new(),
            p => wrong_type("object", p),
        })),
        "FeatureCollection" => member(obj, "features", |f| {
            array_of(f, 0, |x| object(x, &["Feature"]))
        }),
        geometry => member(obj, "coordinates", |c| coordinates(geometry, c)),
    };
    if let Some(b) = obj.get("bbox") {
        err = err.merge(bbox(b).with_prefix(field("bbox")));
    }
    err
}

impl VldSchema for ZGeoJson {
    type Output = Value;

    fn parse_value(&self, value: &Value) -> Result<Value, VldError> {
        let allowed: Vec<&str> = self.types.iter().map(String::as_str).collect();
        let err = object(value, &allowed);
        if err.is_empty() {
            Ok(value.clone())
        } else {
            Err(err)
        }
    }
}

#[cfg(feature = "openapi")]
mod json_schema {
    use serde_json::{Value, json};

    fn array(items: Value, min: usize) -> Value {
        let mut s = json!({"type": "array", "items": items});
        if min > 0 {
            s["minItems"] = min.into();
        }
        s
    }

    fn position() -> Value {
        array(json!({"type": "number"}), 2)
    }

    fn ring() -> Value {
        let mut s = array(position(), 4);
        s["x-closedRing"] = true.into();
        s
    }

    fn with_type(ty: &str, key: &str, member: Value) -> Value {
        json!({
            "type": "object",
            "properties": {"type": {"const": ty}, key: member},
            "required": ["type", key],
        })
    }

    fn geometry(ty: &str) -> Value {
        let coordinates = match ty {
            "Point" => position(),
            "MultiPoint" => array(position(), 0),
            "LineString" => array(position(), 2),
            "MultiLineString" => array(array(position(), 2), 0),
            "Polygon" => array(ring(), 0),
            "MultiPolygon" => array(array(ring(), 0), 0),
            // Nested collections are accepted by vld but not described here.
            _ => {
                let simple = super::GEOMETRIES[..6].iter().map(|t| geometry(t)).collect();
                return with_type(
                    ty,
                    "geometries",
                    array(json!({"anyOf": Value::Array(simple)}), 0),
                );
            }
        };
        with_type(ty, "coordinates", coordinates)
    }

    fn feature() -> Value {
        let mut geometries: Vec<Value> = super::GEOMETRIES.iter().map(|t| geometry(t)).collect();
        geometries.push(json!({"type": "null"}));
        json!({
            "type": "object",
            "properties": {
                "type": {"const": "Feature"},
                "geometry": {"anyOf": geometries},
                "properties": {"type": ["object", "null"]},
            },
            "required": ["type", "geometry", "properties"],
        })
    }

    pub(super) fn of(ty: &str) -> Value {
        match ty {
            "Feature" => feature(),
            "FeatureCollection" => with_type(ty, "features", array(feature(), 0)),
            _ => geometry(ty),
        }
    }
}

/// One object schema per accepted type (`{"anyOf": [...]}` for several),
/// each with `"type": {"const": ..}`. Linear rings carry `"x-closedRing":
/// true` (closure is not expressible in JSON Schema; `vld::fake` honours
/// it); `bbox` and nested geometry collections are not described.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZGeoJson {
    fn json_schema(&self) -> Value {
        let mut all: Vec<Value> = self.types.iter().map(|t| json_schema::of(t)).collect();
        if all.len() == 1 {
            all.remove(0)
        } else {
            serde_json::json!({ "anyOf": all })
        }
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZGeoJson {}
