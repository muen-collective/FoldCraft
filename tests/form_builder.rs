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
    assert!(refused(&|d| d.stand.pole_radius_m = 0.2).contains("thinner"));
    assert!(refused(&|d| d.stand.base_height_m = 0.9).contains("reaches the torso"));
}

#[test]
fn the_same_profile_builds_the_same_mesh() {
    let (a, b) = (decl().build().unwrap(), decl().build().unwrap());
    assert_eq!(a.triangles, b.triangles);
    assert!(a.vertices.iter().zip(&b.vertices).all(|(p, q)| p == q));
}
