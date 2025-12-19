use std::sync::Arc;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
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
			models::project_plan::ProjectPlanCreateRequest,
			models::project_plan::ProjectPlanPoint,
			crate::routes::projects::ActualPoint,
			crate::routes::projects::DashboardResponse,
			crate::routes::projects::CriticalPathResponse,
			crate::routes::health::HealthResponse,
			crate::models::rbac::Role,
			crate::models::rbac::RoleCreateRequest,
			crate::models::rbac::Permission,
			crate::models::rbac::PermissionCreateRequest,
			crate::models::rbac::UserRole,
			crate::models::rbac::RolePermission,
			crate::models::rbac::UserPermission,
			crate::models::rbac::EffectivePermissions,
			crate::models::rbac::EffectivePermission,
			crate::models::rbac::AssignRoleRequest,
			crate::models::rbac::AssignPermissionToRoleRequest,
			crate::models::rbac::GrantPermissionRequest,
			crate::routes::users::CreateUserRequest,
			crate::routes::users::UpdateUserRequest,
			crate::routes::users::DeletedResponse,
			crate::routes::auth::ForgotPasswordRequest,
			crate::routes::auth::ResetPasswordRequest,
			crate::routes::auth::MessageResponse,
		)
	),
	paths(
		crate::routes::auth::register,
		crate::routes::auth::login,
		crate::routes::auth::me,
		crate::routes::auth::logout,
		crate::routes::auth::forgot_password,
		crate::routes::auth::reset_password,

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
		crate::routes::progress::delete_progress,
		crate::routes::health::health,

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
		crate::routes::rbac::get_effective_permissions,
		crate::routes::users::list_users,
		crate::routes::users::create_user,
		crate::routes::users::update_user,
		crate::routes::users::delete_user
	),
	tags(
		(name = "Auth", description = "Authentication endpoints"),
		(name = "Projects", description = "Project management"),
		(name = "Tasks", description = "Task management"),
		(name = "Progress", description = "Task progress entries"),
		(name = "RBAC", description = "Role-Based Access Control"),
		(name = "Users", description = "User management")
	)
)]
pub struct ApiDoc;

