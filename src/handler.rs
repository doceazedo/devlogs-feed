use crate::db::{self, DbPool, NewLike, NewPost};
use crate::scoring::{
    apply_time_decay, has_hashtags, has_keywords, is_first_person, promo_penalty, should_prefilter,
    MLHandle, PostScorer, ScoreBreakdown, ScoringSignals,
};
use crate::utils::{log_accepted_post, log_feed_error, log_feed_served, log_rejected_post};
use chrono::Utc;
use rayon::prelude::*;
use skyfeed::{Embed, FeedHandler, FeedRequest, FeedResult, Post, Uri};

pub const FEED_CUTOFF_HOURS: i64 = 96;
pub const FEED_DEFAULT_LIMIT: usize = 50;
pub const FEED_MAX_LIMIT: usize = 500;

pub const MAX_STORED_POSTS: i64 = 5000;

#[derive(Clone)]
pub struct GameDevFeedHandler {
    pool: DbPool,
    ml_handle: MLHandle,
    scorer: PostScorer,
    pending_posts: Vec<NewPost>,
    pending_likes: Vec<NewLike>,
    pending_deletes: Vec<String>,
    pending_like_deletes: Vec<String>,
}

impl GameDevFeedHandler {
    pub fn new(pool: DbPool, ml_handle: MLHandle) -> Self {
        Self {
            pool,
            ml_handle,
            scorer: PostScorer::default(),
            pending_posts: Vec::new(),
            pending_likes: Vec::new(),
            pending_deletes: Vec::new(),
            pending_like_deletes: Vec::new(),
        }
    }

    fn has_media(post: &Post) -> bool {
        matches!(
            &post.embed,
            Some(Embed::Images(_)) | Some(Embed::Video(_)) | Some(Embed::QuoteWithMedia(_, _))
        )
    }

    fn has_video(post: &Post) -> bool {
        matches!(
            &post.embed,
            Some(Embed::Video(_)) | Some(Embed::QuoteWithMedia(_, skyfeed::MediaEmbed::Video(_)))
        )
    }

    fn image_count(post: &Post) -> usize {
        match &post.embed {
            Some(Embed::Images(images)) => images.len(),
            Some(Embed::QuoteWithMedia(_, skyfeed::MediaEmbed::Images(images))) => images.len(),
            _ => 0,
        }
    }

    async fn score_post(&self, post: &Post) -> Option<ScoreBreakdown> {
        let text = post.text.clone();

        if should_prefilter(&text, post.langs.first().map(|s| s.as_str())).is_some() {
            return None;
        }

        let (found_keywords, keyword_count) = has_keywords(&text);
        let (found_hashtags, hashtag_count) = has_hashtags(&text);
        if !found_keywords && !found_hashtags {
            return None;
        }

        let mut signals = ScoringSignals::new();
        signals.has_keywords = found_keywords;
        signals.keyword_count = keyword_count;
        signals.has_hashtags = found_hashtags;
        signals.hashtag_count = hashtag_count;
        signals.is_first_person = is_first_person(&text);
        signals.has_media = Self::has_media(post);
        signals.has_video = Self::has_video(post);
        signals.image_count = Self::image_count(post);
        let promo_breakdown = promo_penalty(&text);
        signals.promo_penalty = promo_breakdown.total_penalty;
        signals.promo_breakdown = promo_breakdown;

        let ml_scores = self.ml_handle.score(text.clone()).await;
        signals.semantic_score = ml_scores.semantic_score;
        signals.classification_score = ml_scores.classification_score;
        signals.classification_label = ml_scores.best_label.clone();
        signals.negative_rejection = ml_scores.negative_rejection;
        signals.is_negative_label = ml_scores.is_negative_label;
        signals.negative_label = ml_scores.best_label.clone();
        signals.negative_label_score = ml_scores.best_label_score;

        let breakdown = self.scorer.evaluate(&signals);
        let text_preview = crate::utils::truncate_text(&text, 300);

        if breakdown.passes() {
            log_accepted_post(text_preview, &breakdown, &ml_scores);
            Some(breakdown)
        } else {
            log_rejected_post(text_preview, &breakdown);
            None
        }
    }

