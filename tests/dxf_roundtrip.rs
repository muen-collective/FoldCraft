//! Flat DXF export: what we write, our own reader must read back.
use marvelous_cloth::garment::dxf_import::{self, Options};
use marvelous_cloth::garment::GarmentTemplate;

fn template() -> GarmentTemplate {
    let json = include_str!("../examples/templates/tshirt.json");
    serde_json::from_str(json).expect("bundled t-shirt parses")
}

#[test]
fn exported_dxf_reads_back_every_named_piece() {
    let source = template();
    let text = marvelous_cloth::garment::export::dxf::flat_dxf(&source);
    let drawing = dxf_import::read_ascii(
        &text,
        Options {
            metres_per_unit: None, // the file declares millimetres itself
            ..Options::default()
        },
    )
    .expect("our own export parses");
    let expected: std::collections::BTreeSet<String> =
        source.panels.iter().map(|panel| panel.name.clone()).collect();
    let pieces: std::collections::BTreeSet<String> = drawing
        .paths
        .iter()
        .filter(|path| path.closed)
        .filter_map(|path| path.piece.clone())
        .collect();
    assert_eq!(pieces, expected, "every panel name must survive the round trip");
    assert_eq!(drawing.paths.iter().filter(|p| p.closed).count(), 2);
    assert_eq!(drawing.insunits, Some(4), "millimetres declared");
}

#[test]
fn circular_arcs_survive_as_bulges_not_samples() {
    // One edge whose stored curve is a circular arc: control points on the
    // tangents at radius/2 (exactly a 90-degree circular arc's cubic Bezier).
    // The export recognises it and writes one DXF bulge segment; the reader
    // returns a compressed contour. Freeform edges are NOT claimed as arcs -
    // see dxf.rs docs for that boundary.
    let json = include_str!("../examples/templates/two-panel.json");
    let mut source: GarmentTemplate = serde_json::from_str(json).expect("fixture parses");
    let r = 0.05f64;
    let k = r / 2.; // tangent handles at half the radius => true quarter circle
    let points = vec![
        glam::DVec2::new(r, 0.),
        glam::DVec2::new(0., r),
        glam::DVec2::new(-r, 0.),
    ];
    let curves: [(&str, [glam::DVec2; 2]); 2] = [
        ("arc-1", [glam::DVec2::new(r, k), glam::DVec2::new(k, r)]),
        ("arc-2", [glam::DVec2::new(-k, r), glam::DVec2::new(-r, k)]),
    ];
    let polygon = marvelous_cloth::garment::polygon::PolygonPattern {
        points,
        edges: vec!["arc-1".into(), "arc-2".into(), "chord".into()],
        subdivisions: 0,
        meshing: None,
        curves: curves.map(|(name, pair)| (name.to_owned(), pair)).into_iter().collect(),
        holes: vec![],
        lines: vec![],
        target_lengths: Default::default(),
        seam_allowances: Default::default(),
        notches: Default::default(),
        measurements: Vec::new(),
    };
    source.panels[0].polygon = Some(polygon);

    let text = marvelous_cloth::garment::export::dxf::flat_dxf(&source);
    assert!(
        text.lines().filter(|line| line.trim() == "42").count() >= 2,
        "both arc edges must be written as bulged segments"
    );
    let drawing = dxf_import::read_ascii(&text, Options::default()).expect("parses");
    let written = &drawing.paths[0];
    // The chord keeps its two corners; each arc arrives compressed to its
    // endpoints plus samples. A faithful round trip stays well under the
    // dense polyline a naive writer would emit.
    assert!(
        written.points.len() <= 30,
        "expected an arc-compressed contour, got {} points",
        written.points.len()
    );
}
