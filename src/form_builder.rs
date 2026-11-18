//! Make a dress form from a profile: a clean, symmetric, watertight torso and a stand.
//!
//! A scan has noise, craters and artifacts. This builds the same *kind* of body from a table: at each
//! height a cross-section described by its half-width, how far it reaches to the front and the back,
//! and its roundness (a superellipse exponent: 2 is an ellipse, higher is boxier). Consecutive rows
//! are joined into a closed surface, the torso is capped flat top and bottom, and a pole and a round
//! base are added as separate closed pieces.
//!
//! The profile is data in `form.json`, with its provenance. Measurements are applied afterwards by
//! `form_sizing`, so a generated form is "rebuilt" to typed girths and lengths the same way a scan is.
//!
//! Frame: metres, Y up, +Z front, the base of the stand on y = 0, centred on the vertical axis.
use crate::avatar::SurfaceMesh;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

const DEFAULT_SIDES: usize = 64;
/// Gap between the top of the pole and the flat bottom of the torso, so the two never touch.
pub const POLE_GAP_M: f64 = 0.004;
const POLE_SIDES: usize = 24;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRow {
    /// Height above the flat bottom of the torso.
    pub y_m: f64,
    pub half_width_m: f64,
    /// How far the section reaches in front of the axis (positive).
    pub front_m: f64,
    /// How far it reaches behind the axis (negative).
    pub back_m: f64,
    /// 2 is an ellipse; larger is boxier.
    pub exponent: f64,
    /// Slope of this row's plane, rise per metre of depth (dy/dz): 0 is level, negative lowers the front. A neck
    /// that leans forward is cut square to its axis, so its top row takes the negative of the lean's tangent.
    #[serde(default)]
    pub tilt: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StandDecl {
    /// Height of the torso's flat bottom above the floor.
    pub hem_height_m: f64,
    pub pole_radius_m: f64,
    pub base_radius_m: f64,
    pub base_height_m: f64,
}

/// One ring of a limb: where its middle is, and how far it reaches across (rx) and front to back (rz).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LimbRow {
    pub p: [f64; 3],
    pub rx: f64,
    pub rz: f64,
}

/// A tube along a path: rings perpendicular to the path, an ellipse (superellipse) each, capped flat at both ends.
/// `mirror` makes the form's right limb from the left (+x is the form's left). With `part` set, each limb is its own
/// mesh (`<part>_left`, `<part>_right`: a removable arm); without it the limb is joined into the main body (a leg).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LimbDecl {
    pub name: String,
    #[serde(default)]
    pub part: Option<String>,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default = "default_limb_exponent")]
    pub exponent: f64,
    /// Keep every ring horizontal (a leg standing on the floor, cut flat there) instead of square to the path.
    #[serde(default)]
    pub level: bool,
    pub rows: Vec<LimbRow>,
}

