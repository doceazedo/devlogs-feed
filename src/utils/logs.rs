use console::{measure_text_width, Style};

use crate::scoring::{
    ContentSignals, Filter, FilterResult, MediaInfo, PriorityBreakdown, PrioritySignals,
    QualityAssessment,
};
use crate::settings::settings;

pub const TREE_BRANCH: char = '\u{251C}';
pub const TREE_END: char = '\u{2514}';
pub const TREE_HORIZ: char = '\u{2500}';
pub const TREE_VERT: char = '\u{2502}';

const TREE_PREFIX_WIDTH: usize = 4;
const VALUE_COLUMN: usize = 25;

fn tree_branch() -> String {
    dim()
        .apply_to(format!("{}{}{} ", TREE_BRANCH, TREE_HORIZ, TREE_HORIZ))
        .to_string()
}

fn tree_end() -> String {
    dim()
        .apply_to(format!("{}{}{} ", TREE_END, TREE_HORIZ, TREE_HORIZ))
        .to_string()
}

fn tree_indent() -> String {
    dim().apply_to(format!("{}   ", TREE_VERT)).to_string()
}

pub fn dim() -> Style {
    Style::new().dim()
}

fn blue() -> Style {
    Style::new().blue()
}

fn magenta() -> Style {
    Style::new().magenta()
}

fn cyan() -> Style {
    Style::new().cyan()
}

fn green() -> Style {
    Style::new().green()
}

fn red() -> Style {
    Style::new().red()
}

fn yellow() -> Style {
    Style::new().yellow()
}

fn bold() -> Style {
    Style::new().bold()
}

fn init_prefix() -> String {
    blue().apply_to("[INIT]").to_string()
}

fn backfill_prefix() -> String {
    magenta().apply_to("[BACKFILL]").to_string()
}

fn ml_prefix() -> String {
    yellow().apply_to("[ML]").to_string()
}

pub fn pad_label(label: &str, depth: usize) -> String {
    let prefix_width = depth * TREE_PREFIX_WIDTH;
    let target_width = VALUE_COLUMN.saturating_sub(prefix_width);
    let current_width = measure_text_width(label);
    if current_width < target_width {
        format!("{}{}", label, " ".repeat(target_width - current_width))
    } else {
        format!("{} ", label)
    }
}

pub fn format_signed(value: f32) -> String {
    let sign = if value >= 0.0 { "+" } else { "-" };
    format!("{}{:.2}", dim().apply_to(sign), value.abs())
}

pub fn log_init(hostname: &str, port: u16, backfill_enabled: bool) {
    println!(
        "{} starting devlogs-feed on {}...",
        init_prefix(),
        cyan().apply_to(format!("{hostname}:{port}")),
    );
    println!(
        "{} backfill is {}.",
        init_prefix(),
        if backfill_enabled {
            green().apply_to("enabled")
        } else {
            yellow().apply_to("disabled")
        }
    );
}

pub fn log_ml_loading() {
    println!("{} loading models...", ml_prefix());
}

pub fn log_ml_ready() {
    println!("{} models ready!", ml_prefix());
}

pub fn log_feed_served(count: usize, cursor: Option<&String>) {
    let cursor_info = match cursor {
        Some(c) => format!(" (cursor: {})", dim().apply_to(c)),
        None => String::new(),
    };
    println!(
        "{} {} posts{}",
        cyan().apply_to("served"),
        bold().apply_to(count),
        cursor_info
    );
}

#[derive(Debug, Clone, Default)]
pub struct BackfillProgress {
    pub query: String,
    pub fetched: usize,
    pub processed: usize,
    pub accepted: usize,
}

pub fn log_backfill_start() {
    println!("{} starting backfill...", backfill_prefix());
}

pub fn log_backfill_auth_failed(error: &str) {
    println!(
        "{} {} {}",
        backfill_prefix(),
        red().apply_to("failed auth:"),
        dim().apply_to(error)
    );
}

