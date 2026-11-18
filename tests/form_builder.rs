//! Making a dress form from a profile: the surface is closed and consistently wound, stands on the
//! floor, has the size the profile says, and a bad profile is refused with the reason.
use marvelous_cloth::form_builder::{GeneratorDecl, signed_volume};
use std::collections::HashMap;

fn decl() -> GeneratorDecl {
    // A small torso: wide hip, narrow waist, fuller bust, shoulders, a neck. 0.60 m from flat bottom to neck top.
    let rows = [
        (0.00, 0.16, 0.09, -0.09, 2.3),
        (0.10, 0.165, 0.10, -0.12, 2.4),
        (0.22, 0.115, 0.095, -0.09, 2.3),
        (0.34, 0.16, 0.11, -0.11, 2.4),
        (0.44, 0.18, 0.05, -0.11, 3.0),
        (0.52, 0.06, 0.05, -0.05, 2.0),
        (0.60, 0.04, 0.03, -0.03, 2.0),
    ];
    let profile: Vec<_> = rows
        .iter()
        .map(|(y, w, f, b, n)| {
            format!(r#"{{"y_m":{y},"half_width_m":{w},"front_m":{f},"back_m":{b},"exponent":{n}}}"#)
        })
        .collect();
    serde_json::from_str(&format!(
        r#"{{"profile":[{}],"stand":{{"hem_height_m":0.70,"pole_radius_m":0.012,"base_radius_m":0.14,"base_height_m":0.02}},
"basis":"synthetic test profile"}}"#,
        profile.join(",")
    ))
    .unwrap()
}

#[test]
fn the_surface_is_closed_and_every_edge_is_wound_consistently() {
    let mesh = decl().build().unwrap();
    // Closed and consistently oriented: each directed edge appears exactly once and so does its reverse.
    let mut directed: HashMap<(usize, usize), usize> = HashMap::new();
    for t in &mesh.triangles {
        for k in 0..3 {
            *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
        }
    }
    for (&(a, b), &n) in &directed {
        assert_eq!(n, 1, "edge {a}->{b} is used {n} times in one direction");
        assert_eq!(
            directed.get(&(b, a)).copied(),
            Some(1),
            "edge {a}->{b} has no matching reverse"
        );
    }
}

#[test]
fn the_faces_wind_outward() {
    let mesh = decl().build().unwrap();
    assert!(signed_volume(&mesh) > 0.0);
    // A closed torso of this size encloses a few tens of litres.
    let litres = signed_volume(&mesh) * 1000.0;
    assert!((10.0..80.0).contains(&litres), "{litres} litres");
}

#[test]
fn the_form_stands_on_the_floor_and_is_as_tall_as_it_says() {
    let d = decl();
    let mesh = d.build().unwrap();
    let low = mesh
        .vertices
        .iter()
        .map(|v| v.y)
        .fold(f64::INFINITY, f64::min);
    let high = mesh
        .vertices
        .iter()
        .map(|v| v.y)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(low.abs() < 1e-12, "{low}");
    assert!((high - d.height_m()).abs() < 1e-12 && (d.height_m() - 1.30).abs() < 1e-12);
}

#[test]
fn the_profile_sets_the_width_front_and_back_of_each_section() {
    let d = decl();
    let mesh = d.build().unwrap();
    // Row 3 is the bust level: 0.34 above the flat bottom, half width 0.16, 0.11 to the front, 0.11 behind.
    let y = 0.70 + 0.34;
    let slab: Vec<_> = mesh
        .vertices
        .iter()
        .filter(|v| (v.y - y).abs() < 1e-9)
        .collect();
    assert!(slab.len() >= 32);
    let x = slab.iter().map(|v| v.x.abs()).fold(0.0, f64::max);
    let front = slab.iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
    let back = slab.iter().map(|v| v.z).fold(f64::INFINITY, f64::min);
    assert!(
        (x - 0.16).abs() < 1e-9 && (front - 0.11).abs() < 1e-9 && (back + 0.11).abs() < 1e-9,
        "{x} {front} {back}"
    );
}