fn default_limb_exponent() -> f64 {
    2.2
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratorDecl {
    pub profile: Vec<ProfileRow>,
    /// A pole and base under the torso. Without it (a full-body form standing on its legs) give `hem_height_m`.
    #[serde(default)]
    pub stand: Option<StandDecl>,
    /// Height of the torso's flat bottom above the floor, when there is no stand.
    #[serde(default)]
    pub hem_height_m: Option<f64>,
    /// Legs (joined into the body) and arms (separate meshes).
    #[serde(default)]
    pub limbs: Vec<LimbDecl>,
    #[serde(default)]
    pub sides: Option<usize>,
    /// Which row of the sizing chart the form is built to, or none: the profile as it is.
    #[serde(default)]
    pub default_chart_row: Option<usize>,
    /// Where the profile came from. Required: a profile is a set of measurements.
    pub basis: String,
}

impl GeneratorDecl {
    /// Height of the torso's flat bottom above the floor.
    pub fn hem(&self) -> f64 {
        self.stand
            .as_ref()
            .map(|s| s.hem_height_m)
            .or(self.hem_height_m)
            .unwrap_or(0.0)
    }

    pub fn height_m(&self) -> f64 {
        self.hem() + self.profile.last().map_or(0.0, |r| r.y_m)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.basis.trim().is_empty() {
            return Err("generator: basis is required (where the profile came from)".to_owned());
        }
        if self.profile.len() < 4 {
            return Err("generator: the profile needs at least 4 rows".to_owned());
        }
        let sides = self.sides.unwrap_or(DEFAULT_SIDES);
        if !(16..=256).contains(&sides) || sides % 2 != 0 {
            return Err("generator: sides must be an even number from 16 to 256".to_owned());
        }
        let mut last = -1.0;
        for (i, row) in self.profile.iter().enumerate() {
            let ok = row.y_m.is_finite()
                && row.half_width_m.is_finite()
                && row.front_m.is_finite()
                && row.back_m.is_finite()
                && row.exponent.is_finite();
            if !ok {
                return Err(format!(
                    "generator: profile row {i} has a value that is not a number"
                ));
            }
            if (i == 0 && row.y_m != 0.0) || row.y_m <= last {
                return Err(format!(
                    "generator: profile heights must start at 0 and rise (row {i})"
                ));
            }
            last = row.y_m;
            if row.half_width_m < 0.01 || row.front_m <= 0.0 || row.back_m >= 0.0 {
                return Err(format!(
                    "generator: row {i} needs half_width_m of at least 1 cm, front_m above 0 and back_m below 0"
                ));
            }
            if !row.tilt.is_finite() || row.tilt.abs() > 0.5 {
                return Err(format!(
                    "generator: row {i} tilt must be between -0.5 and 0.5"
                ));
            }
            if !(1.5..=6.0).contains(&row.exponent) {
                return Err(format!(
                    "generator: row {i} exponent {} is outside 1.5-6",
                    row.exponent
                ));
            }
        }
        match (&self.stand, self.hem_height_m) {
            (Some(s), None) => {
                let narrowest = self.profile[0]
                    .half_width_m
                    .min(self.profile[0].front_m)
                    .min(-self.profile[0].back_m);
                if !(s.hem_height_m > 0.1 && s.pole_radius_m > 0.002 && s.base_height_m > 0.0) {
                    return Err(
                        "generator: stand dimensions must be positive (hem at least 10 cm up)"
                            .to_owned(),
                    );
                }
                if s.pole_radius_m >= narrowest || s.pole_radius_m >= s.base_radius_m {
                    return Err(
                        "generator: the pole must be thinner than the torso's bottom and than the base"
                            .to_owned(),
                    );
                }
                if s.base_height_m + POLE_GAP_M >= s.hem_height_m {
                    return Err("generator: the base reaches the torso".to_owned());
                }
            }
            (None, Some(h)) if h.is_finite() && h > 0.1 => {}
            _ => {
                return Err("generator: give either a stand or hem_height_m (above 0.1), not both and not neither".to_owned());
            }
        }
        for limb in &self.limbs {
            limb.validate()?;
        }
        Ok(())
    }

    /// The closed surface: torso, pole and base, standing on y = 0.
    pub fn build(&self) -> Result<SurfaceMesh, String> {
        self.validate()?;
        let sides = self.sides.unwrap_or(DEFAULT_SIDES);
        let mut mesh = SurfaceMesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        };
        let hem = self.hem();

        // Torso: a ring of `sides` points per row.
        let ring = |row: &ProfileRow| -> Vec<DVec3> {
            (0..sides)
                .map(|i| {
                    let t = TAU * i as f64 / sides as f64;
                    let (c, s) = (t.cos(), t.sin());
                    let p = 2.0 / row.exponent;
                    let z = if s >= 0.0 { row.front_m } else { row.back_m } * s.abs().powf(p);
                    // The plane of the row may be tilted about the middle of its depth.
                    let y = hem + row.y_m + row.tilt * (z - (row.front_m + row.back_m) / 2.0);
                    DVec3::new(row.half_width_m * c.signum() * c.abs().powf(p), y, z)
                })
                .collect()
        };
        let torso_start = mesh.vertices.len();
        for row in &self.profile {
            mesh.vertices.extend(ring(row));
        }
        let at = |r: usize, i: usize| torso_start + r * sides + (i % sides);
        for r in 0..self.profile.len() - 1 {
            for i in 0..sides {
                mesh.triangles
                    .push([at(r, i), at(r, i + 1), at(r + 1, i + 1)]);
                mesh.triangles
                    .push([at(r, i), at(r + 1, i + 1), at(r + 1, i)]);
            }
        }
        for (r, y, up) in [
            (0, hem, false),
            (
                self.profile.len() - 1,
                hem + self.profile[self.profile.len() - 1].y_m,
                true,
            ),
        ] {
            let row = &self.profile[r];
            let centre = mesh.vertices.len();
            mesh.vertices
                .push(DVec3::new(0.0, y, (row.front_m + row.back_m) / 2.0));
            for i in 0..sides {
                mesh.triangles.push(if up {
                    [centre, at(r, i), at(r, i + 1)]
                } else {
                    [centre, at(r, i + 1), at(r, i)]
                });
            }
        }

        // The stand: a pole and a base, each a closed cylinder.
        if let Some(s) = &self.stand {
            cylinder(
                &mut mesh,
                s.base_radius_m,
                0.0,
                s.base_height_m,
                POLE_SIDES * 2,
            );
            cylinder(
                &mut mesh,
                s.pole_radius_m,
                s.base_height_m,
                hem - POLE_GAP_M,
                POLE_SIDES,
            );
        }

        // Make every piece wind outward: the signed volume of the whole must be positive.
        if signed_volume(&mesh) < 0.0 {
            for t in &mut mesh.triangles {
                t.swap(1, 2);
            }
        }
        // Legs and other joined limbs: each is its own closed piece, wound outward on its own.
        for limb in self.limbs.iter().filter(|l| l.part.is_none()) {
            for (_, piece) in limb.build(sides)? {
                let base = mesh.vertices.len();
                mesh.vertices.extend(piece.vertices);
                mesh.triangles
                    .extend(piece.triangles.into_iter().map(|t| t.map(|i| i + base)));
            }
        }
        mesh.validate()?;
        Ok(mesh)
    }

    /// The separate meshes (removable arms): `<part>_left` and `<part>_right`, each a closed surface.
    pub fn build_parts(&self) -> Result<Vec<(String, SurfaceMesh)>, String> {
        self.validate()?;
        let sides = self.sides.unwrap_or(DEFAULT_SIDES);
        let mut out = Vec::new();
        for limb in self.limbs.iter().filter(|l| l.part.is_some()) {
            out.extend(limb.build(sides)?);
        }
        Ok(out)
    }
}

