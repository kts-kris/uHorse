use std::sync::Arc;

use tempfile::NamedTempFile;
use uhorse_config::HealthConfig;
use uhorse_hub::{
    create_router_with_health_config, create_router_with_health_path, Hub, HubConfig, WebState,
};

#[allow(dead_code)]
#[path = "../src/main.rs"]
mod hub_main;

#[test]
fn load_config_keeps_custom_health_path_from_unified_config() {
    let file = NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"
[server]
host = "127.0.0.1"
port = 8765

[server.health]
enabled = true
path = "/readyz"
verbose = false

[database]
path = "./data/uhorse.db"

[channels]
enabled = []

[security]
jwt_secret = "test-secret-with-at-least-32-characters"

[logging]
level = "info"

[observability]
service_name = "uhorse-hub"

[scheduler]
enabled = true

[tools]
sandbox_enabled = true

[llm]
enabled = false
"#,
    )
    .unwrap();

    let args = hub_main::test_args();
    let runtime_config = hub_main::test_load_config(file.path().to_str().unwrap(), &args).unwrap();

    assert_eq!(runtime_config.app_config.server.health.path, "/readyz");
}

#[tokio::test]
async fn custom_health_path_router_exposes_only_configured_path() {
    let (hub, _rx) = Hub::new(HubConfig::default());
    let app = create_router_with_health_path(WebState::new(Arc::new(hub), None, None), "/readyz");

    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    let readyz_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let legacy_response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(readyz_response.status(), StatusCode::OK);
    assert_eq!(legacy_response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(readyz_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("healthy"));
}

#[tokio::test]
async fn disabled_health_config_router_does_not_expose_health_route() {
    let (hub, _rx) = Hub::new(HubConfig::default());
    let app = create_router_with_health_config(
        WebState::new(Arc::new(hub), None, None),
        &HealthConfig {
            enabled: false,
            path: "/readyz".to_string(),
            verbose: false,
        },
    );

    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    let readyz_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let legacy_response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(readyz_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(legacy_response.status(), StatusCode::NOT_FOUND);
}
