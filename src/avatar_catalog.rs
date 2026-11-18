//! Bodies the preview can drape on: the bundled MakeHuman mannequin and any dress forms
//! installed under `assets/dressforms/<id>/form.json`.
//!
//! A dress form is a directory holding a `form.json` and the OBJ it names. **The mesh is not
//! shipped with the code**: whoever installs a form owns its licence, and `form.json` records it.
//! A form whose mesh is missing is still listed, marked unavailable with the reason, so the
//! picker can say what to install instead of hiding the choice.
//!
//! Units are the failure this module exists to catch. The importer treats OBJ coordinates as
//! metres and never rescales, so a mesh whose measured height disagrees with the `height_m` its
//! form file declares is **refused**, not corrected: a silent centimetre-for-metre error is the
//! defect `AGENTS.md` names first.
//!
//! CPU only: nothing here touches wgpu or the UI toolkit.
use crate::avatar::SurfaceMesh;
use crate::form_builder::GeneratorDecl;
use crate::form_sizing::{Resized, Sizer, SizingDecl, Targets};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const MANNEQUIN_ID: &str = "makehuman-mannequin";
/// Largest disagreement, as a fraction, between a form's declared height and its mesh.
pub const HEIGHT_TOLERANCE: f64 = 0.02;
/// How far below or above y = 0 a mesh's lowest point may sit.
pub const FLOOR_TOLERANCE_M: f64 = 0.01;
const MAX_FORM_JSON_BYTES: usize = 64 * 1024;
const MAX_ID_LEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarKind {
    Mannequin,
    DressForm,
}

/// One choice in the picker. The mesh path is not serialised: the API never exposes filesystem paths.
#[derive(Clone, Debug, Serialize)]
pub struct AvatarEntry {
    pub id: String,
    pub title: String,
    pub kind: AvatarKind,
    #[serde(skip)]
    pub mesh: PathBuf,
    pub available: bool,
    pub reason: Option<String>,
    pub height_m: Option<f64>,
    /// Where the garment's top edge is placed. `None` leaves a garment where its template put it.
    pub shoulder_height_m: Option<f64>,
    pub dimensions_cm: BTreeMap<String, f64>,
    /// Landmarks and size chart, when the form can be resized.
    pub sizing: Option<SizingDecl>,
    /// True when the mesh is built from a profile instead of read from a file.
    pub generated: bool,
    #[serde(skip)]
    pub generator: Option<GeneratorDecl>,
    pub source: String,
    pub license: String,
}

/// The on-disk `form.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FormFile {
    id: String,
    title: String,
    /// A file name inside the form's own directory. Give this or `generator`, not both.
    #[serde(default)]
    mesh: Option<String>,
    #[serde(default)]
    generator: Option<GeneratorDecl>,
    height_m: f64,
    shoulder_height_m: f64,
    #[serde(default)]
    dimensions_cm: BTreeMap<String, f64>,
    source: String,
    license: String,
    /// Free text: how the dimensions and the scale were obtained.
    #[serde(default)]
    #[allow(dead_code)]
    provenance: String,
    #[serde(default)]
    sizing: Option<SizingDecl>,
}

/// An avatar whose mesh was read and checked.
pub struct LoadedAvatar {
    pub id: String,
    pub surface: Arc<SurfaceMesh>,
    pub shoulder_height_m: Option<f64>,
    /// Present when the form declares landmarks, so it can be resized.
    pub sizer: Option<Sizer>,
    /// Landmark heights as this form stands, by name (empty when the form declares none).
    pub landmarks: std::collections::BTreeMap<String, f64>,
    /// Vertices below this height are the stand, not the torso.
    pub torso_from_m: f64,
    /// Separate meshes (removable arms), by name: not part of `surface`, so they are not collided with.
    pub parts: std::collections::BTreeMap<String, Arc<SurfaceMesh>>,
}

impl LoadedAvatar {
    /// Tape lines and seam lines on this form, cut from its mesh.
    pub fn guide_lines(&self) -> Vec<crate::form_lines::GuideLine> {
        crate::form_lines::guide_lines(
            &self.surface,
            &crate::form_lines::LineSpec {
                landmarks: &self.landmarks,
                torso_from_m: self.torso_from_m,
            },
        )
    }

    /// The same form resized to `targets`; the shoulder line moves with the landmarks.
    pub fn sized(&self, targets: &Targets) -> Result<(LoadedAvatar, Resized), String> {
        let sizer = self.sizer.as_ref().ok_or("this avatar cannot be resized")?;
        let resized = sizer.resize(targets)?;
        let loaded = LoadedAvatar {
            id: self.id.clone(),
            surface: Arc::new(resized.surface.clone()),
            shoulder_height_m: self.shoulder_height_m.map(|y| resized.map_y(y)),
            sizer: None,
            landmarks: resized.knot_y.clone(),
            torso_from_m: self.torso_from_m,
            parts: self.parts.clone(),
        };
        Ok((loaded, resized))
    }
}

