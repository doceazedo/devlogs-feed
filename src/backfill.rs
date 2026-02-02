use crate::db::{self, DbPool, NewPost};
use crate::scoring::{
    has_hashtags, has_keywords, is_first_person, promo_penalty, should_prefilter, MLHandle,
    PostScorer, ScoringSignals,
};
use crate::utils::{log_accepted_post, log_rejected_post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const BACKFILL_MAX_HOURS: i64 = 24;
const BACKFILL_SEARCH_TERMS: &[&str] = &["#gamedev", "devlog"];
const SEARCH_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    identifier: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    #[serde(rename = "accessJwt")]
    access_jwt: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    posts: Vec<SearchPost>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchPost {
    uri: String,
    record: PostRecord,
    #[serde(rename = "indexedAt")]
    indexed_at: String,
    embed: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PostRecord {
    text: String,
    langs: Option<Vec<String>>,
}

async fn create_session(client: &reqwest::Client) -> anyhow::Result<String> {
    let identifier = std::env::var("BLUESKY_IDENTIFIER")
        .map_err(|_| anyhow::anyhow!("BLUESKY_IDENTIFIER not set (required for backfill)"))?;
    let password = std::env::var("BLUESKY_PASSWORD")
        .map_err(|_| anyhow::anyhow!("BLUESKY_PASSWORD not set (required for backfill)"))?;

    let response = client
        .post("https://bsky.social/xrpc/com.atproto.server.createSession")
        .json(&CreateSessionRequest {
            identifier,
            password,
        })
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Authentication failed: {}", response.status());
    }

    let session: CreateSessionResponse = response.json().await?;
    Ok(session.access_jwt)
}

pub async fn run_backfill(pool: DbPool, ml_handle: MLHandle) -> anyhow::Result<usize> {
    let (since_timestamp, until_timestamp) = calculate_time_range(&pool)?;

    if since_timestamp >= until_timestamp {
        tracing::info!("Backfill: Database is up to date, skipping");
        return Ok(0);
    }

    let since = DateTime::from_timestamp(since_timestamp, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let until = DateTime::from_timestamp(until_timestamp, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();

    tracing::info!(
        "Backfill: Searching for posts from {} to {}",
        since,
        until
    );

    let client = reqwest::Client::new();
    let access_token = create_session(&client).await?;
    tracing::info!("Backfill: Authenticated with Bluesky");

    let scorer = PostScorer::default();
    let mut total_inserted = 0;
    let mut seen_uris = std::collections::HashSet::new();

    for term in BACKFILL_SEARCH_TERMS {
        let inserted = search_and_insert(
            &client,
            &access_token,
            &pool,
            &ml_handle,
            &scorer,
            term,
            &since,
            &until,
            &mut seen_uris,
        )
        .await?;
        total_inserted += inserted;
    }

    tracing::info!("Backfill: Inserted {} posts", total_inserted);
    Ok(total_inserted)
}

fn calculate_time_range(pool: &DbPool) -> anyhow::Result<(i64, i64)> {
    let mut conn = pool.get()?;
    let now = Utc::now().timestamp();
    let max_lookback = now - (BACKFILL_MAX_HOURS * 3600);

    let since = match db::get_latest_post_timestamp(&mut conn)? {
        Some(latest) => latest.max(max_lookback),
        None => max_lookback,
    };

    Ok((since, now))
}

async fn search_and_insert(
    client: &reqwest::Client,
    access_token: &str,
    pool: &DbPool,
    ml_handle: &MLHandle,
    scorer: &PostScorer,
    query: &str,
    since: &str,
    until: &str,
    seen_uris: &mut std::collections::HashSet<String>,
) -> anyhow::Result<usize> {
    let mut cursor: Option<String> = None;
    let mut total_inserted = 0;

    loop {
        let response =
            fetch_search_page(client, access_token, query, since, until, cursor.as_deref()).await?;

        let mut posts_to_insert = Vec::new();

        for post in response.posts {
            if seen_uris.contains(&post.uri) {
                continue;
            }
            seen_uris.insert(post.uri.clone());

            if let Some(new_post) = score_post(&post, ml_handle, scorer).await {
                posts_to_insert.push(new_post);
            }
        }

        if !posts_to_insert.is_empty() {
            let mut conn = pool.get()?;
            let inserted = db::insert_posts(&mut conn, posts_to_insert)?;
            total_inserted += inserted;
        }

        match response.cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }

    Ok(total_inserted)
}

async fn fetch_search_page(
    client: &reqwest::Client,
    access_token: &str,
    query: &str,
    since: &str,
    until: &str,
    cursor: Option<&str>,
) -> anyhow::Result<SearchResponse> {
    let mut url = format!(
        "https://bsky.social/xrpc/app.bsky.feed.searchPosts?q={}&since={}&until={}&limit={}",
        urlencoding::encode(query),
        urlencoding::encode(since),
        urlencoding::encode(until),
        SEARCH_LIMIT
    );

    if let Some(c) = cursor {
        url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
    }

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Search API error: {}", response.status());
    }

    let search_response: SearchResponse = response.json().await?;
    Ok(search_response)
}

async fn score_post(
    post: &SearchPost,
    ml_handle: &MLHandle,
    scorer: &PostScorer,
) -> Option<NewPost> {
    let text = &post.record.text;
    let lang = post
        .record
        .langs
        .as_ref()
        .and_then(|l| l.first())
        .map(|s| s.as_str());

    if should_prefilter(text, lang).is_some() {
        return None;
    }

    let (found_keywords, keyword_count) = has_keywords(text);
    let (found_hashtags, hashtag_count) = has_hashtags(text);
    if !found_keywords && !found_hashtags {
        return None;
    }

    let mut signals = ScoringSignals::new();
    signals.has_keywords = found_keywords;
    signals.keyword_count = keyword_count;
    signals.has_hashtags = found_hashtags;
    signals.hashtag_count = hashtag_count;
    signals.is_first_person = is_first_person(text);
    signals.has_media = has_media(&post.embed);
    signals.has_video = has_video(&post.embed);
    signals.image_count = image_count(&post.embed);
    let promo_breakdown = promo_penalty(text);
    signals.promo_penalty = promo_breakdown.total_penalty;
    signals.promo_breakdown = promo_breakdown;

    let ml_scores = ml_handle.score(text.clone()).await;
    signals.semantic_score = ml_scores.semantic_score;
    signals.classification_score = ml_scores.classification_score;
    signals.classification_label = ml_scores.best_label.clone();
    signals.negative_rejection = ml_scores.negative_rejection;
    signals.is_negative_label = ml_scores.is_negative_label;
    signals.negative_label = ml_scores.best_label.clone();
    signals.negative_label_score = ml_scores.best_label_score;

    let breakdown = scorer.evaluate(&signals);
    let text_preview = crate::utils::truncate_text(text, 300);

    if breakdown.passes() {
        log_accepted_post(text_preview, &breakdown, &ml_scores);

        let timestamp = DateTime::parse_from_rfc3339(&post.indexed_at)
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|_| Utc::now().timestamp());

        Some(NewPost {
            uri: post.uri.clone(),
            text: text.clone(),
            timestamp,
            final_score: breakdown.final_score,
            priority: breakdown.priority,
            confidence: breakdown.confidence.label().to_string(),
            post_type: breakdown.classification_label.clone(),
            keyword_score: if breakdown.has_keywords { 1.0 } else { 0.0 },
            hashtag_score: if breakdown.has_hashtags { 1.0 } else { 0.0 },
            semantic_score: breakdown.semantic_score,
            classification_score: breakdown.classification_score,
            has_media: if breakdown.has_media { 1 } else { 0 },
            is_first_person: if breakdown.is_first_person { 1 } else { 0 },
        })
    } else {
        log_rejected_post(text_preview, &breakdown);
        None
    }
}

fn has_media(embed: &Option<serde_json::Value>) -> bool {
    let Some(embed) = embed else { return false };
    let embed_type = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");
    matches!(
        embed_type,
        "app.bsky.embed.images#view"
            | "app.bsky.embed.video#view"
            | "app.bsky.embed.recordWithMedia#view"
    )
}

fn has_video(embed: &Option<serde_json::Value>) -> bool {
    let Some(embed) = embed else { return false };
    let embed_type = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");
    if embed_type == "app.bsky.embed.video#view" {
        return true;
    }
    if embed_type == "app.bsky.embed.recordWithMedia#view" {
        if let Some(media) = embed.get("media") {
            let media_type = media.get("$type").and_then(|t| t.as_str()).unwrap_or("");
            return media_type == "app.bsky.embed.video#view";
        }
    }
    false
}

fn image_count(embed: &Option<serde_json::Value>) -> usize {
    let Some(embed) = embed else { return 0 };
    let embed_type = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");

    match embed_type {
        "app.bsky.embed.images#view" => embed
            .get("images")
            .and_then(|i| i.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0),
        "app.bsky.embed.recordWithMedia#view" => {
            if let Some(media) = embed.get("media") {
                let media_type = media.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                if media_type == "app.bsky.embed.images#view" {
                    return media
                        .get("images")
                        .and_then(|i| i.as_array())
                        .map(|arr| arr.len())
                        .unwrap_or(0);
                }
            }
            0
        }
        _ => 0,
    }
}
