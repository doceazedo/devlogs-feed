use devlogs_feed::scoring::{
    apply_filters, calculate_priority, extract_content_signals, has_hashtags, has_keywords,
    FilterResult, MLHandle, MediaInfo, PrioritySignals,
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

    let quality = ml_handle.score(text.to_string()).await;

    let content = extract_content_signals(text, media);
    assessment.set_content(content.clone(), media.clone());

    let signals = PrioritySignals::new(&quality, &content);
    let priority = calculate_priority(&signals);
    assessment.set_priority(quality, signals, priority);
    assessment.print();
}

#[cfg(test)]
mod tests {
    use devlogs_feed::scoring::{
        apply_filters, extract_content_signals, has_hashtags, has_keywords, FilterResult, MLHandle,
        MediaInfo, PrioritySignals,
    };
    use devlogs_feed::utils::bluesky::{fetch_post, parse_bluesky_url};

    const POSTS_EXPECTED_ACCEPT: &[&str] = &[
        "at://did:plc:uthii4i7zrmqnbxex5esjxzp/app.bsky.feed.post/3maqv5l6xl22y",
        "at://did:plc:jguiyddnwoie7ddpfjsgacbk/app.bsky.feed.post/3mdt3m3fq422g",
    ];

    const POSTS_EXPECTED_REJECT: &[&str] = &[];

    async fn evaluate_url(url: &str, ml_handle: &MLHandle) -> bool {
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

        let filter_result = apply_filters(&post.text, Some("en"), None, |_| false);
        if matches!(filter_result, FilterResult::Reject(_)) {
            return false;
        }

        let (found_keywords, _) = has_keywords(&post.text);
        let (found_hashtags, _) = has_hashtags(&post.text);
        if !found_keywords && !found_hashtags {
            return false;
        }

        let quality = ml_handle.score(post.text.clone()).await;
        let content = extract_content_signals(&post.text, &media);

        let _signals = PrioritySignals::new(&quality, &content);
        true
    }

    #[tokio::test]
    async fn test_full_evaluation_accept() {
        let ml_handle = MLHandle::spawn().expect("Failed to spawn ML handle");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        for url in POSTS_EXPECTED_ACCEPT {
            let passes = evaluate_url(url, &ml_handle).await;
            assert!(
                passes,
                "Expected post to be ACCEPTED but was rejected: {}",
                url
            );
        }
    }

    #[tokio::test]
    async fn test_full_evaluation_reject() {
        if POSTS_EXPECTED_REJECT.is_empty() {
            return;
        }

        let ml_handle = MLHandle::spawn().expect("Failed to spawn ML handle");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        for url in POSTS_EXPECTED_REJECT {
            let passes = evaluate_url(url, &ml_handle).await;
            assert!(
                !passes,
                "Expected post to be REJECTED but was accepted: {}",
                url
            );
        }
    }
}