/// Where the assets live: `CLOTH_ASSETS`, else `assets` beside the working directory.
pub fn assets_root() -> PathBuf {
    std::env::var_os("CLOTH_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets"))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A bare `*.obj` file name: no separators, no parent references.
fn valid_mesh_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\'])
        && name != "."
        && name != ".."
        && name.to_ascii_lowercase().ends_with(".obj")
}

fn mannequin(root: &Path) -> AvatarEntry {
    let mesh = root.join("makehuman").join("mannequin.obj");
    let present = mesh.is_file();
    AvatarEntry {
        id: MANNEQUIN_ID.to_owned(),
        title: "MakeHuman mannequin (1.75 m)".to_owned(),
        kind: AvatarKind::Mannequin,
        available: present,
        reason: (!present).then(|| "assets/makehuman/mannequin.obj is missing".to_owned()),
        mesh,
        height_m: Some(1.75),
        shoulder_height_m: None,
        dimensions_cm: BTreeMap::new(),
        sizing: None,
        generated: false,
        generator: None,
        source:
            "MakeHuman community base mesh, extracted and normalised by scripts/prepare-avatar.py"
                .to_owned(),
        license: "CC0-1.0".to_owned(),
    }
}

fn unavailable(id: String, title: String, reason: String) -> AvatarEntry {
    AvatarEntry {
        id,
        title,
        kind: AvatarKind::DressForm,
        mesh: PathBuf::new(),
        available: false,
        reason: Some(reason),
        height_m: None,
        shoulder_height_m: None,
        dimensions_cm: BTreeMap::new(),
        sizing: None,
        generated: false,
        generator: None,
        source: String::new(),
        license: String::new(),
    }
}

fn read_form(dir: &Path) -> Result<AvatarEntry, String> {
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_owned();
    let path = dir.join("form.json");
    let bytes = fs::read(&path).map_err(|e| format!("form.json: {e}"))?;
    if bytes.len() > MAX_FORM_JSON_BYTES {
        return Err("form.json is larger than 64 KiB".to_owned());
    }
    let form: FormFile = serde_json::from_slice(&bytes).map_err(|e| format!("form.json: {e}"))?;
    if !valid_id(&form.id) {
        return Err("form.json: id must be lowercase letters, digits and hyphens".to_owned());
    }
    if form.id != dir_name {
        return Err(format!(
            "form.json: id {:?} must equal its directory name {dir_name:?}",
            form.id
        ));
    }
    match (&form.mesh, &form.generator) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(
                "form.json: give either mesh or generator, not both and not neither".to_owned(),
            );
        }
        (Some(mesh), None) if !valid_mesh_name(mesh) => {
            return Err(
                "form.json: mesh must be a bare .obj file name in the same directory".to_owned(),
            );
        }
        _ => {}
    }
    if !(form.height_m.is_finite() && form.height_m > 0.2 && form.height_m < 3.0) {
        return Err("form.json: height_m must be a length in metres between 0.2 and 3".to_owned());
    }
    if !(form.shoulder_height_m.is_finite()
        && form.shoulder_height_m > 0.0
        && form.shoulder_height_m <= form.height_m)
    {
        return Err(
            "form.json: shoulder_height_m must be positive and not above height_m".to_owned(),
        );
    }
    if let Some(sizing) = &form.sizing {
        sizing.validate(form.height_m)?;
    }
    if let Some(generator) = &form.generator {
        generator
            .validate()
            .map_err(|e| format!("form.json: {e}"))?;
        let built = generator.height_m();
        if (built - form.height_m).abs() > HEIGHT_TOLERANCE * form.height_m {
            return Err(format!(
                "form.json: height_m is {:.3} but the generator builds a form {built:.3} m tall",
                form.height_m
            ));
        }
    }
    let mesh_name = form.mesh.clone().unwrap_or_default();
    let mesh = dir.join(&mesh_name);
    let generated = form.generator.is_some();
    let present = generated || mesh.is_file();
    Ok(AvatarEntry {
        id: form.id,
        title: form.title,
        kind: AvatarKind::DressForm,
        available: present,
        reason: (!present).then(|| format!("{mesh_name} is not installed in {dir_name}/")),
        mesh,
        height_m: Some(form.height_m),
        shoulder_height_m: Some(form.shoulder_height_m),
        dimensions_cm: form.dimensions_cm,
        sizing: form.sizing,
        generated,
        generator: form.generator,
        source: form.source,
        license: form.license,
    })
}

