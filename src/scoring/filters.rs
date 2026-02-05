use super::content::{is_promo_domain, MediaInfo};
use super::relevance::{count_all_hashtags, strip_hashtags};
use crate::settings::settings;
use strum::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum FilterResult {
    Pass,
    Reject(Filter),
}

#[derive(Debug, Clone, PartialEq, Display)]
pub enum Filter {
    #[strum(serialize = "min-length")]
    MinLength,
    #[strum(serialize = "english-only")]
    EnglishOnly,
    #[strum(serialize = "blocked-keyword")]
    BlockedKeyword(String),
    #[strum(serialize = "blocked-hashtag")]
    BlockedHashtag(String),
    #[strum(serialize = "spammer")]
    Spammer,
    #[strum(serialize = "blocked-author")]
    BlockedAuthor,
    #[strum(serialize = "promo-link")]
    PromoLink,
    #[strum(serialize = "too-many-hashtags")]
    TooManyHashtags(usize),
    #[strum(serialize = "low-priority")]
    LowPriority,
}

pub fn apply_filters(
    text: &str,
    lang: Option<&str>,
    author_did: Option<&str>,
    media: &MediaInfo,
    mut spammer_check: impl FnMut(&str) -> bool,
    mut blocked_author_check: impl FnMut(&str) -> bool,
) -> FilterResult {
    let s = settings();
    let stripped = strip_hashtags(text);
    if stripped.len() < s.scoring.thresholds.min_text_length {
        return FilterResult::Reject(Filter::MinLength);
    }

    if let Some(lang) = lang {
        if !lang.starts_with("en") {
            return FilterResult::Reject(Filter::EnglishOnly);
        }
    }

    let text_lower = text.to_lowercase();

    for keyword in &s.filters.blocked_keywords {
        if text_lower.contains(keyword) {
            return FilterResult::Reject(Filter::BlockedKeyword(keyword.to_string()));
        }
    }

    for hashtag in &s.filters.blocked_hashtags {
        if text_lower.contains(hashtag) {
            return FilterResult::Reject(Filter::BlockedHashtag(hashtag.to_string()));
        }
    }

    if let Some(did) = author_did {
        if blocked_author_check(did) {
            return FilterResult::Reject(Filter::BlockedAuthor);
        }
        if spammer_check(did) {
            return FilterResult::Reject(Filter::Spammer);
        }
    }

    let has_promo = media.facet_links.iter().any(|uri| is_promo_domain(uri))
        || media
            .external_uri
            .as_ref()
            .is_some_and(|uri| is_promo_domain(uri));
    if has_promo {
        return FilterResult::Reject(Filter::PromoLink);
    }

    let hashtag_count = count_all_hashtags(text);
    if hashtag_count > s.scoring.rejection.max_hashtags as usize {
        return FilterResult::Reject(Filter::TooManyHashtags(hashtag_count));
    }

    FilterResult::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_spammer(_: &str) -> bool {
        false
    }

    fn no_blocked(_: &str) -> bool {
        false
    }

    fn no_media() -> MediaInfo {
        MediaInfo::default()
    }

    #[test]
    fn test_filter_min_length() {
        let result = apply_filters("hi", Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Reject(Filter::MinLength));
    }

    #[test]
    fn test_filter_english_only() {
        let text = "This is a long enough text for testing purposes";
        let result = apply_filters(text, Some("pt"), None, &no_media(), no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Reject(Filter::EnglishOnly));

        let result_en = apply_filters(text, Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert_eq!(result_en, FilterResult::Pass);
    }

    #[test]
    fn test_filter_blocked_keyword() {
        let text = "Check out my new NFT game collection";
        let result = apply_filters(text, Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert!(matches!(
            result,
            FilterResult::Reject(Filter::BlockedKeyword(_))
        ));
    }

    #[test]
    fn test_filter_blocked_hashtag() {
        let text_lower = "working on my game project today #gamedev #nftart".to_lowercase();
        assert!(text_lower.contains("#nftart"));
        assert!(settings()
            .filters
            .blocked_hashtags
            .iter()
            .any(|h| text_lower.contains(h)));
    }

    #[test]
    fn test_filter_spammer() {
        let text = "This is a valid gamedev post about my project";
        let is_spammer = |did: &str| did == "did:plc:spammer123";
        let result = apply_filters(
            text,
            Some("en"),
            Some("did:plc:spammer123"),
            &no_media(),
            is_spammer,
            no_blocked,
        );
        assert_eq!(result, FilterResult::Reject(Filter::Spammer));
    }

    #[test]
    fn test_filter_blocked_author() {
        let text = "This is a valid gamedev post about my project";
        let is_blocked = |did: &str| did == "did:plc:blocked456";
        let result = apply_filters(
            text,
            Some("en"),
            Some("did:plc:blocked456"),
            &no_media(),
            no_spammer,
            is_blocked,
        );
        assert_eq!(result, FilterResult::Reject(Filter::BlockedAuthor));
    }

    #[test]
    fn test_filter_promo_link() {
        let text = "Check out my game on Steam! Really proud of it";
        let media = MediaInfo {
            external_uri: Some("https://store.steampowered.com/app/12345".to_string()),
            ..Default::default()
        };
        let result = apply_filters(text, Some("en"), None, &media, no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Reject(Filter::PromoLink));
    }

    #[test]
    fn test_filter_promo_link_in_facets() {
        let text = "Wishlist my game now! Really excited about launch";
        let media = MediaInfo {
            facet_links: vec!["https://itch.io/game/test".to_string()],
            ..Default::default()
        };
        let result = apply_filters(text, Some("en"), None, &media, no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Reject(Filter::PromoLink));
    }

    #[test]
    fn test_filter_too_many_hashtags() {
        let text = "My game #one #two #three #four #five #six #seven is great";
        let result = apply_filters(text, Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert!(matches!(
            result,
            FilterResult::Reject(Filter::TooManyHashtags(7))
        ));
    }

    #[test]
    fn test_filter_hashtags_at_limit() {
        let text = "My game #one #two #three #four #five #six is great";
        let result = apply_filters(text, Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Pass);
    }

    #[test]
    fn test_filter_pass() {
        let text = "Just implemented a new combat system in my game #gamedev";
        let result = apply_filters(text, Some("en"), None, &no_media(), no_spammer, no_blocked);
        assert_eq!(result, FilterResult::Pass);
    }
}
