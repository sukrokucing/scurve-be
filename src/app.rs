use std::sync::Arc;

use axum::http::Method;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::errors::AppError;
use crate::jwt::JwtConfig;
use crate::routes::{auth, projects, tasks, progress};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt: Arc<JwtConfig>,
}

impl AppState {
    pub fn new(pool: SqlitePool, jwt: JwtConfig) -> Self {
        Self {
            pool,
            jwt: Arc::new(jwt),
        }
    }
}

pub async fn create_app(pool: SqlitePool) -> Result<Router, AppError> {
    let jwt_config = JwtConfig::from_env()?;
    let state = AppState::new(pool, jwt_config);

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_origin(Any)
        .allow_headers(Any);

    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/me", get(auth::me))
        .route("/logout", post(auth::logout));

    let project_routes = Router::new()
        .route("/", get(projects::list_projects))
        .route("/", post(projects::create_project))
        .route("/:id/dashboard", get(projects::get_project_dashboard))
        .route("/:id", get(projects::get_project))
        .route("/:id", put(projects::update_project))
        .route("/:id", delete(projects::delete_project))
        .route("/:id/plan", post(projects::update_project_plan))
        .route("/:id/plan", delete(projects::clear_project_plan));

    // Tasks are scoped to a project: /projects/:project_id/tasks
    let task_routes = Router::new()
        .route("/batch", put(tasks::batch_update_tasks))
        .route("/", get(tasks::list_tasks))
        .route("/", post(tasks::create_task))
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

    let router = Router::new()
        .nest("/auth", auth_routes)
        .nest("/projects", project_routes)
        // nest tasks under project scope
        .nest("/projects/:project_id/tasks", task_routes)
        // nest progress under task scope
        .nest("/projects/:project_id/tasks/:task_id/progress", progress_routes)
        // nest dependencies under project scope
        .nest("/projects/:project_id/dependencies", dependency_routes)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    Ok(router)
}
