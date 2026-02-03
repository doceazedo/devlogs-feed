use super::score::ScoreBreakdown;
use crate::utils::logs::{dim, format_signed, pad_label};
use strum::Display;

pub const POOR_QUALITY_PENALTY_MIN: f32 = 0.5;
pub const GOOD_QUALITY_BOOST_MIN: f32 = 0.1;
pub const ENGAGEMENT_BOOST_MIN: f32 = 0.05;

pub const BONUS_FIRST_PERSON: f32 = 0.4;
pub const BONUS_VIDEO: f32 = 0.2;
pub const BONUS_IMAGE_WITH_ALT: f32 = 0.2;
pub const PENALTY_MANY_IMAGES: f32 = 0.6;
pub const PENALTY_LINK_EXPO: f32 = 3.0;
pub const PENALTY_PROMO_LINK: f32 = 1.5;

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
    pub topic_label: String,
    pub label_boost: f32,

    pub engagement_bait_score: f32,
    pub synthetic_score: f32,
    pub authenticity_score: f32,

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
    pub quality_penalty: f32,
    pub content_modifier: f32,
    pub engagement_boost: f32,
    pub authenticity_boost: f32,
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
            quality_penalty: 0.0,
            content_modifier: 0.0,
            engagement_boost: 0.0,
            authenticity_boost: 0.0,
            label_boost: 0.0,
            final_priority: 0.0,
            confidence: ConfidenceTier::Low,
            topic_label: String::new(),
            boost_reasons: Vec::new(),
            penalty_reasons: Vec::new(),
        }
    }
}

