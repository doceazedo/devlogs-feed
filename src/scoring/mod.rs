mod authenticity;
mod classification;
mod relevance;
mod semantic;

pub use authenticity::{
    is_first_person, promo_penalty_detailed, PromoPenaltyBreakdown, BONUS_FIRST_PERSON,
    BONUS_MEDIA, BONUS_VIDEO, MANY_IMAGES_THRESHOLD, PENALTY_MANY_IMAGES,
};
pub use classification::{label_multiplier, MLHandle, MLScores, WEIGHT_CLASSIFICATION};
pub use relevance::{has_hashtags, has_keywords, strip_hashtags, WEIGHT_HASHTAG, WEIGHT_KEYWORD};
pub use semantic::{REFERENCE_POSTS, WEIGHT_SEMANTIC};

use chrono::{DateTime, Utc};
use strum::Display;

pub const MIN_TEXT_LENGTH: usize = 20;
pub const MIN_FINAL_SCORE: f32 = ConfidenceTier::MODERATE_THRESHOLD;
pub const DECAY_EVERY_X_HOURS: f32 = 24.0;
pub const DECAY_FACTOR: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum Filter {
    #[strum(serialize = "min-length")]
    MinLength,
    #[strum(serialize = "english-only")]
    EnglishOnly,
    #[strum(serialize = "has-gamedev-signals")]
    HasGamedevSignals,
    #[strum(serialize = "negative-classification")]
    NegativeClassification,
    #[strum(serialize = "min-score")]
    MinScore,
}

#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum Bonus {
    #[strum(serialize = "keyword")]
    Keyword,
    #[strum(serialize = "hashtag")]
    Hashtag,
    #[strum(serialize = "semantic")]
    Semantic,
    #[strum(serialize = "classification")]
    Classification,
    #[strum(serialize = "first-person")]
    FirstPerson,
    #[strum(serialize = "has-media")]
    HasMedia,
    #[strum(serialize = "has-video")]
    HasVideo,
    #[strum(serialize = "many-images")]
    ManyImages,
    #[strum(serialize = "promo-penalty")]
    PromoPenalty,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfidenceTier {
    Strong,
    High,
    Moderate,
    Low,
}

impl ConfidenceTier {
    pub const STRONG_THRESHOLD: f32 = 0.85;
    pub const HIGH_THRESHOLD: f32 = 0.70;
    pub const MODERATE_THRESHOLD: f32 = 0.60;

