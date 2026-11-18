//! The flat guide: developed (along the surface) half-widths, landmark lines and points. Synthetic mesh forms, so nothing third-party is
//! needed; a second test checks a bundled mannequin-geometry form when its OBJ files are installed.
use marvelous_cloth::avatar_catalog::{self, assets_root};
use marvelous_cloth::form_guide::flat_guide;
use std::fs;

fn tube(rx: f64, rz: f64, y0: f64, y1: f64) -> String {
    let (rings, around) = (40usize, 96usize);
    let mut obj = String::new();
    for r in 0..=rings {
        let y = y0 + (y1 - y0) * r as f64 / rings as f64;
        for a in 0..around {
            let t = std::f64::consts::TAU * a as f64 / around as f64;
            obj += &format!("v {:.6} {:.6} {:.6}\n", rx * t.cos(), y, rz * t.sin());
        }
    }
    let (bottom, top) = ((rings + 1) * around + 1, (rings + 1) * around + 2);
    obj += &format!("v 0 {y0} 0\nv 0 {y1} 0\n");
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

const FORM: &str = r#"{"id":"g","title":"Guide test","mesh":"body.obj","height_m":1.5,"shoulder_height_m":1.4,
"dimensions_cm":{"bust":90.0},"source":"synthetic","license":"CC0-1.0",
"landmarks":{"bust":1.1,"waist":0.8,"hip":0.5,"underbust":1.03,"neck":1.35},"torso_from_m":0.3}"#;

#[test]
fn the_guide_is_developed_so_its_half_width_is_a_quarter_of_the_girth() {
    let root = std::env::temp_dir().join(format!("guide-{}", std::process::id()));
    let dir = root.join("dressforms").join("g");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("form.json"), FORM).unwrap();
    fs::write(dir.join("body.obj"), tube(0.16, 0.10, 0.0, 1.5)).unwrap();
    let all = avatar_catalog::discover(&root);
    let a = all
        .iter()
        .find(|e| e.id == "g")
        .unwrap()
        .load()
        .expect("loads");
    let g = flat_guide(&a, 0.0).expect("a guide");
    // An ellipse 0.16 x 0.10: perimeter about 0.8 m, so each face is half of it and each half-width a quarter, about 0.2 m.
    let girth = 2.0 * std::f64::consts::PI * (((0.16f64).powi(2) + (0.10f64).powi(2)) / 2.0).sqrt();
    let bust = g
        .landmarks
        .iter()
        .find(|l| l.name == "bust")
        .expect("bust line");
    assert!(
        (bust.half_front - girth / 4.0).abs() < 0.01 * girth,
        "front {} vs quarter girth {}",
        bust.half_front,
        girth / 4.0
    );
    assert!(
        (bust.half_back - bust.half_front).abs() < 0.005,
        "a symmetric tube has equal faces"
    );
    // The developed half-width is wider than the plain silhouette half-width (0.16): that is the point.
    assert!(bust.half_front > 0.16 + 0.02);
    // Landmark lines are in the order and at the heights declared; the outline closes up and down.
    let names: Vec<_> = g.landmarks.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["neck", "bust", "underbust", "waist", "hip"]);
    assert!((bust.y - 1.1).abs() < 1e-12);
    let o = &g.front.outline;
    assert!(
        o.len() > 100 && o.first().unwrap()[1] > o[o.len() / 2 - 1][1] - 1e-9,
        "the left edge runs top to bottom"
    );
    assert!(o.iter().any(|p| p[0] < -0.19) && o.iter().any(|p| p[0] > 0.19));
    assert_eq!(g.front.centre[0][0], 0.0);
    // Points: the neck's centre, both HPS and both shoulder points, mirrored.
    let get = |n: &str| {
        g.points
            .iter()
            .find(|p| p.name == n)
            .unwrap_or_else(|| panic!("no point {n}"))
    };
    assert!((get("hps_left").x + get("hps_right").x).abs() < 1e-12);
    assert!((get("shoulder_point_left").x + get("shoulder_point_right").x).abs() < 1e-12);
    assert!(g.top_m > g.bottom_m);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_form_without_landmarks_has_no_guide() {
    let entries = avatar_catalog::discover(&assets_root());
    let e = entries
        .iter()
        .find(|e| e.id == avatar_catalog::MANNEQUIN_ID)
        .unwrap();
    if !e.available {
        return eprintln!("the bundled mannequin OBJ is not installed here");
    }
    assert!(flat_guide(&e.load().unwrap(), 0.0).is_none());
}

