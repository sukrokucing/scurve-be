mod app;
mod db;
mod errors;
mod jwt;
mod models;
mod routes;
mod utils;

// route modules are referenced by their `#[utoipa::path]` annotations; we don't need
// to import the module symbols here, avoid unused-import warnings.
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    // Use per-handler `#[utoipa::path]` annotations to register paths. Do not list `paths(...)` here
    // to avoid duplicate registrations when handlers also emit path metadata via macros.
    components(
        schemas(
            models::user::User,
            models::user::AuthResponse,
            models::user::LoginRequest,
            models::user::RegisterRequest,
            models::project::Project,
            models::project::ProjectCreateRequest,
            models::project::ProjectUpdateRequest,
            models::task::Task,
            models::task::TaskCreateRequest,
            models::task::TaskUpdateRequest
        )
    ),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Projects", description = "Project management"),
        (name = "Tasks", description = "Task management")
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    init_tracing();

    let pool = db::init().await?;
    let app = app::create_app(pool).await?;

    // compute port early so we can inject a `servers` entry into the OpenAPI doc
    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8000);

    // generate OpenAPI and inject a bearer auth security scheme so Swagger UI can use the Authorize dialog
    let openapi = ApiDoc::openapi();

    // serialize to JSON and mutate the JSON to add the bearerAuth scheme and global security requirement
    let mut openapi_json = serde_json::to_value(&openapi)
        .expect("Failed to serialize OpenAPI to JSON");

    // Helper: merge two serde_json::Value objects recursively (objects and arrays).
    fn merge_values(a: &mut serde_json::Value, b: &serde_json::Value) {
        match (a, b) {
            (serde_json::Value::Object(ma), serde_json::Value::Object(mb)) => {
                for (k, vb) in mb {
                    if let Some(va) = ma.get_mut(k) {
                        merge_values(va, vb);
                    } else {
                        ma.insert(k.clone(), vb.clone());
                    }
                }
            }
            (serde_json::Value::Array(aa), serde_json::Value::Array(ba)) => {
                for item in ba {
                    if !aa.contains(item) {
                        aa.push(item.clone());
                    }
                }
            }
            // For primitives or mismatched types, prefer keeping the existing value `a`.
            _ => {}
        }
    }

    // If the generated OpenAPI has no `paths` (some derive states won't collect per-handler paths),
    // synthesize minimal path objects for the primary endpoints so Swagger UI has a usable "Try it out"
    // experience. We only add these if paths is empty.
    let _need_synth = match openapi_json.get("paths") {
        Some(v) => v.as_object().map(|m| m.is_empty()).unwrap_or(true),
        None => true,
    };

        // If paths are absent, synthesize minimal endpoints for Auth/Projects/Tasks so
        // Swagger UI's Try-it-out works without requiring the derive to populate paths.
        if _need_synth {
            let mut paths = serde_json::Map::new();

            paths.insert(
                "/auth/register".to_string(),
                serde_json::json!({
                    "post": {
                        "tags": ["Auth"],
                        "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/RegisterRequest"}}}},
                        "responses": {"201": {"description": "User registered", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AuthResponse"}}}}, "409": {"description": "Email already in use"}}
                    }
                }),
            );

            paths.insert(
                "/auth/login".to_string(),
                serde_json::json!({
                    "post": {
                        "tags": ["Auth"],
                        "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/LoginRequest"}}}},
                        "responses": {"200": {"description": "Login successful", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AuthResponse"}}}}, "401": {"description": "Invalid credentials"}}
                    }
                }),
            );

            paths.insert(
                "/auth/me".to_string(),
                serde_json::json!({
                    "get": {
                        "tags": ["Auth"],
                        "security": [{"bearerAuth": []}],
                        "responses": {"200": {"description": "Current user", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/User"}}}}}
                    }
                }),
            );

            paths.insert(
                "/auth/logout".to_string(),
                serde_json::json!({
                    "post": {"tags": ["Auth"], "security": [{"bearerAuth": []}], "responses": {"200": {"description": "Logout acknowledged"}}}
                }),
            );

            paths.insert(
                "/projects".to_string(),
                serde_json::json!({
                    "get": {
                        "tags": ["Projects"],
                        "security": [{"bearerAuth": []}],
                        "responses": {
                            "200": {
                                "description": "List projects",
                                "content": {
                                    "application/json": {
                                        "schema": {"type": "array", "items": {"$ref": "#/components/schemas/Project"}}
                                    }
                                }
                            }
                        }
                    },
                    "post": {
                        "tags": ["Projects"],
                        "security": [{"bearerAuth": []}],
                        "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/ProjectCreateRequest"}}}},
                        "responses": {"201": {"description": "Project created", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Project"}}}}}
                    }
                }),
            );

            paths.insert(
                "/projects/{id}".to_string(),
                serde_json::json!({
                    "get": {
                        "tags": ["Projects"],
                        "security": [{"bearerAuth": []}],
                        "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
                        "responses": {"200": {"description": "Project detail", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Project"}}}}}
                    },
                    "put": {
                        "tags": ["Projects"],
                        "security": [{"bearerAuth": []}],
                        "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
                        "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/ProjectUpdateRequest"}}}},
                        "responses": {"200": {"description": "Project updated", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Project"}}}}}
                    },
                    "delete": {
                        "tags": ["Projects"],
                        "security": [{"bearerAuth": []}],
                        "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
                        "responses": {"204": {"description": "Project soft deleted"}}
                    }
                }),
            );

            paths.insert(
                "/tasks".to_string(),
                serde_json::json!({
                    "get": {
                        "tags": ["Tasks"],
                        "security": [{"bearerAuth": []}],
                        "responses": {"200": {"description": "List tasks", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Task"}}}}}}
                    },
                    "post": {
                        "tags": ["Tasks"],
                        "security": [{"bearerAuth": []}],
                        "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/TaskCreateRequest"}}}},
                        "responses": {"201": {"description": "Task created", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}}}
                    }
                }),
            );

            paths.insert(
                "/tasks/{id}".to_string(),
                serde_json::json!({
                    "put": {"tags": ["Tasks"], "security": [{"bearerAuth": []}], "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/TaskUpdateRequest"}}}}, "responses": {"200": {"description": "Task updated", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}}}},
                    "delete": {"tags": ["Tasks"], "security": [{"bearerAuth": []}], "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"204": {"description": "Task soft deleted"}}}
                }),
            );

            // Merge synthesized `paths` into the generated OpenAPI `paths` to avoid
            // duplicated mapping keys. If a path/method already exists (emitted by
            // per-handler `#[utoipa::path]` derive), merge the operation objects
            // instead of inserting a second method key.

            // Ensure openapi_json has an object for paths
            if openapi_json.get("paths").is_none() {
                openapi_json["paths"] = serde_json::Value::Object(serde_json::Map::new());
            }

            if let Some(paths_obj) = openapi_json.get_mut("paths").and_then(|p| p.as_object_mut()) {
                for (pname, pval) in paths {
                    if let Some(existing) = paths_obj.get_mut(pname.as_str()) {
                        // both should be objects
                        if let (Some(existing_obj), Some(new_obj)) = (existing.as_object_mut(), pval.as_object()) {
                            for (method, new_op) in new_obj {
                                if let Some(existing_op) = existing_obj.get_mut(method.as_str()) {
                                    // merge operation objects (requestBody, responses, parameters, tags, security)
                                    merge_values(existing_op, new_op);
                                } else {
                                    existing_obj.insert(method.clone(), new_op.clone());
                                }
                            }
                        } else {
                            // Replace non-object existing entry with the synthesized one
                            paths_obj.insert(pname, pval);
                        }
                    } else {
                        paths_obj.insert(pname, pval);
                    }
                }
            }
            }
    // Note: removed synthesized `paths` fallback to avoid large inline json! blocks and potential
    // mismatches. The OpenAPI path metadata should be emitted by per-handler `#[utoipa::path]`
    // annotations. If the derive produces an empty `paths` in some environments, we can add a
    // smaller, safer fallback later.

    // Sanitize paths: ensure there are no duplicate operation keys per path (defensive)
    // Some generator states can accidentally produce duplicate method entries which break Swagger's parser.
    if let Some(paths) = openapi_json.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (path, item) in paths.clone() {
            if let Some(obj) = item.as_object() {
                let mut seen = std::collections::HashSet::new();
                let mut new_obj = serde_json::Map::new();
                for (method, val) in obj.iter() {
                    // keep the first occurrence of a method key
                    if seen.insert(method.clone()) {
                        new_obj.insert(method.clone(), val.clone());
                    }
                }
                paths.insert(path, serde_json::Value::Object(new_obj));
            }
        }
    }

    // Additional normalization: ensure method keys are lowercased and fully deduplicated
    // by merging any remaining duplicates (case-insensitive). This prevents YAML/OpenAPI
    // parsers from reporting "duplicated mapping key" when keys differ only by case.
    if let Some(paths) = openapi_json.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (path, item) in paths.clone() {
            if let Some(obj) = item.as_object() {
                let mut normalized = serde_json::Map::new();
                for (method, val) in obj.iter() {
                    let key = method.to_lowercase();
                    if !normalized.contains_key(&key) {
                        normalized.insert(key.clone(), val.clone());
                    } else {
                        // merge the values to preserve examples/responses without duplicating keys
                        if let Some(existing) = normalized.get_mut(&key) {
                            merge_values(existing, val);
                        }
                    }
                }
                paths.insert(path, serde_json::Value::Object(normalized));
            }
        }
    }

    // components.securitySchemes.bearerAuth
    openapi_json
        .pointer_mut("/components")
        .and_then(|c| c.as_object_mut())
        .unwrap()
        .entry("securitySchemes")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(schemes) = openapi_json
        .pointer_mut("/components/securitySchemes")
        .and_then(|s| s.as_object_mut())
    {
        schemes.insert(
            "bearerAuth".to_string(),
            serde_json::json!({
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "JWT"
            }),
        );
    }

        // Ensure there's a top-level security requirement for bearerAuth so Swagger UI's
        // Authorize dialog will send the Authorization header for endpoints that use the
        // scheme. Some endpoints (register/login) can still omit operation-level security.
        if openapi_json.get("security").is_none() {
            openapi_json["security"] = serde_json::json!([{"bearerAuth": []}]);
        }

    // set global security requirement
    openapi_json
        .as_object_mut()
        .unwrap()
        .entry("openapi")
        .or_insert_with(|| serde_json::json!("3.1.0"));

    // Add helpful examples for parameters and request bodies so Swagger UI's "Try it out" shows usable payloads.
    if let Some(paths) = openapi_json.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (_path, item) in paths.iter_mut() {
            if let Some(op_map) = item.as_object_mut() {
                for (_method, op_val) in op_map.iter_mut() {
                    // parameters: add example for common path param `id`
                    if let Some(params) = op_val.get_mut("parameters").and_then(|p| p.as_array_mut()) {
                        for param in params.iter_mut() {
                            if let Some(name) = param.get("name").and_then(|n| n.as_str()) {
                                if name == "id" {
                                    if let Some(obj) = param.as_object_mut() {
                                        obj.entry("example").or_insert_with(|| {
                                            serde_json::json!("00000000-0000-0000-0000-000000000000")
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // requestBody: insert sensible examples for known request schemas
                    if let Some(rb) = op_val.get_mut("requestBody") {
                        if let Some(content) = rb.get_mut("content").and_then(|c| c.as_object_mut()) {
                            if let Some(appjson) = content.get_mut("application/json").and_then(|a| a.as_object_mut()) {
                                if let Some(schema) = appjson.get_mut("schema") {
                                    if let Some(obj) = schema.as_object() {
                                        if let Some(r) = obj.get("$ref").and_then(|r| r.as_str()) {
                                            match r {
                                                "#/components/schemas/LoginRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "email": "ada@example.com",
                                                            "password": "S3cureP@ssw0rd"
                                                        }),
                                                    );
                                                }
                                                "#/components/schemas/RegisterRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "name": "Ada Lovelace",
                                                            "email": "ada@example.com",
                                                            "password": "S3cureP@ssw0rd"
                                                        }),
                                                    );
                                                }
                                                "#/components/schemas/ProjectCreateRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "name": "Launch Planning",
                                                            "description": "Prepare milestones for the product launch.",
                                                            "theme_color": "#3498db"
                                                        }),
                                                    );
                                                }
                                                "#/components/schemas/ProjectUpdateRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "name": "Launch Planning - Updated",
                                                            "description": "Updated description",
                                                            "theme_color": "#2ecc71"
                                                        }),
                                                    );
                                                }
                                                "#/components/schemas/TaskCreateRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "project_id": "00000000-0000-0000-0000-000000000000",
                                                            "title": "Define launch checklist",
                                                            "status": "pending",
                                                            "due_date": "2025-10-10T10:00:00Z"
                                                        }),
                                                    );
                                                }
                                                "#/components/schemas/TaskUpdateRequest" => {
                                                    appjson.insert(
                                                        "example".to_string(),
                                                        serde_json::json!({
                                                            "title": "Refine checklist",
                                                            "status": "in_progress",
                                                            "due_date": "2025-11-01T10:00:00Z"
                                                        }),
                                                    );
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Add examples for response bodies (single objects and arrays) so Try-it-out shows realistic responses.
    if let Some(paths) = openapi_json.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (_path, item) in paths.iter_mut() {
            if let Some(op_map) = item.as_object_mut() {
                for (_method, op_val) in op_map.iter_mut() {
                    if let Some(responses) = op_val.get_mut("responses").and_then(|r| r.as_object_mut()) {
                        for (_status, resp_val) in responses.iter_mut() {
                            if let Some(content) = resp_val.get_mut("content").and_then(|c| c.as_object_mut()) {
                                if let Some(appjson) = content.get_mut("application/json").and_then(|a| a.as_object_mut()) {
                                    if let Some(schema_val) = appjson.get("schema").cloned() {
                                        // direct $ref responses: inspect a cloned schema Value to avoid
                                        // multiple mutable borrows of `appjson`.
                                        if let Some(obj) = schema_val.as_object() {
                                            if let Some(rref) = obj.get("$ref").and_then(|r| r.as_str()) {
                                                match rref {
                                                    "#/components/schemas/AuthResponse" => {
                                                        appjson.insert(
                                                            "example".to_string(),
                                                            serde_json::json!({
                                                                "token": "eyJhbGciOiJIUzI1Ni...",
                                                                "user": {
                                                                    "id": "00000000-0000-0000-0000-000000000000",
                                                                    "name": "Ada Lovelace",
                                                                    "email": "ada@example.com",
                                                                    "provider": "local",
                                                                    "provider_id": null,
                                                                    "created_at": "2025-10-01T10:00:00Z",
                                                                    "updated_at": "2025-10-01T10:00:00Z",
                                                                    "deleted_at": null
                                                                }
                                                            }),
                                                        );
                                                    }
                                                    "#/components/schemas/User" => {
                                                        appjson.insert(
                                                            "example".to_string(),
                                                            serde_json::json!({
                                                                "id": "00000000-0000-0000-0000-000000000000",
                                                                "name": "Ada Lovelace",
                                                                "email": "ada@example.com",
                                                                "provider": "local",
                                                                "provider_id": null,
                                                                "created_at": "2025-10-01T10:00:00Z",
                                                                "updated_at": "2025-10-01T10:00:00Z",
                                                                "deleted_at": null
                                                            }),
                                                        );
                                                    }
                                                    "#/components/schemas/Project" => {
                                                        appjson.insert(
                                                            "example".to_string(),
                                                            serde_json::json!({
                                                                "id": "00000000-0000-0000-0000-000000000000",
                                                                "user_id": "11111111-1111-1111-1111-111111111111",
                                                                "name": "Launch Planning",
                                                                "description": "Prepare milestones for the product launch.",
                                                                "theme_color": "#3498db",
                                                                "created_at": "2025-10-01T10:00:00Z",
                                                                "updated_at": "2025-10-01T10:00:00Z",
                                                                "deleted_at": null
                                                            }),
                                                        );
                                                    }
                                                    "#/components/schemas/Task" => {
                                                        appjson.insert(
                                                            "example".to_string(),
                                                            serde_json::json!({
                                                                "id": "22222222-2222-2222-2222-222222222222",
                                                                "project_id": "00000000-0000-0000-0000-000000000000",
                                                                "title": "Define launch checklist",
                                                                "status": "pending",
                                                                "due_date": "2025-10-10T10:00:00Z",
                                                                "created_at": "2025-10-01T10:00:00Z",
                                                                "updated_at": "2025-10-01T10:00:00Z",
                                                                "deleted_at": null
                                                            }),
                                                        );
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }

                                        // array responses: inspect cloned schema for type == array
                                        if let Some(obj) = schema_val.as_object() {
                                            if obj.get("type").and_then(|t| t.as_str()) == Some("array") {
                                                if let Some(items) = obj.get("items").and_then(|i| i.as_object()) {
                                                    if let Some(item_ref) = items.get("$ref").and_then(|r| r.as_str()) {
                                                        match item_ref {
                                                            "#/components/schemas/Project" => {
                                                                appjson.insert(
                                                                    "example".to_string(),
                                                                    serde_json::json!([
                                                                        {
                                                                            "id": "00000000-0000-0000-0000-000000000000",
                                                                            "user_id": "11111111-1111-1111-1111-111111111111",
                                                                            "name": "Launch Planning",
                                                                            "description": "Prepare milestones for the product launch.",
                                                                            "theme_color": "#3498db",
                                                                            "created_at": "2025-10-01T10:00:00Z",
                                                                            "updated_at": "2025-10-01T10:00:00Z",
                                                                            "deleted_at": null
                                                                        }
                                                                    ]),
                                                                );
                                                            }
                                                            "#/components/schemas/Task" => {
                                                                appjson.insert(
                                                                    "example".to_string(),
                                                                    serde_json::json!([
                                                                        {
                                                                            "id": "22222222-2222-2222-2222-222222222222",
                                                                            "project_id": "00000000-0000-0000-0000-000000000000",
                                                                            "title": "Define launch checklist",
                                                                            "status": "pending",
                                                                            "due_date": "2025-10-10T10:00:00Z",
                                                                            "created_at": "2025-10-01T10:00:00Z",
                                                                            "updated_at": "2025-10-01T10:00:00Z",
                                                                            "deleted_at": null
                                                                        }
                                                                    ]),
                                                                );
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // add servers entry so Swagger UI's Try-it-out uses the running backend by default
    if openapi_json.get("servers").is_none() {
        openapi_json["servers"] = serde_json::json!([
            { "url": format!("http://localhost:{}", port) }
        ]);
    }

    // convert back to the typed OpenApi
    let mut openapi_with_security: utoipa::openapi::OpenApi = serde_json::from_value(openapi_json)?;

    // Final sanitize pass: round-trip the typed OpenApi back to a JSON Value and
    // ensure method keys are unique (lowercased) and merged. This protects against
    // duplicate mapping keys that may still appear due to generator states or
    // previous merge ordering. We then convert the sanitized JSON back into the
    // typed OpenApi to hand to the Swagger UI.
    let mut sanit_val = serde_json::to_value(&openapi_with_security)?;
    if let Some(paths) = sanit_val.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (path, item) in paths.clone() {
            if let Some(obj) = item.as_object() {
                let mut normalized = serde_json::Map::new();
                for (method, val) in obj.iter() {
                    let key = method.to_lowercase();
                    if !normalized.contains_key(&key) {
                        normalized.insert(key.clone(), val.clone());
                    } else {
                        if let Some(existing) = normalized.get_mut(&key) {
                            merge_values(existing, val);
                        }
                    }
                }
                paths.insert(path, serde_json::Value::Object(normalized));
            }
        }
    }

    let openapi_final: utoipa::openapi::OpenApi = serde_json::from_value(sanit_val)?;

    // Use the fully sanitized OpenAPI document for Swagger UI (overwrite the
    // earlier value produced from the first pass).
    openapi_with_security = openapi_final;

    // Configure Swagger UI to enable Try-it-out by default and allow credentials to be sent
    let swagger_config = utoipa_swagger_ui::Config::new(["/api-docs/openapi.json"])
        .try_it_out_enabled(true)
        .with_credentials(true)
        .persist_authorization(true);

    // Instead of embedding the OpenAPI typed object into the Swagger UI (which may
    // re-serialize generator-internal structures and re-introduce duplicate keys),
    // serve the already-sanitized OpenAPI JSON at /api-docs/openapi.json and let the
    // Swagger UI fetch it at runtime. This guarantees the client sees the normalized
    // JSON we produced above.
    let openapi_value = serde_json::to_value(&openapi_with_security)?;
    let openapi_value_clone = openapi_value.clone();

    // register a simple GET route that returns the sanitized JSON
    let docs_route = axum::Router::new().route(
        "/api-docs/openapi.json",
        axum::routing::get(move || {
            let v = openapi_value_clone.clone();
            async move { axum::Json(v) }
        }),
    );

    let app = app.merge(docs_route).merge(
        SwaggerUi::new("/docs").config(swagger_config),
    );

    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8000);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

fn load_env() {
    if dotenvy::dotenv().is_ok() {
        return;
    }

    let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    let _ = dotenvy::from_path(crate_env);
}

fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
