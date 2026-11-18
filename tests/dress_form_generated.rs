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

// ---- dressform-female-us8: the shaped one -------------------------------------------------------------------

const US8: &str = "dressform-female-us8";

fn load_us8() -> avatar_catalog::LoadedAvatar {
    let entries = avatar_catalog::discover(&assets_root());
    let e = entries
        .iter()
        .find(|e| e.id == US8)
        .expect("bundled form is discovered");
    assert!(e.generated && e.available, "{:?}", e.reason);
    e.load().expect("loads")
}

#[test]
fn us8_is_built_to_its_chart_row_within_a_millimetre() {
    let a = load_us8();
    let m = &a.sizer.as_ref().unwrap().base.girth_cm;
    // The Shop Company's women's US 8: bust 35.5, waist 26.5, hips 36.5, neck 13.5 in. It publishes no underbust.
    for (key, want) in [
        ("bust", 90.17),
        ("waist", 67.31),
        ("hip", 92.71),
        ("neck", 34.29),
    ] {
        assert!((m[key] - want).abs() < 0.1, "{key}: {} vs {want}", m[key]);
    }
}

#[test]
fn us8_keeps_a_dense_shoulder_and_a_tapering_neck() {
    let entries = avatar_catalog::discover(&assets_root());
    let g = entries
        .iter()
        .find(|e| e.id == US8)
        .unwrap()
        .generator
        .as_ref()
        .unwrap();
    // The shoulder is its own run of rows, 1 cm apart or closer, so it reads as a sloped plane.
    let shoulder: Vec<_> = g
        .profile
        .iter()
        .filter(|r| (0.68..=0.73).contains(&r.y_m))
        .collect();
    assert!(shoulder.len() >= 6, "{} shoulder rows", shoulder.len());
    for w in shoulder.windows(2) {
        assert!(
            w[1].y_m - w[0].y_m <= 0.0101,
            "gap {} -> {}",
            w[0].y_m,
            w[1].y_m
        );
    }
    // No row anywhere is more than 5 cm from its neighbour: no banding.
    for w in g.profile.windows(2) {
        assert!(
            w[1].y_m - w[0].y_m <= 0.0501,
            "banding risk between {} and {}",
            w[0].y_m,
            w[1].y_m
        );
    }
    // The neck is a cone: it narrows going up, by less than a third.
    let neck: Vec<_> = g.profile.iter().filter(|r| r.y_m >= 0.74).collect();
    assert!(neck.len() >= 4);
    assert!(
        neck.windows(2)
            .all(|w| w[1].half_width_m <= w[0].half_width_m + 1e-9),
        "the neck never widens going up"
    );
    assert!(neck.last().unwrap().half_width_m > 0.8 * neck[0].half_width_m);
}

#[test]
fn us8_front_of_neck_rises_out_of_the_chest_without_a_step() {
    let entries = avatar_catalog::discover(&assets_root());
    let g = entries
        .iter()
        .find(|e| e.id == US8)
        .unwrap()
        .generator
        .as_ref()
        .unwrap();
    let front = |y: f64| {
        g.profile
            .iter()
            .find(|r| (r.y_m - y).abs() < 1e-9)
            .unwrap()
            .front_m
    };
    // On the earlier neck the front stood 1.9 cm further forward at 0.72 m than at 0.68 m: an overhang at the neck base.
    // Now the front line moves by under 1 cm from the chest's low point to the neck base, and never steps.
    assert!(
        front(0.72) - front(0.68) < 0.01,
        "front steps forward: {} -> {}",
        front(0.68),
        front(0.72)
    );
    let neck: Vec<_> = g.profile.iter().filter(|r| r.y_m >= 0.70).collect();
    for w in neck.windows(2) {
        assert!(
            w[1].front_m - w[0].front_m < 0.006,
            "front jumps {} -> {} at {}",
            w[0].front_m,
            w[1].front_m,
            w[1].y_m
        );
    }
}

#[test]
fn us8_neck_top_is_one_plane_cut_square_to_the_leaning_neck() {
    let a = load_us8();
    let top = a
        .surface
        .vertices
        .iter()
        .map(|v| v.y)
        .fold(f64::MIN, f64::max);
    // The vertices of the top ring and its cap centre (the ring is within 2 cm of the top; the stand is far below).
    let ring: Vec<_> = a
        .surface
        .vertices
        .iter()
        .filter(|v| v.y > top - 0.02)
        .collect();
    assert!(ring.len() > 90, "{} top vertices", ring.len());
    let (front, back) = (
        ring.iter().max_by(|p, q| p.z.total_cmp(&q.z)).unwrap(),
        ring.iter().min_by(|p, q| p.z.total_cmp(&q.z)).unwrap(),
    );
    let slope = (back.y - front.y) / (front.z - back.z);
    assert!(
        (slope - 8f64.to_radians().tan()).abs() < 0.01,
        "top slope {slope}, expected tan 8 degrees"
    );
    assert!(
        front.y < back.y,
        "the front of the cut is lower than the back"
    );
    // One plane: every top-ring vertex lies on the line through front and back (the plane is y = y0 - slope z).
    let y0 = back.y + slope * back.z;
    let on_plane = ring
        .iter()
        .filter(|v| (v.y - (y0 - slope * v.z)).abs() < 1e-9)
        .count();
    assert!(
        on_plane > 90,
        "only {on_plane} top vertices lie on the cut plane"
    );
}

#[test]
fn us8_resizing_never_moves_the_stand() {
    let a = load_us8();
    let stand = |s: &marvelous_cloth::avatar::SurfaceMesh| -> Vec<_> {
        s.vertices.iter().filter(|v| v.y < 0.718).cloned().collect()
    };
    let before = stand(&a.surface);
    assert!(before.len() > 100);
    let mut flat = BTreeMap::new();
    flat.insert("bust".to_owned(), 96.0);
    flat.insert("waist".to_owned(), 74.0);
    flat.insert("hip".to_owned(), 100.0);
    let (b, r) = a.sized(&Targets::from_flat(&flat).unwrap()).unwrap();
    assert_eq!(before, stand(&b.surface), "stand vertices must not move");
    assert!((r.measured.girth_cm["bust"] - 96.0).abs() < 0.1);
}