impl LimbDecl {
    fn validate(&self) -> Result<(), String> {
        let n = &self.name;
        if self.rows.len() < 3 {
            return Err(format!("limb {n}: needs at least 3 rows"));
        }
        if !(1.5..=6.0).contains(&self.exponent) {
            return Err(format!(
                "limb {n}: exponent {} is outside 1.5-6",
                self.exponent
            ));
        }
        for (i, r) in self.rows.iter().enumerate() {
            if !(r.p.iter().all(|v| v.is_finite()) && r.rx.is_finite() && r.rz.is_finite()) {
                return Err(format!(
                    "limb {n}: row {i} has a value that is not a number"
                ));
            }
            if r.rx < 0.01 || r.rz < 0.01 {
                return Err(format!(
                    "limb {n}: row {i} needs rx and rz of at least 1 cm"
                ));
            }
            if i > 0 {
                let (a, b) = (DVec3::from(self.rows[i - 1].p), DVec3::from(r.p));
                if (b - a).length() < 0.005 {
                    return Err(format!(
                        "limb {n}: rows {} and {i} are less than 5 mm apart",
                        i - 1
                    ));
                }
            }
        }
        Ok(())
    }

    /// The limb's mesh, or its left and right pair when mirrored (left is +x).
    fn build(&self, sides: usize) -> Result<Vec<(String, SurfaceMesh)>, String> {
        let part = self.part.clone().unwrap_or_else(|| self.name.clone());
        let one = |sign: f64| -> Result<SurfaceMesh, String> {
            let centres: Vec<DVec3> = self
                .rows
                .iter()
                .map(|r| DVec3::new(sign * r.p[0], r.p[1], r.p[2]))
                .collect();
            let mut mesh = SurfaceMesh {
                vertices: Vec::new(),
                triangles: Vec::new(),
            };
            let p = 2.0 / self.exponent;
            for (i, row) in self.rows.iter().enumerate() {
                let (a, b) = (
                    centres[i.saturating_sub(1)],
                    centres[(i + 1).min(centres.len() - 1)],
                );
                let t = (b - a).normalize();
                // A frame around the path: e1 is across (x as far as the path allows), e2 front to back.
                let across = if t.x.abs() > 0.99 { DVec3::Z } else { DVec3::X };
                let (e1, e2) = if self.level {
                    (DVec3::X, DVec3::Z)
                } else {
                    let e1 = (across - t * across.dot(t)).normalize();
                    (e1, t.cross(e1))
                };
                for k in 0..sides {
                    let a = TAU * k as f64 / sides as f64;
                    let (c, s) = (a.cos(), a.sin());
                    mesh.vertices.push(
                        centres[i]
                            + e1 * (row.rx * c.signum() * c.abs().powf(p))
                            + e2 * (row.rz * s.signum() * s.abs().powf(p)),
                    );
                }
            }
            let at = |r: usize, k: usize| r * sides + (k % sides);
            for r in 0..self.rows.len() - 1 {
                for k in 0..sides {
                    mesh.triangles
                        .push([at(r, k), at(r, k + 1), at(r + 1, k + 1)]);
                    mesh.triangles
                        .push([at(r, k), at(r + 1, k + 1), at(r + 1, k)]);
                }
            }
            for (r, up) in [(0, false), (self.rows.len() - 1, true)] {
                let centre = mesh.vertices.len();
                mesh.vertices.push(centres[r]);
                for k in 0..sides {
                    mesh.triangles.push(if up {
                        [centre, at(r, k), at(r, k + 1)]
                    } else {
                        [centre, at(r, k + 1), at(r, k)]
                    });
                }
            }
            if signed_volume(&mesh) < 0.0 {
                for t in &mut mesh.triangles {
                    t.swap(1, 2);
                }
            }
            mesh.validate()?;
            Ok(mesh)
        };
        let mut out = vec![(format!("{part}_left"), one(1.0)?)];
        if self.mirror {
            out.push((format!("{part}_right"), one(-1.0)?));
        }
        Ok(out)
    }
}

