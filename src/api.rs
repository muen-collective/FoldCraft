//! Local HTTP API for the React studio.
//!
//! Loopback only, one resident document, no authentication: this is the local
//! editor's own socket, not a service. Every mutation goes through the same
//! Session the MCP server uses, so guards and validation cannot drift apart.
use crate::inspect::{preview_paths, summarize_document};
use crate::session_client::{Client, SessionError};
use crate::template_catalog::Selection;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, MutexGuard};

type Shared = Arc<Mutex<Client>>;
static SIM: std::sync::Mutex<Option<crate::preview::Preview>> = std::sync::Mutex::new(None);
/// The body the preview drapes on: an `avatar_catalog` id, or none. Selecting resets the preview.
static AVATAR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

const MAX_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;

pub fn router() -> std::io::Result<Router> {
    let state: Shared = Arc::new(Mutex::new(Client::new()?));
    Ok(Router::new()
        .route("/api/health", get(health))
        .route("/api/catalog", get(catalog))
        .route("/api/document", get(document))
        .route("/api/preview", get(preview))
        .route("/api/open", post(open))
        .route("/api/upload", post(upload))
        .route("/api/edges", get(edges))
        .route("/api/import/dxf/preview", post(import_preview))
        .route("/api/import/dxf/apply", post(import_apply))
        .route("/api/command", post(command))
        .route("/api/prepare", post(prepare))
        .route("/api/commit", post(commit))
        .route("/api/cancel", post(cancel))
        .route("/api/export/svg", get(export_svg))
        .route("/api/export/print-svg", get(export_print_svg))
        .route("/api/export/dxf", get(export_dxf))
        .route("/api/scene", get(scene))
        .route("/api/sim/step", post(sim_step))
        .route("/api/sim/reset", post(sim_reset))
        .route("/api/avatars", get(avatars))
        .route("/api/avatar", post(select_avatar))
        .route("/api/avatar/mesh", get(avatar_mesh))
        .route("/api/undo", post(undo))
        .route("/api/redo", post(redo))
        .with_state(state))
}

fn lock(state: &Shared) -> Result<MutexGuard<'_, Client>, (StatusCode, Json<Value>)> {
    state.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": {"code": "poisoned", "detail": "The document session did not recover"}})),
        )
    })
}

fn rejected(error: SessionError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": error.code, "detail": error.detail}})),
    )
}

fn bad_request(detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": "invalid_arguments", "detail": detail.into()}})),
    )
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "service": "cloth-server", "version": env!("CARGO_PKG_VERSION")}))
}

async fn catalog() -> Json<Value> {
    Json(json!({
        "templates": crate::template_catalog::ENTRIES
            .iter()
            .map(|entry| json!({
                "id": entry.id,
                "title": entry.title,
                "description": entry.description,
                "license": entry.license,
            }))
            .collect::<Vec<_>>(),
    }))
}

async fn document(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.snapshot().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn preview(State(state): State<Shared>) -> ApiResult {
    let template = lock(&state)?.template().map_err(rejected)?;
    Ok(Json(preview_paths(&template).map_err(rejected)?))
}

/// The annotated pattern preview SVG of the resident document. Rest geometry
/// only; never a simulated shape.
async fn export_svg(State(state): State<Shared>) -> Result<String, (StatusCode, Json<Value>)> {
    let template = lock(&state)?.template().map_err(rejected)?;
    let parsed = parse_template(&template)?;
    Ok(marvelous_cloth::garment::export::pattern_svg(&parsed))
}

/// The physical 1:1 millimetre sheet with stored allowances and notches when
/// present. Untiled single sheet: not a certified cutting pattern.
async fn export_print_svg(
    State(state): State<Shared>,
) -> Result<String, (StatusCode, Json<Value>)> {
    let template = lock(&state)?.template().map_err(rejected)?;
    let parsed = parse_template(&template)?;
    marvelous_cloth::garment::export::print_pattern_svg(&parsed)
        .map_err(|error| bad_request(format!("print svg failed: {error}")))
}

/// Plain flat DXF of the resident document: millimetres, one closed contour
/// per panel with a PIECE NAME record, curved edges as bulged segments. No
/// AAMA/ASTM grade or size records - that dialect is separate work.
async fn export_dxf(State(state): State<Shared>) -> Result<String, (StatusCode, Json<Value>)> {
    let template = lock(&state)?.template().map_err(rejected)?;
    let parsed = parse_template(&template)?;
    Ok(marvelous_cloth::garment::export::dxf::flat_dxf(&parsed))
}

/// Store an uploaded .dxf/.json under .uploads; return its absolute path.
async fn upload(request: axum::extract::Request) -> ApiResult {
    crate::upload::upload(request).await
}

/// Named boundaries sampled into board coordinates, for picking and the edge
/// inspector. Read-only; rest geometry only, never draped positions.
async fn edges(State(state): State<Shared>) -> ApiResult {
    let template = lock(&state)?.template().map_err(rejected)?;
    let list = crate::inspect::board_edges(&template).map_err(rejected)?;
    Ok(Json(serde_json::json!({ "units": "metres", "edges": list })))
}

/// Preview a DXF without touching the resident document.
/// Body: {"path": string, "metres_per_unit"?: number}
async fn import_preview(Json(body): Json<Value>) -> ApiResult {
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("path is required"))?;
    let mut payload = json!({"op": "import_preview", "path": path});
    if let Some(scale) = body.get("metres_per_unit").and_then(Value::as_f64) {
        payload["metres_per_unit"] = json!(scale);
    }
    // A fresh session answers previews; the resident document is never read or
    // written, so a preview cannot disturb an editing person.
    let mut client = Client::new().map_err(|e| rejected(SessionError::new("io_failure", e.to_string())))?;
    let data = client.call(payload).map_err(rejected)?;
    Ok(Json(data))
}

