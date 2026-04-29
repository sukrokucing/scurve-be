use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::authz::Principal;
use crate::errors::AppError;
use crate::jwt::AuthUser;
use crate::models::menu::{DbMenu, MenuResponse, MenusResponse};

/// Get navigation menus for the current user
///
/// Returns only the menus the authenticated user has permission to see.
/// The list is filtered server-side using the user's real RBAC permissions —
/// the client renders whatever it receives without additional permission checks.
///
/// **Caching:** The response includes an `ETag` header and `Cache-Control: private, max-age=300`.
/// On subsequent requests, send the ETag as `If-None-Match`; a `304 Not Modified` response
/// means the client's cached list is still valid and no body is returned.
///
/// The ETag encodes both the menu `version` (bumped when menus change) and a fingerprint
/// of the user's current permission set, so it correctly invalidates when permissions change.
#[utoipa::path(
    get,
    path = "/menus",
    tag = "Menus",
    params(
        ("If-None-Match" = Option<String>, Header,
         description = "ETag from a previous response. Returns 304 if still valid.")
    ),
    responses(
        (status = 200, description = "Filtered menu list for the current user", body = MenusResponse),
        (status = 304, description = "Not Modified — ETag matches, client cache is current"),
        (status = 401, description = "Authentication required"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn list_menus(
    State(state): State<AppState>,
    auth: AuthUser,
    req_headers: HeaderMap,
) -> Result<Response, AppError> {
    // Load the user's full permission set (cached in-process).
    let principal = Principal::load(auth.user_id, &state.pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;

    // Current menu schema version — used for cache invalidation.
    let version: i64 = sqlx::query_scalar("SELECT version FROM menu_version WHERE id = 1")
        .fetch_one(&state.pool)
        .await?;

    // ETag = "v{version}-{first 16 hex chars of sha256(sorted permissions)}"
    // Changes whenever version is bumped OR the user's permission set changes.
    let mut sorted_perms: Vec<&str> = principal.permissions.iter().map(String::as_str).collect();
    sorted_perms.sort_unstable();
    let perm_hash = format!("{:x}", Sha256::digest(sorted_perms.join(",").as_bytes()));
    let etag = format!("\"v{}-{}\"", version, &perm_hash[..16]);

    // Short-circuit: client's cached copy is still valid.
    if let Some(if_none_match) = req_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        if if_none_match == etag {
            let mut h = HeaderMap::new();
            h.insert(
                header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or_else(|_| HeaderValue::from_static("")),
            );
            return Ok((StatusCode::NOT_MODIFIED, h).into_response());
        }
    }

    // Fetch all non-hidden menus in display order.
    let db_menus = sqlx::query_as::<_, DbMenu>(
        "SELECT id, label, description, route, section, priority, surfaces, icon, keywords, \
         required_permission, disabled \
         FROM menus WHERE hidden = 0 ORDER BY section, priority ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Filter: keep menus where required_permission is NULL or the user has it.
    let menus: Vec<MenuResponse> = db_menus
        .into_iter()
        .filter(|m| {
            !m.disabled
                && m.required_permission
                    .as_deref()
                    .map_or(true, |p| principal.permissions.contains(p))
        })
        .map(|m| MenuResponse {
            id: m.id,
            label: m.label,
            description: m.description,
            route: m.route,
            section: m.section,
            priority: m.priority,
            surfaces: serde_json::from_str(&m.surfaces).unwrap_or_default(),
            icon: m.icon,
            keywords: serde_json::from_str(&m.keywords).unwrap_or_default(),
        })
        .collect();

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).map_err(|_| AppError::internal("invalid etag value"))?,
    );
    resp_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=300"),
    );

    Ok((
        StatusCode::OK,
        resp_headers,
        Json(MenusResponse { version, menus }),
    )
        .into_response())
}
