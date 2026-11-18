//! The bundled parametric dress form: generated, built to a chart row, resizable, stand untouched.
use marvelous_cloth::avatar_catalog::{self, assets_root};
use marvelous_cloth::form_sizing::{Targets, girth_cm};
use std::collections::BTreeMap;
use std::fs;

const ID: &str = "dressform-parametric-v1";

fn load() -> avatar_catalog::LoadedAvatar {
    let entries = avatar_catalog::discover(&assets_root());
    let e = entries
        .iter()
        .find(|e| e.id == ID)
        .expect("bundled form is discovered");
    assert!(e.generated && e.available, "{:?}", e.reason);
    e.load().expect("loads")
}

fn at_hip_waist_bust(a: &avatar_catalog::LoadedAvatar) -> [f64; 3] {
    let s = a.sizer.as_ref().unwrap();
    let m = &s.base;
    [m.girth_cm["bust"], m.girth_cm["waist"], m.girth_cm["hip"]]
        .map(|g| g)
        .to_owned()
}

#[test]
fn it_is_built_to_the_default_chart_row() {
    let a = load();
    let [bust, waist, hip] = at_hip_waist_bust(&a);
    // US 8 / UK 12: bust 89.99, waist 67.49, hip 92.99, within a centimetre of the chart.
    assert!((bust - 89.99).abs() < 1.0, "bust {bust}");
    assert!((waist - 67.49).abs() < 1.0, "waist {waist}");
    assert!((hip - 92.99).abs() < 1.0, "hip {hip}");
}

#[test]
fn the_form_stands_on_the_floor_with_a_closed_surface() {
    let a = load();
    let ymin = a
        .surface
        .vertices
        .iter()
        .map(|v| v.y)
        .fold(f64::MAX, f64::min);
    assert!(ymin.abs() < 1e-9, "base at {ymin}");
    a.surface.validate().unwrap();
}

#[test]
fn resizing_moves_the_torso_and_never_the_stand() {
    let a = load();
    let before: Vec<_> = a
        .surface
        .vertices
        .iter()
        .filter(|v| v.y < 0.718)
        .cloned()
        .collect();
    let mut flat = BTreeMap::new();
    flat.insert("bust".to_owned(), 100.0);
    flat.insert("waist".to_owned(), 80.0);
    flat.insert("hip".to_owned(), 104.0);
    let (b, r) = a.sized(&Targets::from_flat(&flat).unwrap()).unwrap();
    let after: Vec<_> = b
        .surface
        .vertices
        .iter()
        .filter(|v| v.y < 0.718)
        .cloned()
        .collect();
    assert_eq!(before, after, "stand vertices must not move");
    assert!((r.measured.girth_cm["bust"] - 100.0).abs() < 1.0);
    assert!((girth_cm(&b.surface, r.knot_y["waist"]) - 80.0).abs() < 1.5);
}

fn write(dir: &std::path::Path, extra: &str) {
    let base =
        fs::read_to_string(assets_root().join("dressforms").join(ID).join("form.json")).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&base).unwrap();
    v["id"] = "t".into();
    let extra: serde_json::Value = serde_json::from_str(extra).unwrap();
    for (k, val) in extra.as_object().unwrap() {
        if val.is_null() {
            v.as_object_mut().unwrap().remove(k);
        } else {
            v[k] = val.clone();
        }
    }
    fs::create_dir_all(dir.join("dressforms/t")).unwrap();
    fs::write(dir.join("dressforms/t/form.json"), v.to_string()).unwrap();
}

#[test]
fn a_form_needs_exactly_one_of_mesh_or_generator() {
    let root = std::env::temp_dir().join(format!("gen-both-{}", std::process::id()));
    write(&root, r#"{"mesh":"torso.obj"}"#);
    let all = avatar_catalog::discover(&root);
    let t = all.iter().find(|e| e.id == "t").expect("listed");
    assert!(!t.available);
    assert!(
        t.reason
            .as_deref()
            .unwrap()
            .contains("either mesh or generator"),
        "{:?}",
        t.reason
    );
    write(&root, r#"{"generator":null}"#);
    let all = avatar_catalog::discover(&root);
    let t = all.iter().find(|e| e.id == "t").unwrap();
    assert!(!t.available, "neither is refused too");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_generator_that_disagrees_with_the_declared_height_is_refused() {
    let root = std::env::temp_dir().join(format!("gen-height-{}", std::process::id()));
    write(&root, r#"{"height_m":1.2}"#);
    let all = avatar_catalog::discover(&root);
    let t = all.iter().find(|e| e.id == "t").unwrap();
    assert!(!t.available);
    assert!(
        t.reason.as_deref().unwrap().contains("height_m"),
        "{:?}",
        t.reason
    );
    let _ = fs::remove_dir_all(&root);
}
