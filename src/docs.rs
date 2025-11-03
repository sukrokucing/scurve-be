use std::sync::Arc;

use axum::{routing::get, Json, Router};
use serde_json::{json, Map, Value};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::models;

#[derive(OpenApi)]
#[openapi(
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
pub struct ApiDoc;

pub fn build_openapi(port: u16) -> anyhow::Result<utoipa::openapi::OpenApi> {
	let mut doc = serde_json::to_value(&ApiDoc::openapi())?;

	ensure_paths(&mut doc);
	normalize_path_operations(&mut doc);
	ensure_security_components(&mut doc);
	ensure_global_security(&mut doc);
	ensure_openapi_version(&mut doc);
	add_examples(&mut doc);
	ensure_servers(&mut doc, port);

	let doc: utoipa::openapi::OpenApi = serde_json::from_value(doc)?;
	sanitize_methods(doc)
}

pub fn swagger_routes(doc: utoipa::openapi::OpenApi) -> Router {
	let swagger_config = utoipa_swagger_ui::Config::new(["/api-docs/openapi.json"])
		.try_it_out_enabled(true)
		.with_credentials(true)
		.persist_authorization(true);

	let doc_json = Arc::new(serde_json::to_value(&doc).expect("OpenAPI serialization must succeed"));

	let json_route = {
		let doc_json = Arc::clone(&doc_json);
		get(move || {
			let doc_json = Arc::clone(&doc_json);
			async move { Json((*doc_json).clone()) }
		})
	};

	Router::new()
		.route("/api-docs/openapi.json", json_route)
		.merge(SwaggerUi::new("/docs").config(swagger_config))
}

fn sanitize_methods(doc: utoipa::openapi::OpenApi) -> anyhow::Result<utoipa::openapi::OpenApi> {
	let mut value = serde_json::to_value(&doc)?;
	normalize_path_operations(&mut value);
	Ok(serde_json::from_value(value)?)
}

fn ensure_paths(doc: &mut Value) {
	let need_synth = doc
		.get("paths")
		.and_then(Value::as_object)
		.map(|paths| paths.is_empty())
		.unwrap_or(true);

	if !need_synth {
		return;
	}

	let paths_object = doc
		.as_object_mut()
		.expect("OpenAPI root must be an object")
		.entry("paths")
		.or_insert_with(|| Value::Object(Map::new()))
		.as_object_mut()
		.expect("paths must be an object");

	for (path, value) in synthetic_paths() {
		if let Some(existing) = paths_object.get_mut(path.as_str()) {
			merge_values(existing, &value);
		} else {
			paths_object.insert(path, value);
		}
	}
}

fn synthetic_paths() -> Map<String, Value> {
	let mut paths = Map::new();

	paths.insert(
		"/auth/register".to_string(),
		json!({
			"post": {
				"tags": ["Auth"],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/RegisterRequest"}}}},
				"responses": {
					"201": {"description": "User registered", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AuthResponse"}}}},
					"409": {"description": "Email already in use"}
				}
			}
		}),
	);

	paths.insert(
		"/auth/login".to_string(),
		json!({
			"post": {
				"tags": ["Auth"],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/LoginRequest"}}}},
				"responses": {
					"200": {"description": "Login successful", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AuthResponse"}}}},
					"401": {"description": "Invalid credentials"}
				}
			}
		}),
	);

	paths.insert(
		"/auth/me".to_string(),
		json!({
			"get": {
				"tags": ["Auth"],
				"security": [{"bearerAuth": []}],
				"responses": {
					"200": {
						"description": "Current user",
						"content": {"application/json": {"schema": {"$ref": "#/components/schemas/User"}}}
					}
				}
			}
		}),
	);

	paths.insert(
		"/auth/logout".to_string(),
		json!({
			"post": {
				"tags": ["Auth"],
				"security": [{"bearerAuth": []}],
				"responses": {"200": {"description": "Logout acknowledged"}}
			}
		}),
	);

	paths.insert(
		"/projects".to_string(),
		json!({
			"get": {
				"tags": ["Projects"],
				"security": [{"bearerAuth": []}],
				"responses": {
					"200": {
						"description": "List projects",
						"content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Project"}}}}
					}
				}
			},
			"post": {
				"tags": ["Projects"],
				"security": [{"bearerAuth": []}],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/ProjectCreateRequest"}}}},
				"responses": {
					"201": {
						"description": "Project created",
						"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Project"}}}
					}
				}
			}
		}),
	);

	paths.insert(
		"/projects/{id}".to_string(),
		json!({
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
		json!({
			"get": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"responses": {
					"200": {
						"description": "List tasks",
						"content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/Task"}}}}
					}
				}
			},
			"post": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/TaskCreateRequest"}}}},
				"responses": {
					"201": {
						"description": "Task created",
						"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}
					}
				}
			}
		}),
	);

	paths.insert(
		"/tasks/{id}".to_string(),
		json!({
			"put": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/TaskUpdateRequest"}}}},
				"responses": {"200": {"description": "Task updated", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}}}
			},
			"delete": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
				"responses": {"204": {"description": "Task soft deleted"}}
			}
		}),
	);

	paths
}

