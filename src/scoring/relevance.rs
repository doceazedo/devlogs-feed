use regex::Regex;
use std::sync::LazyLock;

static WORD_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9]+").unwrap());
static HASHTAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\w+").unwrap());

pub fn strip_hashtags(text: &str) -> String {
    HASHTAG_PATTERN.replace_all(text, "").trim().to_string()
}

pub const GAMEDEV_KEYWORDS: &[&str] = &[
    "gamedev",
    "game dev",
    "game development",
    "game animation",
    "game art",
    "game audio",
    "game design",
    "indiedev",
    "indie dev",
    "devlog",
    "game jam",
    "gamejam",
    "ue5",
    "ue4",
    "godot",
    "gamemaker",
    "game maker",
    "rpg maker",
    "construct",
    "defold",
    "monogame",
    "libgdx",
    "phaser",
    "pygame",
    "love2d",
    "bevy",
    "macroquad",
    "raylib",
    "sdl",
    "sfml",
    "aseprite",
    "fmod",
    "wwise",
    "game engine",
    "environment design",
    "level design",
    "level editor",
    "glsl",
    "wgsl",
];

pub const GAMEDEV_HASHTAGS: &[&str] = &[
    "#gamedev",
    "#indiedev",
    "#indiegame",
    "#indiegames",
    "#screenshotsaturday",
    "#madewithunity",
    "#madewithgodot",
    "#madewithunreal",
    "#godot",
    "#bevy",
    "#pixelart",
    "#devlog",
    "#gamedevelopment",
    "#rust",
    "#rustlang",
];

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
    let text_lower = text.to_lowercase();
    let count = GAMEDEV_KEYWORDS
        .iter()
        .filter(|kw| contains_keyword(&text_lower, kw))
        .count();
    (count > 0, count)
}

pub fn has_hashtags(text: &str) -> (bool, usize) {
    let text_lower = text.to_lowercase();
    let count = GAMEDEV_HASHTAGS
        .iter()
        .filter(|tag| text_lower.contains(*tag))
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
    fn test_hashtag_case_insensitivity() {
        let (found, _) = has_hashtags("Working on my project #GAMEDEV");
        assert!(found);

        let (found2, _) = has_hashtags("Progress #GameDev");
        assert!(found2);
    }
}
