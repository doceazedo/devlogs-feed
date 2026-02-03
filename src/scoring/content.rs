use regex::Regex;
use std::sync::LazyLock;

const FIRST_PERSON: &[&str] = &["i ", "i'", "we ", "we'", "my ", "our "];

const PROMO_DOMAINS: &[&str] = &[
    "store.steampowered.com",
    "steampowered.com",
    "itch.io",
    "twitch.tv",
    "fiverr.com",
    "kickstarter.com",
    "indiegogo.com",
    "patreon.com",
    "ko-fi.com",
    "buymeacoffee.com",
    "gog.com",
    "epicgames.com",
    "humblebundle.com",
    "gamejolt.com",
    "youtube.com",
    "youtu.be",
    "playtester.io",
    "buff.ly",
    "bit.ly",
];

static URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://[^\s]+").unwrap());

#[derive(Debug, Clone, Default)]
pub struct ContentSignals {
    pub is_first_person: bool,
    pub images: u8,
    pub has_video: bool,
    pub has_alt_text: bool,
    pub link_count: u8,
    pub promo_link_count: u8,
}

#[derive(Debug, Clone, Default)]
pub struct MediaInfo {
    pub image_count: u8,
    pub has_video: bool,
    pub has_alt_text: bool,
    pub external_uri: Option<String>,
    pub facet_links: Vec<String>,
}

pub fn extract_content_signals(text: &str, media: &MediaInfo) -> ContentSignals {
    let is_first_person = detect_first_person(text);
    let (mut link_count, mut promo_link_count) = (0u8, 0u8);

    for uri in &media.facet_links {
        link_count = link_count.saturating_add(1);
        if is_promo_domain(uri) {
            promo_link_count = promo_link_count.saturating_add(1);
        }
    }

    if let Some(ref uri) = media.external_uri {
        link_count = link_count.saturating_add(1);
        if is_promo_domain(uri) {
            promo_link_count = promo_link_count.saturating_add(1);
        }
    }

    ContentSignals {
        is_first_person,
        images: media.image_count,
        has_video: media.has_video,
        has_alt_text: media.has_alt_text,
        link_count,
        promo_link_count,
    }
}

pub fn detect_first_person(text: &str) -> bool {
    let text_lower = text.to_lowercase();
    FIRST_PERSON.iter().any(|fp| text_lower.contains(fp))
}

pub fn is_first_person(text: &str) -> bool {
    detect_first_person(text)
}

pub fn count_links(text: &str) -> (u8, u8) {
    let text_lower = text.to_lowercase();
    let links: Vec<&str> = URL_PATTERN
        .find_iter(&text_lower)
        .map(|m| m.as_str())
        .collect();

    let total = links.len().min(255) as u8;
    let promo = links
        .iter()
        .filter(|url| {
            if let Some(domain_start) = url.find("://") {
                let domain_part = &url[domain_start + 3..];
                let domain_end = domain_part.find('/').unwrap_or(domain_part.len());
                let domain = &domain_part[..domain_end];
                PROMO_DOMAINS.iter().any(|d| domain.contains(d))
            } else {
                false
            }
        })
        .count()
        .min(255) as u8;

    (total, promo)
}

pub fn is_promo_domain(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    if let Some(domain_start) = url_lower.find("://") {
        let domain_part = &url_lower[domain_start + 3..];
        let domain_end = domain_part.find('/').unwrap_or(domain_part.len());
        let domain = &domain_part[..domain_end];
        PROMO_DOMAINS.iter().any(|d| domain.contains(d))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_first_person() {
        assert!(detect_first_person("I built this game"));
        assert!(detect_first_person("We're working on a new feature"));
        assert!(detect_first_person("This is my game"));
        assert!(detect_first_person("Our team released the update"));
        assert!(!detect_first_person("The game is ready"));
        assert!(!detect_first_person("They built a great game"));
    }

    #[test]
    fn test_count_links() {
        let (total, promo) = count_links("Check out https://example.com");
        assert_eq!(total, 1);
        assert_eq!(promo, 0);

        let (total, promo) = count_links("Wishlist on https://store.steampowered.com/app/123");
        assert_eq!(total, 1);
        assert_eq!(promo, 1);

        let (total, promo) = count_links(
            "Links: https://example.com https://itch.io/game/test https://twitter.com/x",
        );
        assert_eq!(total, 3);
        assert_eq!(promo, 1);
    }

    #[test]
    fn test_is_promo_domain() {
        assert!(is_promo_domain("https://store.steampowered.com/app/123"));
        assert!(is_promo_domain("https://itch.io/game/test"));
        assert!(is_promo_domain("https://twitch.tv/channel"));
        assert!(is_promo_domain("https://kickstarter.com/project"));
        assert!(!is_promo_domain("https://example.com"));
        assert!(!is_promo_domain("https://twitter.com/user"));
        assert!(is_promo_domain("https://youtube.com/watch"));
    }

    #[test]
    fn test_extract_content_signals() {
        let media = MediaInfo {
            image_count: 2,
            has_video: false,
            has_alt_text: true,
            external_uri: None,
            facet_links: vec!["https://itch.io/game".to_string()],
        };
        let signals = extract_content_signals("I'm working on my game", &media);

        assert!(signals.is_first_person);
        assert_eq!(signals.images, 2);
        assert!(!signals.has_video);
        assert!(signals.has_alt_text);
        assert_eq!(signals.link_count, 1);
        assert_eq!(signals.promo_link_count, 1);
    }
}
