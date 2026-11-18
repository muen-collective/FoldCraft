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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratorDecl {
    pub profile: Vec<ProfileRow>,
    pub stand: StandDecl,
    #[serde(default)]
    pub sides: Option<usize>,
    /// Which row of the sizing chart the form is built to, or none: the profile as it is.
    #[serde(default)]
    pub default_chart_row: Option<usize>,
    /// Where the profile came from. Required: a profile is a set of measurements.
    pub basis: String,
}

impl GeneratorDecl {
    pub fn height_m(&self) -> f64 {
        self.stand.hem_height_m + self.profile.last().map_or(0.0, |r| r.y_m)
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
        let s = &self.stand;
        let narrowest = self.profile[0]
            .half_width_m
            .min(self.profile[0].front_m)
            .min(-self.profile[0].back_m);
        if !(s.hem_height_m > 0.1 && s.pole_radius_m > 0.002 && s.base_height_m > 0.0) {
            return Err(
                "generator: stand dimensions must be positive (hem at least 10 cm up)".to_owned(),
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
        let hem = self.stand.hem_height_m;

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
        let s = &self.stand;
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

        // Make every piece wind outward: the signed volume of the whole must be positive.
        if signed_volume(&mesh) < 0.0 {
            for t in &mut mesh.triangles {
                t.swap(1, 2);
            }
        }
        mesh.validate()?;
        Ok(mesh)
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