pub fn log_backfill_query(query: &str, fetched: usize) {
    println!(
        "{} fetched {} {} posts",
        backfill_prefix(),
        bold().apply_to(fetched),
        cyan().apply_to(query),
    );
}

pub fn log_backfill_query_failed(query: &str, error: &str) {
    println!(
        "{}searching {}: {}",
        tree_branch(),
        cyan().apply_to(query),
        red().apply_to(error)
    );
}

pub fn log_backfill_stats(duplicates: usize, filtered: usize, no_relevance: usize) {
    println!("{} done.", backfill_prefix());
    println!("{}skipped:", tree_branch());
    println!(
        "{}{}{} {}",
        tree_indent(),
        tree_branch(),
        pad_label("duplicates", 2),
        dim().apply_to(duplicates)
    );
    println!(
        "{}{}{} {}",
        tree_indent(),
        tree_branch(),
        pad_label("filtered", 2),
        dim().apply_to(filtered)
    );
    println!(
        "{}{}{} {}",
        tree_indent(),
        tree_end(),
        pad_label("no relevance", 2),
        dim().apply_to(no_relevance)
    );
}

pub fn log_backfill_progress(current: usize, total: usize) {
    println!(
        "{} progress: {}{}",
        backfill_prefix(),
        bold().apply_to(current),
        dim().apply_to(format!("/{total}"))
    );
}

pub fn log_backfill_complete(total_accepted: usize, total_processed: usize) {
    println!(
        "{}{}/{} posts accepted",
        tree_end(),
        bold().apply_to(total_accepted),
        dim().apply_to(total_processed)
    );
}

#[derive(Debug, Clone)]
pub enum AssessmentResult {
    Rejected(String),
    NoRelevance,
    Accepted,
}

#[derive(Debug, Clone, Default)]
pub struct PostAssessment {
    pub text_preview: String,
    pub filter_result: Option<FilterResult>,
    pub has_keywords: bool,
    pub has_hashtags: bool,
    pub quality: Option<QualityAssessment>,
    pub content_signals: Option<ContentSignals>,
    pub media_info: Option<MediaInfo>,
    pub signals: Option<PrioritySignals>,
    pub priority: Option<PriorityBreakdown>,
    pub result: Option<AssessmentResult>,
}

impl PostAssessment {
    pub fn new(text: &str) -> Self {
        let preview = if text.chars().count() > 60 {
            format!("{}...", text.chars().take(57).collect::<String>())
        } else {
            text.to_string()
        };
        Self {
            text_preview: preview.replace('\n', " "),
            ..Default::default()
        }
    }

    pub fn set_filter_result(&mut self, result: FilterResult) {
        if let FilterResult::Reject(ref filter) = result {
            self.result = Some(AssessmentResult::Rejected(format_filter(filter)));
        }
        self.filter_result = Some(result);
    }

    pub fn set_relevance(&mut self, keywords: bool, hashtags: bool) {
        self.has_keywords = keywords;
        self.has_hashtags = hashtags;
        if !keywords && !hashtags {
            self.result = Some(AssessmentResult::NoRelevance);
        }
    }

    pub fn set_content(&mut self, signals: ContentSignals, media: MediaInfo) {
        self.content_signals = Some(signals);
        self.media_info = Some(media);
    }

    pub fn set_priority(
        &mut self,
        quality: QualityAssessment,
        signals: PrioritySignals,
        priority: PriorityBreakdown,
    ) {
        self.quality = Some(quality);
        self.signals = Some(signals);
        self.priority = Some(priority);
        if self.result.is_none() {
            self.result = Some(AssessmentResult::Accepted);
        }
    }

    pub fn reject_low_priority(&mut self) {
        self.result = Some(AssessmentResult::Rejected("low-priority".into()));
    }

