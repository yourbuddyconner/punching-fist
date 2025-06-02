use axum::http::StatusCode;
use punching_fist_operator::{
    config::Config,
    server::Server,
    sources::WebhookHandler,
    store::{create_store, DatabaseConfig, DatabaseType},
};
use serde_json::json;
use std::sync::Arc;
use std::path::PathBuf;
use tokio;

#[tokio::test]
async fn test_server_endpoints() {
    // Create a test configuration with SQLite in memory
    let database_config = DatabaseConfig {
        db_type: DatabaseType::Sqlite,
        sqlite_path: Some(PathBuf::from(":memory:")),
        connection_string: None,
    };

    // Create the store and initialize it
    let store = create_store(&database_config)
        .await
        .expect("Failed to create store");
    store.init().await.expect("Failed to initialize store");

    // Create webhook handler - pass None for the Kubernetes client in tests
    let webhook_handler = Arc::new(WebhookHandler::new(
        store.clone(),
        None,
    ));

    // Create config for server
    let mut config = Config::default();
    config.database = database_config;

    // Create and start the server
    let server = Server::new(&config, store, webhook_handler);
    let app = server.build_router();

    // Use axum's test client
    let client = axum_test::TestServer::new(app).unwrap();

    // Test health endpoint
    let response = client.get("/health").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "healthy");

    // Test create alert
    let create_payload = json!({
        "alert_name": "TestAlert",
        "severity": "warning",
        "summary": "This is a test alert",
        "labels": {
            "service": "test-service",
            "environment": "test"
        }
    });

    let response = client.post("/alerts")
        .json(&create_payload)
        .await;
    
    // Print response body if not successful
    if response.status_code() != StatusCode::CREATED {
        let body_text = response.text();
        eprintln!("Response status: {}", response.status_code());
        eprintln!("Response body: {}", body_text);
    }
    
    assert_eq!(response.status_code(), StatusCode::CREATED);
    let body: serde_json::Value = response.json();
    let alert_id = body["id"].as_str().unwrap();
    assert_eq!(body["message"], "Alert created successfully");

    // Test get alert
    let response = client.get(&format!("/alerts/{}", alert_id)).await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["alert_name"], "TestAlert");
    assert_eq!(body["severity"], "warning");

    // Test list alerts
    let response = client.get("/alerts?limit=10&offset=0").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: Vec<serde_json::Value> = response.json();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["id"], alert_id);

    // Test get non-existent alert
    let fake_id = "00000000-0000-0000-0000-000000000000";
    let response = client.get(&format!("/alerts/{}", fake_id)).await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"], "Alert not found");
}

#[tokio::test]
async fn test_create_alert_validation() {
    // Create a test configuration with SQLite in memory
    let database_config = DatabaseConfig {
        db_type: DatabaseType::Sqlite,
        sqlite_path: Some(PathBuf::from(":memory:")),
        connection_string: None,
    };

    // Create the store and initialize it
    let store = create_store(&database_config)
        .await
        .expect("Failed to create store");
    store.init().await.expect("Failed to initialize store");

    // Create webhook handler - pass None for the Kubernetes client in tests
    let webhook_handler = Arc::new(WebhookHandler::new(
        store.clone(),
        None,
    ));

    // Create config for server
    let mut config = Config::default();
    config.database = database_config;

    // Create and start the server
    let server = Server::new(&config, store, webhook_handler);
    let app = server.build_router();

    // Use axum's test client
    let client = axum_test::TestServer::new(app).unwrap();

    // Test invalid severity
    let invalid_payload = json!({
        "alert_name": "TestAlert",
        "severity": "invalid_severity",
        "summary": "This should fail"
    });

    let response = client.post("/alerts")
        .json(&invalid_payload)
        .await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json();
    assert!(body["message"].as_str().unwrap().contains("Invalid severity"));
} 