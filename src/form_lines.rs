//! Guide lines on a dress form: tape lines (the girth bands at the landmarks) and seam lines (centre front and back,
//! princess, side).
//!
//! They are **cut from whatever mesh is loaded** by a plane, so they follow every form and every resize, and nothing
//! is added to the mesh: a tape line is exactly where a tape measure would lie. Sections are chained by the mesh edge
//! each point sits on, not by matching coordinates, so they are exact and cannot leak.
//!
//! Frame as everywhere: metres, Y up, +Z front. The form's *left* is +X (it faces +Z).
use crate::avatar::SurfaceMesh;
use glam::DVec3;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Tape,
    Seam,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuideLine {
    pub name: String,
    pub kind: LineKind,
    pub closed: bool,
    pub points: Vec<[f64; 3]>,
}

#[derive(Clone, Copy, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

fn coord(v: DVec3, axis: Axis) -> f64 {
    match axis {
        Axis::X => v.x,
        Axis::Y => v.y,
        Axis::Z => v.z,
    }
}

type Edge = (usize, usize);

/// Every polyline where the mesh crosses the plane `axis = value`. A closed mesh gives closed loops.
pub fn section(mesh: &SurfaceMesh, axis: Axis, value: f64) -> Vec<(bool, Vec<DVec3>)> {
    // A vertex exactly on the plane counts as above it, so a plane through a ring of vertices still cuts cleanly.
    let above = |i: usize| coord(mesh.vertices[i], axis) >= value;
    let point = |e: Edge| {
        let (a, b) = (mesh.vertices[e.0], mesh.vertices[e.1]);
        let (da, db) = (coord(a, axis) - value, coord(b, axis) - value);
        let t = if (db - da).abs() < 1e-15 {
            0.5
        } else {
            (0.0 - da) / (db - da)
        };
        a + (b - a) * t.clamp(0.0, 1.0)
    };
    let mut pts: BTreeMap<Edge, DVec3> = BTreeMap::new();
    let mut next: BTreeMap<Edge, Vec<Edge>> = BTreeMap::new();
    for t in &mesh.triangles {
        let crossing: Vec<Edge> = [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]
            .into_iter()
            .filter(|&(a, b)| above(a) != above(b))
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        if let [e, f] = crossing[..] {
            pts.entry(e).or_insert_with(|| point(e));
            pts.entry(f).or_insert_with(|| point(f));
            next.entry(e).or_default().push(f);
            next.entry(f).or_default().push(e);
        }
    }
    let mut seen: BTreeMap<Edge, bool> = BTreeMap::new();
    let mut out = Vec::new();
    // Open chains first (they start at a node with one neighbour), then the loops.
    let mut starts: Vec<Edge> = next
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(e, _)| *e)
        .collect();
    starts.extend(next.keys().copied());
    for start in starts {
        if seen.contains_key(&start) {
            continue;
        }
        let (mut chain, mut here, mut prev) = (vec![start], start, None::<Edge>);
        seen.insert(start, true);
        loop {
            let step = next[&here]
                .iter()
                .copied()
                .find(|n| Some(*n) != prev && !seen.contains_key(n));
            let Some(n) = step else { break };
            seen.insert(n, true);
            chain.push(n);
            prev = Some(here);
            here = n;
        }
        let closed = chain.len() > 2 && next[&here].contains(&start);
        out.push((closed, chain.into_iter().map(|e| pts[&e]).collect()));
    }
    out
}

/// The longest run of consecutive points of a loop (or chain) that satisfy `keep`.
fn longest_run(closed: bool, pts: &[DVec3], keep: impl Fn(DVec3) -> bool) -> Vec<DVec3> {
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }
    let flags: Vec<bool> = pts.iter().map(|p| keep(*p)).collect();
    if flags.iter().all(|f| *f) {
        return pts.to_vec();
    }
    // Start just after a point that fails, so a run is never split by the loop's seam.
    let start = if closed {
        flags.iter().position(|f| !*f).map_or(0, |i| (i + 1) % n)
    } else {
        0
    };
    let (mut best, mut cur): (Vec<DVec3>, Vec<DVec3>) = (Vec::new(), Vec::new());
    for k in 0..n {
        let i = (start + k) % n;
        if flags[i] {
            cur.push(pts[i]);
        } else {
            if cur.len() > best.len() {
                best = std::mem::take(&mut cur);
            }
            cur.clear();
        }
    }
    if cur.len() > best.len() {
        best = cur;
    }
    best
}

