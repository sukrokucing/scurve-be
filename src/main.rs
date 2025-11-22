mod app;
#[path = "db/mod.rs"]
mod db;
mod docs;
mod errors;
mod jwt;
mod models;
mod routes;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    init_tracing();

    let pool = db::init().await?;
    let router = app::create_app(pool).await?;
    let port = resolve_port();

    let openapi = docs::build_openapi(port)?;
    let router = router.merge(docs::swagger_routes(openapi));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);

    // Prefer TLS if cert and key are provided via environment (CERT_PATH, KEY_PATH)
    // which are loaded by `load_env()` earlier. When TLS is enabled axum-server
    // will advertise ALPN and support HTTP/2 to browsers.
    let cert = std::env::var("CERT_PATH").ok();
    let key = std::env::var("KEY_PATH").ok();

    if let (Some(cert_path), Some(key_path)) = (cert, key) {
        tracing::info!("starting TLS with cert={} key={}", cert_path, key_path);
        let cfg = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path).await?;
        // Rustls+ALPN will negotiate HTTP/2 with clients (browsers) automatically.
        axum_server::bind_rustls(addr, cfg)
            .serve(router.into_make_service())
            .await?;
    } else {
        tracing::info!("starting plaintext HTTP (no CERT_PATH/KEY_PATH provided)");
        // plaintext (no TLS)
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router.into_make_service()).await?;
    }

    Ok(())
}

fn resolve_port() -> u16 {
    std::env::var("APP_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8000)
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
