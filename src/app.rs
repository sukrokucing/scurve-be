use std::sync::Arc;

use axum::http::Method;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::errors::AppError;
use crate::jwt::JwtConfig;
use crate::routes::{auth, projects, tasks};

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
        .route("/:id", get(projects::get_project))
        .route("/:id", put(projects::update_project))
        .route("/:id", delete(projects::delete_project));

    let task_routes = Router::new()
        .route("/", get(tasks::list_tasks))
        .route("/", post(tasks::create_task))
        .route("/:id", put(tasks::update_task))
        .route("/:id", delete(tasks::delete_task));

    let router = Router::new()
        .nest("/auth", auth_routes)
        .nest("/projects", project_routes)
        .nest("/tasks", task_routes)
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    Ok(router)
}
