use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde:: Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(
    form: web::Form<FormData>,
    connection_pool: web::Data<PgPool>,
) -> HttpResponse {
    let request_id = Uuid::new_v4();
    tracing::info!(
        "id={} - adding '{}' '{}' as a new subscriber",
        request_id,
        form.name,
        form.email
    );

    match sqlx::query!(
        r#"
INSERT INTO subscriptions (id, email, name, subscribed_at)
VALUES ($1, $2, $3, $4)
"#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(connection_pool.get_ref())
    .await
    {
        Ok(_) => {
            tracing::info!("id={} - new subscriber data saved", request_id);
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::info!("id={} - failed to execute query: {}", request_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
