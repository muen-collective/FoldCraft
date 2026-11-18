//! A CPU drape preview beside the authoring session.
//!
//! The scene rebuilds when the template revision changes. Stepping runs the
//! interactive VBD solver on CPU - the same one the desktop worker uses - and
//! returns draped positions. This is a preview: it names its solver and never
//! claims a fit. No avatar collider is attached; body contact needs an
//! authored fitting project and stays out of this door.
use crate::{
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
        let garment = Arc::new(template.compile()?);
        let state = garment.initial_state.clone();
        let solver = VbdSolver::new(Arc::clone(&garment), None, &state)?;
        Ok(Self {
            signature,
            garment,
            state,
            settings: VbdSettings::default(),
            solver,
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
            "solver": "CPU VBD (interactive, no avatar contact)",
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
