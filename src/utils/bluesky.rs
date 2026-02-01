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

    Ok(FetchedPost {
        text: thread.thread.post.record.text,
        has_media: thread.thread.post.embed.is_some(),
    })
}
