//! The standard dress forms without legs: men's are generated, so they are checked here; the women's are cut from a
//! mesh that is not committed, so they are checked only when installed.
use marvelous_cloth::avatar::MeshCollider;
use marvelous_cloth::avatar_catalog::{MANNEQUIN_ID, discover};
use std::path::Path;
use std::sync::Arc;

#[test]
fn generated_torso_forms_have_a_stand_and_no_legs() {
    let entries = discover(Path::new("assets"));
    let torsos: Vec<_> = entries
        .iter()
        .filter(|e| e.id.starts_with("male-torso-"))
        .collect();
    assert_eq!(torsos.len(), 10, "men's 34..52");
    for e in torsos {
        let a = e.load().unwrap_or_else(|err| panic!("{}: {err}", e.id));
        let hem = e.generator.as_ref().unwrap().hem();
        // Between the base and the hem there is only the pole: nothing wider than 3 cm, so no leg.
        let widest = a
            .surface
            .vertices
            .iter()
            .filter(|p| p.y > 0.03 && p.y < hem - 0.02)
            .map(|p| p.x.abs().max(p.z.abs()))
            .fold(0.0, f64::max);
        assert!(
            widest < 0.03,
            "{}: something wider than the pole below the hem ({widest} m)",
            e.id
        );
        assert!(
            a.parts.contains_key("arm_left") && a.parts.contains_key("arm_right"),
            "{}: arms are kept",
            e.id
        );
        assert!(
            MeshCollider::with_clearance(Arc::clone(&a.surface), 500_000.0, 0.0).is_ok(),
            "{}",
            e.id
        );
        let full = entries
            .iter()
            .find(|f| f.id == e.id.replace("torso", "full"))
            .unwrap();
        assert_eq!(
            e.height_m, full.height_m,
            "{}: same stature as its full-body form",
            e.id
        );
    }
    assert!(entries.iter().any(|e| e.id == MANNEQUIN_ID));
}

#[test]
fn installed_womens_torso_forms_load() {
    let entries = discover(Path::new("assets"));
    for e in entries
        .iter()
        .filter(|e| e.id.starts_with("female-torso-") && e.available)
    {
        let a = e.load().unwrap_or_else(|err| panic!("{}: {err}", e.id));
        assert!(
            a.landmarks.get("crotch").is_none(),
            "{}: no crotch landmark without legs",
            e.id
        );
        assert!(
            MeshCollider::with_clearance(Arc::clone(&a.surface), 500_000.0, 0.0).is_ok(),
            "{}",
            e.id
        );
    }
}
