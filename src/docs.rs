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
				models::project_member::ProjectMember,
				models::project_member::ProjectMemberCreateRequest,
				models::project_member::MyProjectScopeSummary,
				models::resource_role::ResourceRoleRef,
				models::resource_role::ResourceRole,
				models::resource_role::ResourceRoleCreateRequest,
				models::resource_role::ResourceRoleUpdateRequest,
				models::resource_role::ProjectResourceRoleRate,
				models::resource_role::ProjectResourceRoleRateUpsertRequest,
				models::task::Task,
				models::task::TaskCreateRequest,
				models::task::TaskUpdateRequest,
			models::task::TaskBatchDeleteRequest,
			models::task::TaskBatchDeleteResponse,
			models::task::TaskAssignee,
			models::task::TaskActivityEntry,
			crate::routes::tasks::TaskSortBy,
			crate::routes::tasks::TaskSortDir,
				models::progress::Progress,
				models::progress::ProgressCreateRequest,
				models::progress::ProgressUpdateRequest,
				models::work_log::WorkLogSource,
				models::work_log::WorkLog,
				models::work_log::WorkLogCreateRequest,
				models::work_log::WorkLogUpdateRequest,
				models::dependency::TaskDependency,
				models::dependency::DependencyCreateRequest,
				models::task::TaskBatchUpdatePayload,
			models::task::TaskBatchUpdateRequest,
			models::project_plan::ProjectPlanCreateRequest,
			models::project_plan::ProjectPlanPoint,
			crate::routes::projects::ActualPoint,
			crate::routes::projects::DashboardMetricPoint,
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
			crate::models::audit_log::AuditLogEntry,
			crate::models::audit_log::PaginatedAuditLogs,
			crate::models::telemetry::TelemetryEventName,
			crate::models::telemetry::TelemetryEventRequest,
				crate::models::telemetry::TelemetryBatchRequest,
				crate::models::telemetry::TelemetryIngestResponse,
				crate::models::telemetry::TelemetryErrorResponse,
				crate::models::s_curve::SCurveMetric,
				crate::models::s_curve::SCurveStage,
				crate::models::s_curve::SCurveDataStatus,
				crate::models::s_curve::Rule5070Status,
				crate::models::s_curve::SCurveHealthResponse,
				crate::models::s_curve::PortfolioSCurveProjectSummary,
				crate::models::s_curve::PortfolioSCurveSummaryResponse,
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
			crate::routes::projects::list_project_members,
			crate::routes::projects::create_project_member,
			crate::routes::projects::delete_project_member,
			crate::routes::projects::list_my_project_scopes,
			crate::routes::projects::get_project_s_curve_health,
			crate::routes::projects::get_portfolio_s_curve_summary,
			crate::routes::resource_roles::list_resource_roles,
			crate::routes::resource_roles::create_resource_role,
			crate::routes::resource_roles::update_resource_role,
			crate::routes::resource_roles::delete_resource_role,
			crate::routes::resource_roles::list_project_resource_roles,
			crate::routes::resource_roles::upsert_project_resource_role_rate,
			crate::routes::resource_roles::delete_project_resource_role_rate,

		crate::routes::tasks::list_tasks,
		crate::routes::tasks::create_task,
		crate::routes::tasks::get_task,
		crate::routes::tasks::update_task,
		crate::routes::tasks::delete_task,
		crate::routes::tasks::batch_update_tasks,
		crate::routes::tasks::batch_delete_tasks,
		crate::routes::tasks::list_project_assignees,
		crate::routes::tasks::list_task_activity,
		crate::routes::tasks::list_dependencies,
		crate::routes::tasks::create_dependency,
		crate::routes::tasks::delete_dependency,

		crate::routes::progress::list_progress,
		crate::routes::progress::list_progress_by_task,
		crate::routes::progress::list_project_progress,
		crate::routes::progress::get_progress,
		crate::routes::progress::create_progress,
		crate::routes::progress::update_progress,
		crate::routes::progress::delete_progress,
		crate::routes::work_logs::list_work_logs,
		crate::routes::work_logs::create_work_log,
		crate::routes::work_logs::update_work_log,
		crate::routes::work_logs::delete_work_log,
		crate::routes::health::health,
		crate::routes::telemetry::ingest_events,

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
		crate::routes::rbac::list_audit_logs,
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
		(name = "Telemetry", description = "Frontend telemetry ingestion"),
		(name = "RBAC", description = "Role-Based Access Control"),
		(name = "Users", description = "User management")
	)
)]
pub struct ApiDoc;

