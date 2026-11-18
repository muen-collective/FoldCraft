//! A form read from an OBJ may declare separate parts (removable arms), landmark heights, the torso's start and preset lines
//! (the armhole seams from the cut). Synthetic meshes, so nothing third-party is needed. A second test checks the bundled
//! mannequin-geometry forms when their (ignored) OBJ files are installed, and says so when they are not.
use marvelous_cloth::avatar_catalog::{self, assets_root};
use marvelous_cloth::form_lines::LineKind;
use std::{fs, path::Path};

/// A closed elliptical tube about the vertical axis from y0 to y1, centred at (cx, cz), as OBJ text.
fn tube(cx: f64, cz: f64, rx: f64, rz: f64, y0: f64, y1: f64) -> String {
    let (rings, around) = (12usize, 24usize);
    let mut obj = String::new();
    for r in 0..=rings {
        let y = y0 + (y1 - y0) * r as f64 / rings as f64;
        for a in 0..around {
            let t = std::f64::consts::TAU * a as f64 / around as f64;
            obj += &format!(
                "v {:.6} {:.6} {:.6}\n",
                cx + rx * t.cos(),
                y,
                cz + rz * t.sin()
            );
        }
    }
    let (bottom, top) = ((rings + 1) * around + 1, (rings + 1) * around + 2);
    obj += &format!("v {cx} {y0} {cz}\nv {cx} {y1} {cz}\n");
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

fn install(root: &Path, id: &str, form: &str, arm: bool) {
    let dir = root.join("dressforms").join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("form.json"), form).unwrap();
    fs::write(dir.join("body.obj"), tube(0.0, 0.0, 0.16, 0.10, 0.0, 1.5)).unwrap();
    if arm {
        fs::write(
            dir.join("arm-left.obj"),
            tube(0.22, 0.0, 0.04, 0.04, 0.9, 1.4),
        )
        .unwrap();
    }
}

const FORM: &str = r#"{"id":"t","title":"Mesh form with parts","mesh":"body.obj","parts":{"arm_left":"arm-left.obj"},
"height_m":1.5,"shoulder_height_m":1.4,"dimensions_cm":{"bust":90.0},"source":"synthetic","license":"CC0-1.0",
"landmarks":{"bust":1.1,"waist":0.8,"hip":0.5,"underbust":1.03,"neck":1.35},"torso_from_m":0.3,
"lines":[{"name":"armhole_left","kind":"seam","closed":true,"points":[[0.16,1.0,0.0],[0.16,1.2,0.05],[0.16,1.2,-0.05]]}]}"#;