/// Where each landmark is, and the stretch of the mesh that is the torso (the stand is below `torso_from`).
pub struct LineSpec<'a> {
    pub landmarks: &'a BTreeMap<String, f64>,
    pub torso_from_m: f64,
}

/// Tape lines at the landmarks that bound the torso, and the seam lines between them.
pub fn guide_lines(mesh: &SurfaceMesh, spec: &LineSpec) -> Vec<GuideLine> {
    let torso: Vec<DVec3> = mesh
        .vertices
        .iter()
        .copied()
        .filter(|v| v.y >= spec.torso_from_m)
        .collect();
    if torso.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = (
        torso.iter().map(|v| v.y).fold(f64::MAX, f64::min),
        torso.iter().map(|v| v.y).fold(f64::MIN, f64::max),
    );
    let in_torso = |p: DVec3| p.y > lo + 1e-6 && p.y < hi - 1e-6;
    let mut lines = Vec::new();

    // Tape lines: the closed cut of the torso at each landmark (never the stand, which is below the torso).
    for name in ["bust", "underbust", "waist", "hip"] {
        let Some(&y) = spec.landmarks.get(name) else {
            continue;
        };
        let loops = section(mesh, Axis::Y, y);
        if let Some((closed, pts)) = loops
            .into_iter()
            .filter(|(_, p)| p.iter().all(|q| q.y >= spec.torso_from_m))
            .max_by_key(|(_, p)| p.len())
        {
            lines.push(GuideLine {
                name: name.to_owned(),
                kind: LineKind::Tape,
                closed,
                points: pts.iter().map(|p| p.to_array()).collect(),
            });
        }
    }

    // The mid-depth of the torso, to tell front from back, and the bust half-width, to place the princess lines.
    let zc = {
        let at = spec
            .landmarks
            .get("waist")
            .copied()
            .unwrap_or((lo + hi) / 2.0);
        let band: Vec<f64> = torso
            .iter()
            .filter(|v| (v.y - at).abs() < 0.01)
            .map(|v| v.z)
            .collect();
        if band.is_empty() {
            0.0
        } else {
            (band.iter().cloned().fold(f64::MIN, f64::max)
                + band.iter().cloned().fold(f64::MAX, f64::min))
                / 2.0
        }
    };
    let bust_hw = {
        let at = spec
            .landmarks
            .get("bust")
            .copied()
            .unwrap_or((lo + hi) / 2.0);
        torso
            .iter()
            .filter(|v| (v.y - at).abs() < 0.01)
            .map(|v| v.x.abs())
            .fold(0.0, f64::max)
    };

    let mut seam = |name: &str, axis: Axis, value: f64, keep: &dyn Fn(DVec3) -> bool| {
        let best = section(mesh, axis, value)
            .into_iter()
            .filter(|(_, p)| p.iter().any(|q| q.y >= spec.torso_from_m))
            .map(|(c, p)| longest_run(c, &p, |q| in_torso(q) && keep(q)))
            .max_by_key(|p| p.len())
            .unwrap_or_default();
        if best.len() > 1 {
            lines.push(GuideLine {
                name: name.to_owned(),
                kind: LineKind::Seam,
                closed: false,
                points: best.iter().map(|p| p.to_array()).collect(),
            });
        }
    };
    seam("centre_front", Axis::X, 0.0, &|p| p.z >= zc);
    seam("centre_back", Axis::X, 0.0, &|p| p.z < zc);
    for (side, sign) in [("left", 1.0), ("right", -1.0)] {
        let x = sign * 0.55 * bust_hw;
        seam(&format!("princess_front_{side}"), Axis::X, x, &|p| {
            p.z >= zc
        });
        seam(&format!("princess_back_{side}"), Axis::X, x, &|p| p.z < zc);
        // The side seam: the plane through the mid-depth, on one side or the other.
        seam(&format!("side_{side}"), Axis::Z, zc, &move |p| {
            p.x * sign > 0.0
        });
    }
    lines
}