/// Append selected closed contours as editable panels, one history entry.
/// Body: {"path": string, "selected": [index], "fabric": id, "metres_per_unit"?: number}
async fn import_apply(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("path is required"))?;
    let fabric = body
        .get("fabric")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("fabric is required"))?;
    let selected = body
        .get("selected")
        .cloned()
        .ok_or_else(|| bad_request("selected is required"))?;
    let mut client = lock(&state)?;
    let mut payload = json!({
        "op": "import_apply",
        "guard": client.guard(),
        "path": path,
        "selected": selected,
        "fabric": fabric,
    });
    if let Some(scale) = body.get("metres_per_unit").and_then(Value::as_f64) {
        payload["metres_per_unit"] = json!(scale);
    }
    let data = client.call(payload).map_err(rejected)?;
    Ok(Json(summarize_document(&data)))
}

/// Body: {"catalog": id} | {"path": path} | {"template": {...}}
async fn open(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let template = if let Some(id) = body.get("catalog").and_then(Value::as_str) {
        let selection =
            Selection::bundled(id).map_err(|error| bad_request(error))?;
        serde_json::to_value(selection.template())
            .map_err(|error| bad_request(error.to_string()))?
    } else if let Some(path) = body.get("path").and_then(Value::as_str) {
        let bytes = std::fs::read(path).map_err(|error| bad_request(format!("{path}: {error}")))?;
        if bytes.len() > MAX_TEMPLATE_BYTES {
            return Err(bad_request("Template exceeds 8 MiB"));
        }
        serde_json::from_slice::<Value>(&bytes).map_err(|error| bad_request(error.to_string()))?
    } else if let Some(template) = body.get("template") {
        template.clone()
    } else {
        return Err(bad_request("Pass catalog, path or template"));
    };
    let snapshot = lock(&state)?.open(template).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn command(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let command = body
        .get("command")
        .cloned()
        .ok_or_else(|| bad_request("command object is required"))?;
    let snapshot = lock(&state)?.edit(command).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn prepare(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let command = body
        .get("command")
        .cloned()
        .ok_or_else(|| bad_request("command object is required"))?;
    let data = lock(&state)?.prepare(command).map_err(rejected)?;
    Ok(Json(json!({
        "change": data.get("change"),
        "documentSession": data.get("documentSession"),
        "revision": data.get("revision"),
        "resetsPhysicalCaches": data.get("resetsPhysicalCaches"),
        "preview": crate::inspect::summarize_template(data.get("template").unwrap_or(&Value::Null)),
    })))
}

async fn commit(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let change = body
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("change token is required"))?;
    let snapshot = lock(&state)?.commit(change).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn cancel(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let change = body
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("change token is required"))?;
    let snapshot = lock(&state)?.cancel(change).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

// Rest-shape mesh of the resident template. Static geometry; no solver has
/// run and nothing here claims a fit.
async fn scene(State(state): State<Shared>) -> ApiResult {
    let template = lock(&state)?.template().map_err(rejected)?;
    let parsed = parse_template(&template)?;
    let mesh = crate::preview::rest_mesh(&parsed).map_err(server_error)?;
    Ok(Json(mesh))
}

async fn sim_step(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let steps = body.get("steps").and_then(Value::as_u64).unwrap_or(1).clamp(1, 240) as usize;
    let (template, revision) = {
        let mut client = lock(&state)?;
        let snapshot = client.snapshot().map_err(rejected)?;
        (
            snapshot.get("template").cloned().ok_or_else(|| bad_request("no document"))?,
            snapshot.get("revision").and_then(Value::as_u64).unwrap_or(0),
        )
    };
    let parsed = parse_template(&template)?;
    let mut guard = SIM.lock().map_err(|e| bad_request(e.to_string()))?;
    if guard.as_ref().is_some_and(|preview| preview.stale(revision)) || guard.is_none() {
        let avatar = load_selected().map_err(avatar_failure)?;
        *guard = Some(
            crate::preview::Preview::from_template_with_avatar(&parsed, revision, avatar.as_ref())
                .map_err(server_error)?,
        );
    }
    let preview = guard.as_mut().expect("just built");
    let (done, failure) = preview.step(steps);
    let mut snapshot = preview.snapshot();
    snapshot["stepsDone"] = json!(done);
    if let Some(error) = failure {
        snapshot["solverError"] = json!(error);
    }
    Ok(Json(snapshot))
}

async fn sim_reset(State(state): State<Shared>) -> ApiResult {
    if let Ok(mut guard) = SIM.lock() {
        *guard = None;
    }
    let _ = state;
    Ok(Json(json!({"reset": true})))
}

fn avatar_failure(detail: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": "avatar_unavailable", "detail": detail}})),
    )
}

fn selected_avatar_id() -> Option<String> {
    AVATAR.lock().ok().and_then(|guard| guard.clone())
}

/// The selected body, read and checked; `None` when no body is selected.
fn load_selected() -> Result<Option<crate::avatar_catalog::LoadedAvatar>, String> {
    let Some(id) = selected_avatar_id() else {
        return Ok(None);
    };
    let entries = crate::avatar_catalog::discover(&crate::avatar_catalog::assets_root());
    let entry = entries
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("avatar {id} is no longer installed"))?;
    entry.load().map(Some)
}

