use strum::Display;

pub const WEIGHT_TOPIC_CLASSIFICATION: f32 = 0.35;
pub const WEIGHT_SEMANTIC: f32 = 0.20;

pub const WEIGHT_LOW_EFFORT: f32 = 0.15;
pub const WEIGHT_ENGAGEMENT_BAIT: f32 = 0.10;

pub const BONUS_FIRST_PERSON: f32 = 0.10;
pub const BONUS_VIDEO: f32 = 0.10;
pub const BONUS_IMAGE_WITH_ALT: f32 = 0.05;
pub const PENALTY_MANY_IMAGES: f32 = 0.10;
pub const PENALTY_PER_LINK: f32 = 0.03;
pub const PENALTY_PROMO_LINK: f32 = 0.08;

pub const MANY_IMAGES_THRESHOLD: u8 = 3;

pub const REPLY_WEIGHT: f32 = 3.0;
pub const REPOST_WEIGHT: f32 = 2.0;
pub const LIKE_WEIGHT: f32 = 1.0;
pub const VELOCITY_SCALE: f32 = 0.1;
pub const MAX_ENGAGEMENT_BOOST: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Display)]
pub enum ConfidenceTier {
    #[strum(serialize = "STRONG")]
    Strong,
    #[strum(serialize = "HIGH")]
    High,
    #[strum(serialize = "MODERATE")]
    Moderate,
    #[strum(serialize = "LOW")]
    Low,
}

impl ConfidenceTier {
    pub const STRONG_THRESHOLD: f32 = 0.85;
    pub const HIGH_THRESHOLD: f32 = 0.70;
    pub const MODERATE_THRESHOLD: f32 = 0.50;

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
}

#[derive(Debug, Clone, Default)]
pub struct PrioritySignals {
    pub topic_classification_score: f32,
    pub semantic_score: f32,
    pub topic_label: String,
    pub label_multiplier: f32,

    pub engagement_bait_score: f32,
    pub synthetic_score: f32,

    pub is_first_person: bool,
    pub images: u8,
    pub has_video: bool,
    pub has_alt_text: bool,
    pub link_count: u8,
    pub promo_link_count: u8,

    pub engagement_velocity: f32,
    pub reply_count: i32,
    pub repost_count: i32,
    pub like_count: i32,
}

#[derive(Debug, Clone)]
pub struct PriorityBreakdown {
    pub topic_score: f32,
    pub quality_penalty: f32,
    pub content_modifier: f32,
    pub engagement_boost: f32,
    pub label_boost: f32,
    pub final_priority: f32,
    pub confidence: ConfidenceTier,
    pub topic_label: String,
    pub boost_reasons: Vec<String>,
    pub penalty_reasons: Vec<String>,
}

impl Default for PriorityBreakdown {
    fn default() -> Self {
        Self {
            topic_score: 0.0,
            quality_penalty: 0.0,
            content_modifier: 0.0,
            engagement_boost: 0.0,
            label_boost: 0.0,
            final_priority: 0.0,
            confidence: ConfidenceTier::Low,
            topic_label: String::new(),
            boost_reasons: Vec::new(),
            penalty_reasons: Vec::new(),
        }
    }
}

