mod backfill;
mod db;
mod engagement;
mod handler;
mod schema;
pub mod scoring;
pub mod utils;

use anyhow::Result;
use db::{configure_connection, establish_pool};
use handler::GameDevFeedHandler;
use scoring::MLHandle;
use skyfeed::{start, Config};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use utils::logs;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let publisher_did =
        std::env::var("PUBLISHER_DID").unwrap_or_else(|_| "did:web:example.com".to_string());
    let hostname = std::env::var("FEED_HOSTNAME").unwrap_or_else(|_| "example.com".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3030);
    let firehose_limit: usize = std::env::var("FIREHOSE_LIMIT")
        .ok()
        .and_then(|l| l.parse().ok())
        .unwrap_or(5000);
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "feed.db".to_string());
    let enable_backfill = std::env::var("ENABLE_BACKFILL")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    logs::log_init(&hostname, port, enable_backfill);

    let pool = establish_pool(&database_url);

    {
        let mut conn = pool.get().expect("Failed to get initial connection");
        configure_connection(&mut conn).expect("Failed to configure SQLite connection");
    }

    logs::log_ml_loading();
    let ml_handle = MLHandle::spawn()?;
    logs::log_ml_ready();

    if enable_backfill {
        tokio::time::sleep(Duration::from_secs(10)).await;
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
        publisher_did,
        feed_generator_hostname: hostname.clone(),
    };

    start(config, firehose_limit, handler, ([0, 0, 0, 0], port)).await;

    Ok(())
}