#[test]
fn a_mesh_form_loads_its_parts_landmarks_and_preset_lines() {
    let root = std::env::temp_dir().join(format!("mesh-parts-{}", std::process::id()));
    install(&root, "t", FORM, true);
    let all = avatar_catalog::discover(&root);
    let e = all.iter().find(|e| e.id == "t").expect("listed");
    assert!(e.available, "{:?}", e.reason);
    let a = e.load().expect("loads");
    assert_eq!(a.parts.keys().collect::<Vec<_>>(), ["arm_left"]);
    assert!(
        a.parts["arm_left"].vertices.iter().all(|v| v.x > 0.1),
        "the arm is a separate mesh, left is +x"
    );
    assert!(
        a.surface.vertices.iter().all(|v| v.x.abs() <= 0.1601),
        "the arm is not in the body mesh"
    );
    assert!((a.landmarks["bust"] - 1.1).abs() < 1e-12 && a.torso_from_m == 0.3);
    let lines = a.guide_lines();
    // The preset armhole replaces the computed oval of the same name, once.
    assert_eq!(lines.iter().filter(|l| l.name == "armhole_left").count(), 1);
    assert_eq!(
        lines
            .iter()
            .find(|l| l.name == "armhole_left")
            .unwrap()
            .points
            .len(),
        3
    );
    // The computed tape lines come from the declared landmarks.
    let tapes: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == LineKind::Tape)
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(tapes, ["bust", "underbust", "waist", "hip"]);
    // A side seam stops at the armpit: nothing above the bust line plus 2 cm.
    for l in lines.iter().filter(|l| l.name.starts_with("side_")) {
        assert!(
            l.points.iter().all(|p| p[1] <= 1.1 + 0.02 + 1e-9),
            "{} reaches the shoulder",
            l.name
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_part_makes_the_form_unavailable_and_says_which() {
    let root = std::env::temp_dir().join(format!("mesh-parts-missing-{}", std::process::id()));
    install(&root, "t", FORM, false);
    let all = avatar_catalog::discover(&root);
    let e = all.iter().find(|e| e.id == "t").unwrap();
    assert!(!e.available);
    assert!(
        e.reason.as_deref().unwrap().contains("arm-left.obj"),
        "{:?}",
        e.reason
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_part_name_or_file_that_could_escape_is_refused() {
    for bad in [
        r#""parts":{"../x":"arm-left.obj"}"#,
        r#""parts":{"arm":"../arm.obj"}"#,
        r#""parts":{"arm":"arm.txt"}"#,
    ] {
        let root = std::env::temp_dir().join(format!(
            "mesh-parts-bad-{}-{}",
            std::process::id(),
            bad.len()
        ));
        install(
            &root,
            "t",
            &FORM.replace(r#""parts":{"arm_left":"arm-left.obj"}"#, bad),
            true,
        );
        let all = avatar_catalog::discover(&root);
        let e = all
            .iter()
            .find(|e| e.id == "t")
            .expect("listed, with a reason");
        assert!(!e.available, "{bad}");
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn bundled_mannequin_geometry_forms_are_sized_to_their_row_and_graded_when_installed() {
    let entries = avatar_catalog::discover(&assets_root());
    // The women's forms: 4, 6 and 8 (8 is the BASE form, graded down by one and two steps), chart rows from The Shop Company (US 4/6/8 bust, waist, hips in cm).
    let rows = [
        ("4", 85.09, 63.5, 88.9),
        ("6", 87.63, 64.77, 90.17),
        ("8", 90.17, 67.31, 92.71),
    ];
    let mut neck_heights = Vec::new();
    for (size, bust, waist, hip) in rows {
        let id = format!("female-full-{size}");
        let Some(e) = entries.iter().find(|e| e.id == id) else {
            return eprintln!("{id} is not in this checkout");
        };
        if !e.available {
            return eprintln!(
                "{id} is listed but its OBJ files are not installed (they are not committed): {:?}",
                e.reason
            );
        }
        let a = e.load().expect("loads");
        let lo = a
            .surface
            .vertices
            .iter()
            .map(|v| v.y)
            .fold(f64::MAX, f64::min);
        assert!(lo.abs() < 0.01, "{id} stands on the floor: {lo}");
        assert_eq!(
            a.parts.keys().collect::<Vec<_>>(),
            ["arm_left", "arm_right"]
        );
        let lines = a.guide_lines();
        for name in [
            "armhole_left",
            "armhole_right",
            "centre_front",
            "bust",
            "waist",
            "hip",
        ] {
            assert!(
                lines.iter().any(|l| l.name == name),
                "{id}: missing line {name}"
            );
        }
        // Tape girths of the tape lines are the chart's to within 5 mm.
        for (name, want) in [("bust", bust), ("waist", waist), ("hip", hip)] {
            let l = lines.iter().find(|l| l.name == name).unwrap();
            let n = l.points.len();
            let tape: f64 = (0..n)
                .map(|i| {
                    let (p, q) = (l.points[i], l.points[(i + 1) % n]);
                    ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
                })
                .sum::<f64>()
                * 100.0;
            assert!(
                (tape - want).abs() < 0.5,
                "{id} {name}: tape {tape:.2} cm vs {want}"
            );
        }
        neck_heights.push(a.landmarks["neck"]);
    }
    // Graded vertically from the base form 8: about 1 cm per size at the high-point shoulder (the founder's approximation).
    assert!(
        ((neck_heights[1] - neck_heights[0]) - 0.01).abs() < 0.0015,
        "4 -> 6: {:?}",
        neck_heights
    );
    assert!(
        ((neck_heights[2] - neck_heights[1]) - 0.01).abs() < 0.0015,
        "6 -> 8: {:?}",
        neck_heights
    );
}