pub fn calculate_priority(signals: &PrioritySignals) -> PriorityBreakdown {
    let mut boosts = Vec::new();
    let mut penalties = Vec::new();

    let topic_score = signals.topic_classification_score * WEIGHT_TOPIC_CLASSIFICATION
        + signals.semantic_score * WEIGHT_SEMANTIC;

    let mut quality_penalty = 0.0;

    if signals.engagement_bait_score > 0.5 {
        let penalty = signals.engagement_bait_score * WEIGHT_ENGAGEMENT_BAIT;
        quality_penalty += penalty;
        penalties.push(format!("engagement-bait (-{:.0}%)", penalty * 100.0));
    }

    if signals.synthetic_score > 0.5 {
        let penalty = signals.synthetic_score * WEIGHT_LOW_EFFORT;
        quality_penalty += penalty;
        penalties.push(format!("synthetic (-{:.0}%)", penalty * 100.0));
    }

    let mut content_modifier = 0.0;

    if signals.is_first_person {
        content_modifier += BONUS_FIRST_PERSON;
        boosts.push(format!(
            "first-person (+{:.0}%)",
            BONUS_FIRST_PERSON * 100.0
        ));
    }

    if signals.has_video {
        content_modifier += BONUS_VIDEO;
        boosts.push(format!("video (+{:.0}%)", BONUS_VIDEO * 100.0));
    }

    if signals.images > 0 && signals.has_alt_text {
        content_modifier += BONUS_IMAGE_WITH_ALT;
        boosts.push(format!(
            "image-with-alt (+{:.0}%)",
            BONUS_IMAGE_WITH_ALT * 100.0
        ));
    }

    if signals.images >= MANY_IMAGES_THRESHOLD {
        content_modifier -= PENALTY_MANY_IMAGES;
        penalties.push(format!(
            "{}+ images (-{:.0}%)",
            signals.images,
            PENALTY_MANY_IMAGES * 100.0
        ));
    }

    if signals.link_count > 0 {
        let link_penalty = signals.link_count as f32 * PENALTY_PER_LINK;
        content_modifier -= link_penalty;
        penalties.push(format!(
            "{} links (-{:.0}%)",
            signals.link_count,
            link_penalty * 100.0
        ));
    }

    if signals.promo_link_count > 0 {
        let promo_penalty = signals.promo_link_count as f32 * PENALTY_PROMO_LINK;
        content_modifier -= promo_penalty;
        penalties.push(format!(
            "{} promo-links (-{:.0}%)",
            signals.promo_link_count,
            promo_penalty * 100.0
        ));
    }

    let engagement_boost = calculate_engagement_boost(signals);
    if engagement_boost > 0.05 {
        boosts.push(format!("trending (+{:.0}%)", engagement_boost * 100.0));
    }

    let label_boost = signals.label_multiplier - 1.0;
    if label_boost > 0.0 {
        boosts.push(format!(
            "{} (+{:.0}%)",
            signals.topic_label,
            label_boost * 100.0
        ));
    } else if label_boost < 0.0 {
        penalties.push(format!(
            "{} ({:.0}%)",
            signals.topic_label,
            label_boost * 100.0
        ));
    }

    let final_priority = (topic_score + content_modifier + engagement_boost + label_boost
        - quality_penalty)
        .clamp(0.0, 2.0);

    let confidence = ConfidenceTier::from_score(topic_score + content_modifier);

    PriorityBreakdown {
        topic_score,
        quality_penalty,
        content_modifier,
        engagement_boost,
        label_boost,
        final_priority,
        confidence,
        topic_label: signals.topic_label.clone(),
        boost_reasons: boosts,
        penalty_reasons: penalties,
    }
}

fn calculate_engagement_boost(signals: &PrioritySignals) -> f32 {
    if signals.engagement_velocity > 0.0 {
        (signals.engagement_velocity.ln_1p() * VELOCITY_SCALE).min(MAX_ENGAGEMENT_BOOST)
    } else {
        let weighted = signals.reply_count as f32 * REPLY_WEIGHT
            + signals.repost_count as f32 * REPOST_WEIGHT
            + signals.like_count as f32 * LIKE_WEIGHT;
        (weighted.ln_1p() * VELOCITY_SCALE).min(MAX_ENGAGEMENT_BOOST)
    }
}

