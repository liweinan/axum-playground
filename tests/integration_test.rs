use serde_json::json;
use uuid::Uuid;
use std::time::Duration;

fn generate_unique_username() -> String {
    format!("test_user_{}", Uuid::new_v4().to_string().split('-').next().unwrap())
}

#[tokio::test]
async fn test_create_user() {
    let client = reqwest::Client::new();
    let username = generate_unique_username();
    
    let response = client
        .post("http://localhost:3000/users")
        .json(&json!({
            "username": username
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body.is_object());
    assert!(body.get("id").is_some());
    assert_eq!(body["username"], username);
}

#[tokio::test]
async fn test_get_users() {
    let client = reqwest::Client::new();
    
    let response = client
        .get("http://localhost:3000/users?page=1&page_size=10")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["r"].as_bool().unwrap());
    assert!(body["d"].is_array());
    assert!(body["e"].is_null());
}

#[tokio::test]
async fn test_find_all_sql_users() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    
    // First create a user to ensure we have data
    let username = generate_unique_username();
    let _ = client
        .post("http://localhost:3000/users")
        .json(&json!({
            "username": username
        }))
        .send()
        .await
        .unwrap();
    
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Now try to get all SQL users
    let response = client
        .get("http://localhost:3000/find_all_sql_users")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["r"].as_bool().unwrap());
    assert!(body["d"].is_array());
    assert!(body["e"].is_null());
} 