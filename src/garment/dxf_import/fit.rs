//! Fit sampled contours back to named cubic edges with a node budget.
//!
//! A DXF contour arrives as hundreds of short segments; the editor's polygon
//! model wants few, stable, editable nodes. This module simplifies the polyline
//! (Douglas-Peucker at an explicit tolerance), then fits cubic control points
//! per edge so curvature survives without a handle on every sample. It never
//! changes rest geometry semantics: output is still metres in material space.

use glam::DVec2;

/// Default maximum nodes for one fitted contour.
pub const DEFAULT_NODE_BUDGET: usize = 32;

/// Simplify a closed or open polyline by removing points whose deviation from
/// the chord between kept neighbours stays within `tolerance_m`.
pub fn simplify(points: &[DVec2], tolerance_m: f64) -> Vec<DVec2> {
    if points.len() < 3 || tolerance_m <= 0. {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    reduce(points, 0, points.len() - 1, tolerance_m, &mut keep);
    points
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

fn reduce(points: &[DVec2], first: usize, last: usize, tolerance_m: f64, keep: &mut [bool]) {
    if last <= first + 1 {
        return;
    }
    let a = points[first];
    let b = points[last];
    let chord = b - a;
    let length = chord.length();
    let mut worst = -1f64;
    let mut index = 0usize;
    for i in first + 1..last {
        let p = points[i];
        let d = if length == 0. {
            (p - a).length()
        } else {
            ((p - a).perp_dot(chord) / length).abs()
        };
        if d > worst {
            worst = d;
            index = i;
        }
    }
    if worst > tolerance_m {
        keep[index] = true;
        reduce(points, first, index, tolerance_m, keep);
        reduce(points, index, last, tolerance_m, keep);
    }
}

/// One fitted edge: endpoints plus two cubic control points, all in metres.
#[derive(Clone, Copy, Debug)]
pub struct FittedEdge {
    pub start: DVec2,
    pub c1: DVec2,
    pub c2: DVec2,
    pub end: DVec2,
}

/// Fit cubic control points through a run of sampled points between fixed
/// endpoints using tangent estimation (Catmull-Rom to Bezier conversion).
pub fn fit_edges(points: &[DVec2], closed: bool) -> Vec<FittedEdge> {
    if points.len() < 2 {
        return vec![];
    }
    // A closed contour whose stored ring omits the repeated seam point gets
    // its wrap-around neighbour back; otherwise tangents at the seam break.
    let mut ring: Vec<DVec2> = points.to_vec();
    let true_closed = closed && ring.first() != ring.last();
    if true_closed {
        ring.push(ring[0]);
    }
    let n = ring.len();
    let prev = |i: usize| ring[(i + n - 1) % n];
    let next = |i: usize| ring[(i + 1) % n];
    let count = if closed { n - 1 } else { n - 1 };
    let mut edges = Vec::with_capacity(count);
    for i in 0..count {
        let a = ring[i];
        let b = ring[i + 1];
        let ta = if closed || i > 0 {
            (next(i) - prev(i)) * 0.25
        } else {
            (b - a) * 0.5
        };
        let tb = if closed || i + 1 < n - 1 {
            (next(i + 1) - prev(i + 1)) * 0.25
        } else {
            (b - a) * 0.5
        };
        edges.push(FittedEdge {
            start: a,
            c1: a + ta / 3.0 * 2.0,
            c2: b - tb / 3.0 * 2.0,
            end: b,
        });
    }
    edges
}

/// Maximum deviation in metres between the original samples and the fitted
/// piecewise-cubic approximation, evaluated by flattening each edge.
pub fn max_deviation(samples: &[DVec2], edges: &[FittedEdge], steps_per_edge: usize) -> f64 {
    let mut worst = 0f64;
    for edge in edges {
        for s in 0..=steps_per_edge {
            let t = s as f64 / steps_per_edge as f64;
            let mt = 1. - t;
            let p = edge.start * (mt * mt * mt)
                + edge.c1 * (3. * mt * mt * t)
                + edge.c2 * (3. * mt * t * t)
                + edge.end * (t * t * t);
            // distance from this curve point to the sample polyline
            let mut best = f64::INFINITY;
            for w in samples.windows(2) {
                let ab = w[1] - w[0];
                let length2 = ab.dot(ab);
                let u = if length2 == 0. {
                    0.
                } else {
                    ((p - w[0]).dot(ab) / length2).clamp(0., 1.)
                };
                best = best.min((p - (w[0] + ab * u)).length());
            }
            worst = worst.max(best);
        }
    }
    worst
}
