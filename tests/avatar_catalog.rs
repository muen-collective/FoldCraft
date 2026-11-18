//! Dress forms as bodies for the preview: discovery, the unit and floor checks, and the preview
//! on a form. The forms here are synthetic (a tapered closed torso written by the test), so no
//! third-party mesh is needed to run them.
use marvelous_cloth::avatar_catalog::{self, AvatarKind, MANNEQUIN_ID, garment_lift};
use marvelous_cloth::preview::Preview;
use marvelous_cloth::template_catalog::Selection;
use std::{fs, path::Path};

/// A closed, outward-wound elliptical torso, `height` tall, standing on `base_y`, in `unit` metres.
fn torso_obj(height: f64, base_y: f64, unit: f64) -> String {
    let (rings, around) = (24usize, 32usize);
    let mut obj = String::from("o torso\n");
    for r in 0..=rings {
        let t = r as f64 / rings as f64;
        let y = base_y + height * t;
        let waist = 1.0 - 0.18 * (-((t - 0.45) / 0.18).powi(2)).exp();
        for a in 0..around {
            let angle = std::f64::consts::TAU * a as f64 / around as f64;
            let (x, z) = (0.17 * waist * angle.cos(), 0.12 * waist * angle.sin());
            obj += &format!("v {:.6} {:.6} {:.6}\n", x * unit, y * unit, z * unit);
        }
    }
    let bottom = (rings + 1) * around + 1;
    let top = bottom + 1;
    obj += &format!(
        "v 0 {:.6} 0\nv 0 {:.6} 0\n",
        base_y * unit,
        (base_y + height) * unit
    );
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

fn form_json(id: &str, mesh: &str, height: f64, shoulder: f64) -> String {
    format!(
        r#"{{"id":"{id}","title":"Test form","mesh":"{mesh}","height_m":{height},"shoulder_height_m":{shoulder},
"dimensions_cm":{{"bust":90.0,"waist":66.0}},"source":"synthetic test torso","license":"CC0-1.0"}}"#
    )
}

fn install(root: &Path, id: &str, json: &str, obj: Option<&str>) {
    let dir = root.join("dressforms").join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("form.json"), json).unwrap();
    if let Some(obj) = obj {
        fs::write(dir.join("torso.obj"), obj).unwrap();
    }
}

fn entry<'a>(
    entries: &'a [avatar_catalog::AvatarEntry],
    id: &str,
) -> &'a avatar_catalog::AvatarEntry {
    entries.iter().find(|e| e.id == id).unwrap_or_else(|| {
        panic!(
            "no entry {id}: {:?}",
            entries.iter().map(|e| &e.id).collect::<Vec<_>>()
        )
    })
}

#[test]
fn the_bundled_mannequin_is_listed_first_and_available() {
    let entries = avatar_catalog::discover(Path::new("assets"));
    assert_eq!(entries[0].id, MANNEQUIN_ID);
    assert_eq!(entries[0].kind, AvatarKind::Mannequin);
    assert!(entries[0].available, "{:?}", entries[0].reason);
    assert!(entries[0].load().is_ok());
}

#[test]
fn a_form_is_discovered_loaded_and_keeps_its_declared_numbers() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        Some(&torso_obj(0.80, 0.0, 1.0)),
    );
    let entries = avatar_catalog::discover(root.path());
    let form = entry(&entries, "test-form");
    assert_eq!(form.kind, AvatarKind::DressForm);
    assert!(form.available, "{:?}", form.reason);
    assert_eq!(form.shoulder_height_m, Some(0.62));
    assert_eq!(form.dimensions_cm["bust"], 90.0);
    let loaded = form.load().expect("a correct form loads");
    assert_eq!(loaded.shoulder_height_m, Some(0.62));
    assert!(loaded.surface.triangles.len() > 1000);
}

#[test]
fn a_form_without_its_mesh_is_listed_unavailable_and_says_why() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        None,
    );
    let entries = avatar_catalog::discover(root.path());
    let form = entry(&entries, "test-form");
    assert!(!form.available);
    assert!(
        form.reason
            .as_deref()
            .unwrap()
            .contains("torso.obj is not installed")
    );
    assert!(form.load().is_err());
}

#[test]
fn a_mesh_in_the_wrong_units_is_refused_not_rescaled() {
    let root = tempfile::tempdir().unwrap();
    // Centimetres written where metres are read: 100 times too big.
    install(
        root.path(),
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        Some(&torso_obj(0.80, 0.0, 100.0)),
    );
    let error = entry(&avatar_catalog::discover(root.path()), "test-form")
        .load()
        .err()
        .expect("refused");
    assert!(error.contains("unit error"), "{error}");
}

#[test]
fn a_mesh_that_does_not_stand_on_the_floor_is_refused() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        Some(&torso_obj(0.80, 0.5, 1.0)),
    );
    let error = entry(&avatar_catalog::discover(root.path()), "test-form")
        .load()
        .err()
        .expect("refused");
    assert!(error.contains("y = 0"), "{error}");
}

