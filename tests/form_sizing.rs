//! Resizing a dress form to measurements. The form is a synthetic torso with a different girth at
//! each landmark, so no third-party mesh is needed.
use marvelous_cloth::avatar::SurfaceMesh;
use marvelous_cloth::avatar_catalog;
use marvelous_cloth::form_sizing::{Sizer, SizingDecl, Targets, girth_cm};
use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

const HEIGHT: f64 = 0.80;

/// A closed elliptical torso, 0.80 m tall, whose size changes with height: wide hip, narrow waist,
/// fuller bust, narrow neck. Rings are dense (every 5 mm) so a thin slab always holds a full ring.
fn torso_obj() -> String {
    torso_obj_with_rings(160)
}

fn torso_obj_with_rings(rings: usize) -> String {
    let around = 64usize;
    let radius = |y: f64| -> f64 {
        let bump = |c: f64, w: f64, a: f64| a * (-((y - c) / w).powi(2)).exp();
        0.70 + bump(0.20, 0.08, 0.30) - bump(0.36, 0.06, 0.18) + bump(0.54, 0.07, 0.22)
            - bump(0.74, 0.05, 0.30)
    };
    let mut obj = String::from("o torso\n");
    for r in 0..=rings {
        let y = HEIGHT * r as f64 / rings as f64;
        for a in 0..around {
            let angle = std::f64::consts::TAU * a as f64 / around as f64;
            obj += &format!(
                "v {:.6} {:.6} {:.6}\n",
                0.17 * radius(y) * angle.cos(),
                y,
                0.12 * radius(y) * angle.sin()
            );
        }
    }
    let (bottom, top) = ((rings + 1) * around + 1, (rings + 1) * around + 2);
    obj += &format!("v 0 0 0\nv 0 {HEIGHT:.6} 0\n");
    let at = |r: usize, a: usize| r * around + (a % around) + 1;
    for r in 0..rings {
        for a in 0..around {
            obj += &format!("f {} {} {}\n", at(r, a), at(r, a + 1), at(r + 1, a + 1));
            obj += &format!("f {} {} {}\n", at(r, a), at(r + 1, a + 1), at(r + 1, a));
        }
    }
    for a in 0..around {
        obj += &format!("f {} {} {}\n", bottom, at(0, a + 1), at(0, a));
        obj += &format!("f {} {} {}\n", top, at(rings, a), at(rings, a + 1));
    }
    obj
}

fn decl() -> SizingDecl {
    serde_json::from_str(
        r#"{"knots":[{"name":"hip","y_m":0.20,"basis":"test"},{"name":"waist","y_m":0.36,"basis":"test"},
            {"name":"underbust","y_m":0.47,"basis":"assumed"},{"name":"bust","y_m":0.54,"basis":"test"},
            {"name":"neck","y_m":0.74,"basis":"test"}],
            "chart":[{"label":"Test size","cm":{"bust":95.0,"waist":72.0}}]}"#,
    )
    .unwrap()
}

fn sizer() -> Sizer {
    Sizer::new(
        Arc::new(SurfaceMesh::parse_obj(&torso_obj()).unwrap()),
        &decl(),
    )
    .unwrap()
}

fn targets(pairs: &[(&str, f64)]) -> Targets {
    let flat: BTreeMap<String, f64> = pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    Targets::from_flat(&flat).unwrap()
}

fn close(actual: f64, want: f64, tolerance_percent: f64, what: &str) {
    let error = 100.0 * (actual - want).abs() / want;
    assert!(
        error <= tolerance_percent,
        "{what}: {actual:.3} vs {want:.3} ({error:.2} % off)"
    );
}

#[test]
fn the_scan_is_measured_at_each_landmark_and_the_girths_differ() {
    let s = sizer();
    let g = &s.base.girth_cm;
    assert!(
        g["hip"] > g["waist"] && g["bust"] > g["waist"] && g["neck"] < g["waist"],
        "{g:?}"
    );
    assert!((s.base.waist_to_hip_cm - 16.0).abs() < 1e-9);
    assert!((s.base.back_waist_length_cm - 38.0).abs() < 1e-9);
}

#[test]
fn no_targets_leaves_every_vertex_where_it_was() {
    let s = sizer();
    let r = s.resize(&Targets::default()).unwrap();
    let original = SurfaceMesh::parse_obj(&torso_obj()).unwrap();
    assert_eq!(r.surface.triangles, original.triangles);
    for (a, b) in r.surface.vertices.iter().zip(&original.vertices) {
        assert!(a.distance(*b) < 1e-9);
    }
    assert!((r.map_y(0.5) - 0.5).abs() < 1e-12);
}

