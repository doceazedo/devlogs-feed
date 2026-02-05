mod classification;
pub mod content;
pub mod filters;
pub mod priority;
mod relevance;

pub use classification::{MLHandle, QualityAssessment};
pub use content::{extract_content_signals, ContentSignals, MediaInfo};
pub use filters::{apply_filters, Filter, FilterResult};
pub use priority::{calculate_priority, PriorityBreakdown, PrioritySignals};
pub use relevance::{has_hashtags, has_keywords};

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
        let mut signals = PrioritySignals::default();

        let breakdown_no_video = calculate_priority(&signals);

        signals.has_video = true;
        let breakdown_with_video = calculate_priority(&signals);

        assert!(breakdown_with_video.priority > breakdown_no_video.priority);
        assert!(breakdown_with_video
            .boost_reasons
            .iter()
            .any(|r| r.contains("video")));
    }

    #[test]
    fn test_penalty_many_images() {
        let mut signals = PrioritySignals {
            images: 2,
            ..Default::default()
        };

        let breakdown_few_images = calculate_priority(&signals);

        signals.images = 3;
        let breakdown_many_images = calculate_priority(&signals);

        assert!(breakdown_many_images.priority < breakdown_few_images.priority);
        assert!(breakdown_many_images
            .penalty_reasons
            .iter()
            .any(|r| r.contains("images")));
    }
}
