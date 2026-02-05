mod backfill;
mod db;
mod engagement;
mod handler;
mod schema;
pub mod scoring;
pub mod settings;
pub mod utils;

use anyhow::Result;
use db::{configure_connection, establish_pool};
use handler::GameDevFeedHandler;
use scoring::MLHandle;
use settings::settings;
use skyfeed::{start, Config};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use utils::logs;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let s = settings();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "feed.db".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3030);

    logs::log_init(&s.server.feed_hostname, port, s.server.enable_backfill);

    let pool = establish_pool(&database_url);

    {
        let mut conn = pool.get().expect("Failed to get initial connection");
        configure_connection(&mut conn).expect("Failed to configure SQLite connection");
    }

    logs::log_ml_loading();
    let ml_handle = MLHandle::spawn()?;
    logs::log_ml_ready();

    if s.server.enable_backfill {
        backfill::run_backfill(pool.clone(), &ml_handle).await;
    }

    let handler = Arc::new(Mutex::new(GameDevFeedHandler::new(pool, ml_handle)));

    let handler_flush = handler.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let mut h = handler_flush.lock().await;
            let _ = h.flush_pending();
        }
    });

    let handler_cleanup = handler.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let h = handler_cleanup.lock().await;
            let _ = h.cleanup_old_posts();
        }
    });

    let config = Config {
        publisher_did: s.server.publisher_did.clone(),
        feed_generator_hostname: s.server.feed_hostname.clone(),
    };

    start(
        config,
        s.server.firehose_limit,
        handler,
        ([0, 0, 0, 0], port),
    )
    .await;

    Ok(())
}