fn normalize_path_operations(doc: &mut Value) {
	if let Some(paths) = doc.get_mut("paths").and_then(Value::as_object_mut) {
		let snapshot = paths.clone();
		for (path, item) in snapshot {
			if let Some(ops) = item.as_object() {
				let mut normalized = Map::new();
				for (method, val) in ops {
					let key = method.to_lowercase();
					if let Some(existing) = normalized.get_mut(&key) {
						merge_values(existing, &val);
					} else {
						normalized.insert(key, val.clone());
					}
				}
				paths.insert(path, Value::Object(normalized));
			}
		}
	}
}

fn ensure_security_components(doc: &mut Value) {
	let components = doc
		.as_object_mut()
		.expect("OpenAPI root must be an object")
		.entry("components")
		.or_insert_with(|| Value::Object(Map::new()))
		.as_object_mut()
		.expect("components must be an object");

	let schemes = components
		.entry("securitySchemes")
		.or_insert_with(|| Value::Object(Map::new()))
		.as_object_mut()
		.expect("securitySchemes must be an object");

	schemes.insert(
		"bearerAuth".to_string(),
		json!({
			"type": "http",
			"scheme": "bearer",
			"bearerFormat": "JWT"
		}),
	);
}

fn ensure_global_security(doc: &mut Value) {
	doc
		.as_object_mut()
		.expect("OpenAPI root must be an object")
		.entry("security")
		.or_insert_with(|| json!([{ "bearerAuth": [] }]));
}

fn ensure_openapi_version(doc: &mut Value) {
	doc
		.as_object_mut()
		.expect("OpenAPI root must be an object")
		.entry("openapi")
		.or_insert_with(|| Value::String("3.1.0".to_string()));
}

fn add_examples(doc: &mut Value) {
	if let Some(paths) = doc.get_mut("paths").and_then(Value::as_object_mut) {
		for item in paths.values_mut() {
			if let Some(operations) = item.as_object_mut() {
				for operation in operations.values_mut() {
					apply_parameter_examples(operation);
					apply_request_examples(operation);
					apply_response_examples(operation);
				}
			}
		}
	}
}

fn apply_parameter_examples(operation: &mut Value) {
	if let Some(parameters) = operation
		.get_mut("parameters")
		.and_then(Value::as_array_mut)
	{
		for parameter in parameters.iter_mut() {
			if let Some(name) = parameter.get("name").and_then(Value::as_str) {
				if name == "id" {
					if let Some(obj) = parameter.as_object_mut() {
						obj.entry("example").or_insert_with(|| {
							json!("00000000-0000-0000-0000-000000000000")
						});
					}
				}
			}
		}
	}
}

fn apply_request_examples(operation: &mut Value) {
	let Some(request_body) = operation.get_mut("requestBody") else { return; };
	let Some(content) = request_body.get_mut("content").and_then(Value::as_object_mut) else { return; };
	let Some(app_json) = content.get_mut("application/json").and_then(Value::as_object_mut) else { return; };
	let Some(schema) = app_json.get("schema").and_then(Value::as_object) else { return; };
	let Some(reference) = schema.get("$ref").and_then(Value::as_str) else { return; };

	let example = match reference {
		"#/components/schemas/LoginRequest" => Some(json!({
			"email": "ada@example.com",
			"password": "S3cureP@ssw0rd"
		})),
		"#/components/schemas/RegisterRequest" => Some(json!({
			"name": "Ada Lovelace",
			"email": "ada@example.com",
			"password": "S3cureP@ssw0rd"
		})),
		"#/components/schemas/ProjectCreateRequest" => Some(json!({
			"name": "Launch Planning",
			"description": "Prepare milestones for the product launch.",
			"theme_color": "#3498db"
		})),
		"#/components/schemas/ProjectUpdateRequest" => Some(json!({
			"name": "Launch Planning - Updated",
			"description": "Updated description",
			"theme_color": "#2ecc71"
		})),
		"#/components/schemas/TaskCreateRequest" => Some(json!({
			"project_id": "00000000-0000-0000-0000-000000000000",
			"title": "Define launch checklist",
			"status": "pending",
			"due_date": "2025-10-10T10:00:00Z"
		})),
		"#/components/schemas/TaskUpdateRequest" => Some(json!({
			"title": "Refine checklist",
			"status": "in_progress",
			"due_date": "2025-11-01T10:00:00Z"
		})),
		_ => None,
	};

	if let Some(example) = example {
		app_json.insert("example".to_string(), example);
	}
}

