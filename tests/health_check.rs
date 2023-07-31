use std::net::TcpListener;

use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    startup::run,
};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Failed to connect to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&configuration.database).await;

    let server = run(listener, connection_pool.clone()).expect("failed to start server");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }
}

#[tokio::test]
async fn health_check_works() {
    let test_app_data = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", test_app_data.address))
        .send()
        .await
        .expect("failed to send GET request");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let test_app_data = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=test&email=test%40testmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &test_app_data.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&test_app_data.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");
    assert_eq!(saved.name, "test");
    assert_eq!(saved.email, "test@testmail.com");
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_missing() {
    let test_app_data = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=test", "missing email"),
        ("email=test%40testmail.com", "missing name"),
        ("", "missing both email and name"),
    ];

    for (form_data, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &test_app_data.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_data)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 when presented with bad data, test-case: {}",
            error_message
        );
    }
}
