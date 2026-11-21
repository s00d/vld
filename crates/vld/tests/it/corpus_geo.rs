//! Corpus: geographic schemas (feature `geo`) — `latitude()` / `longitude()` bounds,
//! `geo_point()` in object and GeoJSON `[lng, lat]` order, and `geojson()` per RFC 7946 with the
//! exact path of every issue; valid documents also pass a JSON Schema validator on vld's schema.

use crate::common::*;
use serde_json::{Value, json};
use vld::prelude::*;

fn at(path: &str, code: &str) -> (String, String) {
    (path.to_string(), code.to_string())
}

/// Given `latitude()`, `longitude()` and `geo_point()`
/// When coordinates at, inside and just beyond ±90 / ±180 arrive, as objects (`lng` or `lon`)
/// and as `[lng, lat]` pairs, plus wrong shapes
/// Then the ends are inclusive; an out-of-range coordinate is `too_small` / `too_big` at its
/// field or index (pairs are longitude first); both coordinates' issues are reported; a pair
/// must have 2 items (`invalid_length`); other values are `invalid_type` (`geo_point`).
#[test]
fn coordinates_and_points() {
    for v in [-90.0, 0.0, 45.5, 90.0] {
        assert_eq!(vld::latitude().parse_value(&json!(v)).unwrap(), v);
    }
    for v in [-180.0, 179.999, 180.0] {
        assert_eq!(vld::longitude().parse_value(&json!(v)).unwrap(), v);
    }
    for (s, v, code) in [
        (vld::latitude(), -90.000001, "too_small"),
        (vld::latitude(), 90.5, "too_big"),
        (vld::longitude(), -180.5, "too_small"),
        (vld::longitude(), 180.000001, "too_big"),
    ] {
        assert_eq!(
            issues(&s.parse_value(&json!(v)).unwrap_err()),
            [at("", code)]
        );
    }

    let p = vld::geo_point();
    let berlin = vld::primitives::GeoPoint {
        lat: 52.52,
        lng: 13.405,
    };
    for v in [
        json!({"lat": 52.52, "lng": 13.405}),
        json!({"lat": 52.52, "lon": 13.405}),
        json!({"lng": 13.405, "lat": 52.52, "name": "Berlin"}),
        json!([13.405, 52.52]),
    ] {
        assert_eq!(p.parse_value(&v).unwrap(), berlin, "{v}");
    }
    assert!(p.parse_value(&json!([180, -90])).is_ok());
    for (v, want) in [
        (json!({"lat": 91, "lng": 0}), vec![at(".lat", "too_big")]),
        (
            json!({"lat": 0, "lon": -181}),
            vec![at(".lon", "too_small")],
        ),
        (json!({"lat": 0}), vec![at(".lng", "missing_field")]),
        (
            json!({}),
            vec![at(".lat", "missing_field"), at(".lng", "missing_field")],
        ),
        (
            json!({"lat": "52", "lng": null}),
            vec![at(".lat", "invalid_type"), at(".lng", "invalid_type")],
        ),
        // [lat, lng] by mistake: 52.52 is a valid longitude, 213.4 no latitude.
        (json!([52.52, 213.4]), vec![at("[1]", "too_big")]),
        (
            json!([200, 100]),
            vec![at("[1]", "too_big"), at("[0]", "too_big")],
        ),
        (json!([1]), vec![at("", "invalid_length")]),
        (json!([1, 2, 3]), vec![at("", "invalid_length")]),
        (json!("52.52,13.4"), vec![at("", "invalid_type")]),
    ] {
        assert_eq!(issues(&p.parse_value(&v).unwrap_err()), want, "{v}");
    }
    let err = p.parse_value(&json!(true)).unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidType {
            expected: "geo_point".into(),
            received: "boolean".into()
        }
    );
    assert_eq!(
        serde_json::to_value(berlin).unwrap(),
        json!({"lat": 52.52, "lng": 13.405})
    );

    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        let lat = json!({"type": "number", "minimum": -90.0, "maximum": 90.0});
        let lng = json!({"type": "number", "minimum": -180.0, "maximum": 180.0});
        assert_eq!(vld::latitude().json_schema(), lat);
        assert_eq!(vld::longitude().json_schema(), lng);
        assert_eq!(
            p.json_schema(),
            json!({"anyOf": [
                {"type": "object", "properties": {"lat": lat, "lng": lng}, "required": ["lat", "lng"]},
                {"type": "array", "prefixItems": [lng, lat], "items": false, "minItems": 2, "maxItems": 2},
            ]})
        );
        use vld::schema::VldType;
        assert_eq!(
            vld::primitives::GeoPoint::vld_schema().json_schema(),
            p.json_schema()
        );
    }
}