pub fn build_openapi(port: u16) -> anyhow::Result<utoipa::openapi::OpenApi> {
	let mut doc = serde_json::to_value(&ApiDoc::openapi())?;

	// Post-processing to refine the generated spec
	ensure_security_components(&mut doc);
	ensure_global_security(&mut doc);
	ensure_openapi_version(&mut doc);
	add_examples(&mut doc);
	ensure_servers(&mut doc, port);

	let doc: utoipa::openapi::OpenApi = serde_json::from_value(doc)?;
	Ok(doc)
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

fn ensure_security_components(doc: &mut Value) {
	let components = doc
		.as_object_mut()
		.expect("OpenAPI root must be an object")
		.entry("components")
		.or_insert_with(|| json!({}))
		.as_object_mut()
		.expect("components must be an object");

	let schemes = components
		.entry("securitySchemes")
		.or_insert_with(|| json!({}))
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
				if name == "id" || name.contains("_id") {
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
	let get_examples = |r: &str| -> Option<Value> {
		match r {
			"#/components/schemas/LoginRequest" => Some(json!({ "email": "user@example.com", "password": "password123" })),
			"#/components/schemas/RegisterRequest" => Some(json!({ "name": "Test User", "email": "test@example.com", "password": "password123" })),
			"#/components/schemas/ProjectCreateRequest" => Some(json!({ "name": "Launch Planning", "description": "Prepare milestones.", "theme_color": "#3498db" })),
			"#/components/schemas/TaskCreateRequest" => Some(json!({ "title": "Define launch checklist", "status": "pending" })),
			"#/components/schemas/ProgressCreateRequest" => Some(json!({ "progress": 50, "note": "Halfway there" })),
			"#/components/schemas/DependencyCreateRequest" => Some(json!({ "source_task_id": "0000-...", "target_task_id": "1111-...", "type": "finish_to_start" })),
			"#/components/schemas/TaskBatchUpdatePayload" => Some(json!({ "tasks": [{ "id": "0000-...", "status": "in_progress", "progress": 50 }] })),
			"#/components/schemas/RoleCreateRequest" => Some(json!({ "name": "project_manager", "description": "Can manage project tasks" })),
			"#/components/schemas/PermissionCreateRequest" => Some(json!({ "name": "project.view", "description": "View projects" })),
			"#/components/schemas/AssignRoleRequest" => Some(json!({ "role_id": "00000000-0000-0000-0000-000000000000" })),
			"#/components/schemas/GrantPermissionRequest" => Some(json!({ "permission_id": "00000000-0000-0000-0000-000000000000", "scope": { "project_id": "00000000-0000-0000-0000-000000000000" } })),
			"#/components/schemas/CreateUserRequest" => Some(json!({ "name": "Developer One", "email": "dev1@example.com", "password": "SecurePassword123!" })),
			"#/components/schemas/UpdateUserRequest" => Some(json!({ "name": "Developer Two", "email": "dev2@example.com" })),
			"#/components/schemas/ForgotPasswordRequest" => Some(json!({ "email": "user@example.com" })),
			"#/components/schemas/ResetPasswordRequest" => Some(json!({ "token": "raw-token-from-email", "new_password": "NewSecurePassword456!" })),
			_ => None,
		}
	};

	if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
		if let Some(example) = get_examples(reference) {
			app_json.insert("example".to_string(), example);
		}
	} else if schema.get("type").and_then(Value::as_str) == Some("array") {
		if let Some(items) = schema.get("items").and_then(Value::as_object) {
			if let Some(reference) = items.get("$ref").and_then(Value::as_str) {
				if let Some(example) = get_examples(reference) {
					app_json.insert("example".to_string(), json!([example]));
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
			let get_ref_example = |r: &str| -> Option<Value> {
				match r {
					"#/components/schemas/AuthResponse" => Some(json!({
						"token": "eyJhbGciOiJIUzI1Ni...",
						"user": { "id": "0000-0000...", "name": "Ada", "email": "ada@eg.com" }
					})),
					"#/components/schemas/Project" => Some(json!({
						"id": "uuid", "name": "Launch Planning", "theme_color": "#3498db"
					})),
					"#/components/schemas/Task" => Some(json!({
						"id": "uuid", "title": "Define checklist", "status": "pending", "progress": 0
					})),
					"#/components/schemas/Role" => Some(json!({
						"id": "uuid", "name": "super_admin", "description": "Full access"
					})),
					"#/components/schemas/EffectivePermissions" => Some(json!({
						"user_id": "uuid",
						"roles": ["super_admin"],
						"permissions": [{ "name": "*", "source": "role", "role_name": "super_admin" }]
					})),
					"#/components/schemas/MessageResponse" => Some(json!({ "message": "Operation successful" })),
					"#/components/schemas/DeletedResponse" => Some(json!({ "message": "User deleted" })),
					_ => None,
				}
			};

			if let Some(r#ref) = schema.get("$ref").and_then(Value::as_str) {
				if let Some(example) = get_ref_example(r#ref) {
					app_json.insert("example".to_string(), example);
				}
			} else if schema.get("type").and_then(Value::as_str) == Some("array") {
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
	let tls_enabled = std::env::var("CERT_PATH").is_ok() && std::env::var("KEY_PATH").is_ok()
		|| std::env::var("USE_SELF_SIGNED_TLS").is_ok();

	let scheme = if tls_enabled { "https" } else { "http" };
	let server_url = format!("{}://localhost:{}", scheme, port);
	let internal_url = "https://rust-service:8800".to_string();

	doc["servers"] = json!([{ "url": server_url }, { "url": internal_url }]);
}