/// The mannequin first, then every dress form directory in name order. A directory that cannot
/// be read is listed as unavailable with the reason, never silently skipped.
pub fn discover(root: &Path) -> Vec<AvatarEntry> {
    let mut entries = vec![mannequin(root)];
    let Ok(listing) = fs::read_dir(root.join("dressforms")) else {
        return entries;
    };
    let mut dirs: Vec<PathBuf> = listing
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();
        entries.push(match read_form(&dir) {
            Ok(entry) => entry,
            Err(reason) => unavailable(name.clone(), name, reason),
        });
    }
    entries
}

impl AvatarEntry {
    /// Read the mesh and check it against what the entry declares.
    pub fn load(&self) -> Result<LoadedAvatar, String> {
        if !self.available {
            return Err(self
                .reason
                .clone()
                .unwrap_or_else(|| "avatar is not available".to_owned()));
        }
        let mut surface = match &self.generator {
            Some(generator) => generator.build()?,
            None => SurfaceMesh::load_obj(&self.mesh)?,
        };
        if let Some(declared) = self.height_m.filter(|_| self.kind == AvatarKind::DressForm) {
            check_height(&surface, declared)?;
        }
        let mut shoulder = self.shoulder_height_m;
        let mut sizing = self.sizing.clone();
        // A generated form is built to a row of its size chart, so what it measures at first is that row.
        if let (Some(generator), Some(decl)) = (&self.generator, &sizing) {
            if let Some(row) = generator.default_chart_row {
                let chart = decl
                    .chart
                    .get(row)
                    .ok_or("default_chart_row is not a row of the chart")?;
                let sizer = Sizer::new(Arc::new(surface.clone()), decl)?;
                let resized = sizer.resize(&Targets::from_flat(&chart.cm)?)?;
                shoulder = shoulder.map(|y| resized.map_y(y));
                let mut moved = decl.clone();
                for knot in &mut moved.knots {
                    knot.y_m = resized.knot_y[&knot.name];
                }
                surface = resized.surface;
                sizing = Some(moved);
            }
        }
        let surface = Arc::new(surface);
        let sizer = match &sizing {
            Some(decl) => Some(Sizer::new(Arc::clone(&surface), decl)?),
            None => None,
        };
        Ok(LoadedAvatar {
            id: self.id.clone(),
            surface,
            parts: match &self.generator {
                Some(g) => g
                    .build_parts()?
                    .into_iter()
                    .map(|(name, mesh)| (name, Arc::new(mesh)))
                    .collect(),
                None => Default::default(),
            },
            landmarks: sizer.as_ref().map(|s| s.knot_heights()).unwrap_or_default(),
            torso_from_m: sizing
                .as_ref()
                .and_then(|d| d.fixed_below_m)
                .unwrap_or(f64::NEG_INFINITY),
            shoulder_height_m: shoulder,
            sizer,
        })
    }
}

/// Refuse a mesh whose size or floor disagrees with its declared height.
pub fn check_height(surface: &SurfaceMesh, declared_m: f64) -> Result<(), String> {
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in &surface.vertices {
        low = low.min(v.y);
        high = high.max(v.y);
    }
    if !low.is_finite() || !high.is_finite() {
        return Err("the mesh has no vertices".to_owned());
    }
    let measured = high - low;
    if (measured - declared_m).abs() > HEIGHT_TOLERANCE * declared_m {
        return Err(format!(
            "the mesh is {measured:.3} m tall but its form.json declares {declared_m:.3} m; \
             OBJ files are read as metres and never rescaled, so this looks like a unit error"
        ));
    }
    if low.abs() > FLOOR_TOLERANCE_M {
        return Err(format!(
            "the mesh starts at y = {low:.3} m; a form is expected to stand on y = 0"
        ));
    }
    Ok(())
}

/// The vertical shift that puts a garment's top edge on the avatar's shoulder line.
/// `None` (the mannequin) leaves the garment where its template placed it.
pub fn garment_lift(garment_top_y: f64, shoulder_height_m: Option<f64>) -> f64 {
    shoulder_height_m.map_or(0.0, |shoulder| shoulder - garment_top_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_mesh_names_are_restricted() {
        assert!(valid_id("dressform-scan-v01"));
        for bad in ["", "Form", "a b", "a/b", "..", "a_b"] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert!(valid_mesh_name("torso.obj"));
        for bad in ["../torso.obj", "a/b.obj", "torso.glb", "", ".."] {
            assert!(!valid_mesh_name(bad), "{bad}");
        }
    }

    #[test]
    fn lift_puts_the_garment_top_on_the_shoulder() {
        assert_eq!(garment_lift(1.40, Some(0.64)), 0.64 - 1.40);
        assert_eq!(garment_lift(1.40, None), 0.0);
    }
}
