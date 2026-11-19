//! Bounded file upload for the studio: a browser picks a DXF or native
//! template, the server stores it under .uploads and returns its absolute
//! path. The stored name is a generated slug; no client filename reaches the
//! filesystem. Only .dxf and .json suffixes are accepted, at most 16 MiB.
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

const MAX_UPLOAD_BYTES: usize = 16 * 1024 * 1024;
const DOUBLE_QUOTE: char = '"';
const SLASHES: [char; 2] = ['/', '\\'];

pub async fn upload(
    request: axum::extract::Request,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let deny = |detail: String| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": {"code": "invalid_upload", "detail": detail}})),
        )
    };
    let content_type = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .map(|rest| rest.trim_matches(';').trim().trim_matches('"').to_owned())
        .ok_or_else(|| deny("multipart boundary missing".into()))?;
    let bytes = axum::body::to_bytes(request.into_body(), MAX_UPLOAD_BYTES)
        .await
        .map_err(|error| deny(format!("upload too large: {error}")))?;

    // Find the part carrying a filename; that payload is what we store.
    let marker = format!("--{boundary}");
    let mut suffix: Option<String> = None;
    let mut payload: Option<Vec<u8>> = None;
    for part in split_bytes(&bytes, marker.as_bytes()).into_iter().skip(1) {
        let header_end = find_subsequence(part, b"\r\n\r\n")
            .map(|at| at + 4)
            .or_else(|| find_subsequence(part, b"\n\n").map(|at| at + 2));
        let Some(header_end) = header_end else {
            continue;
        };
        let headers = String::from_utf8_lossy(&part[..header_end]).to_string();
        let Some(name_at) = headers.find("filename=") else {
            continue;
        };
        let after = &headers[name_at + "filename=".len()..];
        let after = after.trim_start_matches('=');
        let quoted = after.starts_with(DOUBLE_QUOTE);
        let candidate = if quoted {
            let rest = &after[1..];
            rest.find(DOUBLE_QUOTE).map_or(rest, |end| &rest[..end])
        } else {
            after.split([';', '\r', '\n']).next().unwrap_or("")
        };
        let base = candidate.rsplit(SLASHES).next().unwrap_or(candidate);
        let extension = base.rsplit('.').next().unwrap_or("").to_lowercase();
        if !matches!(extension.as_str(), "dxf" | "json") {
            return Err(deny(format!(
                "rejected suffix .{extension}: only .dxf and .json"
            )));
        }
        suffix = Some(extension);
        let mut body = part[header_end..].to_vec();
        while body.ends_with(b"\r\n") {
            body.truncate(body.len() - 2);
        }
        while body.ends_with(&[b'\n']) {
            body.pop();
        }
        payload = Some(body);
        break;
    }
    let payload = payload.ok_or_else(|| deny("no file part found".into()))?;
    let suffix = suffix.expect("set with payload");
    let slug = format!("{:016x}.{}", rand_token(), suffix);
    std::fs::create_dir_all(".uploads").map_err(|e| deny(e.to_string()))?;
    let full = std::path::PathBuf::from(".uploads").join(&slug);
    std::fs::write(&full, &payload).map_err(|e| deny(e.to_string()))?;
    let absolute = std::fs::canonicalize(&full).map_err(|e| deny(e.to_string()))?;
    Ok(Json(json!({
        "path": absolute.to_string_lossy(),
        "bytes": payload.len(),
    })))
}

fn rand_token() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    hasher.finish()
}

/// Split a byte slice on a multi-byte separator.
fn split_bytes<'a>(haystack: &'a [u8], needle: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = vec![];
    let mut start = 0usize;
    while let Some(offset) = find_subsequence(&haystack[start..], needle) {
        parts.push(&haystack[start..start + offset]);
        start += offset + needle.len();
    }
    parts.push(&haystack[start..]);
    parts
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
