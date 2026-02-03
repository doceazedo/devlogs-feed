use crate::db::{self, get_post_author, DbPool, NewLike, NewPost};
use crate::engagement::EngagementTracker;
use crate::scoring::{
    apply_filters, apply_ml_filter, apply_time_decay, calculate_priority, calculate_score,
    extract_content_signals, has_hashtags, has_keywords, label_boost, FilterResult, MLHandle,
    MediaInfo, PrioritySignals,
};
use crate::utils::logs::{self, PostAssessment};
use chrono::Utc;
use rand::Rng;
use skyfeed::{Embed, FeedHandler, FeedRequest, FeedResult, Post, Uri};
use std::cmp::Ordering;

pub const FEED_CUTOFF_HOURS: i64 = 96;
pub const FEED_DEFAULT_LIMIT: usize = 50;
pub const FEED_MAX_LIMIT: usize = 500;
pub const MAX_STORED_POSTS: i64 = 5000;
pub const SHUFFLE_VARIANCE: f32 = 0.05;

#[derive(Clone)]
pub struct GameDevFeedHandler {
    pool: DbPool,
    ml_handle: MLHandle,
    engagement: EngagementTracker,
    pending_posts: Vec<NewPost>,
    pending_likes: Vec<NewLike>,
    pending_deletes: Vec<String>,
    pending_like_deletes: Vec<String>,
}

impl GameDevFeedHandler {
    pub fn new(pool: DbPool, ml_handle: MLHandle) -> Self {
        let engagement = EngagementTracker::new(pool.clone());
        Self {
            pool,
            ml_handle,
            engagement,
            pending_posts: Vec::new(),
            pending_likes: Vec::new(),
            pending_deletes: Vec::new(),
            pending_like_deletes: Vec::new(),
        }
    }

    fn extract_media_info(post: &Post) -> MediaInfo {
        match &post.embed {
            Some(Embed::Images(images)) => MediaInfo {
                image_count: images.len().min(255) as u8,
                has_video: false,
                has_alt_text: images.iter().any(|img| !img.alt_text.is_empty()),
                external_uri: None,
                facet_links: Vec::new(),
            },
            Some(Embed::Video(_)) => MediaInfo {
                image_count: 0,
                has_video: true,
                has_alt_text: false,
                external_uri: None,
                facet_links: Vec::new(),
            },
            Some(Embed::External(external)) => MediaInfo {
                image_count: 0,
                has_video: false,
                has_alt_text: false,
                external_uri: Some(external.uri.clone()),
                facet_links: Vec::new(),
            },
            Some(Embed::QuoteWithMedia(_, skyfeed::MediaEmbed::Images(images))) => MediaInfo {
                image_count: images.len().min(255) as u8,
                has_video: false,
                has_alt_text: images.iter().any(|img| !img.alt_text.is_empty()),
                external_uri: None,
                facet_links: Vec::new(),
            },
            Some(Embed::QuoteWithMedia(_, skyfeed::MediaEmbed::Video(_))) => MediaInfo {
                image_count: 0,
                has_video: true,
                has_alt_text: false,
                external_uri: None,
                facet_links: Vec::new(),
            },
            Some(Embed::QuoteWithMedia(_, skyfeed::MediaEmbed::External(external))) => MediaInfo {
                image_count: 0,
                has_video: false,
                has_alt_text: false,
                external_uri: Some(external.uri.clone()),
                facet_links: Vec::new(),
            },
            _ => MediaInfo::default(),
        }
    }

    fn is_spammer(&self, did: &str) -> bool {
        self.engagement.is_spammer(did)
    }