#[test]
fn a_girth_target_is_reached_on_the_resized_mesh_and_the_others_stay() {
    let s = sizer();
    let want = s.base.girth_cm["bust"] * 1.10;
    let r = s.resize(&targets(&[("bust", want)])).unwrap();
    close(r.measured.girth_cm["bust"], want, 0.3, "bust");
    for key in ["hip", "waist", "neck"] {
        close(r.measured.girth_cm[key], s.base.girth_cm[key], 0.3, key);
    }
}

#[test]
fn several_girths_together_each_reach_their_target() {
    let s = sizer();
    let asks = [
        ("bust", 1.08),
        ("waist", 1.15),
        ("hip", 0.93),
        ("neck", 1.05),
        ("underbust", 1.04),
    ];
    let t: Vec<(&str, f64)> = asks
        .iter()
        .map(|(k, f)| (*k, s.base.girth_cm[*k] * f))
        .collect();
    let r = s.resize(&targets(&t)).unwrap();
    for (key, want) in t {
        close(r.measured.girth_cm[key], want, 0.5, key);
    }
}

#[test]
fn lengths_move_the_landmarks_by_exactly_what_was_asked() {
    let s = sizer();
    let r = s
        .resize(&targets(&[
            ("waist_to_hip", 18.0),
            ("back_waist_length", 35.0),
        ]))
        .unwrap();
    assert!((r.knot_y["waist"] - r.knot_y["hip"] - 0.18).abs() < 1e-9);
    assert!((r.knot_y["neck"] - r.knot_y["waist"] - 0.35).abs() < 1e-9);
    assert_eq!(r.knot_y["hip"], 0.20, "nothing below the hip moves");
    assert!((r.measured.back_waist_length_cm - 35.0).abs() < 1e-9);
    let floor = r
        .surface
        .vertices
        .iter()
        .map(|v| v.y)
        .fold(f64::INFINITY, f64::min);
    assert!(
        floor.abs() < 1e-9,
        "the form still stands on y = 0, not {floor}"
    );
    // The underbust and bust landmarks sit between waist and neck in the same proportion as before.
    let before = (0.54 - 0.36) / (0.74 - 0.36);
    let after = (r.knot_y["bust"] - r.knot_y["waist"]) / (r.knot_y["neck"] - r.knot_y["waist"]);
    assert!((before - after).abs() < 1e-9);
}

#[test]
fn a_taller_form_stays_watertight_and_finite() {
    let s = sizer();
    let waist = s.base.girth_cm["waist"] * 1.2;
    let r = s
        .resize(&targets(&[("back_waist_length", 44.0), ("waist", waist)]))
        .unwrap();
    assert!(r.surface.vertices.iter().all(|v| v.is_finite()));
    r.surface
        .validate()
        .expect("the resized mesh is still a valid surface");
    assert_eq!(
        r.surface.triangles.len(),
        SurfaceMesh::parse_obj(&torso_obj())
            .unwrap()
            .triangles
            .len()
    );
}

#[test]
fn a_request_outside_the_range_is_refused_with_the_range() {
    let s = sizer();
    let error = s
        .resize(&targets(&[("bust", s.base.girth_cm["bust"] * 1.6)]))
        .err()
        .expect("refused");
    assert!(
        error.contains("bust") && error.contains("outside") && error.contains("80-125"),
        "{error}"
    );
    assert!(s.resize(&targets(&[("waist_to_hip", 2.0)])).is_err());
    assert!(s.resize(&targets(&[("back_waist_length", 90.0)])).is_err());
}

#[test]
fn an_unknown_or_non_finite_measurement_is_refused() {
    let flat = |k: &str, v: f64| BTreeMap::from([(k.to_owned(), v)]);
    assert!(
        Targets::from_flat(&flat("shoe_size", 40.0))
            .unwrap_err()
            .contains("unknown")
    );
    assert!(Targets::from_flat(&flat("bust", f64::NAN)).is_err());
    assert!(Targets::from_flat(&flat("bust", f64::INFINITY)).is_err());
}

