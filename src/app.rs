use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sqlx::SqlitePool;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{Any, CorsLayer};
use tower_http::cors::AllowOrigin;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::authz::RoutePermissionCache;
use crate::errors::AppError;
use crate::events::{self, EventBus};
use crate::jwt::JwtConfig;
use crate::realtime::RealtimeHub;
use crate::routes::{
    auth, health, notifications, progress, projects, rbac, realtime, resource_roles, tasks,
    telemetry, users, work_logs,
};

fn env_var_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt: Arc<JwtConfig>,
    pub event_bus: EventBus,
    pub realtime_hub: RealtimeHub,
    pub route_permission_cache: RoutePermissionCache,
}

impl AppState {
    #[allow(dead_code)]
    pub fn new(
        pool: SqlitePool,
        jwt: JwtConfig,
        event_bus: EventBus,
        route_cache: RoutePermissionCache,
    ) -> Self {
        Self::new_with_realtime(pool, jwt, event_bus, RealtimeHub::new(), route_cache)
    }

    pub fn new_with_realtime(
        pool: SqlitePool,
        jwt: JwtConfig,
        event_bus: EventBus,
        realtime_hub: RealtimeHub,
        route_cache: RoutePermissionCache,
    ) -> Self {
        Self {
            pool,
            jwt: Arc::new(jwt),
            event_bus,
            realtime_hub,
            route_permission_cache: route_cache,
        }
    }
}

use crate::authz;
use axum::middleware::from_fn_with_state;

pub async fn create_app(pool: SqlitePool) -> Result<Router, AppError> {
    let jwt_config = JwtConfig::from_env()?;

    // Initialize Event Bus and Listener
    let (event_bus, rx) = events::init_event_bus();
    let realtime_hub = RealtimeHub::new();
    let listener_pool = pool.clone();
    tokio::spawn(events::start_activity_listener(rx, listener_pool));
    let realtime_pool = pool.clone();
    let realtime_rx = event_bus.subscribe();
    tokio::spawn(crate::realtime::start_realtime_dispatcher(
        realtime_rx,
        realtime_pool,
        realtime_hub.clone(),
    ));

    let route_cache = RoutePermissionCache::load(&pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to load route permissions: {}", e)))?;

    // Validate cached routes against OpenAPI documentation
    route_cache
        .validate_against_openapi(&crate::docs::ApiDoc::openapi())
        .await;

    let state = AppState::new_with_realtime(pool, jwt_config, event_bus, realtime_hub, route_cache);

    Ok(api_routes(state))
}

// Custom Key Extractor that falls back to localhost if ConnectInfo is missing (useful for tests)
#[derive(Clone, Copy)]
struct SafeIpKeyExtractor;

impl tower_governor::key_extractor::KeyExtractor for SafeIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<B>(
        &self,
        req: &axum::http::Request<B>,
    ) -> Result<Self::Key, tower_governor::GovernorError> {
        req.extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|axum::extract::ConnectInfo(addr)| addr.ip())
            .or_else(|| {
                // If missing (e.g. in tests using oneshot), fallback to localhost
                Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)))
            })
            .ok_or(tower_governor::GovernorError::UnableToExtractKey)
    }
}