pub fn build_openapi(port: u16) -> anyhow::Result<utoipa::openapi::OpenApi> {
    let mut doc = serde_json::to_value(ApiDoc::openapi())?;

    // Post-processing to refine the generated spec
    ensure_security_components(&mut doc);
    ensure_global_security(&mut doc);
    ensure_public_route_security_overrides(&mut doc);
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

    let doc_json =
        Arc::new(serde_json::to_value(&doc).expect("OpenAPI serialization must succeed"));

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
    doc.as_object_mut()
        .expect("OpenAPI root must be an object")
        .entry("security")
        .or_insert_with(|| json!([{ "bearerAuth": [] }]));
}

fn ensure_public_route_security_overrides(doc: &mut Value) {
    let public_ops = [
        ("/api/health", "get"),
        ("/auth/login", "post"),
        ("/auth/register", "post"),
        ("/auth/forgot-password", "post"),
        ("/auth/reset-password", "post"),
    ];

    let Some(paths) = doc.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };

    for (path, method) in public_ops {
        let Some(path_item) = paths.get_mut(path).and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(op) = path_item.get_mut(method).and_then(Value::as_object_mut) else {
            continue;
        };
        op.insert("security".to_string(), json!([]));
    }
}

fn ensure_openapi_version(doc: &mut Value) {
    doc.as_object_mut()
        .expect("OpenAPI root must be an object")
        .entry("openapi")
        .or_insert_with(|| Value::String("3.1.0".to_string()));
}

fn add_examples(doc: &mut Value) {
    if let Some(paths) = doc.get_mut("paths").and_then(Value::as_object_mut) {
        for (path, item) in paths.iter_mut() {
            if let Some(operations) = item.as_object_mut() {
                for (method, operation) in operations.iter_mut() {
                    if is_http_method(method) {
                        apply_operation_metadata(operation, path, method);
                    }
                    apply_parameter_examples(operation);
                    apply_request_examples(operation);
                    apply_response_examples(operation);
                }
            }
        }
    }
}

fn is_http_method(method: &str) -> bool {
    matches!(
        method,
        "get" | "post" | "put" | "delete" | "patch" | "head" | "options"
    )
}

fn apply_operation_metadata(operation: &mut Value, path: &str, method: &str) {
    let Some(operation_obj) = operation.as_object_mut() else {
        return;
    };
    let operation_id = operation_obj
        .get("operationId")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let has_summary = operation_obj
        .get("summary")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let summary = if has_summary {
        operation_obj
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        let generated = operation_id
            .as_deref()
            .map(humanize_operation_id)
            .unwrap_or_else(|| format!("{} {}", method.to_uppercase(), path));
        operation_obj.insert("summary".to_string(), Value::String(generated.clone()));
        generated
    };

    let has_description = operation_obj
        .get("description")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if !has_description {
        let explicitly_public = operation_obj
            .get("security")
            .and_then(Value::as_array)
            .map(|arr| arr.is_empty())
            .unwrap_or(false);
        let requires_auth = !explicitly_public;
        let auth_suffix = if requires_auth {
            " Requires bearer authentication."
        } else {
            " Does not require authentication."
        };
        let description = format!(
            "{} Handles `{}` requests for `{}`.{}",
            summary,
            method.to_uppercase(),
            path,
            auth_suffix
        );
        operation_obj.insert("description".to_string(), Value::String(description));
    }

    if !has_summary {
        operation_obj.insert("summary".to_string(), Value::String(summary));
    }
}