    pub fn flush_pending(&mut self) -> Result<(), diesel::result::Error> {
        if self.pending_posts.is_empty()
            && self.pending_likes.is_empty()
            && self.pending_deletes.is_empty()
            && self.pending_like_deletes.is_empty()
        {
            return Ok(());
        }

        let mut conn = self.pool.get().expect("Failed to get connection");

        let deletes: Vec<_> = self.pending_deletes.drain(..).collect();
        let like_deletes: Vec<_> = self.pending_like_deletes.drain(..).collect();

        for uri in &deletes {
            db::delete_post(&mut conn, uri)?;
        }
        for uri in &like_deletes {
            db::delete_like(&mut conn, uri)?;
        }

        let posts_to_insert: Vec<_> = self.pending_posts.drain(..).collect();
        let likes_to_insert: Vec<_> = self
            .pending_likes
            .drain(..)
            .filter(|like| !deletes.contains(&like.post_uri))
            .collect();

        if !posts_to_insert.is_empty() {
            db::insert_posts(&mut conn, posts_to_insert)?;
        }
        if !likes_to_insert.is_empty() {
            db::insert_likes(&mut conn, likes_to_insert)?;
        }

        Ok(())
    }

    pub fn cleanup_old_posts(&self) -> Result<usize, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");
        let now = Utc::now().timestamp();
        let cutoff = now - (FEED_CUTOFF_HOURS * 3600);

        db::cleanup_old_posts(&mut conn, cutoff, MAX_STORED_POSTS)
    }
}

impl FeedHandler for GameDevFeedHandler {
    async fn available_feeds(&mut self) -> Vec<String> {
        vec!["Game Dev Progress".to_string()]
    }

    async fn insert_post(&mut self, post: Post) {
        if let Some(breakdown) = self.score_post(&post).await {
            let new_post = NewPost {
                uri: post.uri.0.clone(),
                text: post.text.clone(),
                timestamp: post.timestamp.timestamp(),
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
            };

            self.pending_posts.push(new_post);
        }
    }

    async fn delete_post(&mut self, uri: Uri) {
        self.pending_deletes.push(uri.0.clone());
    }

    async fn insert_like(&mut self, like_uri: Uri, liked_post_uri: Uri) {
        self.pending_likes.push(NewLike {
            post_uri: liked_post_uri.0.clone(),
            like_uri: like_uri.0.clone(),
        });
    }

    async fn delete_like(&mut self, like_uri: Uri) {
        self.pending_like_deletes.push(like_uri.0.clone());
    }

    async fn serve_feed(&self, request: FeedRequest) -> FeedResult {
        let now = Utc::now();
        let cutoff = now.timestamp() - (FEED_CUTOFF_HOURS * 3600);

        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                log_feed_error(&format!("Failed to get DB connection: {e}"));
                return FeedResult {
                    cursor: None,
                    feed: vec![],
                };
            }
        };

        let posts = match db::get_feed(&mut conn, cutoff) {
            Ok(p) => p,
            Err(e) => {
                log_feed_error(&format!("Failed to query feed: {e}"));
                return FeedResult {
                    cursor: None,
                    feed: vec![],
                };
            }
        };

        let start_index = request
            .cursor
            .as_ref()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);

        let limit = request
            .limit
            .map(|l| (l as usize).min(FEED_MAX_LIMIT))
            .unwrap_or(FEED_DEFAULT_LIMIT);

        let mut scored_posts: Vec<_> = posts
            .par_iter()
            .map(|p| {
                let post_time = chrono::DateTime::from_timestamp(p.timestamp, 0).unwrap_or(now);
                let decayed_score = apply_time_decay(p.priority, post_time, now);
                (p, decayed_score)
            })
            .collect();

        scored_posts.par_sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let page_posts: Vec<_> = scored_posts
            .into_iter()
            .skip(start_index)
            .take(limit)
            .collect();

        let total_count = posts.len();
        let next_cursor = if start_index + limit < total_count {
            Some((start_index + limit).to_string())
        } else {
            None
        };

        let feed: Vec<Uri> = page_posts.iter().map(|(p, _)| Uri(p.uri.clone())).collect();

        log_feed_served(feed.len(), total_count);

        FeedResult {
            cursor: next_cursor,
            feed,
        }
    }
}