#[test]
fn the_stand_is_a_pole_and_a_base_apart_from_the_torso() {
    let d = decl();
    let mesh = d.build().unwrap();
    let below: Vec<_> = mesh.vertices.iter().filter(|v| v.y < 0.70 - 1e-9).collect();
    assert!(!below.is_empty());
    let reach = below.iter().map(|v| v.x.hypot(v.z)).fold(0.0, f64::max);
    assert!(
        (reach - 0.14).abs() < 1e-9,
        "the base is 14 cm in radius, not {reach}"
    );
    let pole_top = below.iter().map(|v| v.y).fold(f64::NEG_INFINITY, f64::max);
    assert!(
        pole_top < 0.70 - 0.003,
        "the pole stops short of the torso: {pole_top}"
    );
}

#[test]
fn a_bad_profile_is_refused_with_the_reason() {
    let refused = |edit: &dyn Fn(&mut GeneratorDecl)| {
        let mut d = decl();
        edit(&mut d);
        d.build().err().expect("refused")
    };
    assert!(refused(&|d| d.basis = " ".into()).contains("basis"));
    assert!(refused(&|d| d.profile.truncate(3)).contains("at least 4"));
    assert!(refused(&|d| d.profile[1].y_m = 0.0).contains("rise"));
    assert!(refused(&|d| d.profile[0].y_m = 0.01).contains("start at 0"));
    assert!(refused(&|d| d.profile[2].back_m = 0.05).contains("back_m below 0"));
    assert!(refused(&|d| d.profile[2].half_width_m = 0.005).contains("1 cm"));
    assert!(refused(&|d| d.profile[2].exponent = 9.0).contains("exponent"));
    assert!(refused(&|d| d.profile[2].half_width_m = f64::NAN).contains("not a number"));
    assert!(refused(&|d| d.sides = Some(15)).contains("even"));
    assert!(refused(&|d| d.stand.as_mut().unwrap().pole_radius_m = 0.2).contains("thinner"));
    assert!(
        refused(&|d| d.stand.as_mut().unwrap().base_height_m = 0.9).contains("reaches the torso")
    );
}

#[test]
fn the_same_profile_builds_the_same_mesh() {
    let (a, b) = (decl().build().unwrap(), decl().build().unwrap());
    assert_eq!(a.triangles, b.triangles);
    assert!(a.vertices.iter().zip(&b.vertices).all(|(p, q)| p == q));
}

#[test]
fn a_tilted_top_row_is_cut_square_to_a_leaning_neck_and_stays_closed() {
    let level = decl().build().unwrap();
    let top = |m: &marvelous_cloth::avatar::SurfaceMesh| {
        m.vertices.iter().map(|v| v.y).fold(f64::MIN, f64::max)
    };
    let mut d = decl();
    d.profile.last_mut().unwrap().tilt = -0.14; // front lower than back, as a neck leaning forward 8 degrees
    let tilted = d.build().unwrap();
    // Level: every top vertex is at one height. Tilted: the back of the cut is higher than the front by tilt x depth.
    let top_y = top(&level);
    assert_eq!(
        level
            .vertices
            .iter()
            .filter(|v| (v.y - top_y).abs() < 1e-12)
            .count(),
        65
    );
    let row_top: Vec<_> = tilted
        .vertices
        .iter()
        .filter(|v| v.y > top_y - 0.02 && v.y < top_y + 0.02)
        .collect();
    let (front, back) = (
        row_top.iter().max_by(|a, b| a.z.total_cmp(&b.z)).unwrap(),
        row_top.iter().min_by(|a, b| a.z.total_cmp(&b.z)).unwrap(),
    );
    let depth = front.z - back.z; // 0.06 m
    assert!(
        ((back.y - front.y) - 0.14 * depth).abs() < 1e-9,
        "back {} front {}",
        back.y,
        front.y
    );
    // Still closed and outward after tilting.
    assert!(signed_volume(&tilted) > 0.0);
    tilted.validate().unwrap();
    // A tilt that would fold the surface is refused.
    let mut d = decl();
    d.profile[3].tilt = 0.9;
    assert!(d.build().err().unwrap().contains("tilt"));
}

// ---- full body: legs joined to the body, arms as separate meshes, no stand --------------------------------------

