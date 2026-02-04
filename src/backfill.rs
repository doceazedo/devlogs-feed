use crate::db::{self, DbPool, NewPost};
use crate::scoring::{
    apply_filters, apply_ml_filter, calculate_priority, calculate_score, extract_content_signals,
    has_hashtags, has_keywords, label_boost, FilterResult, MLHandle, MediaInfo, PrioritySignals,
};
use crate::utils::bluesky::{create_session, extract_facet_links, search_posts, SearchPost};
use crate::utils::logs::{self, PostAssessment};
use chrono::Utc;

pub const BACKFILL_LIMIT: usize = 200;
pub const BACKFILL_HOURS: i64 = 96;
const SEARCH_LIMIT: u32 = 50;

pub async fn run_backfill(pool: DbPool, ml_handle: &MLHandle) {
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
    let since = (Utc::now() - chrono::Duration::hours(BACKFILL_HOURS))
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

        if all_posts.len() >= BACKFILL_LIMIT {
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

    let total_to_process = all_posts.len().min(BACKFILL_LIMIT);
    let mut new_posts: Vec<NewPost> = Vec::new();
    let mut current = 0;
    let mut processed = 0;
    let mut duplicates = 0;
    let mut filtered = 0;
    let mut no_relevance = 0;
    let mut ml_rejected = 0;
    let mut below_threshold = 0;

    for post in all_posts.iter().take(BACKFILL_LIMIT) {
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

        let filter_result = apply_filters(text, lang, Some(&post.author.did), |_| false);
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

        let ml_scores = ml_handle.score(text.clone()).await;
        assessment.set_ml_scores(ml_scores.clone());

        let ml_filter_result = apply_ml_filter(
            &ml_scores.best_label,
            ml_scores.best_label_score,
            ml_scores.is_negative_label,
        );
        if matches!(ml_filter_result, FilterResult::Reject(_)) {
            ml_rejected += 1;
            continue;
        }

        let mut media_info = extract_media_from_embed(&post.embed);
        media_info.facet_links = extract_facet_links(&post.record.facets);
        let content = extract_content_signals(text, &media_info);
        assessment.set_content(content.clone(), media_info.clone());

        let score = calculate_score(ml_scores.classification_score, ml_scores.semantic_score);

        let signals = PrioritySignals {
            topic_label: ml_scores.best_label.clone(),
            label_boost: label_boost(&ml_scores.best_label),
            engagement_bait_score: ml_scores.quality.engagement_bait_score,
            synthetic_score: ml_scores.quality.synthetic_score,
            authenticity_score: ml_scores.quality.authenticity_score,
            is_first_person: content.is_first_person,
            images: content.images,
            has_video: content.has_video,
            has_alt_text: content.has_alt_text,
            link_count: content.link_count,
            promo_link_count: content.promo_link_count,
            ..Default::default()
        };

        let priority = calculate_priority(&score, &signals);
        assessment.set_score_and_priority(score.clone(), signals.clone(), priority.clone());

        let passed = score.passes_threshold();
        assessment.set_threshold_result(passed);
        assessment.print();

        if passed {
            let new_post = NewPost {
                uri: post.uri.clone(),
                text: text.clone(),
                timestamp,
                final_score: score.final_score,
                priority: priority.final_priority,
                confidence: priority.confidence.to_string(),
                post_type: priority.topic_label.clone(),
                keyword_score: if found_keywords { 1.0 } else { 0.0 },
                hashtag_score: if found_hashtags { 1.0 } else { 0.0 },
                semantic_score: score.semantic_score,
                classification_score: score.classification_score,
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
        } else {
            below_threshold += 1;
        }
    }

    let accepted = new_posts.len();
    logs::log_backfill_stats(
        duplicates,
        filtered,
        no_relevance,
        ml_rejected,
        below_threshold,
    );
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