/// A closed cylinder about the vertical axis from `y0` to `y1`.
fn cylinder(mesh: &mut SurfaceMesh, radius: f64, y0: f64, y1: f64, sides: usize) {
    let start = mesh.vertices.len();
    for y in [y0, y1] {
        for i in 0..sides {
            let t = TAU * i as f64 / sides as f64;
            mesh.vertices
                .push(DVec3::new(radius * t.cos(), y, radius * t.sin()));
        }
    }
    let (lo, hi) = (
        |i: usize| start + i % sides,
        |i: usize| start + sides + i % sides,
    );
    for i in 0..sides {
        mesh.triangles.push([lo(i), lo(i + 1), hi(i + 1)]);
        mesh.triangles.push([lo(i), hi(i + 1), hi(i)]);
    }
    let (bottom, top) = (mesh.vertices.len(), mesh.vertices.len() + 1);
    mesh.vertices.push(DVec3::new(0.0, y0, 0.0));
    mesh.vertices.push(DVec3::new(0.0, y1, 0.0));
    for i in 0..sides {
        mesh.triangles.push([bottom, lo(i + 1), lo(i)]);
        mesh.triangles.push([top, hi(i), hi(i + 1)]);
    }
}

/// Signed volume of a triangle soup; positive when the faces wind outward.
pub fn signed_volume(mesh: &SurfaceMesh) -> f64 {
    mesh.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.vertices[i]);
            a.dot(b.cross(c))
        })
        .sum::<f64>()
        / 6.0
}