    fn track_reply_engagement(&self, parent_uri: &str, reply_uri: &str, reply_author: &str) {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        if let Some(parent_author) = get_post_author(&mut conn, parent_uri) {
            self.engagement
                .record_reply(parent_uri, reply_uri, reply_author, &parent_author)
                .ok();
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

        let post_count = posts_to_insert.len();
        let like_count = likes_to_insert.len();

        if !posts_to_insert.is_empty() {
            db::insert_posts(&mut conn, posts_to_insert)?;
        }
        if !likes_to_insert.is_empty() {
            db::insert_likes(&mut conn, likes_to_insert)?;
        }

        logs::log_flush(post_count, like_count);

        Ok(())
    }

    pub fn cleanup_old_posts(&self) -> Result<usize, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");
        let now = Utc::now().timestamp();
        let cutoff = now - (FEED_CUTOFF_HOURS * 3600);

        let engagement_deleted = self.engagement.cleanup_old_engagement(cutoff).unwrap_or(0);
        let posts_deleted = db::cleanup_old_posts(&mut conn, cutoff, MAX_STORED_POSTS)?;

        let total_deleted = engagement_deleted + posts_deleted;
        logs::log_cleanup(total_deleted);

        Ok(total_deleted)
    }

    #[allow(dead_code)]
    pub fn engagement_tracker(&self) -> &EngagementTracker {
        &self.engagement
    }
}

impl FeedHandler for GameDevFeedHandler {
    async fn available_feeds(&mut self) -> Vec<String> {
        vec!["Game Dev Progress".to_string()]
    }

    async fn insert_post(&mut self, post: Post) {
        let text = &post.text;
        let lang = post.langs.first().map(|s| s.as_str());
        let author_did = post.author_did.0.as_str();

        if let Some(reply) = &post.reply {
            self.track_reply_engagement(&reply.parent.0, &post.uri.0, author_did);
        }

        let mut assessment = PostAssessment::new(text);

        let filter_result = apply_filters(text, lang, Some(author_did), |did| self.is_spammer(did));
        assessment.set_filter_result(filter_result.clone());

        if let FilterResult::Reject(_) = filter_result {
            return;
        }

        let (found_keywords, _keyword_count) = has_keywords(text);
        let (found_hashtags, _hashtag_count) = has_hashtags(text);
        assessment.set_relevance(found_keywords, found_hashtags);

        if !found_keywords && !found_hashtags {
            return;
        }

        let ml_scores = self.ml_handle.score(text.clone()).await;
        assessment.set_ml_scores(ml_scores.clone());

        let ml_filter_result = apply_ml_filter(
            &ml_scores.best_label,
            ml_scores.best_label_score,
            ml_scores.is_negative_label,
        );

        if let FilterResult::Reject(_) = ml_filter_result {
            return;
        }

        let media_info = Self::extract_media_info(&post);
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

        if passed {
            let new_post = NewPost {
                uri: post.uri.0.clone(),
                text: text.clone(),
                timestamp: post.timestamp.timestamp(),
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
                author_did: Some(author_did.to_string()),
                image_count: content.images as i32,
                has_alt_text: if content.has_alt_text { 1 } else { 0 },
                link_count: content.link_count as i32,
                promo_link_count: content.promo_link_count as i32,
            };

            self.pending_posts.push(new_post);
        }
    }

    async fn delete_post(&mut self, uri: Uri) {
        self.pending_deletes.push(uri.0.clone());
    }

    async fn insert_like(&mut self, like_uri: Uri, liked_post_uri: Uri) {
        self.engagement.record_like(&liked_post_uri.0).ok();
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
            Err(_) => {
                return FeedResult {
                    cursor: None,
                    feed: vec![],
                };
            }
        };

        let posts = match db::get_feed(&mut conn, cutoff) {
            Ok(p) => p,
            Err(_) => {
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

        let mut rng = rand::rng();

        let mut scored_posts: Vec<_> = posts
            .iter()
            .map(|p| {
                let post_time = chrono::DateTime::from_timestamp(p.timestamp, 0).unwrap_or(now);
                let base_score = apply_time_decay(p.priority, post_time, now);

                // let velocity = self.engagement.get_velocity(&p.uri);
                // let engagement_boost = (velocity.ln_1p() * 0.1).min(0.5);

                let variance = rng.random_range(-SHUFFLE_VARIANCE..SHUFFLE_VARIANCE);
                let final_score = base_score * (1.0 + variance);

                (p, final_score)
            })
            .collect();

        scored_posts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

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

        logs::log_feed_served(feed.len(), request.cursor.as_ref());

        FeedResult {
            cursor: next_cursor,
            feed,
        }
    }
}
