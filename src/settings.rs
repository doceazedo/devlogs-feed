use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server: Server,
    pub scoring: Scoring,
    pub engagement: Engagement,
    pub feed: Feed,
    pub ml: Ml,
    pub spam: Spam,
    pub backfill: Backfill,
    pub decay: Decay,
    pub filters: Filters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filters {
    pub gamedev_keywords: Vec<String>,
    pub gamedev_hashtags: Vec<String>,
    pub blocked_keywords: Vec<String>,
    pub blocked_hashtags: Vec<String>,
    pub promo_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub publisher_did: String,
    pub feed_hostname: String,
    pub firehose_limit: usize,
    pub enable_backfill: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scoring {
    pub thresholds: ScoringThresholds,
    pub weights: ScoringWeights,
    pub bonuses: ContentBonuses,
    pub penalties: ContentPenalties,
    pub quality: QualityThresholds,
    pub confidence: ConfidenceThresholds,
    pub topic_boosts: TopicBoosts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringThresholds {
    pub score: f32,
    pub ml_rejection: f32,
    pub min_text_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub topic: f32,
    pub semantic: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBonuses {
    pub first_person: f32,
    pub video: f32,
    pub image_with_alt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPenalties {
    pub many_images: f32,
    pub many_images_threshold: u8,
    pub link_exponential_base: f32,
    pub promo_link: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityThresholds {
    pub poor_quality_penalty_min: f32,
    pub good_quality_boost_min: f32,
    pub engagement_boost_min: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceThresholds {
    pub strong: f32,
    pub high: f32,
    pub moderate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicBoosts {
    pub game_dev_sharing_work: f32,
    pub game_dev_rant: f32,
    pub game_programming: f32,
    pub default: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engagement {
    pub weights: EngagementWeights,
    pub velocity_scale: f32,
    pub max_boost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementWeights {
    pub reply: f32,
    pub repost: f32,
    pub like: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub cutoff_hours: i64,
    pub default_limit: usize,
    pub max_limit: usize,
    pub max_stored_posts: i64,
    pub shuffle_variance: f32,
    pub preference_boost: f32,
    pub preference_penalty: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ml {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spam {
    pub repost_threshold: f32,
    pub velocity_window_hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backfill {
    pub limit: usize,
    pub hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decay {
    pub every_x_hours: f32,
    pub factor: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: Server {
                publisher_did: "did:web:example.com".to_string(),
                feed_hostname: "example.com".to_string(),
                firehose_limit: 5000,
                enable_backfill: false,
            },
            scoring: Scoring {
                thresholds: ScoringThresholds {
                    score: 0.50,
                    ml_rejection: 0.85,
                    min_text_length: 20,
                },
                weights: ScoringWeights {
                    topic: 0.8,
                    semantic: 0.20,
                },
                bonuses: ContentBonuses {
                    first_person: 0.2,
                    video: 0.1,
                    image_with_alt: 0.1,
                },
                penalties: ContentPenalties {
                    many_images: 0.2,
                    many_images_threshold: 3,
                    link_exponential_base: 3.0,
                    promo_link: 1.5,
                },
                quality: QualityThresholds {
                    poor_quality_penalty_min: 0.5,
                    good_quality_boost_min: 0.1,
                    engagement_boost_min: 0.05,
                },
                confidence: ConfidenceThresholds {
                    strong: 0.85,
                    high: 0.70,
                    moderate: 0.50,
                },
                topic_boosts: TopicBoosts {
                    game_dev_sharing_work: 0.4,
                    game_dev_rant: 0.6,
                    game_programming: 0.2,
                    default: 0.6,
                },
            },
            engagement: Engagement {
                weights: EngagementWeights {
                    reply: 3.0,
                    repost: 2.0,
                    like: 1.0,
                },
                velocity_scale: 0.1,
                max_boost: 0.5,
            },
            feed: Feed {
                cutoff_hours: 24 * 7,
                default_limit: 50,
                max_limit: 500,
                max_stored_posts: 5000,
                shuffle_variance: 0.05,
                preference_boost: 1.5,
                preference_penalty: 0.3,
            },
            ml: Ml {
                batch_size: 16,
                batch_timeout_ms: 10,
            },
            spam: Spam {
                repost_threshold: 10.0,
                velocity_window_hours: 1,
            },
            backfill: Backfill {
                limit: 200,
                hours: 96,
            },
            decay: Decay {
                every_x_hours: 2.0,
                factor: 0.75,
            },
            filters: Filters {
                gamedev_keywords: vec![
                    "gamedev".into(),
                    "game dev".into(),
                    "game development".into(),
                    "game animation".into(),
                    "game art".into(),
                    "game audio".into(),
                    "game design".into(),
                    "indiedev".into(),
                    "indie dev".into(),
                    "devlog".into(),
                    "game jam".into(),
                    "gamejam".into(),
                    "ue5".into(),
                    "ue4".into(),
                    "godot".into(),
                    "gamemaker".into(),
                    "rpg maker".into(),
                    "defold".into(),
                    "monogame".into(),
                    "libgdx".into(),
                    "phaser".into(),
                    "pygame".into(),
                    "love2d".into(),
                    "macroquad".into(),
                    "raylib".into(),
                    "sdl".into(),
                    "sfml".into(),
                    "aseprite".into(),
                    "fmod".into(),
                    "wwise".into(),
                    "game engine".into(),
                    "environment design".into(),
                    "level design".into(),
                    "level editor".into(),
                    "glsl".into(),
                    "wgsl".into(),
                ],
                gamedev_hashtags: vec![
                    "#gamedev".into(),
                    "#indiedev".into(),
                    "#indiegame".into(),
                    "#indiegames".into(),
                    "#screenshotsaturday".into(),
                    "#madewithunity".into(),
                    "#madewithgodot".into(),
                    "#madewithunreal".into(),
                    "#godot".into(),
                    "#bevy".into(),
                    "#pixelart".into(),
                    "#devlog".into(),
                    "#gamedevelopment".into(),
                    "#rust".into(),
                    "#rustlang".into(),
                ],
                blocked_keywords: vec![
                    "crypto".into(),
                    "nft".into(),
                    "web3".into(),
                    "blockchain".into(),
                    "mint".into(),
                    "airdrop".into(),
                    "whitelist".into(),
                    "rugpull".into(),
                    "solana".into(),
                    "ethereum".into(),
                    "bitcoin".into(),
                    "token sale".into(),
                    "ico".into(),
                    "hodl".into(),
                ],
                blocked_hashtags: vec![
                    "#nft".into(),
                    "#nfts".into(),
                    "#nftart".into(),
                    "#nftgaming".into(),
                    "#crypto".into(),
                    "#cryptocurrency".into(),
                    "#web3".into(),
                    "#web3gaming".into(),
                    "#blockchain".into(),
                    "#freemint".into(),
                    "#airdrop".into(),
                    "#solana".into(),
                    "#ethereum".into(),
                    "#bitcoin".into(),
                ],
                promo_domains: vec![
                    "store.steampowered.com".into(),
                    "steampowered.com".into(),
                    "itch.io".into(),
                    "twitch.tv".into(),
                    "fiverr.com".into(),
                    "kickstarter.com".into(),
                    "indiegogo.com".into(),
                    "patreon.com".into(),
                    "ko-fi.com".into(),
                    "buymeacoffee.com".into(),
                    "gog.com".into(),
                    "epicgames.com".into(),
                    "humblebundle.com".into(),
                    "gamejolt.com".into(),
                    "youtube.com".into(),
                    "youtu.be".into(),
                    "playtester.io".into(),
                    "buff.ly".into(),
                    "bit.ly".into(),
                ],
            },
        }
    }
}

impl Settings {
    pub fn load() -> &'static Settings {
        SETTINGS.get_or_init(|| Self::load_from_files())
    }

    fn load_from_files() -> Settings {
        let default_path = Path::new("settings.default.ron");
        let override_path = Path::new("settings.ron");

        let mut settings = if default_path.exists() {
            fs::read_to_string(default_path)
                .ok()
                .and_then(|content| ron::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Settings::default()
        };

        if override_path.exists() {
            if let Ok(content) = fs::read_to_string(override_path) {
                if let Ok(overrides) = ron::from_str::<Settings>(&content) {
                    settings = overrides;
                }
            }
        }

        settings
    }
}

pub fn settings() -> &'static Settings {
    Settings::load()
}