    pub fn print(&self) {
        let mut lines: Vec<String> = Vec::new();

        let is_accepted = matches!(&self.result, Some(AssessmentResult::Accepted));

        lines.push(format!(
            "{} \"{}\"",
            magenta().apply_to(bold().apply_to("[POST ASSESSMENT]")),
            dim().apply_to(&self.text_preview)
        ));

        if let Some(ref filter_result) = self.filter_result {
            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("FILTERS")));

            let filter_str = match filter_result {
                FilterResult::Pass => format!("{}", green().apply_to("pass")),
                FilterResult::Reject(f) => {
                    format!("{} ({})", red().apply_to("reject"), format_filter(f))
                }
            };

            let kw_style = if self.has_keywords { green() } else { dim() };
            let ht_style = if self.has_hashtags { green() } else { dim() };

            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("status", 1),
                filter_str
            ));
            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("keywords", 1),
                kw_style.apply_to(self.has_keywords)
            ));
            lines.push(format!(
                "{}{} {}",
                tree_end(),
                pad_label("hashtags", 1),
                ht_style.apply_to(self.has_hashtags)
            ));
        }

        if let (Some(ref quality), Some(ref priority)) = (&self.quality, &self.priority) {
            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("QUALITY")));

            let s = settings();
            let bait_style =
                if quality.engagement_bait_score >= s.scoring.quality.poor_quality_penalty_min {
                    yellow()
                } else {
                    dim()
                };
            let synth_style =
                if quality.synthetic_score >= s.scoring.quality.poor_quality_penalty_min {
                    yellow()
                } else {
                    dim()
                };

            let auth_style =
                if quality.authenticity_score >= s.scoring.quality.good_quality_boost_min {
                    green()
                } else {
                    dim()
                };

            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("bait", 1),
                bait_style.apply_to(format!("{:.0}%", quality.engagement_bait_score * 100.0))
            ));
            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("synthetic", 1),
                synth_style.apply_to(format!("{:.0}%", quality.synthetic_score * 100.0))
            ));
            lines.push(format!(
                "{}{} {}",
                tree_end(),
                pad_label("authentic", 1),
                auth_style.apply_to(format!("{:.0}%", quality.authenticity_score * 100.0))
            ));

            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("PRIORITY")));

            lines.push(format!("{}{}", tree_branch(), pad_label("boosts", 1)));
            if priority.boost_reasons.is_empty() {
                lines.push(format!(
                    "{}{}{}",
                    tree_indent(),
                    tree_end(),
                    dim().apply_to("none")
                ));
            } else {
                let count = priority.boost_reasons.len();
                for (i, boost) in priority.boost_reasons.iter().enumerate() {
                    let branch = if i == count - 1 {
                        tree_end()
                    } else {
                        tree_branch()
                    };
                    lines.push(format!("{}{}{}", tree_indent(), branch, boost));
                }
            }

            lines.push(format!("{}{}", tree_branch(), pad_label("penalties", 1)));
            if priority.penalty_reasons.is_empty() {
                lines.push(format!(
                    "{}{}{}",
                    tree_indent(),
                    tree_end(),
                    dim().apply_to("none")
                ));
            } else {
                let count = priority.penalty_reasons.len();
                for (i, penalty) in priority.penalty_reasons.iter().enumerate() {
                    let branch = if i == count - 1 {
                        tree_end()
                    } else {
                        tree_branch()
                    };
                    lines.push(format!("{}{}{}", tree_indent(), branch, penalty));
                }
            }

            let total_boosts = priority.content_modifier.max(0.0)
                + priority.engagement_boost
                + priority.authenticity_boost;
            let total_penalties =
                priority.quality_penalty + priority.content_modifier.min(0.0).abs();

            lines.push(format!("{}{}", tree_branch(), pad_label("modifiers", 1)));
            lines.push(format!(
                "{}{}{}{}",
                tree_indent(),
                tree_branch(),
                pad_label("boosts", 2),
                format_signed(total_boosts)
            ));
            lines.push(format!(
                "{}{}{}{}",
                tree_indent(),
                tree_end(),
                pad_label("penalties", 2),
                format_signed(-total_penalties)
            ));

            lines.push(format!(
                "{}{}{}",
                tree_end(),
                pad_label("total", 1),
                format_signed(priority.priority),
            ));

            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("RESULT")));

            let status_str = if is_accepted { "ACCEPTED" } else { "REJECTED" };
            let status_style = if is_accepted {
                green().bold()
            } else {
                red().bold()
            };

            lines.push(format!(
                "{}{}{}",
                tree_branch(),
                pad_label("priority", 1),
                format_signed(priority.priority),
            ));
            lines.push(format!(
                "{}{} {}",
                tree_end(),
                pad_label("status", 1),
                status_style.apply_to(status_str)
            ));
        } else {
            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("result")));

            let status_str = if is_accepted { "accepted" } else { "rejected" };
            let status_style = if is_accepted {
                green().bold()
            } else {
                red().bold()
            };

            let reason = match &self.result {
                Some(AssessmentResult::Rejected(r)) => r.clone(),
                Some(AssessmentResult::NoRelevance) => "no relevant keywords/hashtags".into(),
                _ => "unknown".into(),
            };

            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("status", 1),
                status_style.apply_to(status_str)
            ));
            lines.push(format!(
                "{}{} {}",
                tree_end(),
                pad_label("reason", 1),
                dim().apply_to(reason)
            ));
        }

        println!("{}\n", lines.join("\n"));
    }
}