fn apply_response_examples(operation: &mut Value) {
	let Some(responses) = operation.get_mut("responses").and_then(Value::as_object_mut) else { return; };

	for response in responses.values_mut() {
		let Some(content) = response.get_mut("content").and_then(Value::as_object_mut) else { continue; };
		let Some(app_json) = content.get_mut("application/json").and_then(Value::as_object_mut) else { continue; };

		let schema = app_json.get("schema").cloned();
		if let Some(schema) = schema {
			if let Some(r#ref) = schema.get("$ref").and_then(Value::as_str) {
				let example = match r#ref {
					"#/components/schemas/AuthResponse" => Some(json!({
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
					})),
					"#/components/schemas/User" => Some(json!({
						"id": "00000000-0000-0000-0000-000000000000",
						"name": "Ada Lovelace",
						"email": "ada@example.com",
						"provider": "local",
						"provider_id": null,
						"created_at": "2025-10-01T10:00:00Z",
						"updated_at": "2025-10-01T10:00:00Z",
						"deleted_at": null
					})),
					"#/components/schemas/Project" => Some(json!({
						"id": "00000000-0000-0000-0000-000000000000",
						"user_id": "11111111-1111-1111-1111-111111111111",
						"name": "Launch Planning",
						"description": "Prepare milestones for the product launch.",
						"theme_color": "#3498db",
						"created_at": "2025-10-01T10:00:00Z",
						"updated_at": "2025-10-01T10:00:00Z",
						"deleted_at": null
					})),
					"#/components/schemas/Task" => Some(json!({
						"id": "22222222-2222-2222-2222-222222222222",
						"project_id": "00000000-0000-0000-0000-000000000000",
						"title": "Define launch checklist",
						"status": "pending",
						"due_date": "2025-10-10T10:00:00Z",
						"created_at": "2025-10-01T10:00:00Z",
						"updated_at": "2025-10-01T10:00:00Z",
						"deleted_at": null
					})),
					_ => None,
				};

				if let Some(example) = example {
					app_json.insert("example".to_string(), example);
					continue;
				}
			}

			if schema
				.get("type")
				.and_then(Value::as_str)
				.map(|kind| kind == "array")
				.unwrap_or(false)
			{
				if let Some(items) = schema.get("items").and_then(Value::as_object) {
					if let Some(item_ref) = items.get("$ref").and_then(Value::as_str) {
						let example = match item_ref {
							"#/components/schemas/Project" => Some(json!([{
								"id": "00000000-0000-0000-0000-000000000000",
								"user_id": "11111111-1111-1111-1111-111111111111",
								"name": "Launch Planning",
								"description": "Prepare milestones for the product launch.",
								"theme_color": "#3498db",
								"created_at": "2025-10-01T10:00:00Z",
								"updated_at": "2025-10-01T10:00:00Z",
								"deleted_at": null
							}])),
							"#/components/schemas/Task" => Some(json!([{
								"id": "22222222-2222-2222-2222-222222222222",
								"project_id": "00000000-0000-0000-0000-000000000000",
								"title": "Define launch checklist",
								"status": "pending",
								"due_date": "2025-10-10T10:00:00Z",
								"created_at": "2025-10-01T10:00:00Z",
								"updated_at": "2025-10-01T10:00:00Z",
								"deleted_at": null
							}])),
							_ => None,
						};

						if let Some(example) = example {
							app_json.insert("example".to_string(), example);
						}
					}
				}
			}
		}
	}
}

fn ensure_servers(doc: &mut Value, port: u16) {
	// Determine whether the running server will use TLS. If CERT_PATH+KEY_PATH are
	// provided (or USE_SELF_SIGNED_TLS is set), prefer https so Swagger Try-it-out
	// will call the backend over TLS.
	let tls_enabled = std::env::var("CERT_PATH").is_ok() && std::env::var("KEY_PATH").is_ok()
		|| std::env::var("USE_SELF_SIGNED_TLS").is_ok();

	let scheme = if tls_enabled { "https" } else { "http" };

	let server_url = format!("{}://localhost:{}", scheme, port);

	match doc.get_mut("servers") {
		Some(Value::Array(arr)) => {
			// ensure an entry for our server_url exists
			let has = arr.iter().any(|v| v.get("url").and_then(Value::as_str) == Some(server_url.as_str()));
			if !has {
				arr.push(json!({ "url": server_url }));
			}
		}
		_ => {
			doc["servers"] = json!([{ "url": server_url }]);
		}
	}
}

fn merge_values(target: &mut Value, addition: &Value) {
	match (target, addition) {
		(Value::Object(dest), Value::Object(src)) => {
			for (key, value) in src {
				if let Some(existing) = dest.get_mut(key) {
					merge_values(existing, value);
				} else {
					dest.insert(key.clone(), value.clone());
				}
			}
		}
		(Value::Array(dest), Value::Array(src)) => {
			for item in src {
				if !dest.contains(item) {
					dest.push(item.clone());
				}
			}
		}
		_ => {}
	}
}
