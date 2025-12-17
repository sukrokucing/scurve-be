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
			models::task::TaskUpdateRequest,
			models::progress::Progress,
			models::progress::ProgressCreateRequest,
			models::progress::ProgressUpdateRequest,
			models::dependency::TaskDependency,
			models::dependency::DependencyCreateRequest,
			models::task::TaskBatchUpdatePayload,
			models::task::TaskBatchUpdateRequest,
			models::project_plan::ProjectPlanCreateRequest
			,models::project_plan::ProjectPlanPoint
			,crate::routes::projects::ActualPoint
			,crate::routes::projects::DashboardResponse
			,crate::routes::projects::CriticalPathResponse
			,crate::routes::health::HealthResponse
			,crate::models::rbac::Role
			,crate::models::rbac::RoleCreateRequest
			,crate::models::rbac::Permission
			,crate::models::rbac::PermissionCreateRequest
			,crate::models::rbac::UserRole
			,crate::models::rbac::RolePermission
			,crate::models::rbac::UserPermission
			,crate::models::rbac::EffectivePermissions
			,crate::models::rbac::EffectivePermission
			,crate::models::rbac::AssignRoleRequest
			,crate::models::rbac::AssignPermissionToRoleRequest
			,crate::models::rbac::GrantPermissionRequest
		)
	),
	paths(
		crate::routes::auth::register,
		crate::routes::auth::login,
		crate::routes::auth::me,
		crate::routes::auth::logout,

		crate::routes::projects::list_projects,
		crate::routes::projects::create_project,
		crate::routes::projects::get_project,
		crate::routes::projects::update_project,
		crate::routes::projects::delete_project,
		crate::routes::projects::update_project_plan,
		crate::routes::projects::clear_project_plan,
		crate::routes::projects::get_project_dashboard,
		crate::routes::projects::get_project_critical_path,

		crate::routes::tasks::list_tasks,
		crate::routes::tasks::create_task,
		crate::routes::tasks::get_task,
		crate::routes::tasks::update_task,
		crate::routes::tasks::delete_task,
		crate::routes::tasks::batch_update_tasks,
		crate::routes::tasks::list_dependencies,
		crate::routes::tasks::create_dependency,
		crate::routes::tasks::delete_dependency,

		crate::routes::progress::list_progress,
		crate::routes::progress::get_progress,
		crate::routes::progress::create_progress,
		crate::routes::progress::update_progress,
		crate::routes::progress::delete_progress
		,crate::routes::health::health,

		crate::routes::rbac::list_roles,
		crate::routes::rbac::create_role,
		crate::routes::rbac::get_role,
		crate::routes::rbac::delete_role,
		crate::routes::rbac::get_role_permissions,
		crate::routes::rbac::assign_permission_to_role,
        crate::routes::rbac::delete_permission_from_role,
		crate::routes::rbac::list_permissions,
		crate::routes::rbac::create_permission,
		crate::routes::rbac::get_user_roles,
		crate::routes::rbac::assign_role_to_user,
		crate::routes::rbac::revoke_role_from_user,
		crate::routes::rbac::get_user_permissions,
		crate::routes::rbac::grant_permission_to_user,
		crate::routes::rbac::get_effective_permissions
	),
	tags(
		(name = "Auth", description = "Authentication endpoints"),
		(name = "Projects", description = "Project management"),
		(name = "Tasks", description = "Task management"),
		(name = "Progress", description = "Task progress entries"),
		(name = "RBAC", description = "Role-Based Access Control")
	)
)]
pub struct ApiDoc;

