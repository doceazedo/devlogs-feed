use crate::settings::settings;
use regex::Regex;
use std::sync::LazyLock;

static WORD_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9]+").unwrap());
static HASHTAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\w+").unwrap());

pub fn strip_hashtags(text: &str) -> String {
    HASHTAG_PATTERN.replace_all(text, "").trim().to_string()
}

pub fn count_all_hashtags(text: &str) -> usize {
    HASHTAG_PATTERN.find_iter(text).count()
}

fn contains_keyword(text: &str, keyword: &str) -> bool {
    let keyword_parts: Vec<&str> = WORD_SPLIT
        .split(keyword)
        .filter(|s| !s.is_empty())
        .collect();
    let words: Vec<&str> = WORD_SPLIT.split(text).filter(|s| !s.is_empty()).collect();

    if keyword_parts.len() == 1 {
        words.iter().any(|w| w.eq_ignore_ascii_case(keyword))
    } else {
        words.windows(keyword_parts.len()).any(|window| {
            window
                .iter()
                .zip(keyword_parts.iter())
                .all(|(w, kw)| w.eq_ignore_ascii_case(kw))
        })
    }
}

pub fn has_keywords(text: &str) -> (bool, usize) {
    let s = settings();
    let keywords = &s.filters.gamedev_keywords;
    let text_lower = text.to_lowercase();
    let count = keywords
        .iter()
        .filter(|kw| contains_keyword(&text_lower, kw))
        .count();
    (count > 0, count)
}

pub fn has_hashtags(text: &str) -> (bool, usize) {
    let s = settings();
    let hashtags = &s.filters.gamedev_hashtags;
    let text_lower = text.to_lowercase();
    let text_hashtags: Vec<&str> = HASHTAG_PATTERN
        .find_iter(&text_lower)
        .map(|m| m.as_str())
        .collect();
    let count = hashtags
        .iter()
        .filter(|tag| text_hashtags.iter().any(|h| *h == tag.as_str()))
        .count();
    (count > 0, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bonus_keyword_detection() {
        let (found, count) = has_keywords("Working on a gamedev project");
        assert!(found);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_bonus_hashtag_detection() {
        let (found, count) = has_hashtags("Progress update #gamedev #indiedev");
        assert!(found);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_keyword_substring_matching() {
        let (found, _) = has_keywords("Community radio pioneers since 1996");
        assert!(!found);
    }

    #[test]
    fn test_hashtag_substring_match() {
        let (found, _) = has_hashtags("Rust Belt homeowners! #RustBeltLiving #PropertyValue");
        assert!(!found);
    }

    #[test]
    fn test_hashtag_case_insensitivity() {
        let (found, _) = has_hashtags("Working on my project #GAMEDEV");
        assert!(found);

        let (found2, _) = has_hashtags("Progress #GameDev");
        assert!(found2);
    }
}
