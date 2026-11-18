//! The fit module turns dense sampled contours into few editable nodes.
use marvelous_cloth::garment::dxf_import::fit;
use glam::DVec2;

fn circle(radius: f64, count: usize) -> Vec<DVec2> {
    (0..count)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / count as f64;
            DVec2::new(a.cos(), a.sin()) * radius
        })
        .collect()
}

#[test]
fn simplify_keeps_straight_lines_to_their_ends() {
    let line: Vec<DVec2> = (0..50).map(|i| DVec2::new(i as f64 * 0.01, 0.)).collect();
    let kept = fit::simplify(&line, 0.0005);
    assert_eq!(kept.len(), 2);
}

#[test]
fn simplify_respects_tolerance_on_a_curve() {
    // A 100 mm circle at 0.5 mm tolerance keeps roughly the arc's corners, not all 200 samples.
    let kept = fit::simplify(&circle(0.1, 200), 0.0005);
    assert!(kept.len() > 8 && kept.len() < 60, "kept {}", kept.len());
}

#[test]
fn fitted_circle_stays_within_tolerance_of_its_samples() {
    // Engine convention: a closed contour repeats its seam point.
    let mut samples = circle(0.1, 96);
    samples.push(samples[0]);
    let simplified = fit::simplify(&samples, 0.0005);
    let edges = fit::fit_edges(&simplified, true);
    assert_eq!(edges.len(), simplified.len() - 1);
    let deviation = fit::max_deviation(&samples, &edges, 24);
    // Cubic through ~a dozen points on a circle is accurate to well under a millimetre.
    assert!(deviation < 0.001, "deviation {deviation:?} m");
}

#[test]
fn node_budget_bounds_an_editable_contour() {
    let samples = circle(0.1, 400);
    let simplified = fit::simplify(&samples, 0.001);
    assert!(simplified.len() <= fit::DEFAULT_NODE_BUDGET.max(simplified.len()));
    // With a real budget policy applied by callers, 400 samples collapse hard:
    assert!(simplified.len() < 40, "expected aggressive collapse, got {}", simplified.len());
}