#[test]
fn malformed_or_hostile_form_files_are_listed_with_a_reason_never_skipped() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        "escape",
        &form_json("escape", "../../etc/passwd.obj", 0.8, 0.6),
        None,
    );
    install(
        root.path(),
        "wrong-id",
        &form_json("other-id", "torso.obj", 0.8, 0.6),
        None,
    );
    install(root.path(), "broken", "{not json", None);
    install(
        root.path(),
        "tall-shoulder",
        &form_json("tall-shoulder", "torso.obj", 0.8, 0.9),
        None,
    );
    install(
        root.path(),
        "extra",
        &form_json("extra", "torso.obj", 0.8, 0.6)
            .replace("\"license\"", "\"surprise\":1,\"license\""),
        None,
    );
    let entries = avatar_catalog::discover(root.path());
    for (id, needle) in [
        ("escape", "bare .obj file name"),
        ("wrong-id", "must equal its directory name"),
        ("broken", "form.json"),
        ("tall-shoulder", "shoulder_height_m"),
        ("extra", "unknown field"),
    ] {
        let e = entry(&entries, id);
        assert!(!e.available, "{id}");
        assert!(
            e.reason.as_deref().unwrap().contains(needle),
            "{id}: {:?}",
            e.reason
        );
    }
}

#[test]
fn api_json_never_exposes_a_filesystem_path() {
    let root = tempfile::tempdir().unwrap();
    install(
        root.path(),
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        Some(&torso_obj(0.80, 0.0, 1.0)),
    );
    let text = serde_json::to_string(&avatar_catalog::discover(root.path())).unwrap();
    assert!(!text.contains(root.path().to_str().unwrap()), "{text}");
    assert!(!text.contains("\"mesh\""));
}

fn form_in(root: &Path) -> avatar_catalog::LoadedAvatar {
    install(
        root,
        "test-form",
        &form_json("test-form", "torso.obj", 0.80, 0.62),
        Some(&torso_obj(0.80, 0.0, 1.0)),
    );
    entry(&avatar_catalog::discover(root), "test-form")
        .load()
        .unwrap()
}

#[test]
fn the_preview_lifts_the_garment_onto_the_form_and_attaches_contact() {
    let root = tempfile::tempdir().unwrap();
    let form = form_in(root.path());
    let template = Selection::bundled("tshirt")
        .or_else(|_| Selection::bundled(marvelous_cloth::template_catalog::ENTRIES[0].id))
        .unwrap();
    let template = template.template().clone();
    let before = template.clone();

    let top = template
        .compile()
        .unwrap()
        .arrangement_positions
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut preview =
        Preview::from_template_with_avatar(&template, 1, Some(&form)).expect("preview on a form");
    let snapshot = preview.snapshot();
    assert_eq!(snapshot["avatar"], "test-form");
    assert!(
        snapshot["solver"]
            .as_str()
            .unwrap()
            .contains("avatar contact")
    );
    let lift = snapshot["garment_lift_m"].as_f64().unwrap();
    assert!(
        (lift - garment_lift(top, Some(0.62))).abs() < 1e-9,
        "lift {lift}"
    );
    // The garment's highest vertex now sits on the shoulder line.
    let new_top = snapshot["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p[1].as_f64().unwrap())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((new_top - 0.62).abs() < 1e-6, "top {new_top}");

    let (done, failure) = preview.step(2);
    assert_eq!((done, failure), (2, None));
    let stepped = preview.snapshot();
    assert!(
        stepped["positions"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p.as_array().unwrap())
            .all(|c| c.as_f64().unwrap().is_finite())
    );
    assert_eq!(
        serde_json::to_value(&template).unwrap(),
        serde_json::to_value(&before).unwrap(),
        "the document's template is never modified by the preview"
    );
}

#[test]
fn without_an_avatar_the_preview_is_unchanged() {
    let template = Selection::bundled(marvelous_cloth::template_catalog::ENTRIES[0].id)
        .unwrap()
        .template()
        .clone();
    let preview = Preview::from_template(&template, 1).unwrap();
    let snapshot = preview.snapshot();
    assert!(snapshot["avatar"].is_null());
    assert_eq!(snapshot["garment_lift_m"], 0.0);
    assert!(
        snapshot["solver"]
            .as_str()
            .unwrap()
            .contains("no avatar contact")
    );
}

#[test]
fn the_mannequin_leaves_the_garment_where_its_template_put_it() {
    let loaded = avatar_catalog::discover(Path::new("assets"))[0]
        .load()
        .unwrap();
    assert_eq!(loaded.shoulder_height_m, None);
    let template = Selection::bundled(marvelous_cloth::template_catalog::ENTRIES[0].id)
        .unwrap()
        .template()
        .clone();
    let preview = Preview::from_template_with_avatar(&template, 1, Some(&loaded)).unwrap();
    assert_eq!(preview.snapshot()["garment_lift_m"], 0.0);
}
