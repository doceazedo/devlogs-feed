mod backfill;
mod db;
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
use tracing::subscriber::set_global_default;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use utils::{
    log_cleanup_done, log_db_error, log_db_ready, log_db_status, log_server_starting,
    log_startup_config,
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("devlogs_feed=info".parse()?))
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact(),
        );
    set_global_default(subscriber).expect("Failed to set tracing subscriber");

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
    let ml_workers: usize = std::env::var("ML_WORKERS")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or(1);

    log_startup_config(
        &publisher_did,
        &hostname,
        port,
        &database_url,
        firehose_limit,
        ml_workers,
    );

    log_db_status("Initializing SQLite connection pool...");
    let pool = establish_pool(&database_url);

    {
        let mut conn = pool.get().expect("Failed to get initial connection");
        configure_connection(&mut conn).expect("Failed to configure SQLite connection");
    }
    log_db_ready();

    utils::log_ml_loading(&format!("Spawning {ml_workers} ML worker thread(s)..."));
    utils::log_ml_loading("Models will load in background (this may take a while on first run)");
    let ml_handle = MLHandle::spawn(ml_workers)?;

    utils::log_backfill_start();
    match backfill::run_backfill(pool.clone(), ml_handle.clone()).await {
        Ok(count) => utils::log_backfill_done(count),
        Err(e) => utils::log_backfill_error(&e.to_string()),
    }

    let handler = Arc::new(Mutex::new(GameDevFeedHandler::new(pool, ml_handle)));

    let handler_flush = handler.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let mut h = handler_flush.lock().await;
            if let Err(e) = h.flush_pending() {
                log_db_error(&format!("Flush error: {}", e));
            }
        }
    });

    let handler_cleanup = handler.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let h = handler_cleanup.lock().await;
            match h.cleanup_old_posts() {
                Ok(deleted) if deleted > 0 => {
                    log_cleanup_done(deleted);
                }
                Err(e) => {
                    log_db_error(&format!("Cleanup error: {}", e));
                }
                _ => {}
            }
        }
    });

    let config = Config {
        publisher_did,
        feed_generator_hostname: hostname,
    };

    log_server_starting(port);

    start(config, firehose_limit, handler, ([0, 0, 0, 0], port)).await;

    Ok(())
}