fn humanize_operation_id(operation_id: &str) -> String {
    let mut words = Vec::new();
    for token in operation_id.split('_').filter(|token| !token.is_empty()) {
        words.push(match token {
            "id" => "ID".to_string(),
            "rbac" => "RBAC".to_string(),
            "api" => "API".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        });
    }

    if words.is_empty() {
        "Operation".to_string()
    } else {
        words.join(" ")
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
                        obj.entry("example")
                            .or_insert_with(|| json!("00000000-0000-0000-0000-000000000000"));
                    }
                }
            }
        }
    }
}

fn apply_request_examples(operation: &mut Value) {
    let Some(request_body) = operation.get_mut("requestBody") else {
        return;
    };
    let Some(content) = request_body
        .get_mut("content")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(app_json) = content
        .get_mut("application/json")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(schema) = app_json.get("schema").and_then(Value::as_object) else {
        return;
    };

    // Helper to get examples based on ref
    let get_examples = |r: &str| -> Option<Value> {
        match r {
            "#/components/schemas/LoginRequest" => {
                Some(json!({ "email": "user@example.com", "password": "password123" }))
            }
            "#/components/schemas/RegisterRequest" => Some(
                json!({ "name": "Test User", "email": "test@example.com", "password": "password123" }),
            ),
            "#/components/schemas/ProjectCreateRequest" => Some(
                json!({ "name": "Launch Planning", "description": "Prepare milestones.", "theme_color": "#3498db" }),
            ),
            "#/components/schemas/ProjectUpdateRequest" => {
                Some(json!({ "name": "Launch Planning v2", "theme_color": "#2ecc71" }))
            }
            "#/components/schemas/TaskCreateRequest" => {
                Some(json!({ "title": "Define launch checklist", "status": "pending" }))
            }
            "#/components/schemas/TaskUpdateRequest" => Some(
                json!({ "title": "Define final checklist", "status": "in_progress", "progress": 65 }),
            ),
            "#/components/schemas/TaskBatchDeleteRequest" => Some(json!({
                "ids": [
                    "33333333-3333-4333-8333-333333333333",
                    "22222222-2222-4222-8222-222222222222"
                ]
            })),
            "#/components/schemas/ProjectPlanCreateRequest" => Some(json!({
                "date": "2025-12-01T00:00:00Z",
                "planned_progress": 10,
                "planned_hours": 120.5,
                "planned_cost": 15200.0,
                "currency": "USD"
            })),
            "#/components/schemas/ProgressCreateRequest" => Some(json!({
                "progress": 50,
                "note": "Halfway there"
            })),
            "#/components/schemas/ProgressUpdateRequest" => Some(json!({
                "progress": 75,
                "note": "Adjusted after review"
            })),
            "#/components/schemas/ProjectMemberCreateRequest" => Some(json!({
                "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "access_role_id": "55555555-5555-4555-8555-555555555555",
                "resource_role_ids": [
                    "40000000-0000-0000-0000-000000000003",
                    "40000000-0000-0000-0000-000000000006"
                ]
            })),
            "#/components/schemas/ResourceRoleCreateRequest" => Some(json!({
                "name": "data_engineer",
                "description": "Builds and maintains project data pipelines",
                "default_hourly_rate": 72.5,
                "currency": "USD"
            })),
            "#/components/schemas/ResourceRoleUpdateRequest" => Some(json!({
                "description": "Updated description",
                "default_hourly_rate": 78.0
            })),
            "#/components/schemas/ProjectResourceRoleRateUpsertRequest" => Some(json!({
                "hourly_rate": 88.0,
                "currency": "USD"
            })),
            "#/components/schemas/WorkLogCreateRequest" => Some(json!({
                "resource_role_id": "40000000-0000-0000-0000-000000000003",
                "hours": 3.5,
                "work_date": "2026-03-10",
                "note": "Implemented pagination filters"
            })),
            "#/components/schemas/WorkLogUpdateRequest" => Some(json!({
                "hours": 4.0,
                "note": "Expanded to include sorting support"
            })),
            "#/components/schemas/DependencyCreateRequest" => Some(json!({
                "source_task_id": "11111111-1111-4111-8111-111111111111",
                "target_task_id": "22222222-2222-4222-8222-222222222222",
                "type_": "finish_to_start"
            })),
            "#/components/schemas/TaskBatchUpdatePayload" => Some(json!({
                "tasks": [{
                    "id": "33333333-3333-4333-8333-333333333333",
                    "status": "in_progress",
                    "progress": 50
                }]
            })),
            "#/components/schemas/RoleCreateRequest" => Some(
                json!({ "name": "project_manager", "description": "Can manage project tasks" }),
            ),
            "#/components/schemas/PermissionCreateRequest" => {
                Some(json!({ "name": "project.view", "description": "View projects" }))
            }
            "#/components/schemas/AssignPermissionToRoleRequest" => {
                Some(json!({ "permission_id": "66666666-6666-4666-8666-666666666666" }))
            }
            "#/components/schemas/AssignRoleRequest" => {
                Some(json!({ "role_id": "00000000-0000-0000-0000-000000000000" }))
            }
            "#/components/schemas/GrantPermissionRequest" => Some(
                json!({ "permission_id": "00000000-0000-0000-0000-000000000000", "scope": { "project_id": "00000000-0000-0000-0000-000000000000" } }),
            ),
            "#/components/schemas/CreateUserRequest" => Some(
                json!({ "name": "Developer One", "email": "dev1@example.com", "password": "SecurePassword123!" }),
            ),
            "#/components/schemas/UpdateUserRequest" => {
                Some(json!({ "name": "Developer Two", "email": "dev2@example.com" }))
            }
            "#/components/schemas/ForgotPasswordRequest" => {
                Some(json!({ "email": "user@example.com" }))
            }
            "#/components/schemas/ResetPasswordRequest" => Some(
                json!({ "token": "raw-token-from-email", "new_password": "NewSecurePassword456!" }),
            ),
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
    let Some(responses) = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
    else {
        return;
    };

    for response in responses.values_mut() {
        let Some(content) = response.get_mut("content").and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(app_json) = content
            .get_mut("application/json")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };

        let schema = app_json.get("schema").cloned();
        if let Some(schema) = schema {
            let get_ref_example = |r: &str| -> Option<Value> {
                match r {
                    "#/components/schemas/AuthResponse" => Some(json!({
                        "token": "eyJhbGciOiJIUzI1Ni...",
                        "user": {
                            "id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                            "name": "Ada Lovelace",
                            "email": "ada@example.com",
                            "provider": "local",
                            "created_at": "2025-01-15T10:00:00Z",
                            "updated_at": "2025-01-15T10:00:00Z"
                        }
                    })),
                    "#/components/schemas/HealthResponse" => Some(json!({
                        "status": "ok",
                        "db_ok": true
                    })),
                    "#/components/schemas/User" => Some(json!({
                        "id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "name": "Ada Lovelace",
                        "email": "ada@example.com",
                        "provider": "local",
                        "created_at": "2025-01-15T10:00:00Z",
                        "updated_at": "2025-01-15T10:00:00Z"
                    })),
                    "#/components/schemas/Project" => Some(json!({
                        "id": "44444444-4444-4444-8444-444444444444",
                        "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "name": "Launch Planning",
                        "theme_color": "#3498db",
                        "created_at": "2025-01-15T10:00:00Z",
                        "updated_at": "2025-01-15T10:00:00Z"
                    })),
                    "#/components/schemas/Task" => Some(json!({
                        "id": "33333333-3333-4333-8333-333333333333",
                        "project_id": "44444444-4444-4444-8444-444444444444",
                        "title": "Define checklist",
                        "status": "pending",
                        "progress": 0,
                        "created_at": "2025-01-16T09:00:00Z",
                        "updated_at": "2025-01-16T09:00:00Z"
                    })),
                    "#/components/schemas/TaskBatchDeleteResponse" => Some(json!({
                        "deleted": 2
                    })),
                    "#/components/schemas/TaskAssignee" => Some(json!({
                        "id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "name": "Ada Lovelace",
                        "email": "ada@example.com"
                    })),
                    "#/components/schemas/TaskActivityEntry" => Some(json!({
                        "id": "evt_20250120_0002",
                        "action": "task.updated",
                        "actor_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "occurred_at": "2025-01-20T10:16:00Z",
                        "details": {
                            "payload": {
                                "old": { "title": "Define checklist" },
                                "new": { "title": "Define final checklist" }
                            }
                        }
                    })),
                    "#/components/schemas/CriticalPathResponse" => Some(json!({
                        "task_ids": [
                            "33333333-3333-4333-8333-333333333333",
                            "22222222-2222-4222-8222-222222222222"
                        ]
                    })),
                    "#/components/schemas/DashboardResponse" => Some(json!({
                        "project": {
                            "id": "44444444-4444-4444-8444-444444444444",
                            "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                            "name": "Launch Planning",
                            "theme_color": "#3498db",
                            "created_at": "2025-01-15T10:00:00Z",
                            "updated_at": "2025-01-15T10:00:00Z"
                        },
                        "plan": [{
                            "id": "77777777-7777-4777-8777-777777777777",
                            "project_id": "44444444-4444-4444-8444-444444444444",
                            "date": "2025-12-01T00:00:00Z",
                            "planned_progress": 20,
                            "created_at": "2025-11-20T08:00:00Z",
                            "updated_at": "2025-11-20T08:00:00Z"
                        }],
                        "actual": [{ "date": "2025-12-01", "actual": 15 }]
                    })),
                    "#/components/schemas/ProjectPlanPoint" => Some(json!({
                        "id": "77777777-7777-4777-8777-777777777777",
                        "project_id": "44444444-4444-4444-8444-444444444444",
                        "date": "2025-12-01T00:00:00Z",
                        "planned_progress": 20,
                        "created_at": "2025-11-20T08:00:00Z",
                        "updated_at": "2025-11-20T08:00:00Z"
                    })),
                    "#/components/schemas/TaskDependency" => Some(json!({
                        "id": "88888888-8888-4888-8888-888888888888",
                        "source_task_id": "33333333-3333-4333-8333-333333333333",
                        "target_task_id": "22222222-2222-4222-8222-222222222222",
                        "type_": "finish_to_start",
                        "created_at": "2025-01-18T09:00:00Z"
                    })),
                    "#/components/schemas/Progress" => Some(json!({
                        "id": "99999999-9999-4999-8999-999999999999",
                        "project_id": "44444444-4444-4444-8444-444444444444",
                        "task_id": "33333333-3333-4333-8333-333333333333",
                        "progress": 65,
                        "note": "Execution started",
                        "created_at": "2025-01-19T09:00:00Z",
                        "updated_at": "2025-01-19T09:00:00Z"
                    })),
                    "#/components/schemas/WorkLog" => Some(json!({
                        "id": "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
                        "project_id": "44444444-4444-4444-8444-444444444444",
                        "task_id": "33333333-3333-4333-8333-333333333333",
                        "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "user_name": "Ada Lovelace",
                        "resource_role_id": "40000000-0000-0000-0000-000000000003",
                        "resource_role_name": "backend_engineer",
                        "hours": 4.0,
                        "hourly_rate_snapshot": 88.0,
                        "currency_snapshot": "USD",
                        "cost_amount": 352.0,
                        "work_date": "2026-03-10",
                        "note": "Implemented server-side pagination",
                        "source": "manual",
                        "created_at": "2026-03-10T07:00:00Z",
                        "updated_at": "2026-03-10T07:00:00Z",
                        "deleted_at": null
                    })),
                    "#/components/schemas/ResourceRole" => Some(json!({
                        "id": "40000000-0000-0000-0000-000000000003",
                        "name": "backend_engineer",
                        "description": "Backend engineering project contribution role",
                        "default_hourly_rate": 70.0,
                        "currency": "USD",
                        "created_at": "2026-03-10T00:00:00Z",
                        "updated_at": "2026-03-10T00:00:00Z"
                    })),
                    "#/components/schemas/ProjectResourceRoleRate" => Some(json!({
                        "resource_role_id": "40000000-0000-0000-0000-000000000003",
                        "resource_role_name": "backend_engineer",
                        "hourly_rate": 88.0,
                        "currency": "USD",
                        "is_override": true
                    })),
                    "#/components/schemas/ProjectMember" => Some(json!({
                        "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "user_name": "Ada Lovelace",
                        "user_email": "ada@example.com",
                        "access_role_id": "55555555-5555-4555-8555-555555555555",
                        "access_role_name": "project_owner",
                        "resource_roles": [
                            { "id": "40000000-0000-0000-0000-000000000003", "name": "backend_engineer" },
                            { "id": "40000000-0000-0000-0000-000000000006", "name": "project_manager" }
                        ],
                        "created_at": "2026-03-10T00:00:00Z",
                        "updated_at": "2026-03-10T00:00:00Z"
                    })),
                    "#/components/schemas/MyProjectScopeSummary" => Some(json!({
                        "project_id": "44444444-4444-4444-8444-444444444444",
                        "project_name": "Launch Planning",
                        "access_role_id": "55555555-5555-4555-8555-555555555555",
                        "access_role_name": "project_owner",
                        "resource_roles": [
                            { "id": "40000000-0000-0000-0000-000000000003", "name": "backend_engineer" }
                        ],
                        "permissions": ["project.view", "task.create", "task.view", "progress.create", "progress.view"]
                    })),
                    "#/components/schemas/Role" => Some(json!({
                        "id": "55555555-5555-4555-8555-555555555555",
                        "name": "super_admin",
                        "description": "Full access",
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-01-01T00:00:00Z"
                    })),
                    "#/components/schemas/Permission" => Some(json!({
                        "id": "66666666-6666-4666-8666-666666666666",
                        "name": "project.view",
                        "description": "View projects",
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-01-01T00:00:00Z"
                    })),
                    "#/components/schemas/UserPermission" => Some(json!({
                        "id": "abababab-abab-4bab-8bab-abababababab",
                        "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "permission_id": "66666666-6666-4666-8666-666666666666",
                        "scope": { "project_id": "44444444-4444-4444-8444-444444444444" },
                        "created_at": "2025-01-20T10:00:00Z"
                    })),
                    "#/components/schemas/EffectivePermissions" => Some(json!({
                        "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "roles": ["super_admin"],
                        "permissions": [{ "name": "*", "source": "role", "role_name": "super_admin" }]
                    })),
                    "#/components/schemas/PaginatedAuditLogs" => Some(json!({
                        "items": [{
                            "id": "evt_20250120_0001",
                            "action": "role.assign",
                            "details": {
                                "role_id": "55555555-5555-4555-8555-555555555555",
                                "user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
                            },
                            "created_at": "2025-01-20T10:15:00Z",
                            "actor_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                            "actor_name": "Ada Lovelace",
                            "target_user_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                            "target_user_name": "Ada Lovelace"
                        }],
                        "total": 1,
                        "page": 1,
                        "per_page": 20
                    })),
                    "#/components/schemas/MessageResponse" => {
                        Some(json!({ "message": "Operation successful" }))
                    }
                    "#/components/schemas/DeletedResponse" => {
                        Some(json!({ "message": "User deleted" }))
                    }
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