pub fn build_openapi(port: u16) -> anyhow::Result<utoipa::openapi::OpenApi> {
	let mut doc = serde_json::to_value(&ApiDoc::openapi())?;

	ensure_paths(&mut doc);
	// ensure_additional_paths(&mut doc); // Removed as get_project_dashboard is now in paths macro
	normalize_path_operations(&mut doc);
	ensure_security_components(&mut doc);
	ensure_global_security(&mut doc);
	ensure_openapi_version(&mut doc);
	add_examples(&mut doc);
	ensure_servers(&mut doc, port);

	// Debug: dump the generated OpenAPI JSON to a temp file so we can inspect
	// any unexpected shapes that may cause serde deserialization errors.
	if let Ok(s) = serde_json::to_string_pretty(&doc) {
		let _ = std::fs::write("/tmp/openapi-debug.json", s);
	}

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
		"/projects/{id}/dashboard".to_string(),
		json!({
			"get": {
				"tags": ["Projects"],
				"security": [{"bearerAuth": []}],
				"parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
				"responses": {
					"200": {
						"description": "Project dashboard (plan vs actual)",
						"content": {"application/json": {"schema": {"$ref": "#/components/schemas/DashboardResponse"}}}
					}
				}
			}
		}),
	);
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
		"/projects/{project_id}/tasks".to_string(),
		json!({
			"get": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [
					{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
					{"name": "progress", "in": "query", "required": false, "schema": {"type": "boolean"}, "description": "Set to true to list progress entries instead of tasks"},
					{"name": "task_id", "in": "query", "required": false, "schema": {"type": "string", "format": "uuid"}, "description": "Optional task id to filter progress"}
				],
				"responses": {
					"200": {
						"description": "List tasks or progress entries",
						"content": {"application/json": {"schema": {"oneOf": [{"type": "array", "items": {"$ref": "#/components/schemas/Task"}}, {"type": "array", "items": {"$ref": "#/components/schemas/Progress"}}]}}}
					}
				}
			},
			"post": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
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
		"/projects/{project_id}/tasks/{id}".to_string(),
		json!({
			"get": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [
					{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
					{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}
				],
				"responses": {"200": {"description": "Task detail", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}, "404": {"description": "Not found"}}}
			},
			"put": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [
					{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
					{"name": "id", "in": "path", "required": false, "schema": {"type": "string", "format": "uuid"}}
				],
				"requestBody": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/TaskUpdateRequest"}}}},
				"responses": {"200": {"description": "Task updated", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}}}
			},
			"delete": {
				"tags": ["Tasks"],
				"security": [{"bearerAuth": []}],
				"parameters": [
					{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
					{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}
				],
				"responses": {"204": {"description": "Task soft deleted"}}
			}
		}),
	);
		paths.insert(
			"/projects/{project_id}/tasks/{task_id}/progress/{id}".to_string(),
			json!({
				"get": {
					"tags": ["Progress"],
					"security": [{"bearerAuth": []}],
					"parameters": [
						{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
						{"name": "task_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}},
						{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}
					],
					"responses": {
						"200": {
							"description": "Progress detail",
							"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Progress"}}}
						},
						"404": {"description": "Not found"}
					}
				}
			})
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

	// Helper to get examples based on ref
	let get_examples = |r: &str| -> Option<Vec<(&str, Value)>> {
		match r {
			"#/components/schemas/LoginRequest" => Some(vec![
				("minimal", json!({ "email": "user@example.com", "password": "password123" })),
			]),
			"#/components/schemas/RegisterRequest" => Some(vec![
				("minimal", json!({ "name": "Test User", "email": "test@example.com", "password": "password123" })),
				("with_profile", json!({ "name": "Ada Lovelace", "email": "ada@example.com", "password": "S3cureP@ssw0rd" })),
			]),
			"#/components/schemas/ProjectCreateRequest" => Some(vec![
				("minimal", json!({ "name": "My Project" })),
				("full", json!({ "name": "Launch Planning", "description": "Prepare milestones for the product launch.", "theme_color": "#3498db" })),
			]),
			"#/components/schemas/ProjectUpdateRequest" => Some(vec![
				("update_name", json!({ "name": "Launch Planning - Updated" })),
			]),
			"#/components/schemas/TaskCreateRequest" => Some(vec![
				("minimal", json!({ "title": "Quick task" })),
				("with_due", json!({ "title": "Define launch checklist", "status": "pending", "due_date": "2025-10-10T10:00:00Z" })),
			]),
			"#/components/schemas/TaskUpdateRequest" => Some(vec![
				("status_update", json!({ "status": "in_progress" })),
				("full", json!({ "title": "Refine checklist", "status": "in_progress", "due_date": "2025-11-01T10:00:00Z" })),
			]),
			"#/components/schemas/ProgressCreateRequest" => Some(vec![
				("minimal", json!({ "progress": 10 })),
				("with_note", json!({ "progress": 50, "note": "Halfway there" })),
			]),
			"#/components/schemas/ProgressUpdateRequest" => Some(vec![
				("progress_only", json!({ "progress": 75 })),
				("with_note", json!({ "progress": 100, "note": "Done" })),
			]),
			"#/components/schemas/DependencyCreateRequest" => Some(vec![
				("finish_to_start", json!({ "source_task_id": "22222222-2222-2222-2222-222222222222", "target_task_id": "66666666-6666-6666-6666-666666666666", "type": "finish_to_start" })),
			]),
			"#/components/schemas/TaskBatchUpdatePayload" => Some(vec![
				("batch_update", json!({ "tasks": [{ "id": "22222222-2222-2222-2222-222222222222", "status": "in_progress", "progress": 50 }, { "id": "66666666-6666-6666-6666-666666666666", "start_date": "2025-11-01T09:00:00Z", "end_date": "2025-11-05T17:00:00Z" }] })),
			]),
			"#/components/schemas/ProjectPlanCreateRequest" => Some(vec![
				("standard_plan", json!([
					{ "date": "2025-12-01T00:00:00Z", "planned_progress": 10 },
					{ "date": "2025-12-15T00:00:00Z", "planned_progress": 30 },
					{ "date": "2026-01-01T00:00:00Z", "planned_progress": 60 }
				])),
			]),
			_ => None,
		}
	};

	// Try direct ref
	if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
		if let Some(examples) = get_examples(reference) {
			if let Some((_, first)) = examples.first() {
				app_json.insert("example".to_string(), first.clone());
			}
		}
	}
	// Try array of refs
	else if schema.get("type").and_then(Value::as_str) == Some("array") {
		if let Some(items) = schema.get("items").and_then(Value::as_object) {
			if let Some(reference) = items.get("$ref").and_then(Value::as_str) {
				if let Some(examples) = get_examples(reference) {
					if let Some((_, first)) = examples.first() {
						app_json.insert("example".to_string(), first.clone());
					}
				}
			}
		}
	}
}