fn format_filter(filter: &Filter) -> String {
    match filter {
        Filter::BlockedKeyword(kw) => format!("{} ({})", filter, kw),
        Filter::BlockedHashtag(ht) => format!("{} ({})", filter, ht),
        Filter::TooManyHashtags(count) => format!("{} ({})", filter, count),
        _ => filter.to_string(),
    }
}

pub fn log_post_accepted(uri: &str, priority: f32) {
    println!(
        "{} post: {} (priority: {:.2})",
        green().apply_to("queued"),
        dim().apply_to(truncate_uri(uri)),
        bold().apply_to(priority)
    );
}

pub fn log_cleanup(deleted: usize) {
    if deleted > 0 {
        println!(
            "{} {} old entries",
            dim().apply_to("cleaned"),
            bold().apply_to(deleted)
        );
    }
}

pub fn log_flush(posts: usize, likes: usize) {
    if posts > 0 || likes > 0 {
        println!(
            "{} {} posts, {} likes",
            dim().apply_to("flushed"),
            bold().apply_to(posts),
            bold().apply_to(likes)
        );
    }
}

pub fn log_author_blocked(moderator_did: &str, author_did: &str, deleted_posts: usize) {
    println!(
        "{} {} blocked author {} ({} posts removed)",
        red().apply_to("[MOD]"),
        dim().apply_to(truncate_did(moderator_did)),
        bold().apply_to(truncate_did(author_did)),
        bold().apply_to(deleted_posts)
    );
}

pub fn log_influencer_accepted(author_did: &str) {
    println!(
        "{} post from {} (influencer bypass)",
        green().apply_to("accepted"),
        dim().apply_to(truncate_did(author_did))
    );
}

fn truncate_did(did: &str) -> String {
    if did.len() > 24 {
        format!("{}...", &did[..21])
    } else {
        did.to_string()
    }
}

pub fn log_interactions_received(user_did: &str, count: usize) {
    println!(
        "{} {} interactions from {}",
        cyan().apply_to("received"),
        bold().apply_to(count),
        dim().apply_to(truncate_did(user_did))
    );
}

fn truncate_uri(uri: &str) -> String {
    if let Some(rkey_start) = uri.rfind('/') {
        let rkey = &uri[rkey_start + 1..];
        if rkey.len() > 13 {
            format!(".../{}", rkey)
        } else {
            uri.to_string()
        }
    } else if uri.len() > 40 {
        format!("{}...", &uri[..37])
    } else {
        uri.to_string()
    }
}
