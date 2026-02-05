use crate::db::{self, DbPool, NewPost};
use crate::scoring::{
    apply_filters, calculate_priority, extract_content_signals, has_hashtags, has_keywords,
    FilterResult, MLHandle, MediaInfo, PrioritySignals,
};
use crate::settings::settings;
use crate::utils::bluesky::{create_session, extract_facet_links, search_posts, SearchPost};
use crate::utils::logs::{self, PostAssessment};
use chrono::Utc;

const SEARCH_LIMIT: u32 = 50;

pub async fn run_backfill(pool: DbPool, ml_handle: &MLHandle) {
    let s = settings();
    logs::log_backfill_start();

    let client = reqwest::Client::new();

    let access_token = match create_session(&client).await {
        Ok(token) => token,
        Err(e) => {
            logs::log_backfill_auth_failed(&e);
            return;
        }
    };

    let search_queries = vec!["gamedev", "indiedev", "devlog", "game development"];
    let since = (Utc::now() - chrono::Duration::hours(s.backfill.hours))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let mut all_posts: Vec<SearchPost> = Vec::new();

    for query in &search_queries {
        match search_posts(&client, &access_token, query, SEARCH_LIMIT, Some(&since)).await {
            Ok(posts) => {
                logs::log_backfill_query(query, posts.len());
                all_posts.extend(posts);
            }
            Err(e) => {
                logs::log_backfill_query_failed(query, &e);
            }
        }

        if all_posts.len() >= s.backfill.limit {
            break;
        }
    }

    if all_posts.is_empty() {
        logs::log_backfill_complete(0, 0);
        return;
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };

    let total_to_process = all_posts.len().min(s.backfill.limit);
    let mut new_posts: Vec<NewPost> = Vec::new();
    let mut current = 0;
    let mut processed = 0;
    let mut duplicates = 0;
    let mut filtered = 0;
    let mut no_relevance = 0;

    for post in all_posts.iter().take(s.backfill.limit) {
        current += 1;
        logs::log_backfill_progress(current, total_to_process);

        if db::post_exists(&mut conn, &post.uri) {
            duplicates += 1;
            continue;
        }

        if post.record.reply.is_some() {
            continue;
        }

        let timestamp = chrono::DateTime::parse_from_rfc3339(&post.indexed_at)
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|_| Utc::now().timestamp());

        processed += 1;

        let text = &post.record.text;
        let lang = post
            .record
            .langs
            .as_ref()
            .and_then(|l| l.first())
            .map(|s| s.as_str());

        let mut assessment = PostAssessment::new(text);

        let mut media_info = extract_media_from_embed(&post.embed);
        media_info.facet_links = extract_facet_links(&post.record.facets);

        let filter_result =
            apply_filters(text, lang, Some(&post.author.did), &media_info, |_| false);
        assessment.set_filter_result(filter_result.clone());
        if matches!(filter_result, FilterResult::Reject(_)) {
            filtered += 1;
            continue;
        }

        let (found_keywords, _) = has_keywords(text);
        let (found_hashtags, _) = has_hashtags(text);
        assessment.set_relevance(found_keywords, found_hashtags);
        if !found_keywords && !found_hashtags {
            no_relevance += 1;
            continue;
        }

        let quality = ml_handle.score(text.clone()).await;

        let content = extract_content_signals(text, &media_info);
        assessment.set_content(content.clone(), media_info.clone());

        let signals = PrioritySignals::new(&quality, &content);
        let priority = calculate_priority(&signals);
        assessment.set_priority(quality, signals, priority.clone());
        assessment.print();

        if priority.priority < settings().scoring.rejection.min_priority {
            filtered += 1;
            continue;
        }

        let new_post = NewPost::new(
            post.uri.clone(),
            text.clone(),
            timestamp,
            priority.priority,
            &media_info,
            &content,
            Some(post.author.did.clone()),
        );

        new_posts.push(new_post);

        if new_posts.len() >= s.backfill.limit {
            break;
        }
    }

    let accepted = new_posts.len();
    logs::log_backfill_stats(duplicates, filtered, no_relevance);
    if !new_posts.is_empty() {
        let _ = db::insert_posts(&mut conn, new_posts);
    }

    logs::log_backfill_complete(accepted, processed);
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
            external_uri: None,
            facet_links: Vec::new(),
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
                external_uri: None,
                facet_links: Vec::new(),
            }
        }
        "app.bsky.embed.external#view" => {
            let uri = embed
                .get("external")
                .and_then(|e| e.get("uri"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string());
            MediaInfo {
                image_count: 0,
                has_video: false,
                has_alt_text: false,
                external_uri: uri,
                facet_links: Vec::new(),
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
                        external_uri: None,
                        facet_links: Vec::new(),
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
                            external_uri: None,
                            facet_links: Vec::new(),
                        }
                    }
                    "app.bsky.embed.external#view" => {
                        let uri = media
                            .get("external")
                            .and_then(|e| e.get("uri"))
                            .and_then(|u| u.as_str())
                            .map(|s| s.to_string());
                        MediaInfo {
                            image_count: 0,
                            has_video: false,
                            has_alt_text: false,
                            external_uri: uri,
                            facet_links: Vec::new(),
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
