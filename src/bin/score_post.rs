use devlogs_feed::scoring::{
    has_hashtags, has_keywords, is_first_person, promo_penalty, should_prefilter, strip_hashtags,
    Filter, MLHandle, PostScorer, ScoringSignals, MIN_TEXT_LENGTH,
};
use devlogs_feed::utils::bluesky::{fetch_post, parse_bluesky_url};
use devlogs_feed::utils::{
    log_authenticity_signals, log_dimmed, log_fetch_error, log_fetch_start, log_fetch_success,
    log_final_result, log_generic_error, log_header, log_inference_start, log_ml_scores,
    log_ml_step, log_newline, log_post_header, log_prefilter_length_fail, log_prefilter_length_ok,
    log_prefilter_no_signals, log_prefilter_signals, log_relevance_signals, log_score_breakdown,
};
use std::env;
use std::process;

fn print_usage() {
    eprintln!("Usage: score-post <url|text> [--media|-m]");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <url>      Bluesky post URL (https://bsky.app/...) or AT URI (at://...)");
    eprintln!("  <text>     Raw post text to score");
    eprintln!("  --media    Indicate the post has media (only for raw text)");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let has_media_flag = args.iter().any(|a| a == "--media" || a == "-m");

    let text_args: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| *a != "--media" && *a != "-m")
        .collect();

    if text_args.is_empty() {
        print_usage();
        process::exit(1);
    }

    let input = text_args
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let (text, has_media, has_video, image_count) = if let Some(at_uri) = parse_bluesky_url(&input)
    {
        log_fetch_start(&at_uri);

        match fetch_post(&at_uri).await {
            Ok(post) => {
                log_fetch_success();
                (post.text, post.has_media, post.has_video, post.image_count)
            }
            Err(e) => {
                log_fetch_error(&e);
                process::exit(1);
            }
        }
    } else {
        (input, has_media_flag, false, 0)
    };

    log_newline();
    log_ml_step("Loading models...");
    log_dimmed("└─ This may take a while on first run");
    let ml_handle = match MLHandle::spawn() {
        Ok(handle) => handle,
        Err(e) => {
            log_generic_error("[ERROR]", &format!("Failed to load ML models: {e}"));
            process::exit(1);
        }
    };

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    score_post(&text, has_media, has_video, image_count, &ml_handle).await;
}

async fn score_post(
    text: &str,
    has_media: bool,
    has_video: bool,
    image_count: usize,
    ml_handle: &MLHandle,
) {
    log_post_header(text, has_media);
    log_newline();

    log_header("Pre-filter");
    if let Some(filter) = should_prefilter(text, Some("en")) {
        if filter == Filter::MinLength {
            log_prefilter_length_fail(strip_hashtags(text).len(), MIN_TEXT_LENGTH);
        }
        return;
    }
    log_prefilter_length_ok(strip_hashtags(text).len());

    let (found_keywords, keyword_count) = has_keywords(text);
    let (found_hashtags, hashtag_count) = has_hashtags(text);

    if !found_keywords && !found_hashtags {
        log_prefilter_no_signals();
        return;
    }

    log_prefilter_signals(found_keywords, keyword_count, found_hashtags, hashtag_count);
    log_newline();

    log_header("ML scoring");
    log_inference_start();
    let scores = ml_handle.score(text.to_string()).await;
    log_newline();

    log_ml_scores(&scores);
    log_newline();

    log_header("Signal analysis");
    let mut signals = ScoringSignals::new();
    signals.has_keywords = found_keywords;
    signals.keyword_count = keyword_count;
    signals.has_hashtags = found_hashtags;
    signals.hashtag_count = hashtag_count;
    signals.semantic_score = scores.semantic_score;
    signals.classification_score = scores.classification_score;
    signals.classification_label = scores.best_label.clone();
    signals.is_first_person = is_first_person(text);
    signals.has_media = has_media;
    signals.has_video = has_video;
    signals.image_count = image_count;
    let promo = promo_penalty(text);
    signals.promo_penalty = promo.total_penalty;
    signals.promo_breakdown = promo;
    signals.negative_rejection = scores.negative_rejection;
    signals.is_negative_label = scores.is_negative_label;
    signals.negative_label = scores.best_label.clone();
    signals.negative_label_score = scores.best_label_score;

    log_relevance_signals(&signals);
    log_newline();
    log_authenticity_signals(&signals);
    log_newline();

    log_header("Final score");
    let scorer = PostScorer::default();
    let breakdown = scorer.evaluate(&signals);

    log_score_breakdown(&breakdown);
    log_newline();

    log_final_result(breakdown.passes(), &breakdown);
    log_newline();
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
                has_media: post.has_media,
                has_video: post.has_video,
                image_count: post.image_count,
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
