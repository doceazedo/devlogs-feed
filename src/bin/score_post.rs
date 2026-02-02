use devlogs_feed::scoring::{
    apply_filters, apply_ml_filter, calculate_priority, extract_content_signals, has_hashtags,
    has_keywords, label_multiplier, passes_threshold, strip_hashtags, Filter, FilterResult,
    MLHandle, MediaInfo, PrioritySignals, MIN_TEXT_LENGTH,
};
use devlogs_feed::utils::bluesky::{fetch_post, parse_bluesky_url};
use devlogs_feed::utils::{
    log_content_signals, log_dimmed, log_fetch_error, log_fetch_start, log_fetch_success,
    log_final_result, log_generic_error, log_header, log_inference_start, log_ml_scores,
    log_newline, log_post_header, log_prefilter_length_fail, log_prefilter_length_ok,
    log_prefilter_no_signals, log_prefilter_signals, log_priority_breakdown,
};
use std::env;
use std::process;

fn print_usage() {
    eprintln!("Usage: score-post <url|text> [--media|-m] [--video|-v] [--alt|-a]");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <url>      Bluesky post URL (https://bsky.app/...) or AT URI (at://...)");
    eprintln!("  <text>     Raw post text to score");
    eprintln!("  --media    Indicate the post has media (images)");
    eprintln!("  --video    Indicate the post has video");
    eprintln!("  --alt      Indicate images have ALT text");
}

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
        print_usage();
        process::exit(1);
    }

    let input = text_args
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let (text, media_info) = if let Some(at_uri) = parse_bluesky_url(&input) {
        log_fetch_start(&at_uri);

        match fetch_post(&at_uri).await {
            Ok(post) => {
                log_fetch_success();
                let media = MediaInfo {
                    image_count: post.image_count.min(255) as u8,
                    has_video: post.has_video,
                    has_alt_text: false,
                };
                (post.text, media)
            }
            Err(e) => {
                log_fetch_error(&e);
                process::exit(1);
            }
        }
    } else {
        let media = MediaInfo {
            image_count: if has_media_flag { 1 } else { 0 },
            has_video: has_video_flag,
            has_alt_text: has_alt_flag,
        };
        (input, media)
    };

    log_newline();
    log_header("Loading ML models...");
    log_dimmed("└─ This may take a while on first run");
    let ml_handle = match MLHandle::spawn() {
        Ok(handle) => handle,
        Err(e) => {
            log_generic_error("[ERROR]", &format!("Failed to load ML models: {e}"));
            process::exit(1);
        }
    };

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    score_post(&text, &media_info, &ml_handle).await;
}

async fn score_post(text: &str, media: &MediaInfo, ml_handle: &MLHandle) {
    let has_media = media.image_count > 0 || media.has_video;
    log_post_header(text, has_media);
    log_newline();

    log_header("Phase 1: Filters");

    let filter_result = apply_filters(text, Some("en"), None, |_| false);
    if let FilterResult::Reject(filter) = &filter_result {
        match filter {
            Filter::MinLength => {
                log_prefilter_length_fail(strip_hashtags(text).len(), MIN_TEXT_LENGTH);
            }
            _ => {
                println!("  Rejected: {}", filter);
            }
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

    log_header("Phase 2: ML Classification");
    log_inference_start();
    let scores = ml_handle.score(text.to_string()).await;
    log_newline();

    log_ml_scores(&scores);
    log_newline();

    let ml_filter_result = apply_ml_filter(
        &scores.best_label,
        scores.best_label_score,
        scores.is_negative_label,
    );
    if let FilterResult::Reject(filter) = ml_filter_result {
        println!(
            "  {} ML filter rejection: {}",
            "REJECTED".to_string(),
            filter
        );
        return;
    }

    log_header("Phase 3: Content Signals");
    let content = extract_content_signals(text, media);
    log_content_signals(
        content.is_first_person,
        content.images,
        content.has_video,
        content.has_alt_text,
        content.link_count,
        content.promo_link_count,
    );
    log_newline();

    log_header("Phase 4: Priority Calculation");
    let signals = PrioritySignals {
        topic_classification_score: scores.classification_score,
        semantic_score: scores.semantic_score,
        topic_label: scores.best_label.clone(),
        label_multiplier: label_multiplier(&scores.best_label),
        engagement_bait_score: scores.quality.engagement_bait_score,
        synthetic_score: scores.quality.synthetic_score,
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

    let breakdown = calculate_priority(&signals);
    log_priority_breakdown(&breakdown);
    log_newline();

    log_final_result(passes_threshold(&breakdown), &breakdown);
    log_newline();
}
