use regex::Regex;
use std::sync::LazyLock;

pub const BONUS_FIRST_PERSON: f32 = 0.15;
pub const BONUS_MEDIA: f32 = 0.10;
pub const BONUS_VIDEO: f32 = 0.15;
pub const PENALTY_MANY_IMAGES: f32 = 0.15;
pub const MANY_IMAGES_THRESHOLD: usize = 3;

const FIRST_PERSON: &[&str] = &["i ", "i'", "we ", "we'", "my ", "our "];

const PROMO_KEYWORDS: &[&str] = &[
    "wishlist",
    "buy now",
    "steam page",
    "kickstarter",
    "available now",
    "out now",
    "pre-order",
    "preorder",
    "launching soon",
    "on sale",
    "discount",
    "free demo",
    "check out",
    "link in bio",
    "just dropped",
];

const MARKETING_HASHTAGS: &[&str] = &[
    "#marketingmonday",
    "#promo",
    "#ad",
    "#sponsored",
    "#affiliate",
    "#sale",
    "#giveaway",
    "#contest",
    "#linkinbio",
];

const PROMO_LINK_DOMAINS: &[&str] = &[
    "store.steampowered.com",
    "steampowered.com",
    "itch.io",
    "kickstarter.com",
    "indiegogo.com",
    "gog.com",
    "epicgames.com",
    "humblebundle.com",
    "gamejolt.com",
    "patreon.com",
    "ko-fi.com",
    "buymeacoffee.com",
];

static URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://[^\s]+").unwrap());

const MAX_PROMO_PENALTY: f32 = 0.8;

pub fn is_first_person(text: &str) -> bool {
    let text_lower = text.to_lowercase();
    FIRST_PERSON.iter().any(|fp| text_lower.contains(fp))
}

#[derive(Debug, Clone, Default)]
pub struct PromoPenaltyBreakdown {
    pub keyword_count: usize,
    pub marketing_hashtag_count: usize,
    pub marketing_hashtags_found: Vec<String>,
    pub has_link: bool,
    pub link_domains: Vec<String>,
    pub promo_link_count: usize,
    pub total_penalty: f32,
}

pub fn promo_penalty(text: &str) -> PromoPenaltyBreakdown {
    let text_lower = text.to_lowercase();
    let mut breakdown = PromoPenaltyBreakdown::default();

    breakdown.keyword_count = PROMO_KEYWORDS
        .iter()
        .filter(|kw| text_lower.contains(*kw))
        .count();

    for hashtag in MARKETING_HASHTAGS {
        if text_lower.contains(hashtag) {
            breakdown.marketing_hashtag_count += 1;
            breakdown.marketing_hashtags_found.push(hashtag.to_string());
        }
    }

    for url_match in URL_PATTERN.find_iter(&text_lower) {
        breakdown.has_link = true;
        let url = url_match.as_str();

        if let Some(domain_start) = url.find("://") {
            let domain_part = &url[domain_start + 3..];
            let domain_end = domain_part.find('/').unwrap_or(domain_part.len());
            let domain = &domain_part[..domain_end];
            breakdown.link_domains.push(domain.to_string());

            for promo_domain in PROMO_LINK_DOMAINS {
                if domain.contains(promo_domain) {
                    breakdown.promo_link_count += 1;
                    break;
                }
            }
        }
    }

    let keyword_penalty = (breakdown.keyword_count as f32 * 0.15).min(0.3);
    let hashtag_penalty = (breakdown.marketing_hashtag_count as f32 * 0.2).min(0.3);
    let link_penalty = if breakdown.has_link { 0.1 } else { 0.0 };
    let promo_link_penalty = (breakdown.promo_link_count as f32 * 0.15).min(0.3);

    breakdown.total_penalty =
        (keyword_penalty + hashtag_penalty + link_penalty + promo_link_penalty)
            .min(MAX_PROMO_PENALTY);

    breakdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bonus_first_person() {
        assert!(is_first_person("I built this game"));
        assert!(!is_first_person("The game is ready"));
    }

    #[test]
    fn test_promo_penalty_keywords() {
        let penalty = promo_penalty("Wishlist now on Steam!").total_penalty;
        assert!((penalty - 0.15).abs() < 0.01);
    }

    #[test]
    fn test_promo_penalty_domains() {
        let breakdown = promo_penalty("Check it out! https://store.steampowered.com/app/123");
        assert!(breakdown.has_link);
        assert_eq!(breakdown.promo_link_count, 1);
    }

    #[test]
    fn test_promo_penalty_capped() {
        let text = "Wishlist! Buy now! Available now! Pre-order! On sale! Discount! #promo #ad #giveaway https://store.steampowered.com/app/123 https://itch.io/game";
        let breakdown = promo_penalty(text);
        assert!(breakdown.total_penalty <= MAX_PROMO_PENALTY);
    }
}
