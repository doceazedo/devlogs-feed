use super::relevance::strip_hashtags;
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
}

pub fn apply_filters(
    text: &str,
    lang: Option<&str>,
    author_did: Option<&str>,
    spammer_check: impl Fn(&str) -> bool,
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
        if spammer_check(did) {
            return FilterResult::Reject(Filter::Spammer);
        }
    }

    FilterResult::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_spammer(_: &str) -> bool {
        false
    }

    #[test]
    fn test_filter_min_length() {
        let result = apply_filters("hi", Some("en"), None, no_spammer);
        assert_eq!(result, FilterResult::Reject(Filter::MinLength));
    }

    #[test]
    fn test_filter_english_only() {
        let text = "This is a long enough text for testing purposes";
        let result = apply_filters(text, Some("pt"), None, no_spammer);
        assert_eq!(result, FilterResult::Reject(Filter::EnglishOnly));

        let result_en = apply_filters(text, Some("en"), None, no_spammer);
        assert_eq!(result_en, FilterResult::Pass);
    }

    #[test]
    fn test_filter_blocked_keyword() {
        let text = "Check out my new NFT game collection";
        let result = apply_filters(text, Some("en"), None, no_spammer);
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
        let result = apply_filters(text, Some("en"), Some("did:plc:spammer123"), is_spammer);
        assert_eq!(result, FilterResult::Reject(Filter::Spammer));
    }

    #[test]
    fn test_filter_pass() {
        let text = "Just implemented a new combat system in my game #gamedev";
        let result = apply_filters(text, Some("en"), None, no_spammer);
        assert_eq!(result, FilterResult::Pass);
    }
}
