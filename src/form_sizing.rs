//! Resize a dress form to measurements.
//!
//! A scanned form has one size. This module re-shapes it to a set of girths and lengths by the
//! simplest method that can be checked: **radial scaling about the form's vertical axis, by a
//! factor that varies smoothly with height**, plus a **piecewise-linear remap of height** between
//! named landmarks. It is a grading of one scan, not a body model:
//!
//! * a girth target scales width and depth together (the form keeps its proportions);
//! * lengths move the landmarks, and what lies between them is stretched in proportion;
//! * nothing is invented that the scan does not contain: a measurement with no landmark is not
//!   adjustable.
//!
//! The result is **measured back off the resized mesh** (convex-hull girth of a thin slab at each
//! landmark), so what a screen shows is what the form measures, not what was asked for.
//!
//! Units: the mesh is in metres, girths and lengths cross the API in centimetres. CPU only.
use crate::avatar::SurfaceMesh;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// Half the thickness of the slab whose vertices are read as one cross-section.
pub const SLAB_HALF_M: f64 = 0.003;
/// A target may differ from the scan's own measurement by this fraction either way.
pub const RANGE: (f64, f64) = (0.80, 1.25);
/// Landmarks every form must declare, bottom to top.
pub const REQUIRED: [&str; 4] = ["hip", "waist", "bust", "neck"];
/// Every landmark name, in the order the screen lists them.
pub const GIRTH_KEYS: [&str; 5] = ["bust", "underbust", "waist", "hip", "neck"];
pub const LENGTH_KEYS: [&str; 2] = ["back_waist_length", "waist_to_hip"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnotDecl {
    pub name: String,
    pub y_m: f64,
    /// Where the height came from. Required in spirit: an assumed landmark says so.
    #[serde(default)]
    pub basis: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SizeChartRow {
    pub label: String,
    pub cm: BTreeMap<String, f64>,
    /// Where this row's numbers come from (a table, its URL, who read it). Say so when it is unchecked.
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SizingDecl {
    pub knots: Vec<KnotDecl>,
    #[serde(default)]
    pub chart: Vec<SizeChartRow>,
    /// Vertices below this height are not scaled or moved: the stand of a generated form.
    #[serde(default)]
    pub fixed_below_m: Option<f64>,
}

impl SizingDecl {
    /// Names, ordering and heights, without reading any mesh.
    pub fn validate(&self, height_m: f64) -> Result<(), String> {
        for required in REQUIRED {
            if !self.knots.iter().any(|k| k.name == required) {
                return Err(format!("sizing: the landmark {required:?} is required"));
            }
        }
        let mut last = 0.0;
        for knot in &self.knots {
            if !GIRTH_KEYS.contains(&knot.name.as_str()) {
                return Err(format!("sizing: unknown landmark {:?}", knot.name));
            }
            if !(knot.y_m.is_finite() && knot.y_m > last && knot.y_m < height_m) {
                return Err(format!(
                    "sizing: landmarks must rise from the floor in order, below the top of the form ({:?} is not)",
                    knot.name
                ));
            }
            last = knot.y_m;
        }
        let order: Vec<&str> = self.knots.iter().map(|k| k.name.as_str()).collect();
        let expected: Vec<&str> = ["hip", "waist", "underbust", "bust", "neck"]
            .into_iter()
            .filter(|n| order.contains(n))
            .collect();
        if order != expected {
            return Err(format!(
                "sizing: landmarks must be ordered hip, waist, underbust, bust, neck (got {order:?})"
            ));
        }
        if let (Some(fixed), Some(hip)) = (self.fixed_below_m, self.knots.first()) {
            if !(fixed.is_finite() && fixed >= 0.0 && fixed < hip.y_m) {
                return Err(
                    "sizing: fixed_below_m must be at least 0 and below the hip landmark"
                        .to_owned(),
                );
            }
        }
        for row in &self.chart {
            for key in row.cm.keys() {
                if !GIRTH_KEYS.contains(&key.as_str()) && !LENGTH_KEYS.contains(&key.as_str()) {
                    return Err(format!(
                        "sizing: chart row {:?} has unknown measurement {key:?}",
                        row.label
                    ));
                }
            }
        }
        Ok(())
    }
}

/// What a form measures, in centimetres.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Measured {
    pub girth_cm: BTreeMap<String, f64>,
    pub waist_to_hip_cm: f64,
    pub back_waist_length_cm: f64,
}

/// What a person asked for. Anything left out stays as the scan has it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Targets {
    #[serde(default)]
    pub girth_cm: BTreeMap<String, f64>,
    #[serde(default)]
    pub waist_to_hip_cm: Option<f64>,
    #[serde(default)]
    pub back_waist_length_cm: Option<f64>,
}

impl Targets {
    /// From one flat map keyed by the field names the screen uses.
    pub fn from_flat(flat: &BTreeMap<String, f64>) -> Result<Self, String> {
        let mut out = Self::default();
        for (key, value) in flat {
            if !value.is_finite() {
                return Err(format!("{key} must be a finite number of centimetres"));
            }
            match key.as_str() {
                "waist_to_hip" => out.waist_to_hip_cm = Some(*value),
                "back_waist_length" => out.back_waist_length_cm = Some(*value),
                k if GIRTH_KEYS.contains(&k) => {
                    out.girth_cm.insert(key.clone(), *value);
                }
                _ => return Err(format!("unknown measurement {key:?}")),
            }
        }
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.girth_cm.is_empty()
            && self.waist_to_hip_cm.is_none()
            && self.back_waist_length_cm.is_none()
    }
}

#[derive(Clone, Debug)]
struct Knot {
    name: String,
    y: f64,
}

/// A form that can be resized: its scan, its landmarks and what it measures as scanned.
pub struct Sizer {
    surface: Arc<SurfaceMesh>,
    knots: Vec<Knot>,
    centre: [f64; 2],
    fixed_below: f64,
    pub base: Measured,
    pub chart: Vec<SizeChartRow>,
    pub basis: BTreeMap<String, String>,
}

pub struct Resized {
    pub surface: SurfaceMesh,
    /// Landmark heights on the resized form, metres.
    pub knot_y: BTreeMap<String, f64>,
    pub measured: Measured,
    remap: Vec<(f64, f64)>,
}

impl Resized {
    /// Where a height on the scan ends up on the resized form.
    pub fn map_y(&self, y: f64) -> f64 {
        map_y(&self.remap, y)
    }
}

/// Piecewise-linear through the landmarks; below the first and above the last the form just moves.
fn map_y(remap: &[(f64, f64)], y: f64) -> f64 {
    let (first_old, first_new) = remap[0];
    if y <= first_old {
        return y + (first_new - first_old);
    }
    let (last_old, last_new) = remap[remap.len() - 1];
    if y >= last_old {
        return y + (last_new - last_old);
    }
    for pair in remap.windows(2) {
        let ((a, na), (b, nb)) = (pair[0], pair[1]);
        if y <= b {
            return na + (y - a) / (b - a) * (nb - na);
        }
    }
    y
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Perimeter of the convex hull of 2-D points.
fn hull_perimeter(mut points: Vec<[f64; 2]>) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let (mut lower, mut upper): (Vec<[f64; 2]>, Vec<[f64; 2]>) = (vec![], vec![]);
    for &p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    for &p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    (0..lower.len())
        .map(|i| {
            let (a, b) = (lower[i], lower[(i + 1) % lower.len()]);
            (a[0] - b[0]).hypot(a[1] - b[1])
        })
        .sum()
}

/// Girth, in centimetres, of the thin horizontal slab of the surface at height `y`.
pub fn girth_cm(surface: &SurfaceMesh, y: f64) -> f64 {
    let slab: Vec<[f64; 2]> = surface
        .vertices
        .iter()
        .filter(|v| (v.y - y).abs() <= SLAB_HALF_M)
        .map(|v| [v.x, v.z])
        .collect();
    100.0 * hull_perimeter(slab)
}

impl Sizer {
    pub fn new(surface: Arc<SurfaceMesh>, decl: &SizingDecl) -> Result<Self, String> {
        let top = surface
            .vertices
            .iter()
            .map(|v| v.y)
            .fold(f64::NEG_INFINITY, f64::max);
        decl.validate(top)?;
        let knots: Vec<Knot> = decl
            .knots
            .iter()
            .map(|k| Knot {
                name: k.name.clone(),
                y: k.y_m,
            })
            .collect();
        // The axis the form is scaled about: the mean centre of its waist and neck cross-sections.
        let centre_at = |name: &str| -> Result<[f64; 2], String> {
            let y = knots
                .iter()
                .find(|k| k.name == name)
                .map(|k| k.y)
                .unwrap_or(0.0);
            let slab: Vec<_> = surface
                .vertices
                .iter()
                .filter(|v| (v.y - y).abs() <= SLAB_HALF_M)
                .collect();
            if slab.len() < 8 {
                return Err(format!(
                    "sizing: the scan has too few vertices at the {name} landmark ({} in the slab)",
                    slab.len()
                ));
            }
            Ok([
                slab.iter().map(|v| v.x).sum::<f64>() / slab.len() as f64,
                slab.iter().map(|v| v.z).sum::<f64>() / slab.len() as f64,
            ])
        };
        let (w, n) = (centre_at("waist")?, centre_at("neck")?);
        let centre = [(w[0] + n[0]) / 2.0, (w[1] + n[1]) / 2.0];
        let mut sizer = Self {
            surface,
            knots,
            centre,
            fixed_below: decl.fixed_below_m.unwrap_or(f64::NEG_INFINITY),
            base: Measured::default(),
            chart: decl.chart.clone(),
            basis: decl
                .knots
                .iter()
                .map(|k| (k.name.clone(), k.basis.clone()))
                .collect(),
        };
        let heights: Vec<(String, f64)> =
            sizer.knots.iter().map(|k| (k.name.clone(), k.y)).collect();
        sizer.base = measure(&sizer.surface, &heights);
        for (name, girth) in &sizer.base.girth_cm {
            if *girth < 10.0 {
                return Err(format!(
                    "sizing: the scan measures only {girth:.1} cm at the {name} landmark; the height is wrong"
                ));
            }
        }
        Ok(sizer)
    }

    fn y(&self, name: &str) -> Option<f64> {
        self.knots.iter().find(|k| k.name == name).map(|k| k.y)
    }

    /// The allowed range, in centimetres, for each adjustable measurement.
    /// Landmark heights on this form as it stands, metres, by name.
    pub fn knot_heights(&self) -> BTreeMap<String, f64> {
        self.knots.iter().map(|k| (k.name.clone(), k.y)).collect()
    }

    pub fn ranges(&self) -> BTreeMap<String, (f64, f64)> {
        let mut out = BTreeMap::new();
        for (name, base) in &self.base.girth_cm {
            out.insert(name.clone(), (base * RANGE.0, base * RANGE.1));
        }
        for (name, base) in [
            ("waist_to_hip", self.base.waist_to_hip_cm),
            ("back_waist_length", self.base.back_waist_length_cm),
        ] {
            out.insert(name.to_owned(), (base * RANGE.0, base * RANGE.1));
        }
        out
    }

    pub fn resize(&self, targets: &Targets) -> Result<Resized, String> {
        let ranges = self.ranges();
        let check = |name: &str, value: f64| -> Result<(), String> {
            let (low, high) = ranges
                .get(name)
                .ok_or_else(|| format!("this form has no {name} landmark"))?;
            if value < *low || value > *high {
                return Err(format!(
                    "{name} {value:.1} cm is outside {low:.1}-{high:.1} cm ({:.0}-{:.0} % of this scan's own {:.1} cm)",
                    RANGE.0 * 100.0,
                    RANGE.1 * 100.0,
                    low / RANGE.0
                ));
            }
            Ok(())
        };
        for (name, value) in &targets.girth_cm {
            check(name, *value)?;
        }
        if let Some(v) = targets.waist_to_hip_cm {
            check("waist_to_hip", v)?;
        }
        if let Some(v) = targets.back_waist_length_cm {
            check("back_waist_length", v)?;
        }

        // Girth scale at each landmark, from where the scan is.
        let scales: Vec<(f64, f64)> = self
            .knots
            .iter()
            .map(|k| {
                let base = self.base.girth_cm[&k.name];
                (k.y, targets.girth_cm.get(&k.name).map_or(1.0, |t| t / base))
            })
            .collect();
        let scale_at = |y: f64| -> f64 {
            if y <= scales[0].0 {
                return scales[0].1;
            }
            let last = scales[scales.len() - 1];
            if y >= last.0 {
                return last.1;
            }
            for pair in scales.windows(2) {
                let ((a, sa), (b, sb)) = (pair[0], pair[1]);
                if y <= b {
                    return sa + (sb - sa) * smooth((y - a) / (b - a));
                }
            }
            1.0
        };

        // Heights: below the hip nothing moves; hip -> waist and waist -> neck take the lengths asked for.
        let (hip, waist, neck) = (
            self.y("hip").unwrap(),
            self.y("waist").unwrap(),
            self.y("neck").unwrap(),
        );
        let new_wth = targets.waist_to_hip_cm.map_or(waist - hip, |c| c / 100.0);
        let new_bwl = targets
            .back_waist_length_cm
            .map_or(neck - waist, |c| c / 100.0);
        let new_waist = hip + new_wth;
        let new_neck = new_waist + new_bwl;
        let upper = (neck - waist).max(f64::EPSILON);
        let mut remap = Vec::new();
        for k in &self.knots {
            let new = match k.name.as_str() {
                "hip" => hip,
                "waist" => new_waist,
                "neck" => new_neck,
                _ => new_waist + (k.y - waist) / upper * new_bwl,
            };
            remap.push((k.y, new));
        }
        let [cx, cz] = self.centre;
        let vertices = self
            .surface
            .vertices
            .iter()
            .map(|v| {
                if v.y < self.fixed_below {
                    return *v;
                }
                let s = scale_at(v.y);
                glam::DVec3::new(cx + (v.x - cx) * s, map_y(&remap, v.y), cz + (v.z - cz) * s)
            })
            .collect();
        let surface = SurfaceMesh {
            vertices,
            triangles: self.surface.triangles.clone(),
        };
        let knot_y: BTreeMap<String, f64> = self
            .knots
            .iter()
            .zip(&remap)
            .map(|(k, (_, new))| (k.name.clone(), *new))
            .collect();
        let heights: Vec<(String, f64)> = knot_y.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let measured = measure(&surface, &heights);
        Ok(Resized {
            surface,
            knot_y,
            measured,
            remap,
        })
    }
}

fn measure(surface: &SurfaceMesh, heights: &[(String, f64)]) -> Measured {
    let y = |name: &str| heights.iter().find(|(n, _)| n == name).map(|(_, y)| *y);
    Measured {
        girth_cm: heights
            .iter()
            .map(|(n, h)| (n.clone(), girth_cm(surface, *h)))
            .collect(),
        waist_to_hip_cm: 100.0 * (y("waist").unwrap_or(0.0) - y("hip").unwrap_or(0.0)),
        back_waist_length_cm: 100.0 * (y("neck").unwrap_or(0.0) - y("waist").unwrap_or(0.0)),
    }
}