pub fn api_routes(state: AppState) -> Router {
    // Build CORS layer. Set CORS_ALLOWED_ORIGINS to a comma-separated list of origins to
    // restrict cross-origin access (e.g. "https://app.example.com,https://admin.example.com").
    // Leave unset (or set to "*") to allow all origins (suitable only for development).
    let cors = {
        let origins_env = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
        let trimmed = origins_env.trim();
        if trimmed.is_empty() || trimmed == "*" {
            CorsLayer::new()
                .allow_methods(Any)
                .allow_origin(Any)
                .allow_headers(Any)
        } else {
            let allowed: Vec<axum::http::HeaderValue> = trimmed
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            CorsLayer::new()
                .allow_methods(Any)
                .allow_origin(AllowOrigin::list(allowed))
                .allow_headers(Any)
        }
    };

    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/me", get(auth::me))
        .route("/logout", post(auth::logout))
        .route("/forgot-password", post(auth::forgot_password))
        .route("/reset-password", post(auth::reset_password));

    // Rate limit for auth routes (brute force protection) - configurable via env
    // Env vars: AUTH_RATE_PER_SECOND, AUTH_BURST_SIZE
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(env_var_u32("AUTH_RATE_PER_SECOND", 2).into())
            .burst_size(env_var_u32("AUTH_BURST_SIZE", 5))
            .key_extractor(SafeIpKeyExtractor)
            .finish()
            .unwrap(),
    );
    let auth_routes = auth_routes.layer(GovernorLayer {
        config: auth_governor_conf,
    });

    // Project routes (permission checked by global middleware)
    let project_routes = Router::new()
        .route("/", get(projects::list_projects))
        .route("/", post(projects::create_project))
        .route("/:id/dashboard", get(projects::get_project_dashboard))
        .route(
            "/:id/critical-path",
            get(projects::get_project_critical_path),
        )
        .route(
            "/:id/s-curve/health",
            get(projects::get_project_s_curve_health),
        )
        .route("/:id", get(projects::get_project))
        .route("/:id", put(projects::update_project))
        .route("/:id", delete(projects::delete_project))
        .route("/:id/plan", post(projects::update_project_plan))
        .route("/:id/plan", delete(projects::clear_project_plan));

    // Tasks are scoped to a project: /projects/:project_id/tasks
    let task_routes = Router::new()
        .route("/batch", put(tasks::batch_update_tasks))
        .route("/batch", delete(tasks::batch_delete_tasks))
        .route("/", get(tasks::list_tasks))
        .route("/", post(tasks::create_task))
        .route("/:id/activity", get(tasks::list_task_activity))
        .route(
            "/:id/progress-components",
            get(tasks::list_task_progress_components).put(tasks::replace_task_progress_components),
        )
        .route("/:id", get(tasks::get_task))
        .route("/:id", put(tasks::update_task))
        .route("/:id", delete(tasks::delete_task));

    let progress_routes = Router::new()
        .route("/", get(progress::list_progress))
        .route("/", post(progress::create_progress))
        .route("/:id", get(progress::get_progress))
        .route("/:id", put(progress::update_progress))
        .route("/:id", delete(progress::delete_progress));

    let dependency_routes = Router::new()
        .route("/", get(tasks::list_dependencies))
        .route("/", post(tasks::create_dependency))
        .route("/:id", delete(tasks::delete_dependency));

    // User routes
    let user_routes = Router::new()
        .route("/me/projects", get(projects::list_my_project_scopes))
        .route("/", get(users::list_users))
        .route("/", post(users::create_user))
        .route("/:id", put(users::update_user))
        .route("/:id", delete(users::delete_user));

    // Project-level progress route
    let project_progress_routes = Router::new().route("/", get(progress::list_project_progress));

    let project_assignee_routes = Router::new().route("/", get(tasks::list_project_assignees));

    let project_member_routes = Router::new()
        .route("/", get(projects::list_project_members))
        .route("/", post(projects::create_project_member))
        .route("/:user_id", delete(projects::delete_project_member));

    let project_task_health_routes = Router::new().route(
        "/rules",
        get(projects::get_task_health_rules).put(projects::update_task_health_rules),
    );

    let resource_role_routes = Router::new()
        .route("/", get(resource_roles::list_resource_roles))
        .route("/", post(resource_roles::create_resource_role))
        .route("/:id", put(resource_roles::update_resource_role))
        .route("/:id", delete(resource_roles::delete_resource_role));

    let project_resource_role_routes = Router::new()
        .route("/", get(resource_roles::list_project_resource_roles))
        .route(
            "/:resource_role_id/rate",
            put(resource_roles::upsert_project_resource_role_rate)
                .delete(resource_roles::delete_project_resource_role_rate),
        );

    // Backward-compatible route for task progress lookup without project_id in path.
    let legacy_task_progress_routes =
        Router::new().route("/", get(progress::list_progress_by_task));

    let work_log_routes = Router::new()
        .route("/", get(work_logs::list_work_logs))
        .route("/", post(work_logs::create_work_log))
        .route("/:id", put(work_logs::update_work_log))
        .route("/:id", delete(work_logs::delete_work_log));

    let telemetry_routes = Router::new().route("/events", post(telemetry::ingest_events));
    let notification_routes = Router::new()
        .route("/", get(notifications::list_notifications))
        .route(
            "/unread-count",
            get(notifications::get_unread_notification_count),
        )
        .route("/read", post(notifications::mark_notifications_read))
        .route(
            "/read-all",
            post(notifications::mark_notifications_read_all),
        );
    let realtime_routes = Router::new().route("/ws", get(realtime::websocket_feed));

    let portfolio_routes = Router::new().route(
        "/s-curve/summary",
        get(projects::get_portfolio_s_curve_summary),
    );

    // Protected routes (require authentication and authorization)
    let protected_routes = Router::new()
        .nest("/users", user_routes)
        .nest("/resource-roles", resource_role_routes)
        .nest("/projects", project_routes)
        .nest("/projects/:project_id/assignees", project_assignee_routes)
        .nest("/projects/:project_id/members", project_member_routes)
        .nest(
            "/projects/:project_id/task-health",
            project_task_health_routes,
        )
        .nest(
            "/projects/:project_id/resource-roles",
            project_resource_role_routes,
        )
        .nest("/projects/:project_id/tasks", task_routes)
        .nest(
            "/projects/:project_id/tasks/:task_id/work-logs",
            work_log_routes,
        )
        .nest(
            "/projects/:project_id/tasks/:task_id/progress",
            progress_routes,
        )
        .nest("/projects/:project_id/progress", project_progress_routes)
        .nest("/projects/:project_id/dependencies", dependency_routes)
        .nest("/portfolio", portfolio_routes)
        .nest("/notifications", notification_routes)
        .nest("/realtime", realtime_routes)
        .nest("/tasks/:task_id/progress", legacy_task_progress_routes)
        .nest("/telemetry", telemetry_routes)
        .nest("/rbac", rbac::routes(state.clone()))
        // Apply authorization middleware only to protected routes
        .layer(from_fn_with_state(
            state.clone(),
            authz::layer::dynamic_authz,
        ));

    // Public routes (no authentication required)
    let api = Router::new()
        .route("/api/health", get(health::health))
        .nest("/auth", auth_routes)
        // Merge protected routes
        .merge(protected_routes)
        .with_state(state)
        .layer(DefaultBodyLimit::max(1_048_576)) // 1 MiB request body limit
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Global Rate Limit - configurable via env
    // Env vars: GLOBAL_RATE_PER_SECOND, GLOBAL_BURST_SIZE
    let global_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(env_var_u32("GLOBAL_RATE_PER_SECOND", 50).into())
            .burst_size(env_var_u32("GLOBAL_BURST_SIZE", 100))
            .key_extractor(SafeIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    Router::new().merge(api).layer(GovernorLayer {
        config: global_governor_conf,
    })
}
