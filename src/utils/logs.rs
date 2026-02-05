use console::{measure_text_width, Style};

use crate::scoring::{
    ContentSignals, Filter, FilterResult, MLScores, MediaInfo, PriorityBreakdown, PrioritySignals,
    ScoreBreakdown,
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

pub fn log_backfill_stats(
    duplicates: usize,
    filtered: usize,
    no_relevance: usize,
    ml_rejected: usize,
    below_threshold: usize,
) {
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
        tree_branch(),
        pad_label("no relevance", 2),
        dim().apply_to(no_relevance)
    );
    println!(
        "{}{}{} {}",
        tree_indent(),
        tree_branch(),
        pad_label("ml rejected", 2),
        dim().apply_to(ml_rejected)
    );
    println!(
        "{}{}{} {}",
        tree_indent(),
        tree_end(),
        pad_label("below threshold", 2),
        dim().apply_to(below_threshold)
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
    MlRejected(String),
    BelowThreshold(f32),
    Accepted(f32),
}

#[derive(Debug, Clone, Default)]
pub struct PostAssessment {
    pub text_preview: String,
    pub filter_result: Option<FilterResult>,
    pub has_keywords: bool,
    pub has_hashtags: bool,
    pub ml_scores: Option<MLScores>,
    pub content_signals: Option<ContentSignals>,
    pub media_info: Option<MediaInfo>,
    pub score: Option<ScoreBreakdown>,
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

    pub fn set_ml_scores(&mut self, scores: MLScores) {
        if scores.negative_rejection {
            self.result = Some(AssessmentResult::MlRejected(scores.best_label.clone()));
        }
        self.ml_scores = Some(scores);
    }

    pub fn set_content(&mut self, signals: ContentSignals, media: MediaInfo) {
        self.content_signals = Some(signals);
        self.media_info = Some(media);
    }

    pub fn set_score_and_priority(
        &mut self,
        score: ScoreBreakdown,
        signals: PrioritySignals,
        priority: PriorityBreakdown,
    ) {
        self.score = Some(score);
        self.signals = Some(signals);
        self.priority = Some(priority);
    }

    pub fn set_threshold_result(&mut self, passed: bool) {
        if self.result.is_none() {
            let final_score = self.score.as_ref().map(|s| s.final_score).unwrap_or(0.0);
            self.result = Some(if passed {
                AssessmentResult::Accepted(final_score)
            } else {
                AssessmentResult::BelowThreshold(final_score)
            });
        }
    }

    pub fn print(&self) {
        let mut lines: Vec<String> = Vec::new();

        let is_accepted = matches!(&self.result, Some(AssessmentResult::Accepted(_)));

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
                if self.ml_scores.is_some() {
                    tree_branch()
                } else {
                    tree_end()
                },
                pad_label("hashtags", 1),
                ht_style.apply_to(self.has_hashtags)
            ));
        }

        if let (Some(ref ml), Some(ref score), Some(ref priority)) =
            (&self.ml_scores, &self.score, &self.priority)
        {
            let label_style = if ml.is_negative_label { red() } else { green() };
            lines.push(format!(
                "{}{} {} {}",
                tree_end(),
                pad_label("topic", 1),
                label_style.apply_to(&ml.best_label),
                dim().apply_to(format!("({:.0}%)", ml.best_label_score * 100.0))
            ));

            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("SCORE")));
            let s = settings();
            lines.push(format!(
                "{}{} {:.0}% {}",
                tree_branch(),
                pad_label("topic", 1),
                score.classification_score * 100.0,
                dim().apply_to(format!("(weight: {:.1})", s.scoring.weights.topic))
            ));
            lines.push(format!(
                "{}{} {:.0}% {} {}",
                tree_branch(),
                pad_label("semantic", 1),
                score.semantic_score * 100.0,
                dim().apply_to(format!("(idx: {})", ml.best_reference_idx)),
                dim().apply_to(format!("(weight: {:.1})", s.scoring.weights.semantic))
            ));
            let threshold_style = if score.final_score >= s.scoring.thresholds.score {
                green().dim()
            } else {
                red().dim()
            };
            let score_str = format!(
                "{} {}",
                bold().apply_to(format!("{:.2}", score.final_score)),
                threshold_style.apply_to(if score.final_score >= s.scoring.thresholds.score {
                    format!("(>={})", s.scoring.thresholds.score)
                } else {
                    format!("(<{})", s.scoring.thresholds.score)
                })
            );
            lines.push(format!(
                "{}{} {}",
                tree_end(),
                pad_label("total", 1),
                score_str
            ));

            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("QUALITY")));

            let quality = &ml.quality;
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
                + priority.authenticity_boost
                + priority.label_boost.max(0.0);
            let total_penalties = priority.quality_penalty
                + priority.content_modifier.min(0.0).abs()
                + priority.label_boost.min(0.0).abs();

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
                format_signed(priority.final_priority),
            ));

            lines.push(String::new());
            lines.push(format!("{}", bold().apply_to("RESULT")));

            let conf_str = priority.confidence.to_string().to_lowercase();
            let conf_desc = match conf_str.as_str() {
                "strong" => "[strong] definitely gamedev",
                "high" => "[high] likely gamedev",
                "moderate" => "[moderate] maybe gamedev",
                _ => "[low] not gamedev",
            };
            let conf_style = match conf_str.as_str() {
                "strong" => green(),
                "high" => cyan(),
                "moderate" => yellow(),
                _ => red(),
            };

            let status_str = if is_accepted { "ACCEPTED" } else { "REJECTED" };
            let status_style = if is_accepted {
                green().bold()
            } else {
                red().bold()
            };

            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("score", 1),
                score_str
            ));
            lines.push(format!(
                "{}{}{}",
                tree_branch(),
                pad_label("priority", 1),
                format_signed(priority.final_priority),
            ));
            lines.push(format!(
                "{}{} {}",
                tree_branch(),
                pad_label("confidence", 1),
                conf_style.apply_to(conf_desc)
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
                Some(AssessmentResult::MlRejected(label)) => {
                    format!("ml classification ({})", label)
                }
                Some(AssessmentResult::BelowThreshold(p)) => {
                    format!(
                        "below threshold ({:.2} < {})",
                        p,
                        settings().scoring.thresholds.score
                    )
                }
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
        Filter::HighConfidenceNegative(label) => format!("{} ({})", filter, label),
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

pub fn log_interactions_received(user_did: &str, count: usize) {
    let truncated_did = if user_did.len() > 24 {
        format!("{}...", &user_did[..21])
    } else {
        user_did.to_string()
    };
    println!(
        "{} {} interactions from {}",
        cyan().apply_to("received"),
        bold().apply_to(count),
        dim().apply_to(truncated_did)
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