#[test]
fn the_us_8_form_has_a_guide_whose_half_widths_match_its_girths_when_installed() {
    let entries = avatar_catalog::discover(&assets_root());
    let Some(e) = entries.iter().find(|e| e.id == "female-full-8") else {
        return eprintln!("female-full-8 is not in this checkout");
    };
    if !e.available {
        return eprintln!(
            "female-full-8 is listed but its OBJ files are not installed: {:?}",
            e.reason
        );
    }
    let g = flat_guide(&e.load().unwrap(), 0.0).expect("a guide");
    // Each landmark's developed halves add up to a quarter of the girth each way: 2 x (front half + back half) is the girth, to 3 percent.
    for (name, girth) in [
        ("bust", 0.9017),
        ("waist", 0.6731),
        ("hip", 0.9271),
        ("neck", 0.3429),
    ] {
        let l = g.landmarks.iter().find(|l| l.name == name).unwrap();
        let total = 2.0 * (l.half_front + l.half_back);
        assert!(
            (total - girth).abs() < 0.03 * girth,
            "{name}: developed {total:.4} m vs girth {girth}"
        );
    }
    // Print for the record: the numbers a patternmaker compares a bodice with.
    for l in &g.landmarks {
        eprintln!(
            "{:10} y {:.3}  half front {:5.1} cm  half back {:5.1} cm",
            l.name,
            l.y,
            100.0 * l.half_front,
            100.0 * l.half_back
        );
    }
}

#[test]
fn a_full_body_form_has_developed_legs_crotch_knee_ankle_floor_and_arm_outlines_when_installed() {
    let entries = avatar_catalog::discover(&assets_root());
    let Some(e) = entries.iter().find(|e| e.id == "female-full-8") else {
        return eprintln!("female-full-8 is not in this checkout");
    };
    if !e.available {
        return eprintln!(
            "female-full-8 is listed but its OBJ files are not installed: {:?}",
            e.reason
        );
    }
    let a = e.load().unwrap();
    let g = flat_guide(&a, 0.0).expect("a guide");
    assert_eq!(g.legs.len(), 2);
    assert!(g.legs.iter().any(|l| l.side == "left") && g.legs.iter().any(|l| l.side == "right"));
    for l in &g.legs {
        assert!(
            l.front.len() > 40 && l.back.len() > 40,
            "{}: {} / {}",
            l.side,
            l.front.len(),
            l.back.len()
        );
    }
    // The lines a trouser is aligned to, in order down the body.
    let y = |n: &str| {
        g.landmarks
            .iter()
            .find(|l| l.name == n)
            .unwrap_or_else(|| panic!("no landmark {n}"))
            .y
    };
    assert!(
        y("hip") > y("crotch")
            && y("crotch") > y("knee")
            && y("knee") > y("ankle")
            && y("ankle") > y("floor")
    );
    assert_eq!(y("floor"), 0.0);
    // The knee is where a knee is (about half the crotch height) and the ankle low; the knee's two faces add up to its girth, 28-40 cm.
    assert!((0.35..0.55).contains(&y("knee")), "knee at {}", y("knee"));
    assert!(y("ankle") < 0.25, "ankle at {}", y("ankle"));
    let knee = g.landmarks.iter().find(|l| l.name == "knee").unwrap();
    let girth = knee.leg_front.unwrap() + knee.leg_back.unwrap();
    assert!((0.28..0.40).contains(&girth), "knee girth {girth}");
    assert_eq!(g.arms.len(), 2);
    for arm in &g.arms {
        assert!(arm.outline.len() >= 8, "{} outline", arm.name);
    }
    for l in &g.landmarks {
        eprintln!(
            "{:10} y {:.3}  {:?}",
            l.name,
            l.y,
            l.leg_front.map(|f| (f * 100.0).round() / 100.0)
        );
    }
}

#[test]
fn moving_the_side_seam_back_widens_the_front_and_narrows_the_back_without_changing_the_girth() {
    let entries = avatar_catalog::discover(&assets_root());
    let Some(e) = entries.iter().find(|e| e.id == "female-full-8") else {
        return eprintln!("female-full-8 is not in this checkout");
    };
    if !e.available {
        return eprintln!(
            "female-full-8 is listed but its OBJ files are not installed: {:?}",
            e.reason
        );
    }
    let a = e.load().unwrap();
    let bust = |off: f64| {
        let g = flat_guide(&a, off).unwrap();
        let l = g.landmarks.iter().find(|l| l.name == "bust").unwrap();
        (l.half_front, l.half_back)
    };
    let (f0, b0) = bust(0.0);
    let (f1, b1) = bust(0.03);
    assert!(f1 > f0 && b1 < b0, "front {f0} -> {f1}, back {b0} -> {b1}");
    assert!(
        ((f0 + b0) - (f1 + b1)).abs() < 0.004,
        "the girth is the same: {} vs {}",
        f0 + b0,
        f1 + b1
    );
    for off in [0.0, 0.01, 0.02, 0.03, 0.04, 0.05] {
        let (f, b) = bust(off);
        eprintln!(
            "side seam {:+.0} mm: bust front {:.1} cm  back {:.1} cm  ({:.0} percent front)",
            off * 1000.0,
            f * 100.0,
            b * 100.0,
            100.0 * f / (f + b)
        );
    }
}
