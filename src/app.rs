use std::sync::Arc;

use axum::http::Method;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

use crate::events::{self, EventBus};
use crate::errors::AppError;
use crate::jwt::JwtConfig;
use crate::routes::{auth, projects, tasks, progress, health, rbac, users};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt: Arc<JwtConfig>,
    pub event_bus: EventBus,
}

impl AppState {
    pub fn new(pool: SqlitePool, jwt: JwtConfig, event_bus: EventBus) -> Self {
        Self {
            pool,
            jwt: Arc::new(jwt),
            event_bus,
        }
    }
}

use axum::middleware::from_fn_with_state;
use crate::authz;

pub async fn create_app(pool: SqlitePool) -> Result<Router, AppError> {
    let jwt_config = JwtConfig::from_env()?;

    // Initialize Event Bus and Listener
    let (event_bus, rx) = events::init_event_bus();
    let listener_pool = pool.clone();
    tokio::spawn(events::start_activity_listener(rx, listener_pool));

    let state = AppState::new(pool, jwt_config, event_bus);

    Ok(api_routes(state))
}

// Custom Key Extractor that falls back to localhost if ConnectInfo is missing (useful for tests)
#[derive(Clone, Copy)]
struct SafeIpKeyExtractor;

impl tower_governor::key_extractor::KeyExtractor for SafeIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<B>(&self, req: &axum::http::Request<B>) -> Result<Self::Key, tower_governor::GovernorError> {
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
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/me", get(auth::me))
        .route("/logout", post(auth::logout))
        .route("/forgot-password", post(auth::forgot_password))
        .route("/reset-password", post(auth::reset_password));

    // Rate limit: 2 requests per second per IP for auth routes (brute force protection)
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2) // strict! 2 per second = 120 per minute
            .burst_size(5)
            .key_extractor(SafeIpKeyExtractor)
            .finish()
            .unwrap(),
    );
    let auth_routes = auth_routes.layer(GovernorLayer {
        config: auth_governor_conf,
    });

    let project_routes = Router::new()
        .route("/", get(projects::list_projects)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_view)))
        .route("/", post(projects::create_project)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_create)))
        .route("/:id/dashboard", get(projects::get_project_dashboard)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_view)))
        .route("/:id/critical-path", get(projects::get_project_critical_path)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_view)))
        .route("/:id", get(projects::get_project)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_view)))
        .route("/:id", put(projects::update_project)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_update)))
        .route("/:id", delete(projects::delete_project)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_delete)))
        .route("/:id/plan", post(projects::update_project_plan)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_update)))
        .route("/:id/plan", delete(projects::clear_project_plan)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_project_update)));

    // Tasks are scoped to a project: /projects/:project_id/tasks
    let task_routes = Router::new()
        .route("/batch", put(tasks::batch_update_tasks)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_update)))
        .route("/", get(tasks::list_tasks)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_view)))
        .route("/", post(tasks::create_task)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_create)))
        .route("/:id", get(tasks::get_task)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_view)))
        .route("/:id", put(tasks::update_task)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_update)))
        .route("/:id", delete(tasks::delete_task)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_delete)));

    let progress_routes = Router::new()
        .route("/", get(progress::list_progress)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_progress_view)))
        .route("/", post(progress::create_progress)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_progress_create)))
        .route("/:id", get(progress::get_progress)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_progress_view)))
        .route("/:id", put(progress::update_progress)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_progress_create)))
        .route("/:id", delete(progress::delete_progress)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_progress_create)));

    let dependency_routes = Router::new()
        .route("/", get(tasks::list_dependencies)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_view)))
        .route("/", post(tasks::create_dependency)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_update)))
        .route("/:id", delete(tasks::delete_dependency)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_task_update)));

    let api = Router::new()
        .route("/api/health", get(health::health))
        .route("/users", get(users::list_users)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_user_view)))
        .route("/users", post(users::create_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_user_manage)))
        .route("/users/:id", put(users::update_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_user_manage)))
        .route("/users/:id", delete(users::delete_user)
            .layer(from_fn_with_state(state.clone(), authz::layer::require_user_manage)))
        .nest("/auth", auth_routes)
        .nest("/projects", project_routes)
        // nest tasks under project scope
        .nest("/projects/:project_id/tasks", task_routes)
        // nest progress under task scope
        .nest("/projects/:project_id/tasks/:task_id/progress", progress_routes)
        // nest dependencies under project scope
        .nest("/projects/:project_id/dependencies", dependency_routes)
        // RBAC admin routes
        .nest("/rbac", rbac::routes(state.clone()))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Global Rate Limit: 50 requests per second per IP
    let global_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(50)
            .burst_size(100)
            .key_extractor(SafeIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    Router::new()
        .merge(api)
        .layer(GovernorLayer {
            config: global_governor_conf,
        })
}