#[test]
fn a_landmark_declaration_must_be_complete_ordered_and_inside_the_form() {
    let parse = |json: &str| {
        serde_json::from_str::<SizingDecl>(json)
            .unwrap()
            .validate(HEIGHT)
    };
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.36},{"name":"bust","y_m":0.54}]}"#).unwrap_err().contains("\"neck\" is required"));
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.1},{"name":"bust","y_m":0.54},{"name":"neck","y_m":0.74}]}"#).unwrap_err().contains("rise"));
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.36},{"name":"bust","y_m":0.54},{"name":"neck","y_m":0.9}]}"#).unwrap_err().contains("below the top"));
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.36},{"name":"bust","y_m":0.54},{"name":"knee","y_m":0.7},{"name":"neck","y_m":0.74}]}"#).unwrap_err().contains("unknown landmark"));
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.36},{"name":"bust","y_m":0.54},{"name":"neck","y_m":0.74}],"chart":[{"label":"x","cm":{"inseam":80}}]}"#).unwrap_err().contains("unknown measurement"));
    assert!(parse(r#"{"knots":[{"name":"hip","y_m":0.2},{"name":"waist","y_m":0.36},{"name":"bust","y_m":0.54},{"name":"neck","y_m":0.74}]}"#).is_ok());
}

#[test]
fn a_landmark_at_the_wrong_height_is_caught_when_the_scan_is_read() {
    // A coarse scan (a ring every 5 cm) has nothing within 3 mm of a landmark that falls between rings.
    let coarse = Arc::new(SurfaceMesh::parse_obj(&torso_obj_with_rings(16)).unwrap());
    let error = Sizer::new(coarse, &decl()).err().expect("refused");
    assert!(
        error.contains("too few vertices") && error.contains("waist"),
        "{error}"
    );
}

#[test]
fn girth_reads_back_what_the_mesh_measures() {
    let mesh = SurfaceMesh::parse_obj(&torso_obj()).unwrap();
    // an ellipse with semi-axes 0.17 r and 0.12 r has a perimeter near 2 pi sqrt((a^2+b^2)/2)
    // Near the floor the profile is flat (radius 0.70), so the section is a plain ellipse.
    let r: f64 = 0.70;
    let (a, b) = (0.17 * r, 0.12 * r);
    let want = 100.0 * std::f64::consts::TAU * ((a * a + b * b) / 2.0).sqrt();
    close(girth_cm(&mesh, 0.01), want, 3.0, "girth of a plain ellipse");
}

fn install_form(root: &Path, sizing: &str) {
    let dir = root.join("dressforms").join("test-form");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("torso.obj"), torso_obj()).unwrap();
    fs::write(
        dir.join("form.json"),
        format!(
            r#"{{"id":"test-form","title":"Test form","mesh":"torso.obj","height_m":{HEIGHT},"shoulder_height_m":0.62,
"source":"synthetic","license":"CC0-1.0"{sizing}}}"#
        ),
    )
    .unwrap();
}

const SIZING: &str = r#","sizing":{"knots":[{"name":"hip","y_m":0.20},{"name":"waist","y_m":0.36},{"name":"underbust","y_m":0.47},{"name":"bust","y_m":0.54},{"name":"neck","y_m":0.74}]}"#;

#[test]
fn a_sizable_form_loads_with_its_sizer_and_the_shoulder_line_moves_with_the_landmarks() {
    let root = tempfile::tempdir().unwrap();
    install_form(root.path(), SIZING);
    let entries = avatar_catalog::discover(root.path());
    let form = entries.iter().find(|e| e.id == "test-form").unwrap();
    assert!(form.available, "{:?}", form.reason);
    let loaded = form.load().unwrap();
    assert!(loaded.sizer.is_some());
    let (sized, resized) = loaded
        .sized(&targets(&[("back_waist_length", 44.0)]))
        .unwrap();
    assert!((sized.shoulder_height_m.unwrap() - resized.map_y(0.62)).abs() < 1e-12);
    assert!(
        sized.shoulder_height_m.unwrap() > 0.62,
        "a longer back lifts the shoulder line"
    );
    assert!(sized.sizer.is_none(), "a sized avatar is a plain body");
}

#[test]
fn a_form_without_sizing_cannot_be_resized_and_a_bad_declaration_is_listed_with_its_reason() {
    let root = tempfile::tempdir().unwrap();
    install_form(root.path(), "");
    let loaded = avatar_catalog::discover(root.path())
        .into_iter()
        .find(|e| e.id == "test-form")
        .unwrap()
        .load()
        .unwrap();
    assert!(
        loaded
            .sized(&targets(&[("bust", 90.0)]))
            .err()
            .unwrap()
            .contains("cannot be resized")
    );

    let other = tempfile::tempdir().unwrap();
    install_form(
        other.path(),
        r#","sizing":{"knots":[{"name":"hip","y_m":0.2}]}"#,
    );
    let entries = avatar_catalog::discover(other.path());
    let bad = entries.iter().find(|e| e.id == "test-form").unwrap();
    assert!(!bad.available);
    assert!(
        bad.reason.as_deref().unwrap().contains("sizing"),
        "{:?}",
        bad.reason
    );
}
