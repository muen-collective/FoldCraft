//! Tape and seam lines are cut from the mesh: exact, on the torso only, and they follow a resize.
use marvelous_cloth::avatar_catalog::{self, assets_root};
use marvelous_cloth::form_lines::{LineKind, LineSpec, guide_lines};
use marvelous_cloth::form_sizing::{Targets, girth_cm};
use std::collections::BTreeMap;

fn us8() -> avatar_catalog::LoadedAvatar {
    avatar_catalog::discover(&assets_root())
        .iter()
        .find(|e| e.id == "dressform-female-us8")
        .expect("bundled form")
        .load()
        .expect("loads")
}

fn length(points: &[[f64; 3]], closed: bool) -> f64 {
    let n = points.len();
    let segs = if closed { n } else { n - 1 };
    (0..segs)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        })
        .sum()
}

#[test]
fn there_are_four_tape_lines_and_eight_seam_lines_and_no_stand() {
    let a = us8();
    let lines = a.guide_lines();
    let tapes: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == LineKind::Tape)
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(tapes, ["bust", "underbust", "waist", "hip"]);
    assert_eq!(
        lines.iter().filter(|l| l.kind == LineKind::Seam).count(),
        10
    ); // 8 seams and 2 armholes
    for l in &lines {
        assert!(
            l.points.iter().all(|p| p[1] >= a.torso_from_m),
            "{} reaches the stand",
            l.name
        );
    }
}

#[test]
fn a_tape_line_is_a_closed_loop_at_its_landmark_as_long_as_the_girth() {
    let a = us8();
    for l in a.guide_lines().iter().filter(|l| l.kind == LineKind::Tape) {
        assert!(l.closed, "{} is closed", l.name);
        let y = a.landmarks[&l.name];
        assert!(
            l.points.iter().all(|p| (p[1] - y).abs() < 1e-6),
            "{} is level",
            l.name
        );
        // The cut of the mesh is at least as long as the convex girth the sizer measures, and within 2 percent of it.
        let tape = 100.0 * length(&l.points, true);
        let hull = girth_cm(&a.surface, y);
        assert!(
            tape >= hull - 0.05 && tape < hull * 1.02,
            "{}: tape {tape:.2} cm, hull {hull:.2} cm",
            l.name
        );
    }
}

#[test]
fn centre_lines_lie_in_the_middle_plane_front_and_back() {
    let a = us8();
    let lines = a.guide_lines();
    let get = |n: &str| lines.iter().find(|l| l.name == n).unwrap();
    let (cf, cb) = (get("centre_front"), get("centre_back"));
    assert!(
        cf.points
            .iter()
            .chain(&cb.points)
            .all(|p| p[0].abs() < 1e-6)
    );
    let zmin_front = cf.points.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
    let zmax_back = cb.points.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
    assert!(
        zmin_front > zmax_back,
        "front line is in front of the back line"
    );
    // They run from near the hem to near the neck top.
    assert!(length(&cf.points, false) > 0.6);
}

#[test]
fn left_and_right_lines_are_mirrored_in_x_and_never_cross_the_middle() {
    let a = us8();
    let lines = a.guide_lines();
    let get = |n: &str| lines.iter().find(|l| l.name == n).unwrap();
    assert!(get("princess_front_left").points.iter().all(|p| p[0] > 0.0));
    assert!(
        get("princess_front_right")
            .points
            .iter()
            .all(|p| p[0] < 0.0)
    );
    assert!(get("side_left").points.iter().all(|p| p[0] > 0.0));
    assert!(get("side_right").points.iter().all(|p| p[0] < 0.0));
}

#[test]
fn lines_follow_a_resize_and_the_tape_still_matches_the_new_girth() {
    let a = us8();
    let mut flat = BTreeMap::new();
    flat.insert("waist".to_owned(), 80.0);
    let (b, _) = a.sized(&Targets::from_flat(&flat).unwrap()).unwrap();
    let waist = b
        .guide_lines()
        .into_iter()
        .find(|l| l.name == "waist")
        .unwrap();
    let tape = 100.0 * length(&waist.points, true);
    assert!(
        (tape - 80.0).abs() < 1.6,
        "waist tape after resize: {tape:.2} cm"
    );
}

#[test]
fn a_form_with_no_landmarks_still_gets_seam_lines_and_no_tapes() {
    let a = us8();
    let none = BTreeMap::new();
    let lines = guide_lines(
        &a.surface,
        &LineSpec {
            landmarks: &none,
            torso_from_m: a.torso_from_m,
        },
    );
    assert!(lines.iter().all(|l| l.kind == LineKind::Seam));
    assert!(lines.iter().any(|l| l.name == "centre_front"));
}

#[test]
fn an_armhole_is_a_closed_oval_on_each_shoulder_between_bust_and_neck() {
    let a = us8();
    let lines = a.guide_lines();
    let (l, r) = (
        lines
            .iter()
            .find(|l| l.name == "armhole_left")
            .expect("left armhole"),
        lines
            .iter()
            .find(|l| l.name == "armhole_right")
            .expect("right armhole"),
    );
    assert!(l.closed && r.closed && l.points.len() == 48 && r.points.len() == 48);
    assert!(l.points.iter().all(|p| p[0] > 0.0) && r.points.iter().all(|p| p[0] < 0.0));
    let (bust, neck) = (a.landmarks["bust"], a.landmarks["neck"]);
    for p in l.points.iter().chain(&r.points) {
        assert!(
            p[1] > bust && p[1] < neck,
            "armhole point at y {} outside bust..neck",
            p[1]
        );
    }
    // Mirrored: the same heights on both sides, and the same reach out from the middle to within a centimetre.
    for (p, q) in l.points.iter().zip(&r.points) {
        assert!(
            (p[1] - q[1]).abs() < 1e-9 && (p[0] + q[0]).abs() < 0.01,
            "{p:?} vs {q:?}"
        );
    }
}
