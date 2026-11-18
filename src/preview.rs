//! A CPU drape preview beside the authoring session.
//!
//! The scene rebuilds when the template revision changes. Stepping runs the
//! interactive VBD solver on CPU - the same one the desktop worker uses - and
//! returns draped positions. This is a preview: it names its solver and never
//! claims a fit. With no avatar selected no body is attached. With one selected
//! (`avatar_catalog`) the garment is lifted onto the body's shoulder line and the
//! solver gets the body mesh as its collider; the snapshot says which, and by how
//! much it moved the garment.
use crate::{
    avatar_catalog::{LoadedAvatar, garment_lift},
    garment::GarmentTemplate,
    model::State,
    vbd::{VbdSettings, VbdSolver},
};
use serde_json::{Value, json};
use std::sync::Arc;

/// Steps per second the preview aims for at 240 Hz substeps.
pub const STEP_HZ: usize = 240;

pub struct Preview {
    signature: u64,
    garment: Arc<crate::garment::CompiledGarment>,
    state: State,
    settings: VbdSettings,
    solver: VbdSolver,
    avatar_id: Option<String>,
    lift_m: f64,
}

/// Flat rest-shape mesh in world arrangement coordinates. Static geometry;
/// no solver has run.
pub fn rest_mesh(template: &GarmentTemplate) -> Result<Value, String> {
    let compiled = template.compile()?;
    Ok(json!({
        "units": "metres",
        "positions": compiled
            .arrangement_positions
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect::<Vec<_>>(),
        "panels": panel_triangles(&compiled),
    }))
}

fn panel_triangles(garment: &crate::garment::CompiledGarment) -> Vec<Value> {
    garment
        .panels
        .iter()
        .map(|range| {
            let tris: Vec<[u32; 3]> = garment.mesh.triangles[range.triangles.clone()]
                .iter()
                .map(|t| t.vertices.map(|v| v as u32))
                .collect();
            json!({"panel": range.id, "triangles": tris})
        })
        .collect()
}

impl Preview {
    pub fn from_template(template: &GarmentTemplate, signature: u64) -> Result<Self, String> {
        Self::from_template_with_avatar(template, signature, None)
    }

    /// Build the preview, optionally on a body. The template given is never modified: the lift
    /// is applied to a copy, so the document and the preview cannot drift apart.
    pub fn from_template_with_avatar(
        template: &GarmentTemplate,
        signature: u64,
        avatar: Option<&LoadedAvatar>,
    ) -> Result<Self, String> {
        let mut arranged = template.clone();
        let mut lift_m = 0.0;
        if let Some(shoulder) = avatar.and_then(|a| a.shoulder_height_m) {
            let top = arranged
                .compile()?
                .arrangement_positions
                .iter()
                .map(|p| p.y)
                .fold(f64::NEG_INFINITY, f64::max);
            lift_m = garment_lift(top, Some(shoulder));
            for panel in &mut arranged.panels {
                panel.placement.translation.y += lift_m;
            }
        }
        let garment = Arc::new(arranged.compile()?);
        let state = garment.initial_state.clone();
        let solver = VbdSolver::new(
            Arc::clone(&garment),
            avatar.map(|a| a.surface.as_ref()),
            &state,
        )?;
        Ok(Self {
            signature,
            garment,
            state,
            settings: VbdSettings::default(),
            solver,
            avatar_id: avatar.map(|a| a.id.clone()),
            lift_m,
        })
    }

    pub fn stale(&self, signature: u64) -> bool {
        self.signature != signature
    }

    /// Advance the interactive solver. Each call is bounded so one HTTP step
    /// request cannot pin a core indefinitely.
    pub fn step(&mut self, steps: usize) -> (usize, Option<String>) {
        for index in 0..steps {
            if let Err(error) = self.solver.step(&mut self.state, &self.settings, None) {
                return (index, Some(error));
            }
        }
        (steps, None)
    }

    pub fn snapshot(&self) -> Value {
        json!({
            "solver": if self.avatar_id.is_some() {
                "CPU VBD (interactive, avatar contact)"
            } else {
                "CPU VBD (interactive, no avatar contact)"
            },
            "avatar": self.avatar_id,
            "garment_lift_m": self.lift_m,
            "claim": "preview, not a fit",
            "time_s": self.state.time,
            "dt": self.settings.dt,
            "positions": self
                .state
                .x
                .iter()
                .map(|p| [p.x, p.y, p.z])
                .collect::<Vec<_>>(),
            "panels": panel_triangles(&self.garment),
        })
    }
}
