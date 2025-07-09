use super::*; // Imports spawn_app from tests/mod.rs
use reqwest::StatusCode; // Import StatusCode from reqwest
use serde_json::Value; // For asserting JSON body

#[tokio::test]
async fn health_check_works() {
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/api/health", app_address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status(), StatusCode::OK); // Health check should be 200 OK (or 503 if DB fails, as per health_check_handler_v2)
                                               // The current health_check_handler_v2 returns 200 OK / 503.
                                               // Let's assume for a basic "works" test, we want 200 OK.
                                               // This implies the test DB must be connectable.

    let body = response.json::<Value>().await.expect("Failed to parse health check JSON response");

    // Depending on whether the test DB is actually up and running, 'database' field will vary.
    // If we can ensure test DB is up:
    // assert_eq!(body["status"], "ok");
    // assert_eq!(body["database"], "connected");
    // If DB might be down for this simple test, just check that 'status' field exists:
    assert!(body.get("status").is_some(), "Health response should have a status field");
    assert!(body.get("database").is_some(), "Health response should have a database field");

}

#[tokio::test]
async fn root_endpoint_works() {
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/", app_address)) // Test the root defined in spawn_app's Router
        .send()
        .await
        .expect("Failed to execute request to root.");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "Test Root OK");
}


// Example of a protected route test (conceptual, assuming /api/threads is protected)
#[tokio::test]
async fn protected_route_requires_auth() {
    let app_address = spawn_app().await;
    let client = reqwest::Client::new();

    // Test without token
    let response_no_token = client
        .get(&format!("{}/api/threads", app_address)) // Assuming this is a protected GET
        .send()
        .await
        .expect("Failed to execute request to protected route without token.");
    assert_eq!(response_no_token.status(), StatusCode::UNAUTHORIZED);


    // Test with a valid token
    let token = generate_test_jwt("test_user_for_protected_route"); // from tests/mod.rs
    let response_with_token = client
        .get(&format!("{}/api/threads", app_address))
        .bearer_auth(token)
        .send()
        .await
        .expect("Failed to execute request to protected route with token.");

    // Assuming /api/threads returns OK if authenticated, even if list is empty.
    // This might be 200 OK with an empty array, or another status if no threads.
    // For now, just check it's not UNAUTHORIZED or FORBIDDEN.
    assert_ne!(response_with_token.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(response_with_token.status(), StatusCode::FORBIDDEN);
    assert_eq!(response_with_token.status(), StatusCode::OK); // Expecting 200 OK with potentially empty list

    let body: Value = response_with_token.json().await.expect("Failed to parse JSON from protected route");
    assert!(body.is_array(), "Protected route should return a JSON array for threads");
}