/// Valid GeoJSON of every type (RFC 7946 examples, altitude, bbox, nested collections).
fn valid_documents() -> Vec<Value> {
    let ring = json!([[100, 0], [101, 0], [101, 1], [100, 1], [100, 0]]);
    let hole = json!([
        [100.8, 0.8],
        [100.8, 0.2],
        [100.2, 0.2],
        [100.2, 0.8],
        [100.8, 0.8]
    ]);
    let point = json!({"type": "Point", "coordinates": [100.0, 0.0]});
    vec![
        point.clone(),
        json!({"type": "Point", "coordinates": [100.0, 0.0, 12.5]}),
        json!({"type": "MultiPoint", "coordinates": [[100, 0], [101, 1]]}),
        json!({"type": "MultiPoint", "coordinates": []}),
        json!({"type": "LineString", "coordinates": [[100, 0], [101, 1]]}),
        json!({"type": "MultiLineString", "coordinates": [[[100, 0], [101, 1]], [[102, 2], [103, 3]]]}),
        json!({"type": "Polygon", "coordinates": [ring]}),
        json!({"type": "Polygon", "coordinates": [ring, hole]}),
        json!({"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [0, 1], [0.0, 0.0]]]}),
        json!({"type": "MultiPolygon", "coordinates": [[ring], [ring, hole]]}),
        json!({"type": "GeometryCollection", "geometries": [point, {"type": "LineString", "coordinates": [[0, 0], [1, 1]]}]}),
        json!({"type": "GeometryCollection", "geometries": []}),
        json!({"type": "Point", "coordinates": [1, 2], "bbox": [1, 2, 1, 2]}),
        json!({"type": "Point", "coordinates": [1, 2, 3], "bbox": [1, 2, 3, 1, 2, 3], "title": "x"}),
    ]
}

/// Given `geojson()` (geometries) and `geojson().types(...)` (Features, FeatureCollections)
/// When valid documents of every type and documents broken in one place are parsed
/// Then valid ones are returned unchanged (and pass a JSON Schema validator on vld's schema);
/// each broken one reports exactly the predicted `(path, code)` list: wrong / missing `type`,
/// short positions and line strings, short or open rings (`ring_not_closed` at the ring), wrong
/// member types, bad `bbox`, issues inside collections and features at their full path.
#[test]
fn geojson_documents() {
    let g = vld::geojson();
    for doc in valid_documents() {
        assert_eq!(g.parse_value(&doc).unwrap(), doc);
    }
    let open = json!([[0, 0], [1, 0], [1, 1], [0, 1]]);
    for (doc, want) in [
        (json!([1, 2]), vec![at("", "invalid_type")]),
        (
            json!({"coordinates": [1, 2]}),
            vec![at(".type", "missing_field")],
        ),
        (
            json!({"type": "point", "coordinates": [1, 2]}),
            vec![at(".type", "invalid_enum_value")],
        ),
        (
            json!({"type": "Feature", "geometry": null, "properties": null}),
            vec![at(".type", "invalid_enum_value")],
        ),
        (json!({"type": 1}), vec![at(".type", "invalid_type")]),
        (
            json!({"type": "Point"}),
            vec![at(".coordinates", "missing_field")],
        ),
        (
            json!({"type": "Point", "coordinates": [1]}),
            vec![at(".coordinates", "too_small")],
        ),
        (
            json!({"type": "Point", "coordinates": []}),
            vec![at(".coordinates", "too_small")],
        ),
        (
            json!({"type": "Point", "coordinates": "1,2"}),
            vec![at(".coordinates", "invalid_type")],
        ),
        (
            json!({"type": "Point", "coordinates": ["1", null]}),
            vec![
                at(".coordinates[0]", "invalid_type"),
                at(".coordinates[1]", "invalid_type"),
            ],
        ),
        (
            json!({"type": "MultiPoint", "coordinates": [[1, 2], [3]]}),
            vec![at(".coordinates[1]", "too_small")],
        ),
        (
            json!({"type": "LineString", "coordinates": [[1, 2]]}),
            vec![at(".coordinates", "too_small")],
        ),
        (
            json!({"type": "LineString", "coordinates": [1, 2]}),
            vec![
                at(".coordinates[0]", "invalid_type"),
                at(".coordinates[1]", "invalid_type"),
            ],
        ),
        (
            json!({"type": "MultiLineString", "coordinates": [[[1, 2], [3, 4]], [[5, 6]]]}),
            vec![at(".coordinates[1]", "too_small")],
        ),
        (
            json!({"type": "Polygon", "coordinates": [open]}),
            vec![at(".coordinates[0]", "ring_not_closed")],
        ),
        (
            json!({"type": "Polygon", "coordinates": [[[0, 0], [1, 1], [0, 0]]]}),
            vec![at(".coordinates[0]", "too_small")],
        ),
        (
            json!({"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1], [0, 0]]]}),
            vec![at(".coordinates[0][2]", "too_small")],
        ),
        (
            json!({"type": "Polygon", "coordinates": [[0, 0], [1, 1]]}),
            vec![
                at(".coordinates[0][0]", "invalid_type"),
                at(".coordinates[0][1]", "invalid_type"),
                at(".coordinates[0]", "too_small"),
                at(".coordinates[1][0]", "invalid_type"),
                at(".coordinates[1][1]", "invalid_type"),
                at(".coordinates[1]", "too_small"),
            ],
        ),
        (
            json!({"type": "MultiPolygon", "coordinates": [[[[0, 0], [1, 0], [1, 1], [0, 0]]], [open]]}),
            vec![at(".coordinates[1][0]", "ring_not_closed")],
        ),
        (
            json!({"type": "GeometryCollection"}),
            vec![at(".geometries", "missing_field")],
        ),
        (
            json!({"type": "GeometryCollection", "geometries": [
                {"type": "Point", "coordinates": [1, 2]},
                {"type": "Feature", "geometry": null, "properties": null},
                {"type": "GeometryCollection", "geometries": [{"type": "LineString", "coordinates": [[1, 2]]}]},
            ]}),
            vec![
                at(".geometries[1].type", "invalid_enum_value"),
                at(".geometries[2].geometries[0].coordinates", "too_small"),
            ],
        ),
        (
            json!({"type": "Point", "coordinates": [1, 2], "bbox": [1, 2, 3]}),
            vec![at(".bbox", "too_small")],
        ),
        (
            json!({"type": "Point", "coordinates": [1, 2], "bbox": [1, 2, 3, 4, 5]}),
            vec![at(".bbox", "invalid_length")],
        ),
        (
            json!({"type": "Point", "coordinates": [1, 2], "bbox": [1, 2, 3, 4, 5, 6, 7]}),
            vec![at(".bbox", "too_big")],
        ),
        (
            json!({"type": "Point", "coordinates": [1], "bbox": [1, "2", 3, 4]}),
            vec![
                at(".coordinates", "too_small"),
                at(".bbox[1]", "invalid_type"),
            ],
        ),
    ] {
        assert_eq!(issues(&g.parse_value(&doc).unwrap_err()), want, "{doc}");
    }
    let err = g
        .parse_value(&json!({"type": "Polygon", "coordinates": [open]}))
        .unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::RingNotClosed);

    let features = vld::geojson().types(&["Feature", "FeatureCollection"]);
    let feature = json!({"type": "Feature", "geometry": {"type": "Point", "coordinates": [1, 2]}, "properties": {"name": "x"}, "id": 7});
    for doc in [
        feature.clone(),
        json!({"type": "Feature", "geometry": null, "properties": null}),
        json!({"type": "FeatureCollection", "features": [feature]}),
        json!({"type": "FeatureCollection", "features": []}),
    ] {
        assert_eq!(features.parse_value(&doc).unwrap(), doc);
    }
    for (doc, want) in [
        (
            json!({"type": "Point", "coordinates": [1, 2]}),
            vec![at(".type", "invalid_enum_value")],
        ),
        (
            json!({"type": "Feature"}),
            vec![
                at(".geometry", "missing_field"),
                at(".properties", "missing_field"),
            ],
        ),
        (
            json!({"type": "Feature", "geometry": {"type": "Feature"}, "properties": []}),
            vec![
                at(".geometry.type", "invalid_enum_value"),
                at(".properties", "invalid_type"),
            ],
        ),
        (
            json!({"type": "FeatureCollection", "features": [{"type": "Point", "coordinates": [1, 2]}, {"type": "Feature", "geometry": {"type": "LineString", "coordinates": [[1, 2]]}, "properties": null}]}),
            vec![
                at(".features[0].type", "invalid_enum_value"),
                at(".features[1].geometry.coordinates", "too_small"),
            ],
        ),
        (
            json!({"type": "FeatureCollection", "features": {}}),
            vec![at(".features", "invalid_type")],
        ),
    ] {
        assert_eq!(
            issues(&features.parse_value(&doc).unwrap_err()),
            want,
            "{doc}"
        );
    }
    let err = features
        .parse_value(&json!({"type": "Polygon", "coordinates": []}))
        .unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidEnumValue {
            options: vec!["Feature".into(), "FeatureCollection".into()]
        }
    );
    for bad in [&[][..], &["Circle"][..]] {
        let bad = bad.to_vec();
        assert!(std::panic::catch_unwind(move || drop(vld::geojson().types(&bad))).is_err());
    }

    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        let points = vld::geojson().types(&["Point"]).json_schema();
        assert_eq!(
            points,
            json!({
                "type": "object",
                "properties": {
                    "type": {"const": "Point"},
                    "coordinates": {"type": "array", "items": {"type": "number"}, "minItems": 2},
                },
                "required": ["type", "coordinates"],
            })
        );
        let polygon = vld::geojson().types(&["Polygon"]).json_schema();
        assert_eq!(
            polygon["properties"]["coordinates"]["items"]["x-closedRing"],
            json!(true)
        );
        assert_eq!(
            polygon["properties"]["coordinates"]["items"]["minItems"],
            json!(4)
        );
        let all = g.json_schema();
        assert_eq!(all["anyOf"].as_array().unwrap().len(), 7);
        let validator = jsonschema::draft202012::new(&all).unwrap();
        for doc in valid_documents() {
            assert!(validator.is_valid(&doc), "{doc}");
        }
        let fs = features.json_schema();
        let validator = jsonschema::draft202012::new(&fs).unwrap();
        assert!(validator.is_valid(&feature));
        assert!(!validator.is_valid(&json!({"type": "Feature", "geometry": null})));
    }
}