fn full_body() -> GeneratorDecl {
    let mut d = decl();
    d.stand = None;
    d.hem_height_m = Some(0.70);
    let leg = |name: &str, part: Option<&str>, rows: &[([f64; 3], f64, f64)]| {
        serde_json::from_value::<marvelous_cloth::form_builder::LimbDecl>(serde_json::json!({
            "name": name, "part": part, "mirror": true, "level": part.is_none(),
            "rows": rows.iter().map(|(p, rx, rz)| serde_json::json!({"p": p, "rx": rx, "rz": rz})).collect::<Vec<_>>()
        }))
        .unwrap()
    };
    d.limbs = vec![
        // A leg from inside the torso's bottom down to the floor: thigh, knee, calf, ankle.
        leg(
            "leg",
            None,
            &[
                ([0.09, 0.73, 0.0], 0.075, 0.08),
                ([0.09, 0.62, 0.0], 0.08, 0.085),
                ([0.085, 0.36, 0.005], 0.055, 0.055),
                ([0.08, 0.20, -0.005], 0.05, 0.05),
                ([0.075, 0.00, 0.0], 0.035, 0.035),
            ],
        ),
        // An arm hung from the shoulder, a little away from the body.
        leg(
            "arm",
            Some("arm"),
            &[
                ([0.17, 1.28, 0.0], 0.045, 0.045),
                ([0.20, 1.00, 0.0], 0.04, 0.04),
                ([0.22, 0.75, 0.0], 0.025, 0.025),
            ],
        ),
    ];
    d
}

#[test]
fn a_full_body_form_stands_on_its_legs_with_no_stand_and_stays_closed() {
    let d = full_body();
    let mesh = d.build().unwrap();
    let ymin = mesh.vertices.iter().map(|v| v.y).fold(f64::MAX, f64::min);
    assert!(
        ymin.abs() < 1e-9,
        "the legs reach the floor: lowest point {ymin}"
    );
    assert!(signed_volume(&mesh) > 0.0);
    // Every piece (torso, two legs) is closed and wound outward, so every directed edge is used once with its reverse.
    let mut directed: HashMap<(usize, usize), usize> = HashMap::new();
    for t in &mesh.triangles {
        for k in 0..3 {
            *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
        }
    }
    assert!(
        directed
            .iter()
            .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
    );
    // There is no pole: below the torso's bottom the only geometry is the two legs, one each side of the middle.
    assert!(
        mesh.vertices
            .iter()
            .filter(|v| v.y < 0.6)
            .all(|v| v.x.abs() > 0.005),
        "something stands on the middle"
    );
    // The height the form declares is the torso's top, not the arms.
    assert!((d.height_m() - 1.30).abs() < 1e-9);
}

#[test]
fn arms_are_separate_closed_meshes_mirrored_and_not_in_the_body() {
    let d = full_body();
    let body = d.build().unwrap();
    let parts = d.build_parts().unwrap();
    let names: Vec<_> = parts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["arm_left", "arm_right"]);
    let (left, right) = (&parts[0].1, &parts[1].1);
    assert!(signed_volume(left) > 0.0 && signed_volume(right) > 0.0);
    assert!(
        left.vertices.iter().all(|v| v.x > 0.0) && right.vertices.iter().all(|v| v.x < 0.0),
        "left is +x"
    );
    // Mirror images: the same heights.
    let top = |m: &marvelous_cloth::avatar::SurfaceMesh| {
        m.vertices.iter().map(|v| v.y).fold(f64::MIN, f64::max)
    };
    assert!((top(left) - top(right)).abs() < 1e-12);
    // Nothing of the arms is in the body mesh: a vertex at the wrist's height and reach is not there.
    assert!(
        body.vertices
            .iter()
            .all(|v| !(v.x.abs() > 0.2 && v.y > 0.7 && v.y < 1.3))
    );
}

#[test]
fn a_stand_and_a_hem_height_together_or_neither_are_refused_and_bad_limbs_are_refused() {
    let mut d = full_body();
    d.hem_height_m = None;
    assert!(
        d.build()
            .err()
            .unwrap()
            .contains("either a stand or hem_height_m")
    );
    let mut d = full_body();
    d.stand = decl().stand;
    assert!(
        d.build()
            .err()
            .unwrap()
            .contains("either a stand or hem_height_m")
    );
    let mut d = full_body();
    d.limbs[0].rows.truncate(2);
    assert!(d.build().err().unwrap().contains("at least 3 rows"));
    let mut d = full_body();
    d.limbs[0].rows[1].rx = 0.001;
    assert!(d.build().err().unwrap().contains("1 cm"));
    let mut d = full_body();
    d.limbs[0].rows[1].p = d.limbs[0].rows[0].p;
    assert!(d.build().err().unwrap().contains("5 mm apart"));
}
