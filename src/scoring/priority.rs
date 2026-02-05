use super::classification::QualityAssessment;
use super::content::ContentSignals;
use crate::settings::settings;
use crate::utils::logs::{dim, format_signed, pad_label};

#[derive(Debug, Clone, Default)]
pub struct PrioritySignals {
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

impl PrioritySignals {
    pub fn new(quality: &QualityAssessment, content: &ContentSignals) -> Self {
        Self {
            engagement_bait_score: quality.engagement_bait_score,
            synthetic_score: quality.synthetic_score,
            authenticity_score: quality.authenticity_score,
            is_first_person: content.is_first_person,
            images: content.images,
            has_video: content.has_video,
            has_alt_text: content.has_alt_text,
            link_count: content.link_count,
            promo_link_count: content.promo_link_count,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PriorityBreakdown {
    pub quality_penalty: f32,
    pub content_modifier: f32,
    pub engagement_boost: f32,
    pub authenticity_boost: f32,
    pub priority: f32,
    pub boost_reasons: Vec<String>,
    pub penalty_reasons: Vec<String>,
}

pub fn calculate_priority(signals: &PrioritySignals) -> PriorityBreakdown {
    let s = settings();
    let mut boosts = Vec::new();
    let mut penalties = Vec::new();

    let mut quality_penalty = 0.0;

    if signals.engagement_bait_score >= s.scoring.quality.poor_quality_penalty_min {
        quality_penalty += signals.engagement_bait_score;
        penalties.push(format!(
            "{}{}",
            pad_label("engagement-bait:", 2),
            format_signed(signals.engagement_bait_score * -1.0)
        ));
    }

    if signals.synthetic_score >= s.scoring.quality.poor_quality_penalty_min {
        quality_penalty += signals.synthetic_score;
        penalties.push(format!(
            "{}{}",
            pad_label("synthetic:", 2),
            format_signed(signals.synthetic_score * -1.0)
        ));
    }

    let mut content_modifier = 0.0;

    if signals.is_first_person {
        content_modifier += s.scoring.bonuses.first_person;
        boosts.push(format!(
            "{}{}",
            pad_label("first-person:", 2),
            format_signed(s.scoring.bonuses.first_person),
        ));
    }

    if signals.has_video {
        content_modifier += s.scoring.bonuses.video;
        boosts.push(format!(
            "{}{}",
            pad_label("video:", 2),
            format_signed(s.scoring.bonuses.video),
        ));
    }

    if signals.images > 0 && signals.has_alt_text {
        content_modifier += s.scoring.bonuses.image_with_alt;
        boosts.push(format!(
            "{}{}",
            pad_label("alt-text:", 2),
            format_signed(s.scoring.bonuses.image_with_alt),
        ));
    }

    if signals.images >= s.scoring.penalties.many_images_threshold {
        content_modifier -= s.scoring.penalties.many_images;
        penalties.push(format!(
            "{}{} {}",
            pad_label("images:", 2),
            format_signed(s.scoring.penalties.many_images),
            dim().apply_to(format!("({})", signals.images))
        ));
    }

    if signals.link_count > 0 {
        let link_penalty = s
            .scoring
            .penalties
            .link_exponential_base
            .powi(signals.link_count as i32);
        content_modifier -= link_penalty;
        penalties.push(format!(
            "{}{} {}",
            pad_label("links:", 2),
            format_signed(link_penalty),
            dim().apply_to(format!("({})", signals.link_count))
        ));
    }

    if signals.promo_link_count > 0 {
        let promo_penalty = signals.promo_link_count as f32 * s.scoring.penalties.promo_link;
        content_modifier -= promo_penalty;
        penalties.push(format!(
            "{}{} {}",
            pad_label("promo-links:", 2),
            format_signed(promo_penalty),
            dim().apply_to(format!("({})", signals.promo_link_count))
        ));
    }

    let engagement_boost = calculate_engagement_boost(signals);
    if engagement_boost >= s.scoring.quality.engagement_boost_min {
        boosts.push(format!(
            "{}{}",
            pad_label("trending:", 2),
            format_signed(engagement_boost),
        ));
    }

    let authenticity_boost = signals.authenticity_score;
    if authenticity_boost >= s.scoring.quality.good_quality_boost_min {
        boosts.push(format!(
            "{}{}",
            pad_label("authentic:", 2),
            format_signed(authenticity_boost),
        ));
    }

    let priority = content_modifier + engagement_boost + authenticity_boost - quality_penalty;

    PriorityBreakdown {
        quality_penalty,
        content_modifier,
        engagement_boost,
        authenticity_boost,
        priority,
        boost_reasons: boosts,
        penalty_reasons: penalties,
    }
}

fn calculate_engagement_boost(signals: &PrioritySignals) -> f32 {
    let s = settings();
    if signals.engagement_velocity > 0.0 {
        (signals.engagement_velocity.ln_1p() * s.engagement.velocity_scale)
            .min(s.engagement.max_boost)
    } else {
        let weighted = signals.reply_count as f32 * s.engagement.weights.reply
            + signals.repost_count as f32 * s.engagement.weights.repost
            + signals.like_count as f32 * s.engagement.weights.like;
        (weighted.ln_1p() * s.engagement.velocity_scale).min(s.engagement.max_boost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_priority() {
        let signals = PrioritySignals::default();
        let breakdown = calculate_priority(&signals);
        assert_eq!(breakdown.quality_penalty, 0.0);
    }

    #[test]
    fn test_first_person_boost() {
        let mut signals = PrioritySignals::default();

        let without = calculate_priority(&signals);

        signals.is_first_person = true;
        let with = calculate_priority(&signals);

        assert!(with.priority > without.priority);
        assert!(with
            .boost_reasons
            .iter()
            .any(|r| r.contains("first-person")));
    }

    #[test]
    fn test_video_boost() {
        let mut signals = PrioritySignals::default();

        let without = calculate_priority(&signals);

        signals.has_video = true;
        let with = calculate_priority(&signals);

        assert!(with.priority > without.priority);
        assert!(with.boost_reasons.iter().any(|r| r.contains("video")));
    }

    #[test]
    fn test_image_with_alt_boost() {
        let mut signals = PrioritySignals {
            images: 1,
            ..Default::default()
        };

        let without_alt = calculate_priority(&signals);

        signals.has_alt_text = true;
        let with_alt = calculate_priority(&signals);

        assert!(with_alt.priority > without_alt.priority);
        assert!(with_alt
            .boost_reasons
            .iter()
            .any(|r| r.contains("alt-text")));
    }

    #[test]
    fn test_many_images_penalty() {
        let mut signals = PrioritySignals {
            images: 2,
            ..Default::default()
        };

        let few = calculate_priority(&signals);

        signals.images = 3;
        let many = calculate_priority(&signals);

        assert!(many.priority < few.priority);
        assert!(many.penalty_reasons.iter().any(|r| r.contains("images")));
    }

    #[test]
    fn test_link_penalties() {
        let mut signals = PrioritySignals::default();

        let no_links = calculate_priority(&signals);
        assert!(no_links.penalty_reasons.is_empty());

        signals.link_count = 1;
        let with_links = calculate_priority(&signals);
        assert!(with_links.content_modifier < no_links.content_modifier);
        assert!(with_links
            .penalty_reasons
            .iter()
            .any(|r| r.contains("links")));

        signals.link_count = 0;
        signals.promo_link_count = 1;
        let with_promo = calculate_priority(&signals);
        assert!(with_promo.content_modifier < no_links.content_modifier);
        assert!(with_promo
            .penalty_reasons
            .iter()
            .any(|r| r.contains("promo")));
    }

    #[test]
    fn test_quality_penalties() {
        let mut signals = PrioritySignals::default();

        let good = calculate_priority(&signals);

        signals.synthetic_score = 0.8;
        let low_effort = calculate_priority(&signals);

        assert!(low_effort.priority < good.priority);
        assert!(low_effort.quality_penalty > 0.0);

        signals.synthetic_score = 0.0;
        signals.engagement_bait_score = 0.8;
        let bait = calculate_priority(&signals);

        assert!(bait.priority < good.priority);
    }

    #[test]
    fn test_engagement_boost() {
        let mut signals = PrioritySignals::default();

        let no_engagement = calculate_priority(&signals);

        signals.reply_count = 10;
        signals.repost_count = 5;
        signals.like_count = 20;
        let with_engagement = calculate_priority(&signals);

        assert!(with_engagement.engagement_boost > no_engagement.engagement_boost);
    }
}
