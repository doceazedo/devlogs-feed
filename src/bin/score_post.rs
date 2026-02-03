use devlogs_feed::scoring::{
    apply_filters, apply_ml_filter, calculate_priority, calculate_score, extract_content_signals,
    has_hashtags, has_keywords, label_boost, FilterResult, MLHandle, MediaInfo,
    PrioritySignals,
};
use devlogs_feed::utils::bluesky::{fetch_post, parse_bluesky_url};
use devlogs_feed::utils::logs::{self, PostAssessment};
use std::env;
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let has_media_flag = args.iter().any(|a| a == "--media" || a == "-m");
    let has_video_flag = args.iter().any(|a| a == "--video" || a == "-v");
    let has_alt_flag = args.iter().any(|a| a == "--alt" || a == "-a");

    let text_args: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| {
            *a != "--media"
                && *a != "-m"
                && *a != "--video"
                && *a != "-v"
                && *a != "--alt"
                && *a != "-a"
        })
        .collect();

    if text_args.is_empty() {
        eprintln!("usage: score-post <url_or_text> [--media|-m] [--video|-v] [--alt|-a]");
        process::exit(1);
    }

    let input = text_args
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let (text, media_info) = if let Some(at_uri) = parse_bluesky_url(&input) {
        match fetch_post(&at_uri).await {
            Ok(post) => {
                let media = MediaInfo {
                    image_count: post.image_count.min(255) as u8,
                    has_video: post.has_video,
                    has_alt_text: false,
                    external_uri: post.external_uri,
                    facet_links: post.facet_links,
                };
                (post.text, media)
            }
            Err(e) => {
                eprintln!("error: failed to fetch post: {}", e);
                process::exit(1);
            }
        }
    } else {
        let media = MediaInfo {
            image_count: if has_media_flag { 1 } else { 0 },
            has_video: has_video_flag,
            has_alt_text: has_alt_flag,
            external_uri: None,
            facet_links: Vec::new(),
        };
        (input, media)
    };

    logs::log_ml_loading();
    let ml_handle = match MLHandle::spawn() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("error: failed to spawn ml handle: {}", e);
            process::exit(1);
        }
    };

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    logs::log_ml_ready();

    score_post(&text, &media_info, &ml_handle).await;
}

async fn score_post(text: &str, media: &MediaInfo, ml_handle: &MLHandle) {
    let mut assessment = PostAssessment::new(text);

    let filter_result = apply_filters(text, Some("en"), None, |_| false);
    assessment.set_filter_result(filter_result.clone());
    if let FilterResult::Reject(_) = &filter_result {
        assessment.print();
        return;
    }

    let (found_keywords, _) = has_keywords(text);
    let (found_hashtags, _) = has_hashtags(text);
    assessment.set_relevance(found_keywords, found_hashtags);

    if !found_keywords && !found_hashtags {
        assessment.print();
        return;
    }

    let scores = ml_handle.score(text.to_string()).await;
    assessment.set_ml_scores(scores.clone());

    let ml_filter_result = apply_ml_filter(
        &scores.best_label,
        scores.best_label_score,
        scores.is_negative_label,
    );
    if let FilterResult::Reject(_) = ml_filter_result {
        assessment.print();
        return;
    }

    let content = extract_content_signals(text, media);
    assessment.set_content(content.clone(), media.clone());

    let score = calculate_score(scores.classification_score, scores.semantic_score);

    let signals = PrioritySignals {
        topic_label: scores.best_label.clone(),
        label_boost: label_boost(&scores.best_label),
        engagement_bait_score: scores.quality.engagement_bait_score,
        synthetic_score: scores.quality.synthetic_score,
        authenticity_score: scores.quality.authenticity_score,
        is_first_person: content.is_first_person,
        images: content.images,
        has_video: content.has_video,
        has_alt_text: content.has_alt_text,
        link_count: content.link_count,
        promo_link_count: content.promo_link_count,
        engagement_velocity: 0.0,
        reply_count: 0,
        repost_count: 0,
        like_count: 0,
    };

    let priority = calculate_priority(&score, &signals);
    assessment.set_score_and_priority(score.clone(), signals.clone(), priority.clone());

    let passed = score.passes_threshold();
    assessment.set_threshold_result(passed);
    assessment.print();
}

#[cfg(test)]
mod tests {
    use devlogs_feed::scoring::{evaluate_post, MLHandle, MediaInfo};
    use devlogs_feed::utils::bluesky::{fetch_post, parse_bluesky_url};

    const POSTS_EXPECTED_ACCEPT: &[&str] = &[
        "at://did:plc:uthii4i7zrmqnbxex5esjxzp/app.bsky.feed.post/3maqv5l6xl22y",
        "at://did:plc:jguiyddnwoie7ddpfjsgacbk/app.bsky.feed.post/3mdt3m3fq422g",
    ];

    const POSTS_EXPECTED_REJECT: &[&str] = &[
        "at://did:plc:yrry24lfrtfzidba3kbmgdwm/app.bsky.feed.post/3mdrpa7d4qi2b",
        "at://did:plc:uyx65egegsinxxncka7m5ijy/app.bsky.feed.post/3mdt43fab2s2y",
    ];

    async fn evaluate_urls(urls: &[&str], ml_handle: &MLHandle, expect_pass: bool) {
        for url in urls {
            let at_uri = parse_bluesky_url(url).expect("Invalid URL format");
            let post = fetch_post(&at_uri)
                .await
                .unwrap_or_else(|e| panic!("Failed to fetch {}: {}", url, e));

            let media = MediaInfo {
                image_count: post.image_count.min(255) as u8,
                has_video: post.has_video,
                has_alt_text: false,
                external_uri: post.external_uri.clone(),
                facet_links: post.facet_links.clone(),
            };
            let result = evaluate_post(&post.text, media, ml_handle).await;
            let passes = result.passes();

            if expect_pass {
                assert!(
                    passes,
                    "Expected post to be ACCEPTED but was rejected: {}\nText: {}",
                    url, post.text
                );
            } else {
                assert!(
                    !passes,
                    "Expected post to be REJECTED but was accepted: {}\nText: {}",
                    url, post.text
                );
            }
        }
    }

    #[tokio::test]
    async fn test_full_evaluation_accept() {
        let ml_handle = MLHandle::spawn().expect("Failed to spawn ML handle");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        evaluate_urls(POSTS_EXPECTED_ACCEPT, &ml_handle, true).await;
    }

    #[tokio::test]
    async fn test_full_evaluation_reject() {
        let ml_handle = MLHandle::spawn().expect("Failed to spawn ML handle");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        evaluate_urls(POSTS_EXPECTED_REJECT, &ml_handle, false).await;
    }
}
