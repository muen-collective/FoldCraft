//! Write the resident template back out as a plain flat DXF.
//!
//! Scope, stated honestly: R12 ASCII, millimetres ($INSUNITS = 4), one closed
//! POLYLINE per panel carrying its name in an AAMA TEXT record. Curved edges
//! are written as their true circular arcs (a bulge on the segment whose
//! samples lie on one circle); everything else is straight segments between
//! the stored contour points. No layers beyond piece naming, no AAMA/ASTM
//! grade or size records, no notches yet - that dialect is separate work, and
//! claiming it here would be a label, not a capability.
use crate::garment::GarmentTemplate;
use glam::DVec2;

/// Millimetres per metre: this file stores millimetres.
const MM: f64 = 1000.;

pub fn flat_dxf(template: &GarmentTemplate) -> String {
    let mut out = String::new();
    pair(&mut out, 0, "SECTION");
    pair(&mut out, 2, "HEADER");
    named(&mut out, "$ACADVER", "AC1009");
    integer_named(&mut out, "$INSUNITS", 4);
    pair(&mut out, 0, "ENDSEC");
    pair(&mut out, 0, "SECTION");
    pair(&mut out, 2, "ENTITIES");
    for panel in &template.panels {
        text_record(&mut out, &format!("PIECE NAME: {}", panel.name));
        let Some(polygon) = &panel.polygon else {
            // Rectangle panels have no named edges; their outline is still
            // worth writing as four straight vertices.
            let (w, h) = (panel.pattern.width * MM, panel.pattern.height * MM);
            polyline(
                &mut out,
                &[
                    DVec2::new(0., 0.),
                    DVec2::new(w, 0.),
                    DVec2::new(w, h),
                    DVec2::new(0., h),
                ],
                &[],
            );
            continue;
        };
        // Walk the stored contour; for each edge decide once whether the
        // sampled curve is a circular arc (write start + bulge) or freeform
        // (write every sample as a straight vertex).
        let mut points: Vec<DVec2> = vec![];
        let mut bulges: Vec<f64> = vec![];
        for index in 0..polygon.points.len() {
            let Ok(samples) = polygon.edge_points(index) else {
                continue;
            };
            let scaled: Vec<DVec2> = samples.iter().map(|p| *p * MM).collect();
            if scaled.len() < 2 {
                continue;
            }
            match arc_bulge(&scaled) {
                Some(bulge) => {
                    points.push(scaled[0]);
                    bulges.push(bulge);
                }
                None => {
                    // Each edge starts where the last ended; push its first
                    // sample only when it does not duplicate the previous one.
                    if points.last() != Some(&scaled[0]) {
                        points.push(scaled[0]);
                        bulges.push(0.);
                    }
                    for point in &scaled[1..scaled.len() - 1] {
                        points.push(*point);
                        bulges.push(0.);
                    }
                }
            }
        }
        if points.len() >= 3 {
            polyline(&mut out, &points, &bulges);
        }
    }
    pair(&mut out, 0, "ENDSEC");
    pair(&mut out, 0, "EOF");
    out
}

/// When every sample of an edge lies on one circle through its endpoints,
/// return the DXF bulge of that arc. Tolerances are fractions of the edge's
/// own span, so a 5 mm notch and a 40 cm armhole are judged alike. Straight
/// or freeform edges return None so the caller keeps their exact samples.
fn arc_bulge(scaled: &[DVec2]) -> Option<f64> {
    if scaled.len() < 4 {
        return None;
    }
    let a = scaled[0];
    let b = *scaled.last().unwrap();
    let chord = b - a;
    let length = chord.length();
    if length < 1e-6 {
        return None;
    }
    let right = chord.perp() / length;
    let middle_raw = scaled[scaled.len() / 2];
    let t0 = ((middle_raw - a).dot(chord) / (length * length)).clamp(0., 1.);
    let foot0 = a + chord * t0;
    let sag_signed = (middle_raw - foot0).dot(right);
    if sag_signed.abs() < length * 0.004 {
        return None; // under 0.4% of the chord: straight is cheaper and exact
    }
    // Normalised space: divide every coordinate by the chord length. The
    // circle fit and all tolerances are then fractions of the edge's own
    // span, so a 5 mm notch and a 40 cm armhole are judged alike.
    let nrm = |p: DVec2| (p - a) / length;
    let na = DVec2::ZERO;
    let nb = nrm(b);
    let nm = nrm(scaled[scaled.len() / 2]);
    let center = circumcenter(na, nm, nb)?;
    let radius = (na - center).length();
    if !radius.is_finite() || radius <= 0. {
        return None;
    }
    // Sample midpoints: the polyline through samples approximates the curve,
    // so each segment's midpoint is the honest distance to compare against
    // the circle (the endpoints themselves lie on it by construction).
    for window in scaled.windows(2) {
        let midpoint = nrm((window[0] + window[1]) * 0.5);
        let off = ((midpoint - center).length() - radius).abs();
        if off > 0.005 {
            return None; // half a percent of the edge's span off the circle
        }
    }
    let angle = |p: DVec2| (p - center).y.atan2((p - center).x);
    let turn = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
    let sweep = turn(angle(na), angle(nb));
    if turn(angle(na), angle(nm)) > sweep {
        return None; // the middle lies on the major arc; not this edge's shape
    }
    if sweep > std::f64::consts::PI + 1e-9 {
        return None; // beyond a half-turn needs two segments; keep the samples
    }
    Some((sweep / 4.).tan() * sag_signed.signum())
}

fn circumcenter(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2. * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let ux = (a.dot(a) * (b.y - c.y) + b.dot(b) * (c.y - a.y) + c.dot(c) * (a.y - b.y)) / d;
    let uy = (a.dot(a) * (c.x - b.x) + b.dot(b) * (a.x - c.x) + c.dot(c) * (b.x - a.x)) / d;
    Some(DVec2::new(ux, uy))
}

fn text_record(out: &mut String, value: &str) {
    pair(out, 0, "TEXT");
    pair(out, 8, "0");
    pair(out, 10, "0.000");
    pair(out, 20, "0.000");
    pair(out, 40, "12.0");
    pair(out, 1, value);
}

fn polyline(out: &mut String, points: &[DVec2], bulges: &[f64]) {
    pair(out, 0, "POLYLINE");
    pair(out, 8, "0");
    pair(out, 66, "1");
    pair(out, 70, "1"); // closed
    for (index, point) in points.iter().enumerate() {
        pair(out, 0, "VERTEX");
        pair(out, 8, "0");
        pair(out, 10, &format!("{:.3}", point.x));
        pair(out, 20, &format!("{:.3}", point.y));
        pair(out, 30, "0.0");
        if bulges.get(index).copied().unwrap_or(0.) != 0. {
            pair(out, 42, &format!("{:.6}", bulges[index]));
        }
    }
    pair(out, 0, "SEQEND");
    pair(out, 8, "0");
}

fn pair(out: &mut String, code: i32, value: &str) {
    out.push_str(&format!("{code}\n{value}\n"));
}
fn named(out: &mut String, key: &str, value: &str) {
    pair(out, 9, key);
    pair(out, 1, value);
}
fn integer_named(out: &mut String, key: &str, value: i32) {
    pair(out, 9, key);
    pair(out, 70, &value.to_string());
}
