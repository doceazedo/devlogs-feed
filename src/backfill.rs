use crate::db::{self, DbPool, NewPost};
use crate::scoring::{
    apply_filters, apply_ml_filter, calculate_priority, extract_content_signals, has_hashtags,
    has_keywords, label_multiplier, passes_threshold, FilterResult, MLHandle, MediaInfo,
    PrioritySignals,
};
use crate::utils::{
    log_backfill_done, log_backfill_error, log_backfill_progress, log_backfill_skipped,
    log_backfill_start,
};
use chrono::Utc;
use serde::Deserialize;

pub const BACKFILL_LIMIT: usize = 100;
pub const BACKFILL_HOURS: i64 = 96;

#[derive(Debug, Deserialize)]
struct SearchResponse {
    posts: Vec<SearchPost>,
    #[allow(dead_code)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchPost {
    uri: String,
    author: SearchAuthor,
    record: SearchRecord,
    #[serde(rename = "indexedAt")]
    indexed_at: String,
    embed: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SearchAuthor {
    did: String,
}

#[derive(Debug, Deserialize)]
struct SearchRecord {
    text: String,
    langs: Option<Vec<String>>,
}

pub async fn run_backfill(pool: DbPool, ml_handle: &MLHandle) {
    log_backfill_start();

    let search_queries = vec![
        "#gamedev",
        "#indiedev",
        "#devlog",
        "#screenshotsaturday",
        "gamedev progress",
        "devlog update",
    ];

    let mut all_posts: Vec<SearchPost> = Vec::new();
    let client = reqwest::Client::new();

    for query in &search_queries {
        match fetch_search_results(&client, query).await {
            Ok(posts) => {
                all_posts.extend(posts);
            }
            Err(e) => {
                log_backfill_error(&format!("Search failed for '{}': {}", query, e));
            }
        }

        if all_posts.len() >= BACKFILL_LIMIT * 2 {
            break;
        }
    }

    if all_posts.is_empty() {
        log_backfill_skipped("No posts found from search");
        return;
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log_backfill_error(&format!("Failed to get DB connection: {}", e));
            return;
        }
    };

    let cutoff = Utc::now().timestamp() - (BACKFILL_HOURS * 3600);
    let mut scored = 0;
    let mut accepted = 0;
    let mut new_posts: Vec<NewPost> = Vec::new();

    for post in all_posts.iter().take(BACKFILL_LIMIT * 2) {
        if db::post_exists(&mut conn, &post.uri) {
            continue;
        }

        let timestamp = chrono::DateTime::parse_from_rfc3339(&post.indexed_at)
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|_| Utc::now().timestamp());

        if timestamp < cutoff {
            continue;
        }

        let text = &post.record.text;
        let lang = post
            .record
            .langs
            .as_ref()
            .and_then(|l| l.first())
            .map(|s| s.as_str());

        let filter_result = apply_filters(text, lang, Some(&post.author.did), |_| false);
        if matches!(filter_result, FilterResult::Reject(_)) {
            continue;
        }

        let (found_keywords, _) = has_keywords(text);
        let (found_hashtags, _) = has_hashtags(text);
        if !found_keywords && !found_hashtags {
            continue;
        }

        scored += 1;

        let ml_scores = ml_handle.score(text.clone()).await;

        let ml_filter_result = apply_ml_filter(
            &ml_scores.best_label,
            ml_scores.best_label_score,
            ml_scores.is_negative_label,
        );
        if matches!(ml_filter_result, FilterResult::Reject(_)) {
            continue;
        }

        let media_info = extract_media_from_embed(&post.embed);
        let content = extract_content_signals(text, &media_info);