pub fn calculate_priority(score: &ScoreBreakdown, signals: &PrioritySignals) -> PriorityBreakdown {
    let mut boosts = Vec::new();
    let mut penalties = Vec::new();

    let mut quality_penalty = 0.0;

    if signals.engagement_bait_score >= POOR_QUALITY_PENALTY_MIN {
        quality_penalty += signals.engagement_bait_score;
        penalties.push(format!(
            "{}{}",
            pad_label("engagement-bait:", 2),
            format_signed(signals.engagement_bait_score * -1.0)
        ));
    }

    if signals.synthetic_score >= POOR_QUALITY_PENALTY_MIN {
        quality_penalty += signals.synthetic_score;
        penalties.push(format!(
            "{}{}",
            pad_label("synthetic:", 2),
            format_signed(signals.synthetic_score * -1.0)
        ));
    }

    let mut content_modifier = 0.0;

    if signals.is_first_person {
        content_modifier += BONUS_FIRST_PERSON;
        boosts.push(format!(
            "{}{}",
            pad_label("first-person:", 2),
            format_signed(BONUS_FIRST_PERSON),
        ));
    }

    if signals.has_video {
        content_modifier += BONUS_VIDEO;
        boosts.push(format!(
            "{}{}",
            pad_label("video:", 2),
            format_signed(BONUS_VIDEO),
        ));
    }

    if signals.images > 0 && signals.has_alt_text {
        content_modifier += BONUS_IMAGE_WITH_ALT;
        boosts.push(format!(
            "{}{}",
            pad_label("alt-text:", 2),
            format_signed(BONUS_IMAGE_WITH_ALT),
        ));
    }

    if signals.images >= MANY_IMAGES_THRESHOLD {
        content_modifier -= PENALTY_MANY_IMAGES;
        penalties.push(format!(
            "{}{} {}",
            pad_label("images:", 2),
            format_signed(PENALTY_MANY_IMAGES),
            dim().apply_to(format!("({})", signals.images))
        ));
    }

    if signals.link_count > 0 {
        let link_penalty = PENALTY_LINK_EXPO.powi(signals.link_count as i32);
        content_modifier -= link_penalty;
        penalties.push(format!(
            "{}{} {}",
            pad_label("links:", 2),
            format_signed(link_penalty),
            dim().apply_to(format!("({})", signals.link_count))
        ));
    }

    if signals.promo_link_count > 0 {
        let promo_penalty = signals.promo_link_count as f32 * PENALTY_PROMO_LINK;
        content_modifier -= promo_penalty;
        penalties.push(format!(
            "{}{} {}",
            pad_label("promo-links:", 2),
            format_signed(promo_penalty),
            dim().apply_to(format!("({})", signals.promo_link_count))
        ));
    }

    let engagement_boost = calculate_engagement_boost(signals);
    if engagement_boost >= ENGAGEMENT_BOOST_MIN {
        boosts.push(format!(
            "{}{}",
            pad_label("trending:", 2),
            format_signed(engagement_boost),
        ));
    }

    let authenticity_boost = signals.authenticity_score;
    if authenticity_boost >= GOOD_QUALITY_BOOST_MIN {
        boosts.push(format!(
            "{}{}",
            pad_label("authentic:", 2),
            format_signed(authenticity_boost),
        ));
    }

    let label_boost = signals.label_boost;
    boosts.push(format!(
        "{}{}",
        pad_label("topic:", 2),
        format_signed(label_boost),
    ));

    let final_priority = score.final_score + content_modifier + engagement_boost
        + authenticity_boost + label_boost - quality_penalty;

    let confidence = ConfidenceTier::from_score(score.final_score);

    PriorityBreakdown {
        quality_penalty,
        content_modifier,
        engagement_boost,
        authenticity_boost,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::calculate_score;

    #[test]
    fn test_confidence_tiers() {
        assert_eq!(ConfidenceTier::from_score(0.90), ConfidenceTier::Strong);
        assert_eq!(ConfidenceTier::from_score(0.75), ConfidenceTier::High);
        assert_eq!(ConfidenceTier::from_score(0.55), ConfidenceTier::Moderate);
        assert_eq!(ConfidenceTier::from_score(0.30), ConfidenceTier::Low);
    }

    #[test]
    fn test_basic_priority() {
        let score = calculate_score(0.8, 0.7);
        let signals = PrioritySignals {
            topic_label: "game developer sharing work".to_string(),
            label_boost: 1.0,
            ..Default::default()
        };

        let breakdown = calculate_priority(&score, &signals);
        assert_eq!(breakdown.quality_penalty, 0.0);
    }

    #[test]
    fn test_first_person_boost() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let without = calculate_priority(&score, &signals);

        signals.is_first_person = true;
        let with = calculate_priority(&score, &signals);

        assert!(with.final_priority > without.final_priority);
        assert!(with
            .boost_reasons
            .iter()
            .any(|r| r.contains("first-person")));
    }

    #[test]
    fn test_video_boost() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let without = calculate_priority(&score, &signals);

        signals.has_video = true;
        let with = calculate_priority(&score, &signals);

        assert!(with.final_priority > without.final_priority);
        assert!(with.boost_reasons.iter().any(|r| r.contains("video")));
    }

    #[test]
    fn test_image_with_alt_boost() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            images: 1,
            ..Default::default()
        };

        let without_alt = calculate_priority(&score, &signals);

        signals.has_alt_text = true;
        let with_alt = calculate_priority(&score, &signals);

        assert!(with_alt.final_priority > without_alt.final_priority);
        assert!(with_alt
            .boost_reasons
            .iter()
            .any(|r| r.contains("alt-text")));
    }

    #[test]
    fn test_many_images_penalty() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            images: 2,
            ..Default::default()
        };

        let few = calculate_priority(&score, &signals);

        signals.images = 3;
        let many = calculate_priority(&score, &signals);

        assert!(many.final_priority < few.final_priority);
        assert!(many.penalty_reasons.iter().any(|r| r.contains("images")));
    }

    #[test]
    fn test_link_penalties() {
        let score = calculate_score(1.0, 1.0);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let no_links = calculate_priority(&score, &signals);
        assert!(no_links.penalty_reasons.is_empty());

        signals.link_count = 1;
        let with_links = calculate_priority(&score, &signals);
        assert!(with_links.content_modifier < no_links.content_modifier);
        assert!(with_links
            .penalty_reasons
            .iter()
            .any(|r| r.contains("links")));

        signals.link_count = 0;
        signals.promo_link_count = 1;
        let with_promo = calculate_priority(&score, &signals);
        assert!(with_promo.content_modifier < no_links.content_modifier);
        assert!(with_promo
            .penalty_reasons
            .iter()
            .any(|r| r.contains("promo")));
    }

    #[test]
    fn test_quality_penalties() {
        let score = calculate_score(0.8, 0.7);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let good = calculate_priority(&score, &signals);

        signals.synthetic_score = 0.8;
        let low_effort = calculate_priority(&score, &signals);

        assert!(low_effort.final_priority < good.final_priority);
        assert!(low_effort.quality_penalty > 0.0);

        signals.synthetic_score = 0.0;
        signals.engagement_bait_score = 0.8;
        let bait = calculate_priority(&score, &signals);

        assert!(bait.final_priority < good.final_priority);
    }

    #[test]
    fn test_engagement_boost() {
        let score = calculate_score(0.5, 0.5);
        let mut signals = PrioritySignals {
            label_boost: 1.0,
            ..Default::default()
        };

        let no_engagement = calculate_priority(&score, &signals);

        signals.reply_count = 10;
        signals.repost_count = 5;
        signals.like_count = 20;
        let with_engagement = calculate_priority(&score, &signals);

        assert!(with_engagement.engagement_boost > no_engagement.engagement_boost);
    }

    #[test]
    fn test_priority_not_clamped() {
        let score = calculate_score(1.0, 1.0);
        let signals = PrioritySignals {
            label_boost: 2.0,
            is_first_person: true,
            has_video: true,
            engagement_velocity: 100.0,
            ..Default::default()
        };

        let breakdown = calculate_priority(&score, &signals);
        assert!(breakdown.final_priority > 2.0);
    }
}