pub fn passes_threshold(breakdown: &PriorityBreakdown) -> bool {
    breakdown.final_priority >= ConfidenceTier::MODERATE_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_tiers() {
        assert_eq!(ConfidenceTier::from_score(0.90), ConfidenceTier::Strong);
        assert_eq!(ConfidenceTier::from_score(0.75), ConfidenceTier::High);
        assert_eq!(ConfidenceTier::from_score(0.55), ConfidenceTier::Moderate);
        assert_eq!(ConfidenceTier::from_score(0.30), ConfidenceTier::Low);
    }

    #[test]
    fn test_basic_priority() {
        let signals = PrioritySignals {
            topic_classification_score: 0.8,
            semantic_score: 0.7,
            topic_label: "game developer sharing work".to_string(),
            label_multiplier: 1.0,
            ..Default::default()
        };

        let breakdown = calculate_priority(&signals);
        assert!(breakdown.topic_score > 0.0);
        assert_eq!(breakdown.quality_penalty, 0.0);
    }

    #[test]
    fn test_first_person_boost() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let without = calculate_priority(&signals);

        signals.is_first_person = true;
        let with = calculate_priority(&signals);

        assert!(with.final_priority > without.final_priority);
        assert!(with
            .boost_reasons
            .iter()
            .any(|r| r.contains("first-person")));
    }

    #[test]
    fn test_video_boost() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let without = calculate_priority(&signals);

        signals.has_video = true;
        let with = calculate_priority(&signals);

        assert!(with.final_priority > without.final_priority);
        assert!(with.boost_reasons.iter().any(|r| r.contains("video")));
    }

    #[test]
    fn test_image_with_alt_boost() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            images: 1,
            ..Default::default()
        };

        let without_alt = calculate_priority(&signals);

        signals.has_alt_text = true;
        let with_alt = calculate_priority(&signals);

        assert!(with_alt.final_priority > without_alt.final_priority);
        assert!(with_alt
            .boost_reasons
            .iter()
            .any(|r| r.contains("image-with-alt")));
    }

    #[test]
    fn test_many_images_penalty() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            images: 2,
            ..Default::default()
        };

        let few = calculate_priority(&signals);

        signals.images = 3;
        let many = calculate_priority(&signals);

        assert!(many.final_priority < few.final_priority);
        assert!(many.penalty_reasons.iter().any(|r| r.contains("images")));
    }

    #[test]
    fn test_link_penalties() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let no_links = calculate_priority(&signals);

        signals.link_count = 2;
        let with_links = calculate_priority(&signals);

        assert!(with_links.final_priority < no_links.final_priority);

        signals.promo_link_count = 1;
        let with_promo = calculate_priority(&signals);

        assert!(with_promo.final_priority < with_links.final_priority);
    }

    #[test]
    fn test_quality_penalties() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.8,
            semantic_score: 0.7,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let good = calculate_priority(&signals);

        signals.synthetic_score = 0.8;
        let low_effort = calculate_priority(&signals);

        assert!(low_effort.final_priority < good.final_priority);
        assert!(low_effort.quality_penalty > 0.0);

        signals.synthetic_score = 0.0;
        signals.engagement_bait_score = 0.8;
        let bait = calculate_priority(&signals);

        assert!(bait.final_priority < good.final_priority);
    }

    #[test]
    fn test_engagement_boost() {
        let mut signals = PrioritySignals {
            topic_classification_score: 0.5,
            semantic_score: 0.5,
            label_multiplier: 1.0,
            ..Default::default()
        };

        let no_engagement = calculate_priority(&signals);

        signals.reply_count = 10;
        signals.repost_count = 5;
        signals.like_count = 20;
        let with_engagement = calculate_priority(&signals);

        assert!(with_engagement.engagement_boost > no_engagement.engagement_boost);
    }

    #[test]
    fn test_priority_clamped() {
        let signals = PrioritySignals {
            topic_classification_score: 1.0,
            semantic_score: 1.0,
            label_multiplier: 2.0,
            is_first_person: true,
            has_video: true,
            engagement_velocity: 100.0,
            ..Default::default()
        };

        let breakdown = calculate_priority(&signals);
        assert!(breakdown.final_priority <= 2.0);
    }
}