    pub fn from_score(score: f32) -> Self {
        if score >= Self::STRONG_THRESHOLD {
            ConfidenceTier::Strong
        } else if score >= Self::HIGH_THRESHOLD {
            ConfidenceTier::High
        } else if score >= Self::MODERATE_THRESHOLD {
            ConfidenceTier::Moderate
        } else {
            ConfidenceTier::Low
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConfidenceTier::Strong => "STRONG",
            ConfidenceTier::High => "HIGH",
            ConfidenceTier::Moderate => "MODERATE",
            ConfidenceTier::Low => "LOW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoringSignals {
    pub has_keywords: bool,
    pub keyword_count: usize,
    pub has_hashtags: bool,
    pub hashtag_count: usize,
    pub semantic_score: f32,
    pub classification_score: f32,
    pub classification_label: String,
    pub is_first_person: bool,
    pub has_media: bool,
    pub has_video: bool,
    pub image_count: usize,
    pub promo_penalty: f32,
    pub promo_breakdown: PromoPenaltyBreakdown,
    pub negative_rejection: bool,
    pub is_negative_label: bool,
    pub negative_label: String,
    pub negative_label_score: f32,
}

impl ScoringSignals {
    pub fn new() -> Self {
        Self {
            has_keywords: false,
            keyword_count: 0,
            has_hashtags: false,
            hashtag_count: 0,
            semantic_score: 0.0,
            classification_score: 0.0,
            classification_label: String::new(),
            is_first_person: false,
            has_media: false,
            has_video: false,
            image_count: 0,
            promo_penalty: 0.0,
            promo_breakdown: PromoPenaltyBreakdown::default(),
            negative_rejection: false,
            is_negative_label: false,
            negative_label: String::new(),
            negative_label_score: 0.0,
        }
    }
}

impl Default for ScoringSignals {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub relevance_score: f32,
    pub has_keywords: bool,
    pub keyword_count: usize,
    pub has_hashtags: bool,
    pub hashtag_count: usize,
    pub semantic_score: f32,
    pub classification_score: f32,
    pub classification_label: String,
    pub label_multiplier: f32,
    pub is_first_person: bool,
    pub has_media: bool,
    pub has_video: bool,
    pub image_count: usize,
    pub promo_penalty: f32,
    pub authenticity_modifier: f32,
    pub boost_reasons: Vec<String>,
    pub nerf_reasons: Vec<String>,
    pub base_score: f32,
    pub final_score: f32,
    pub priority: f32,
    pub confidence: ConfidenceTier,
    pub negative_rejection: bool,
    pub is_negative_label: bool,
    pub negative_label: String,
    pub negative_label_score: f32,
    pub rejection_filter: Option<Filter>,
}

impl ScoreBreakdown {
    pub fn compute(signals: &ScoringSignals) -> Self {
        let mut boost_reasons = Vec::new();
        let mut nerf_reasons = Vec::new();

        let keyword_weight = if signals.has_keywords { 1.0 } else { 0.0 };
        let hashtag_weight = if signals.has_hashtags { 1.0 } else { 0.0 };

        let relevance = (keyword_weight * WEIGHT_KEYWORD
            + hashtag_weight * WEIGHT_HASHTAG
            + signals.semantic_score * WEIGHT_SEMANTIC
            + signals.classification_score * WEIGHT_CLASSIFICATION)
            .min(1.0);

        let mut authenticity_modifier = 0.0;

        if signals.is_first_person {
            authenticity_modifier += BONUS_FIRST_PERSON;
            boost_reasons.push(format!(
                "first person (+{:.0}%)",
                BONUS_FIRST_PERSON * 100.0
            ));
        }

        if signals.has_media {
            authenticity_modifier += BONUS_MEDIA;
            boost_reasons.push(format!("has media (+{:.0}%)", BONUS_MEDIA * 100.0));
        }

        if signals.has_video {
            authenticity_modifier += BONUS_VIDEO;
            boost_reasons.push(format!("has video (+{:.0}%)", BONUS_VIDEO * 100.0));
        }

        if signals.image_count >= MANY_IMAGES_THRESHOLD {
            authenticity_modifier -= PENALTY_MANY_IMAGES;
            nerf_reasons.push(format!(
                "{}+ images (-{:.0}%)",
                MANY_IMAGES_THRESHOLD,
                PENALTY_MANY_IMAGES * 100.0
            ));
        }

        if signals.promo_penalty > 0.0 {
            let penalty = signals.promo_penalty * 0.5;
            authenticity_modifier -= penalty;
            nerf_reasons.push(format!("promo keywords (-{:.0}%)", penalty * 100.0));
        }

        let base_score = (relevance * 0.75 + (0.5 + authenticity_modifier) * 0.25).clamp(0.0, 1.0);

        let multiplier = label_multiplier(&signals.classification_label);
        if multiplier > 1.0 {
            boost_reasons.push(format!(
                "{} (x{:.1})",
                signals.classification_label, multiplier
            ));
        } else if multiplier < 1.0 {
            nerf_reasons.push(format!(
                "{} (x{:.1})",
                signals.classification_label, multiplier
            ));
        }

        let priority = relevance + authenticity_modifier + (multiplier - 1.0);
        let final_score = priority.min(1.0);
        let confidence = ConfidenceTier::from_score(final_score);

        ScoreBreakdown {
            relevance_score: relevance,
            has_keywords: signals.has_keywords,
            keyword_count: signals.keyword_count,
            has_hashtags: signals.has_hashtags,
            hashtag_count: signals.hashtag_count,
            semantic_score: signals.semantic_score,
            classification_score: signals.classification_score,
            classification_label: signals.classification_label.clone(),
            label_multiplier: multiplier,
            is_first_person: signals.is_first_person,
            has_media: signals.has_media,
            has_video: signals.has_video,
            image_count: signals.image_count,
            promo_penalty: signals.promo_penalty,
            authenticity_modifier,
            boost_reasons,
            nerf_reasons,
            base_score,
            final_score,
            priority,
            confidence,
            negative_rejection: signals.negative_rejection,
            is_negative_label: signals.is_negative_label,
            negative_label: signals.negative_label.clone(),
            negative_label_score: signals.negative_label_score,
            rejection_filter: None,
        }
    }

    pub fn passes(&self) -> bool {
        if self.negative_rejection {
            return false;
        }
        self.final_score >= MIN_FINAL_SCORE
    }

    pub fn rejection_reason(&self) -> Option<Filter> {
        if self.negative_rejection {
            Some(Filter::NegativeClassification)
        } else if self.final_score < MIN_FINAL_SCORE {
            Some(Filter::MinScore)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct PostScorer {
    pub min_score: f32,
}

impl Default for PostScorer {
    fn default() -> Self {
        Self {
            min_score: MIN_FINAL_SCORE,
        }
    }
}

impl PostScorer {
    pub fn evaluate(&self, signals: &ScoringSignals) -> ScoreBreakdown {
        ScoreBreakdown::compute(signals)
    }
}

pub fn should_prefilter(text: &str, lang: Option<&str>) -> Option<Filter> {
    if strip_hashtags(text).len() < MIN_TEXT_LENGTH {
        return Some(Filter::MinLength);
    }
    if let Some(lang) = lang {
        if !lang.starts_with("en") {
            return Some(Filter::EnglishOnly);
        }
    }
    None
}

pub fn apply_time_decay(score: f32, post_time: DateTime<Utc>, now: DateTime<Utc>) -> f32 {
    let hours_old = (now - post_time).num_seconds() as f32 / 3600.0;
    let decay = DECAY_FACTOR.powf(hours_old / DECAY_EVERY_X_HOURS);
    score * decay
}

#[derive(Debug)]
pub enum EvaluationResult {
    Prefiltered(Filter),
    Scored(ScoreBreakdown),
}

impl EvaluationResult {
    pub fn passes(&self) -> bool {
        match self {
            EvaluationResult::Prefiltered(_) => false,
            EvaluationResult::Scored(breakdown) => breakdown.passes(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MediaInfo {
    pub has_media: bool,
    pub has_video: bool,
    pub image_count: usize,
}

pub async fn evaluate_post(text: &str, media: MediaInfo, ml_handle: &MLHandle) -> EvaluationResult {
    if let Some(filter) = should_prefilter(text, Some("en")) {
        return EvaluationResult::Prefiltered(filter);
    }

    let (found_keywords, keyword_count) = has_keywords(text);
    let (found_hashtags, hashtag_count) = has_hashtags(text);

    if !found_keywords && !found_hashtags {
        return EvaluationResult::Prefiltered(Filter::HasGamedevSignals);
    }

    let scores = ml_handle.score(text.to_string()).await;

    let mut signals = ScoringSignals::new();
    signals.has_keywords = found_keywords;
    signals.keyword_count = keyword_count;
    signals.has_hashtags = found_hashtags;
    signals.hashtag_count = hashtag_count;
    signals.semantic_score = scores.semantic_score;
    signals.classification_score = scores.classification_score;
    signals.classification_label = scores.best_label.clone();
    signals.is_first_person = is_first_person(text);
    signals.has_media = media.has_media;
    signals.has_video = media.has_video;
    signals.image_count = media.image_count;
    let promo = promo_penalty_detailed(text);
    signals.promo_penalty = promo.total_penalty;
    signals.promo_breakdown = promo;
    signals.negative_rejection = scores.negative_rejection;
    signals.is_negative_label = scores.is_negative_label;
    signals.negative_label = scores.best_label.clone();
    signals.negative_label_score = scores.best_label_score;

    let scorer = PostScorer::default();
    EvaluationResult::Scored(scorer.evaluate(&signals))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_min_length() {
        let short_text = "hi";
        assert_eq!(
            should_prefilter(short_text, Some("en")),
            Some(Filter::MinLength)
        );
        assert!(short_text.len() < MIN_TEXT_LENGTH);
    }

    #[test]
    fn test_filter_english_only() {
        let text = "This is a long enough text for testing";
        assert_eq!(
            should_prefilter(text, Some("pt")),
            Some(Filter::EnglishOnly)
        );
        assert_eq!(should_prefilter(text, Some("en")), None);
    }

    #[test]
    fn test_filter_has_gamedev_signals() {
        let no_gamedev = "Just had coffee this morning, great day!";
        let (has_kw, _) = has_keywords(no_gamedev);
        let (has_ht, _) = has_hashtags(no_gamedev);
        assert!(!has_kw && !has_ht);
    }

    #[test]
    fn test_filter_negative_classification() {
        let mut signals = ScoringSignals::new();
        signals.negative_rejection = true;
        signals.classification_score = 0.8;
        signals.classification_label = "game development progress or devlog".to_string();
        signals.has_keywords = true;
        signals.semantic_score = 0.8;

        let breakdown = ScoreBreakdown::compute(&signals);
        assert!(!breakdown.passes());
        assert_eq!(
            breakdown.rejection_reason(),
            Some(Filter::NegativeClassification)
        );
    }

    #[test]
    fn test_filter_min_score() {
        let mut signals = ScoringSignals::new();
        signals.classification_score = 0.1;
        signals.semantic_score = 0.1;

        let breakdown = ScoreBreakdown::compute(&signals);
        assert!(!breakdown.passes());
        assert!(breakdown.final_score < MIN_FINAL_SCORE);
    }

    #[test]
    fn test_bonus_video() {
        let mut signals = ScoringSignals::new();
        signals.has_keywords = true;
        signals.semantic_score = 0.5;
        signals.classification_score = 0.5;
        signals.classification_label = "other".to_string();

        let breakdown_no_video = ScoreBreakdown::compute(&signals);

        signals.has_video = true;
        let breakdown_with_video = ScoreBreakdown::compute(&signals);

        assert!(
            breakdown_with_video.authenticity_modifier > breakdown_no_video.authenticity_modifier
        );
        assert!(breakdown_with_video
            .boost_reasons
            .iter()
            .any(|r| r.contains("video")));
    }

    #[test]
    fn test_penalty_many_images() {
        let mut signals = ScoringSignals::new();
        signals.has_keywords = true;
        signals.semantic_score = 0.5;
        signals.classification_score = 0.5;
        signals.classification_label = "other".to_string();
        signals.has_media = true;
        signals.image_count = 2;

        let breakdown_few_images = ScoreBreakdown::compute(&signals);

        signals.image_count = 3;
        let breakdown_many_images = ScoreBreakdown::compute(&signals);

        assert!(
            breakdown_many_images.authenticity_modifier
                < breakdown_few_images.authenticity_modifier
        );
        assert!(breakdown_many_images
            .nerf_reasons
            .iter()
            .any(|r| r.contains("images")));
    }
}