fn apply_response_examples(operation: &mut Value) {
	let Some(responses) = operation.get_mut("responses").and_then(Value::as_object_mut) else { return; };

	for response in responses.values_mut() {
		let Some(content) = response.get_mut("content").and_then(Value::as_object_mut) else { continue; };
		let Some(app_json) = content.get_mut("application/json").and_then(Value::as_object_mut) else { continue; };

		let schema = app_json.get("schema").cloned();
		if let Some(schema) = schema {
			// Helper to get example for a ref
			let get_ref_example = |r: &str| -> Option<Value> {
				match r {
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
					"#/components/schemas/Task" => Some(json!([{
						"id": "22222222-2222-2222-2222-222222222222",
						"project_id": "00000000-0000-0000-0000-000000000000",
						"title": "Define launch checklist",
						"status": "pending",
						"due_date": "2025-10-10T10:00:00Z",
						"start_date": "2025-10-01T09:00:00Z",
						"end_date": "2025-10-10T17:00:00Z",
						"duration_days": 9,
						"assignee": null,
						"parent_id": null,
						"progress": 0,
						"created_at": "2025-10-01T10:00:00Z",
						"updated_at": "2025-10-01T10:00:00Z",
						"deleted_at": null
					}])),
					"#/components/schemas/Progress" => Some(json!({
						"id": "33333333-3333-3333-3333-333333333333",
						"task_id": "22222222-2222-2222-2222-222222222222",
						"project_id": "00000000-0000-0000-0000-000000000000",
						"progress": 50,
						"note": "Halfway done",
						"created_at": "2025-10-05T10:00:00Z",
						"updated_at": "2025-10-05T10:00:00Z",
						"deleted_at": null
					})),
					"#/components/schemas/TaskDependency" => Some(json!({
						"id": "55555555-5555-5555-5555-555555555555",
						"source_task_id": "22222222-2222-2222-2222-222222222222",
						"target_task_id": "66666666-6666-6666-6666-666666666666",
						"type": "finish_to_start",
						"created_at": "2025-10-01T10:00:00Z"
					})),
					"#/components/schemas/DashboardResponse" => Some(json!({
						"project": {
							"id": "00000000-0000-0000-0000-000000000000",
							"user_id": "11111111-1111-1111-1111-111111111111",
							"name": "Launch Planning",
							"description": "Prepare milestones for the product launch.",
							"theme_color": "#3498db",
							"created_at": "2025-10-01T10:00:00Z",
							"updated_at": "2025-10-01T10:00:00Z",
							"deleted_at": null
						},
						"plan": [
							{
								"id": "44444444-4444-4444-4444-444444444444",
								"project_id": "00000000-0000-0000-0000-000000000000",
								"date": "2025-12-01T00:00:00Z",
								"planned_progress": 10,
								"created_at": "2025-10-01T10:00:00Z",
								"updated_at": "2025-10-01T10:00:00Z"
							}
						],
						"actual": [
							{"date": "2025-10-05", "actual": 50}
						]
					})),
					_ => None,
				}
			};

			if let Some(r#ref) = schema.get("$ref").and_then(Value::as_str) {
				if let Some(example) = get_ref_example(r#ref) {
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
						if let Some(item_example) = get_ref_example(item_ref) {
							app_json.insert("example".to_string(), json!([item_example]));
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
	let internal_url = "https://rust-service:8800".to_string();

	match doc.get_mut("servers") {
		Some(Value::Array(arr)) => {
			// ensure an entry for our server_url exists
			let has = arr.iter().any(|v| v.get("url").and_then(Value::as_str) == Some(server_url.as_str()));
			if !has {
				arr.push(json!({ "url": server_url }));
			}
			// ensure the internal docker host is present too
			let has_internal = arr.iter().any(|v| v.get("url").and_then(Value::as_str) == Some(internal_url.as_str()));
			if !has_internal {
				arr.push(json!({ "url": internal_url }));
			}
		}
		_ => {
			doc["servers"] = json!([{ "url": server_url }, { "url": internal_url }]);
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