/// Every body the preview can use, with whether its mesh is installed and why not.
async fn avatars() -> Json<Value> {
    let entries = crate::avatar_catalog::discover(&crate::avatar_catalog::assets_root());
    Json(json!({"selected": selected_avatar_id(), "avatars": entries}))
}

/// Choose the body (`{"id": "..."}`) or none (`{"id": null}`). The mesh is read and checked
/// here, so a unit error is reported at selection, not at the first step.
async fn select_avatar(Json(body): Json<Value>) -> ApiResult {
    let id = match body.get("id") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) => Some(id.clone()),
        Some(_) => return Err(bad_request("id must be a string or null")),
    };
    if let Some(id) = &id {
        let entries = crate::avatar_catalog::discover(&crate::avatar_catalog::assets_root());
        let entry = entries
            .iter()
            .find(|entry| &entry.id == id)
            .ok_or_else(|| bad_request(format!("unknown avatar {id}")))?;
        entry.load().map_err(avatar_failure)?;
    }
    *AVATAR.lock().map_err(|e| bad_request(e.to_string()))? = id.clone();
    if let Ok(mut sim) = SIM.lock() {
        *sim = None;
    }
    Ok(Json(json!({"selected": id})))
}

/// The selected body's surface, for drawing. Static geometry in metres, to the tenth of a millimetre.
async fn avatar_mesh() -> ApiResult {
    let avatar = load_selected()
        .map_err(avatar_failure)?
        .ok_or_else(|| bad_request("no avatar is selected"))?;
    let round = |value: f64| (value * 10_000.0).round() / 10_000.0;
    Ok(Json(json!({
        "id": avatar.id,
        "units": "metres",
        "shoulder_height_m": avatar.shoulder_height_m,
        "positions": avatar
            .surface
            .vertices
            .iter()
            .map(|p| [round(p.x), round(p.y), round(p.z)])
            .collect::<Vec<_>>(),
        "triangles": avatar.surface.triangles.clone(),
    })))
}

fn parse_template(value: &Value) -> Result<crate::garment::GarmentTemplate, (StatusCode, Json<Value>)> {
    serde_json::from_value(value.clone())
        .map_err(|e| bad_request(format!("invalid template: {e}")))
}

fn server_error(error: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": "preview_failed", "detail": error}})),
    )
}

async fn undo(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.undo().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn redo(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.redo().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}