        let signals = PrioritySignals {
            topic_classification_score: ml_scores.classification_score,
            semantic_score: ml_scores.semantic_score,
            topic_label: ml_scores.best_label.clone(),
            label_multiplier: label_multiplier(&ml_scores.best_label),
            engagement_bait_score: ml_scores.quality.engagement_bait_score,
            synthetic_score: ml_scores.quality.synthetic_score,
            is_first_person: content.is_first_person,
            images: content.images,
            has_video: content.has_video,
            has_alt_text: content.has_alt_text,
            link_count: content.link_count,
            promo_link_count: content.promo_link_count,
            ..Default::default()
        };

        let breakdown = calculate_priority(&signals);

        if passes_threshold(&breakdown) {
            accepted += 1;

            let new_post = NewPost {
                uri: post.uri.clone(),
                text: text.clone(),
                timestamp,
                final_score: breakdown.final_priority,
                priority: breakdown.final_priority,
                confidence: breakdown.confidence.to_string(),
                post_type: breakdown.topic_label.clone(),
                keyword_score: if found_keywords { 1.0 } else { 0.0 },
                hashtag_score: if found_hashtags { 1.0 } else { 0.0 },
                semantic_score: ml_scores.semantic_score,
                classification_score: ml_scores.classification_score,
                has_media: if media_info.image_count > 0 || media_info.has_video {
                    1
                } else {
                    0
                },
                is_first_person: if content.is_first_person { 1 } else { 0 },
                author_did: Some(post.author.did.clone()),
                image_count: content.images as i32,
                has_alt_text: if content.has_alt_text { 1 } else { 0 },
                link_count: content.link_count as i32,
                promo_link_count: content.promo_link_count as i32,
            };

            new_posts.push(new_post);

            if new_posts.len() >= BACKFILL_LIMIT {
                break;
            }
        }

        if scored % 20 == 0 {
            log_backfill_progress(scored, scored, accepted);
        }
    }

    if !new_posts.is_empty() {
        match db::insert_posts(&mut conn, new_posts) {
            Ok(_) => {}
            Err(e) => {
                log_backfill_error(&format!("Failed to insert posts: {}", e));
            }
        }
    }

    log_backfill_done(accepted);
}

async fn fetch_search_results(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<SearchPost>, String> {
    let url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.feed.searchPosts?q={}&limit=50",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let search_response: SearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    Ok(search_response.posts)
}

fn extract_media_from_embed(embed: &Option<serde_json::Value>) -> MediaInfo {
    let Some(embed) = embed else {
        return MediaInfo::default();
    };

    let embed_type = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");

    match embed_type {
        "app.bsky.embed.video#view" => MediaInfo {
            image_count: 0,
            has_video: true,
            has_alt_text: false,
        },
        "app.bsky.embed.images#view" => {
            let images = embed.get("images").and_then(|i| i.as_array());
            let count = images.map(|arr| arr.len()).unwrap_or(0).min(255) as u8;
            let has_alt = images
                .map(|arr| {
                    arr.iter().any(|img| {
                        img.get("alt")
                            .and_then(|a| a.as_str())
                            .map(|s| !s.is_empty())
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            MediaInfo {
                image_count: count,
                has_video: false,
                has_alt_text: has_alt,
            }
        }
        "app.bsky.embed.recordWithMedia#view" => {
            if let Some(media) = embed.get("media") {
                let media_type = media.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                match media_type {
                    "app.bsky.embed.video#view" => MediaInfo {
                        image_count: 0,
                        has_video: true,
                        has_alt_text: false,
                    },
                    "app.bsky.embed.images#view" => {
                        let images = media.get("images").and_then(|i| i.as_array());
                        let count = images.map(|arr| arr.len()).unwrap_or(0).min(255) as u8;
                        let has_alt = images
                            .map(|arr| {
                                arr.iter().any(|img| {
                                    img.get("alt")
                                        .and_then(|a| a.as_str())
                                        .map(|s| !s.is_empty())
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);
                        MediaInfo {
                            image_count: count,
                            has_video: false,
                            has_alt_text: has_alt,
                        }
                    }
                    _ => MediaInfo::default(),
                }
            } else {
                MediaInfo::default()
            }
        }
        _ => MediaInfo::default(),
    }
}
