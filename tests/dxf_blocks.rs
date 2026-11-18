//! Block expansion: the two DrapeXD exports keep their geometry inside BLOCKs.
use marvelous_cloth::garment::dxf_import::{self, Options};

fn read(text: &str) -> dxf_import::Drawing {
    dxf_import::read_ascii(
        text,
        Options {
            metres_per_unit: Some(0.001),
            ..Options::default()
        },
    )
    .expect("fixture parses")
}

fn insert_issues(drawing: &dxf_import::Drawing) -> usize {
    drawing
        .issues
        .iter()
        .filter(|issue| issue.entity == "INSERT")
        .count()
}

#[test]
fn drapexd_block_export_yields_its_pieces() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-block.dxf"));
    assert_eq!(insert_issues(&drawing), 0, "block instances must expand");
    assert!(drawing.paths.len() >= 4, "paths: {}", drawing.paths.len());
    assert!(
        drawing.paths.iter().filter(|p| p.closed).count() >= 4,
        "closed: {}",
        drawing.paths.iter().filter(|p| p.closed).count()
    );
    // Four named pieces in millimetres: the drawing must span garment scale.
    let mut min = glam::DVec2::splat(f64::INFINITY);
    let mut max = glam::DVec2::splat(f64::NEG_INFINITY);
    for path in &drawing.paths {
        for p in &path.points {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    let span = max - min;
    assert!(
        (0.1..3.0).contains(&span.x) && (0.1..3.0).contains(&span.y),
        "span {:?}",
        span
    );
}

#[test]
fn bodice_and_sleeve_export_expands_and_stays_metric() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-bodice-sleeve.dxf"));
    assert_eq!(insert_issues(&drawing), 0);
    assert!(drawing.paths.iter().filter(|p| p.closed).count() >= 4);
    assert!(drawing.metres_per_unit == 0.001);
}

#[test]
fn layer_inheritance_follows_the_insert() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-block.dxf"));
    let layers: std::collections::BTreeSet<&str> =
        drawing.paths.iter().map(|p| p.layer.as_str()).collect();
    assert!(!layers.contains("0"), "block contents inherit a real layer: {layers:?}");
}

#[test]
fn piece_names_attach_to_their_block() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-bodice-sleeve.dxf"));
    let named: std::collections::BTreeSet<String> = drawing
        .paths
        .iter()
        .filter(|path| path.closed)
        .filter_map(|path| path.piece.clone())
        .collect();
    for expected in ["Bodice front", "Bodice back", "Sleeve", "Sleeve (mirror)"] {
        assert!(named.contains(expected), "missing piece name in {named:?}");
    }
    // Record lines (PIECE NAME / UNITS) are metadata; they must not appear
    // among unsupported-entity issues. Label texts ("10") legitimately do.
    let record_issues: Vec<_> = drawing
        .issues
        .iter()
        .filter(|i| i.entity == "TEXT" && i.reason.contains("NAME"))
        .collect();
    assert!(record_issues.is_empty(), "record lines leaked as issues");
}

#[test]
fn block_export_carries_its_piece_names() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-block.dxf"));
    let named: std::collections::BTreeSet<String> =
        drawing.paths.iter().filter_map(|path| path.piece.clone()).collect();
    assert!(named.contains("Skirt front"), "{named:?}");
    assert!(named.contains("Skirt back"), "{named:?}");
}

#[test]
fn units_record_survives_without_a_sample_size() {
    let drawing = read(include_str!("fixtures/dxf/drapexd-bodice-sleeve.dxf"));
    assert_eq!(drawing.unit_record.as_deref(), Some("METRIC"));
    assert_eq!(drawing.sample_size, None);
}
