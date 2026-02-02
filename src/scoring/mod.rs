mod classification;
pub mod content;
pub mod filters;
pub mod priority;
mod relevance;
mod semantic;

pub use classification::{
    label_multiplier, ClassificationLabel, MLHandle, MLScores, QualityAssessment,
    TopicClassification, TopicLabel, WEIGHT_CLASSIFICATION,
};
pub use content::{
    count_links, detect_first_person, extract_content_signals, is_first_person, ContentSignals,
    MediaInfo,
};
pub use filters::{apply_filters, apply_ml_filter, Filter, FilterResult, MIN_TEXT_LENGTH};
pub use priority::{
    calculate_priority, passes_threshold, ConfidenceTier, PriorityBreakdown, PrioritySignals,
};
pub use relevance::{
    has_hashtags, has_keywords, strip_hashtags, GAMEDEV_HASHTAGS, GAMEDEV_KEYWORDS,
};
pub use semantic::REFERENCE_POSTS;

use chrono::{DateTime, Utc};

pub const DECAY_EVERY_X_HOURS: f32 = 24.0;
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
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let breakdown_no_video = calculate_priority(&signals);

        signals.has_video = true;
        let breakdown_with_video = calculate_priority(&signals);

        assert!(breakdown_with_video.final_priority > breakdown_no_video.final_priority);
        assert!(breakdown_with_video
            .boost_reasons
            .iter()
            .any(|r| r.contains("video")));
    }

    #[test]
    fn test_penalty_many_images() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            images: 2,
            ..Default::default()
        };

        let breakdown_few_images = calculate_priority(&signals);

        signals.images = 3;
        let breakdown_many_images = calculate_priority(&signals);

        assert!(breakdown_many_images.final_priority < breakdown_few_images.final_priority);
        assert!(breakdown_many_images
            .penalty_reasons
            .iter()
            .any(|r| r.contains("images")));
    }
}
