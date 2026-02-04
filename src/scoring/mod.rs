mod classification;
pub mod content;
pub mod filters;
pub mod priority;
mod relevance;
pub mod score;
mod semantic;

pub use classification::{label_boost, MLHandle, MLScores};
pub use content::{extract_content_signals, ContentSignals, MediaInfo};
pub use filters::{apply_filters, apply_ml_filter, Filter, FilterResult};
pub use priority::{
    calculate_priority, PriorityBreakdown, PrioritySignals, BONUS_FIRST_PERSON,
    BONUS_IMAGE_WITH_ALT, BONUS_VIDEO, MANY_IMAGES_THRESHOLD, PENALTY_MANY_IMAGES,
    PENALTY_PROMO_LINK,
};
pub use relevance::{has_hashtags, has_keywords};
pub use score::{calculate_score, ScoreBreakdown, SCORE_THRESHOLD};

use chrono::{DateTime, Utc};

pub struct EvaluationResult {
    pub passes: bool,
    pub score: Option<ScoreBreakdown>,
    pub priority: Option<PriorityBreakdown>,
}

impl EvaluationResult {
    pub fn passes(&self) -> bool {
        self.passes
    }
}

pub async fn evaluate_post(text: &str, media: MediaInfo, ml_handle: &MLHandle) -> EvaluationResult {
    let filter_result = apply_filters(text, Some("en"), None, |_| false);
    if let FilterResult::Reject(_) = &filter_result {
        return EvaluationResult {
            passes: false,
            score: None,
            priority: None,
        };
    }

    let (found_keywords, _) = has_keywords(text);
    let (found_hashtags, _) = has_hashtags(text);
    if !found_keywords && !found_hashtags {
        return EvaluationResult {
            passes: false,
            score: None,
            priority: None,
        };
    }

    let ml_scores = ml_handle.score(text.to_string()).await;

    let ml_filter_result = apply_ml_filter(
        &ml_scores.best_label,
        ml_scores.best_label_score,
        ml_scores.is_negative_label,
    );
    if let FilterResult::Reject(_) = ml_filter_result {
        return EvaluationResult {
            passes: false,
            score: None,
            priority: None,
        };
    }

    let content = extract_content_signals(text, &media);

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
    let passes = score.passes_threshold();

    EvaluationResult {
        passes,
        score: Some(score),
        priority: Some(priority),
    }
}

pub const DECAY_EVERY_X_HOURS: f32 = 2.0;
pub const DECAY_FACTOR: f32 = 0.75;

pub fn apply_time_decay(score: f32, post_time: DateTime<Utc>, now: DateTime<Utc>) -> f32 {
    let hours_old = (now - post_time).num_seconds() as f32 / 3600.0;
    let decay = DECAY_FACTOR.powf(hours_old / DECAY_EVERY_X_HOURS);
    score * decay
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_min_length() {
        let short_text = "hi";
        let result = apply_filters(short_text, Some("en"), None, |_| false);
        assert!(matches!(result, FilterResult::Reject(Filter::MinLength)));
    }

    #[test]
    fn test_filter_english_only() {
        let text = "This is a long enough text for testing";
        let result = apply_filters(text, Some("pt"), None, |_| false);
        assert!(matches!(result, FilterResult::Reject(Filter::EnglishOnly)));

        let result_en = apply_filters(text, Some("en"), None, |_| false);
        assert!(matches!(result_en, FilterResult::Pass));
    }

    #[test]
    fn test_filter_has_gamedev_signals() {
        let no_gamedev = "Just had coffee this morning, great day!";
        let (has_kw, _) = has_keywords(no_gamedev);
        let (has_ht, _) = has_hashtags(no_gamedev);
        assert!(!has_kw && !has_ht);
    }

    #[test]
    fn test_bonus_video() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let breakdown_no_video = calculate_priority(&score, &signals);

        signals.has_video = true;
        let breakdown_with_video = calculate_priority(&score, &signals);

        assert!(breakdown_with_video.final_priority > breakdown_no_video.final_priority);
        assert!(breakdown_with_video
            .boost_reasons
            .iter()
            .any(|r| r.contains("video")));
    }

    #[test]
    fn test_penalty_many_images() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            images: 2,
            ..Default::default()
        };

        let breakdown_few_images = calculate_priority(&score, &signals);

        signals.images = 3;
        let breakdown_many_images = calculate_priority(&score, &signals);

        assert!(breakdown_many_images.final_priority < breakdown_few_images.final_priority);
        assert!(breakdown_many_images
            .penalty_reasons
            .iter()
            .any(|r| r.contains("images")));
    }
}
