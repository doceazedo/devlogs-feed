use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PostThreadResponse {
    thread: ThreadPost,
}

#[derive(Debug, Deserialize)]
struct ThreadPost {
    post: PostRecord,
}

#[derive(Debug, Deserialize)]
struct PostRecord {
    record: PostContent,
    embed: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PostContent {
    text: String,
}

#[derive(Debug, Clone)]
pub struct FetchedPost {
    pub text: String,
    pub has_media: bool,
    pub has_video: bool,
    pub image_count: usize,
}

pub fn parse_bluesky_url(input: &str) -> Option<String> {
    if input.starts_with("at://") {
        return Some(input.to_string());
    }

    let url_regex = Regex::new(r"https://bsky\.app/profile/([^/]+)/post/([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = url_regex.captures(input) {
        let did_or_handle = caps.get(1)?.as_str();
        let rkey = caps.get(2)?.as_str();
        return Some(format!(
            "at://{}/app.bsky.feed.post/{}",
            did_or_handle, rkey
        ));
    }

    None
}

pub async fn fetch_post(at_uri: &str) -> Result<FetchedPost, String> {
    let url = format!(
        "https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread?uri={}&depth=0",
        urlencoding::encode(at_uri)
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch post: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let thread: PostThreadResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let (has_media, has_video, image_count) = extract_media_info(&thread.thread.post.embed);

    Ok(FetchedPost {
        text: thread.thread.post.record.text,
        has_media,
        has_video,
        image_count,
    })
}

fn extract_media_info(embed: &Option<serde_json::Value>) -> (bool, bool, usize) {
    let Some(embed) = embed else {
        return (false, false, 0);
    };

    let embed_type = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");

    match embed_type {
        "app.bsky.embed.video#view" => (true, true, 0),
        "app.bsky.embed.images#view" => {
            let count = embed
                .get("images")
                .and_then(|i| i.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            (count > 0, false, count)
        }
        "app.bsky.embed.recordWithMedia#view" => {
            let media = embed.get("media");
            if let Some(media) = media {
                let media_type = media.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                match media_type {
                    "app.bsky.embed.video#view" => (true, true, 0),
                    "app.bsky.embed.images#view" => {
                        let count = media
                            .get("images")
                            .and_then(|i| i.as_array())
                            .map(|arr| arr.len())
                            .unwrap_or(0);
                        (count > 0, false, count)
                    }
                    _ => (false, false, 0),
                }
            } else {
                (false, false, 0)
            }
        }
        _ => (false, false, 0),
    }
}
