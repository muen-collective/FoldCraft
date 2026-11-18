//! The flat guide of a dress form: its outline and landmark lines laid flat, to draw behind the pattern pieces so they can be
//! aligned to the form.
//!
//! A pattern is flat and a form is round, so a plain front-view silhouette would be narrower than the pieces. The guide is *developed*:
//! at each height the front half-width is the distance along the surface from the centre front to the side, and the back likewise, so a
//! pattern's half-width at the bust can be compared directly with the form's half-girth. Coordinates: metres; `x` is across (the form's left
//! is +x, centre front/back at 0), `y` is height above the floor (the client flips it for a y-down board).
//!
//! The landmarks (bust, waist, hip ...) and the torso's start come from the form; a form without landmarks has no guide.
use crate::avatar::SurfaceMesh;
use crate::avatar_catalog::LoadedAvatar;
use crate::form_lines::{Axis, section};
use glam::DVec3;
use serde::Serialize;

const STEP_M: f64 = 0.005;

#[derive(Clone, Debug, Serialize)]
pub struct GuideSide {
    /// A closed outline: down the left edge, along the bottom, up the right edge. `[x, height]`.
    pub outline: Vec<[f64; 2]>,
    /// The centre front (or centre back) line, top to bottom.
    pub centre: [[f64; 2]; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct GuideLandmark {
    pub name: String,
    pub y: f64,
    /// Developed half-widths at this height: centre to the side, along the surface.
    pub half_front: f64,
    pub half_back: f64,
    /// A leg landmark (knee, ankle): the developed front and back width of ONE leg, side to side. `None` on the torso's lines.
    pub leg_front: Option<f64>,
    pub leg_back: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuideLeg {
    /// `left` is +x. Each polygon is one leg's front (or back) developed flat: the inner (crotch) edge by the middle, the outer edge a width away.
    pub side: String,
    pub front: Vec<[f64; 2]>,
    pub back: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuideArm {
    pub name: String,
    /// The arm in the front view (convex hull): projected, not developed.
    pub outline: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuidePoint {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FlatGuide {
    pub units: &'static str,
    pub top_m: f64,
    pub bottom_m: f64,
    pub front: GuideSide,
    pub back: GuideSide,
    pub landmarks: Vec<GuideLandmark>,
    pub points: Vec<GuidePoint>,
    pub legs: Vec<GuideLeg>,
    pub arms: Vec<GuideArm>,
}

/// The front and back arcs of one closed loop, full length. The loop is divided at the plane `z = split` (the form's side plane, the same one
/// the drawn side seams lie in; the loop's own mid-depth when `None`): its two crossings are the sides, and the arc that reaches further
/// forward is the front. `None` for a loop that is too small, not closed, or not crossed twice.
fn loop_arcs(closed: bool, pts: &[DVec3], split: Option<f64>) -> Option<(f64, f64)> {
    let n = pts.len();
    if !closed || n < 8 {
        return None;
    }
    let zs = split.unwrap_or_else(|| {
        (pts.iter().map(|p| p.z).fold(f64::MIN, f64::max)
            + pts.iter().map(|p| p.z).fold(f64::MAX, f64::min))
            / 2.0
    });
    // Edges that cross the plane; the two with the extreme x are the sides.
    let mut crossings: Vec<(usize, f64)> = (0..n)
        .filter(|&i| (pts[i].z >= zs) != (pts[(i + 1) % n].z >= zs))
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            (
                i,
                if (b.z - a.z).abs() < 1e-15 {
                    a.x
                } else {
                    a.x + (b.x - a.x) * (zs - a.z) / (b.z - a.z)
                },
            )
        })
        .collect();
    if crossings.len() < 2 {
        return None;
    }
    crossings.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (l, r) = (crossings[0].0, crossings[crossings.len() - 1].0);
    // The arc of vertices from the edge `from` to the edge `to` (cyclic): its length and its mean z.
    let arc = |from: usize, to: usize| -> (f64, f64) {
        let (mut len, mut zsum, mut count, mut i) = (0.0, 0.0, 0usize, from);
        while i != to {
            let j = (i + 1) % n;
            len += (pts[j] - pts[i]).length();
            zsum += pts[i].z;
            count += 1;
            i = j;
        }
        (len, zsum / count.max(1) as f64)
    };
    let (a, b) = (arc(l, r), arc(r, l));
    Some(if a.1 >= b.1 { (a.0, b.0) } else { (b.0, a.0) })
}

fn width(p: &[DVec3]) -> f64 {
    p.iter().map(|q| q.x).fold(f64::MIN, f64::max) - p.iter().map(|q| q.x).fold(f64::MAX, f64::min)
}

/// The developed front and back half-widths of the torso at height `y`, or `None` if there is no torso loop there.
///
/// The cut at this height is one closed loop; half an arc is the developed half-width. (Splitting at the mid-depth and sorting by x, as a
/// first version did, folds back on itself over a bust or a seat.)
/// `side` is `(zc, t)`: the side plane `z = zc` of the form, blended toward the loop's own mid-depth by `t` (0 = the side plane, 1 = the mid-depth),
/// so that above the armpit, where the front and back meet along the shoulder, the split follows the loop.
fn halves(mesh: &SurfaceMesh, y: f64, side: (f64, f64)) -> Option<(f64, f64)> {
    let (closed, pts) = section(mesh, Axis::Y, y)
        .into_iter()
        .max_by(|(_, a), (_, b)| width(a).total_cmp(&width(b)))?;
    let mid = (pts.iter().map(|p| p.z).fold(f64::MIN, f64::max)
        + pts.iter().map(|p| p.z).fold(f64::MAX, f64::min))
        / 2.0;
    let (f, b) = loop_arcs(closed, &pts, Some(side.0 + side.1 * (mid - side.0)))?;
    Some((f / 2.0, b / 2.0))
}

/// One leg's cut at a height: (side +1 left / -1 right, front arc, back arc, girth), full length, side to side.
type LegArc = (f64, f64, f64, f64);

fn leg_arcs(mesh: &SurfaceMesh, y: f64) -> Vec<LegArc> {
    section(mesh, Axis::Y, y)
        .into_iter()
        .filter_map(|(closed, pts)| {
            let (f, b) = loop_arcs(closed, &pts, None)?;
            let girth: f64 = (0..pts.len())
                .map(|i| (pts[(i + 1) % pts.len()] - pts[i]).length())
                .sum();
            let cx = pts.iter().map(|p| p.x).sum::<f64>() / pts.len() as f64;
            (cx.abs() > 0.02).then_some((cx.signum(), f, b, girth))
        })
        .collect()
}

/// The convex hull of points in the front view (x, y): the outline of a part such as an arm, projected (not developed).
fn hull(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let chain = |iter: &mut dyn Iterator<Item = &[f64; 2]>| {
        let mut h: Vec<[f64; 2]> = Vec::new();
        for &q in iter {
            while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
        h
    };
    let mut out = chain(&mut p.iter());
    out.extend(chain(&mut p.iter().rev()));
    out
}

/// The guide, from a loaded form that declares landmarks. `None` when it has none (the bundled mannequin, an unsized scan).
/// `side_offset_m` moves the side seam from the form's side plane toward the back (positive) or the front (negative): a bodice's side seam
/// usually sits behind the form's side, and the front is then the wider piece. 0 is the form's own side plane.
pub fn flat_guide(avatar: &LoadedAvatar, side_offset_m: f64) -> Option<FlatGuide> {
    if avatar.landmarks.is_empty() {
        return None;
    }
    let mesh = &*avatar.surface;
    let torso: Vec<&DVec3> = mesh
        .vertices
        .iter()
        .filter(|v| v.y >= avatar.torso_from_m)
        .collect();
    if torso.is_empty() {
        return None;
    }
    let bottom = torso.iter().map(|v| v.y).fold(f64::MAX, f64::min) + 0.01;
    let top = torso.iter().map(|v| v.y).fold(f64::MIN, f64::max) - 0.004;
    // The side plane: the mid-depth of the waist's cut (the same plane the drawn side seams lie in). Above the bust line it blends to each
    // loop's own mid-depth over 10 cm.
    let waist_y = avatar
        .landmarks
        .get("waist")
        .copied()
        .unwrap_or((top + bottom) / 2.0);
    let zc = {
        let band: Vec<f64> = torso
            .iter()
            .filter(|v| (v.y - waist_y).abs() < 0.01)
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
    let bust_y = avatar.landmarks.get("bust").copied().unwrap_or(top);
    let side_for = |sy: f64| {
        let t = ((sy - bust_y) / 0.10).clamp(0.0, 1.0);
        (zc - side_offset_m, t * t * (3.0 - 2.0 * t))
    };
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    let mut y = top;
    while y >= bottom {
        if let Some((f, b)) = halves(mesh, y, side_for(y)) {
            rows.push((y, f, b));
        }
        y -= STEP_M;
    }
    if rows.len() < 4 {
        return None;
    }
    let side = |pick: &dyn Fn(&(f64, f64, f64)) -> f64| {
        let mut outline: Vec<[f64; 2]> = rows.iter().map(|r| [-pick(r), r.0]).collect();
        outline.extend(rows.iter().rev().map(|r| [pick(r), r.0]));
        GuideSide {
            outline,
            centre: [[0.0, top], [0.0, bottom]],
        }
    };
    let at = |y: f64| {
        rows.iter()
            .min_by(|a, b| (a.0 - y).abs().total_cmp(&(b.0 - y).abs()))
            .copied()
    };
    let mut landmarks = Vec::new();
    for name in ["neck", "bust", "underbust", "waist", "hip"] {
        if let Some(&ly) = avatar.landmarks.get(name) {
            if let Some((_, f, b)) = halves(mesh, ly, side_for(ly)).map(|(f, b)| (ly, f, b)) {
                landmarks.push(GuideLandmark {
                    name: name.to_owned(),
                    y: ly,
                    half_front: f,
                    half_back: b,
                    leg_front: None,
                    leg_back: None,
                });
            }
        }
    }
    let mut points = Vec::new();
    if let Some(l) = landmarks.iter().find(|l| l.name == "neck") {
        points.push(GuidePoint {
            name: "neck_front_centre".into(),
            x: 0.0,
            y: l.y,
        });
        for (sign, side_name) in [(-1.0, "left"), (1.0, "right")] {
            points.push(GuidePoint {
                name: format!("hps_{side_name}"),
                x: sign * l.half_front,
                y: l.y,
            });
        }
    }
    // The shoulder point: the outermost vertex of the upper torso (above the bust line), at its height on the developed edge.
    let bust = avatar.landmarks.get("bust").copied().unwrap_or(bottom);
    if let Some(tip) = torso
        .iter()
        .filter(|v| v.y > bust + 0.05)
        .max_by(|a, b| a.x.abs().total_cmp(&b.x.abs()))
    {
        if let Some((_, f, _)) = at(tip.y) {
            for (sign, side_name) in [(-1.0, "left"), (1.0, "right")] {
                points.push(GuidePoint {
                    name: format!("shoulder_point_{side_name}"),
                    x: sign * f,
                    y: tip.y,
                });
            }
        }
    }
    // The legs, when the mesh has them below the torso: each leg developed flat, and the lines a trouser is aligned to.
    let crotch = avatar.torso_from_m;
    let mut legs: Vec<GuideLeg> = Vec::new();
    if crotch.is_finite() && crotch > 0.2 {
        const GAP: f64 = 0.005;
        let mut rows: Vec<(f64, Vec<LegArc>)> = Vec::new();
        let mut yy = crotch - 0.01;
        while yy > 0.004 {
            let arcs = leg_arcs(mesh, yy);
            if arcs.len() == 2 {
                rows.push((yy, arcs));
            }
            yy -= STEP_M;
        }
        if rows.len() > 8 {
            for sign in [1.0, -1.0] {
                let mut polygons: Vec<Vec<[f64; 2]>> = Vec::new();
                for take_front in [true, false] {
                    let widths: Vec<(f64, f64)> = rows
                        .iter()
                        .filter_map(|(ry, arcs)| {
                            arcs.iter()
                                .find(|a| a.0 == sign)
                                .map(|a| (*ry, if take_front { a.1 } else { a.2 }))
                        })
                        .collect();
                    let mut poly: Vec<[f64; 2]> =
                        widths.iter().map(|&(ry, _)| [sign * GAP, ry]).collect();
                    poly.extend(widths.iter().rev().map(|&(ry, w)| [sign * (GAP + w), ry]));
                    polygons.push(poly);
                }
                let back = polygons.pop().unwrap_or_default();
                let front = polygons.pop().unwrap_or_default();
                legs.push(GuideLeg {
                    side: if sign > 0.0 {
                        "left".into()
                    } else {
                        "right".into()
                    },
                    front,
                    back,
                });
            }
            // Crotch, knee, ankle and floor. The knee and ankle are the narrowest of a leg inside the bands where a leg has them.
            let girth_at = |gy: f64| leg_arcs(mesh, gy).iter().map(|a| a.3).fold(0.0, f64::max);
            let narrowest = |lo: f64, hi: f64| {
                let mut best = (lo, f64::MAX);
                let mut gy = lo;
                while gy <= hi {
                    let g = girth_at(gy);
                    if g > 0.0 && g < best.1 {
                        best = (gy, g);
                    }
                    gy += 0.01;
                }
                best.0
            };
            let knee = narrowest(0.45 * crotch, 0.65 * crotch);
            let ankle = narrowest(0.08 * crotch, 0.30 * crotch);
            let hip_half = landmarks
                .iter()
                .find(|l| l.name == "hip")
                .map(|l| (l.half_front, l.half_back))
                .unwrap_or((0.2, 0.2));
            for (name, ly) in [("crotch", crotch), ("floor", 0.0)] {
                landmarks.push(GuideLandmark {
                    name: name.into(),
                    y: ly,
                    half_front: hip_half.0,
                    half_back: hip_half.1,
                    leg_front: None,
                    leg_back: None,
                });
            }
            for (name, ly) in [("knee", knee), ("ankle", ankle)] {
                if let Some(a) = leg_arcs(mesh, ly).first() {
                    landmarks.push(GuideLandmark {
                        name: name.into(),
                        y: ly,
                        half_front: a.1 / 2.0,
                        half_back: a.2 / 2.0,
                        leg_front: Some(a.1),
                        leg_back: Some(a.2),
                    });
                }
            }
        }
    }
    // The arms, for aligning a sleeve: the outline of each separate arm part in the front view.
    let arms: Vec<GuideArm> = avatar
        .parts
        .iter()
        .filter(|(name, _)| name.starts_with("arm"))
        .map(|(name, m)| GuideArm {
            name: name.clone(),
            outline: hull(&m.vertices.iter().map(|v| [v.x, v.y]).collect::<Vec<_>>()),
        })
        .collect();
    Some(FlatGuide {
        units: "metres",
        top_m: top,
        bottom_m: bottom,
        front: side(&|r| r.1),
        back: side(&|r| r.2),
        landmarks,
        points,
        legs,
        arms,
    })
}
