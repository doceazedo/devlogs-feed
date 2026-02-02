use crate::scoring::{
    priority::{ConfidenceTier, PriorityBreakdown, BONUS_FIRST_PERSON, BONUS_VIDEO},
    MLScores, REFERENCE_POSTS,
};
use colored::{ColoredString, Colorize};

fn tree(last: bool) -> &'static str {
    if last {
        "└─"
    } else {
        "├─"
    }
}
fn yes_no(val: bool) -> ColoredString {
    if val {
        "yes".green()
    } else {
        "no".red()
    }
}
fn yes_no_opt(val: bool) -> ColoredString {
    if val {
        "yes".green()
    } else {
        "no".yellow()
    }
}
fn pct(val: f32) -> String {
    format!("{:.0}%", val * 100.0)
}
fn signed_pct(val: f32) -> ColoredString {
    let s = format!("{:+.0}%", val * 100.0);
    if val > 0.0 {
        s.green()
    } else if val < 0.0 {
        s.yellow()
    } else {
        s.normal()
    }
}
fn separator() {
    println!(
        "{}",
        "─────────────────────────────────────────────────────────".dimmed()
    );
}

pub fn truncate_text(text: &str, max: usize) -> &str {
    text.char_indices()
        .nth(max)
        .map(|(i, _)| &text[..i])
        .unwrap_or(text)
}

pub fn log_header(text: &str) {
    println!("{}", text.bold());
}
pub fn log_newline() {
    println!();
}
pub fn log_dimmed(message: &str) {
    println!("  {}", message.dimmed());
}

pub fn log_startup_config(did: &str, host: &str, port: u16, db: &str, limit: usize) {
    println!("{}", "Game Dev Progress Feed".green().bold());
    println!("════════════════════════════════════════");
    for (l, v) in [("DID", did), ("Hostname", host), ("Database", db)] {
        println!("{} {}: {}", "[CONFIG]".cyan(), l, v);
    }
    println!(
        "{} Port: {}\n{} Firehose limit: {}\n",
        "[CONFIG]".cyan(),
        port,
        "[CONFIG]".cyan(),
        limit
    );
}

pub fn log_ml_step(message: &str) {
    println!("{} {message}", "[ML]".yellow());
}
pub fn log_ml_loading(step: &str) {
    log_ml_step(step);
}
pub fn log_ml_ready() {
    println!("{} Ready to score posts!", "[ML]".green());
}
pub fn log_ml_error(message: &str) {
    eprintln!("{} {message}", "[ML ERROR]".red());
}
pub fn log_ml_model_loaded(name: &str, secs: f32) {
    println!(
        "{} {name} loaded {}",
        "[ML]".yellow(),
        format!("({secs:.1}s)").dimmed()
    );
}
pub fn log_ml_step_done(message: &str, detail: &str) {
    println!(
        "{} {message} {}",
        "[ML]".yellow(),
        format!("({detail})").dimmed()
    );
}

pub fn log_db_status(message: &str) {
    println!("{} {}", "[DB]".cyan(), message);
}
pub fn log_db_ready() {
    println!("{} Connection pool ready", "[DB]".green());
}
pub fn log_db_error(message: &str) {
    eprintln!("{} {}", "[DB ERROR]".red(), message);
}
pub fn log_cleanup_done(deleted: usize) {
    println!("{} Cleaned up {} old posts", "[DB]".cyan(), deleted);
}

pub fn log_server_starting(port: u16) {
    println!(
        "\n{} Starting on port {}\n════════════════════════════════════════\n",
        "[SERVER]".green(),
        port.to_string().bold()
    );
}

pub fn log_feed_served(count: usize, total: usize, shuffle_applied: bool) {
    if count > 0 {
        let shuffle_info = if shuffle_applied { " (shuffled)" } else { "" };
        println!(
            "{} Served {} posts (from {} total){}",
            "[FEED]".blue(),
            count,
            total,
            shuffle_info.dimmed()
        );
    }
}
pub fn log_feed_error(message: &str) {
    eprintln!("{} {}", "[FEED]".blue(), message);
}
pub fn log_generic_error(tag: &str, message: &str) {
    println!("{} {message}", tag.red());
}

pub fn log_filter_rejected(text: &str, filter: &crate::scoring::filters::Filter) {
    let clean = text.replace('\n', " ");
    let preview = truncate_text(&clean, 80);
    println!(
        "{} [{}] \"{}\"",
        "FILTERED".red(),
        filter.to_string().yellow(),
        preview.dimmed()
    );
}

pub fn log_ml_filter_rejected(text: &str, label: &str, confidence: f32) {
    let clean = text.replace('\n', " ");
    let preview = truncate_text(&clean, 80);
    println!(
        "{} [ml-rejection] \"{}\" {} {}",
        "FILTERED".red(),
        preview.dimmed(),
        label.yellow(),
        format!("({:.0}%)", confidence * 100.0).dimmed()
    );
}

pub fn log_post_scored(text: &str, breakdown: &PriorityBreakdown) {
    let clean = text.replace('\n', " ");
    let preview = truncate_text(&clean, 80);

    let confidence_color = match breakdown.confidence {
        ConfidenceTier::Strong | ConfidenceTier::High => pct(breakdown.final_priority).green(),
        ConfidenceTier::Moderate => pct(breakdown.final_priority).yellow(),
        ConfidenceTier::Low => pct(breakdown.final_priority).red(),
    };

    println!(
        "{} [{}] \"{}\" {}",
        "SCORED".cyan(),
        breakdown.confidence.to_string().bold(),
        preview.dimmed(),
        confidence_color
    );

    if !breakdown.boost_reasons.is_empty() {
        println!(
            "  {} {}",
            "Boosts:".green(),
            breakdown.boost_reasons.join(", ")
        );
    }
    if !breakdown.penalty_reasons.is_empty() {
        println!(
            "  {} {}",
            "Penalties:".yellow(),
            breakdown.penalty_reasons.join(", ")
        );
    }
}

pub fn log_engagement_recorded(post_uri: &str, event_type: &str) {
    let uri_short: String = post_uri.chars().take(50).collect();
    println!(
        "{} {} on {}...",
        "[ENGAGEMENT]".magenta(),
        event_type,
        uri_short.dimmed()
    );
}

pub fn log_spammer_flagged(did: &str, reason: &str) {
    println!(
        "{} Flagged {} - {}",
        "[SPAM]".red().bold(),
        did.yellow(),
        reason
    );
}

pub fn log_backfill_start() {
    println!(
        "{} Starting backfill from Bluesky search...",
        "[BACKFILL]".cyan()
    );
}

pub fn log_backfill_progress(fetched: usize, scored: usize, accepted: usize) {
    println!(
        "{} Progress: {} fetched, {} scored, {} accepted",
        "[BACKFILL]".cyan(),
        fetched,
        scored,
        accepted
    );
}

pub fn log_backfill_done(accepted: usize) {
    println!(
        "{} Backfill complete: {} posts added",
        "[BACKFILL]".green(),
        accepted
    );
}

pub fn log_backfill_error(error: &str) {
    eprintln!("{} Error: {}", "[BACKFILL]".red(), error);
}

pub fn log_backfill_skipped(reason: &str) {
    println!("{} Skipped: {}", "[BACKFILL]".yellow(), reason);
}

pub fn log_fetch_start(at_uri: &str) {
    println!(
        "\n{} Fetching post from Bluesky...\n  {} AT URI: {}",
        "[FETCH]".yellow(),
        tree(false).dimmed(),
        at_uri.dimmed()
    );
}
pub fn log_fetch_result(err: Option<&str>) {
    let msg = match err {
        None => "Post fetched successfully".green().to_string(),
        Some(e) => format!("Error: {e}").red().to_string(),
    };
    println!("  {} {}", tree(true).dimmed(), msg);
}
pub fn log_fetch_success() {
    log_fetch_result(None);
}
pub fn log_fetch_error(error: &str) {
    log_fetch_result(Some(error));
}

pub fn log_post_header(text: &str, has_media: bool) {
    let preview = truncate_text(text, 300).replace('\n', " ");
    println!();
    separator();
    println!("{} {}", "Post:".bold(), format!("\"{}\"", preview).cyan());
    println!(
        "{}",
        format!(
            "[{} chars, media: {}]",
            text.len(),
            if has_media { "yes" } else { "no" }
        )
        .dimmed()
    );
}

pub fn log_prefilter_length(ok: bool, chars: usize, min: usize) {
    let (status, detail) = if ok {
        ("OK".green().to_string(), format!("({} chars)", chars))
    } else {
        (
            "TOO SHORT".red().to_string(),
            format!("({} chars < {})", chars, min),
        )
    };
    println!(
        "  {} Length:   {} {}",
        tree(false).dimmed(),
        status,
        detail.dimmed()
    );
}
pub fn log_prefilter_length_ok(chars: usize) {
    log_prefilter_length(true, chars, 0);
}
pub fn log_prefilter_length_fail(chars: usize, min: usize) {
    log_prefilter_length(false, chars, min);
}

pub fn log_prefilter_no_signals() {
    println!(
        "  {} Keywords: {}",
        tree(false).dimmed(),
        "NONE".red().bold()
    );
    println!(
        "  {} Hashtags: {}",
        tree(true).dimmed(),
        "NONE".red().bold()
    );
    println!();
    separator();
    println!(
        "{} - No gamedev keywords or hashtags",
        "REJECTED".red().bold()
    );
}

pub fn log_prefilter_signals(has_kw: bool, kw_count: usize, has_ht: bool, ht_count: usize) {
    let fmt = |found, count| {
        if found {
            format!("{} {}", "OK".green(), format!("({count})").dimmed())
        } else {
            "NONE".red().bold().to_string()
        }
    };
    println!(
        "  {} Keywords: {}",
        tree(false).dimmed(),
        fmt(has_kw, kw_count)
    );
    println!(
        "  {} Hashtags: {}",
        tree(true).dimmed(),
        fmt(has_ht, ht_count)
    );
}

pub fn log_inference_start() {
    println!("  {}", "└─ Running inference...".dimmed());
}

pub fn log_ml_scores(scores: &MLScores) {
    let best_ref = REFERENCE_POSTS
        .get(scores.best_reference_idx)
        .unwrap_or(&"?");
    let ref_preview: String = best_ref.chars().take(50).collect();

    log_header("ML scores:");
    println!(
        "{} Semantic:       {} {}",
        tree(false).dimmed(),
        pct(scores.semantic_score),
        format!("(match: \"{}...\")", ref_preview).dimmed()
    );

    if scores.is_negative_label {
        let marker = if scores.negative_rejection {
            " [REJECTED]".red().bold().to_string()
        } else {
            String::new()
        };
        println!(
            "{} Classification: {} {}{}",
            tree(false).dimmed(),
            pct(scores.best_label_score).red(),
            format!("(label: \"{}\")", scores.best_label).dimmed(),
            marker
        );
    } else {
        println!(
            "{} Classification: {} {}",
            tree(false).dimmed(),
            pct(scores.classification_score),
            format!(
                "(label: \"{}\" at {})",
                scores.best_label,
                pct(scores.best_label_score)
            )
            .dimmed()
        );
    }

    println!(
        "{} Quality:        bait={} synthetic={}",
        tree(true).dimmed(),
        pct(scores.quality.engagement_bait_score),
        pct(scores.quality.synthetic_score)
    );
}

pub fn log_content_signals(
    is_first_person: bool,
    images: u8,
    has_video: bool,
    has_alt_text: bool,
    link_count: u8,
    promo_link_count: u8,
) {
    println!("  {}", "Content signals".bold());
    println!(
        "  {} First person:    {} {}",
        tree(false).dimmed(),
        yes_no(is_first_person),
        format!("(+{})", pct(BONUS_FIRST_PERSON)).dimmed()
    );
    println!(
        "  {} Images:          {} (alt: {})",
        tree(false).dimmed(),
        images,
        yes_no_opt(has_alt_text)
    );
    println!(
        "  {} Video:           {} {}",
        tree(false).dimmed(),
        yes_no_opt(has_video),
        format!("(+{})", pct(BONUS_VIDEO)).dimmed()
    );
    println!(
        "  {} Links:           {} (promo: {})",
        tree(true).dimmed(),
        link_count,
        promo_link_count
    );
}

pub fn log_priority_breakdown(b: &PriorityBreakdown) {
    println!("  {}", "Priority breakdown".bold());
    println!(
        "  {} Topic score:     {}",
        tree(false).dimmed(),
        pct(b.topic_score)
    );
    println!(
        "  {} Quality penalty: {}",
        tree(false).dimmed(),
        signed_pct(-b.quality_penalty)
    );
    println!(
        "  {} Content mod:     {}",
        tree(false).dimmed(),
        signed_pct(b.content_modifier)
    );
    println!(
        "  {} Engagement:      {}",
        tree(false).dimmed(),
        signed_pct(b.engagement_boost)
    );
    println!(
        "  {} Label boost:     {}",
        tree(false).dimmed(),
        signed_pct(b.label_boost)
    );

    let priority_colored = match b.confidence {
        ConfidenceTier::Strong | ConfidenceTier::High => format!("{:.2}", b.final_priority).green(),
        ConfidenceTier::Moderate => format!("{:.2}", b.final_priority).yellow(),
        ConfidenceTier::Low => format!("{:.2}", b.final_priority).red(),
    };
    println!(
        "  {} {}       {}",
        tree(true).dimmed(),
        "Final:".bold(),
        priority_colored.bold()
    );
}

pub fn log_final_result(accepted: bool, b: &PriorityBreakdown) {
    println!();
    separator();
    let status = if accepted {
        "ACCEPTED".green().bold()
    } else {
        "REJECTED".red().bold()
    };
    println!(
        "{} [{}] {}",
        status,
        b.confidence.to_string().bold(),
        b.topic_label.dimmed()
    );

    if accepted {
        println!(
            "  {}",
            format!("Priority: {:.2}", b.final_priority).dimmed()
        );
        if !b.boost_reasons.is_empty() {
            println!("{} {}", "Boosts:".green(), b.boost_reasons.join(", "));
        }
    } else {
        println!(
            "  {}",
            format!("Priority {:.2} below threshold", b.final_priority).dimmed()
        );
        if !b.penalty_reasons.is_empty() {
            println!(
                "  {}",
                format!("Reasons: {}", b.penalty_reasons.join(", ")).dimmed()
            );
        }
    }
}

pub fn log_accepted_post(text: &str, b: &PriorityBreakdown, scores: &MLScores) {
    log_post_result(text, b, Some(scores), true);
}

pub fn log_rejected_post(text: &str, b: &PriorityBreakdown) {
    log_post_result(text, b, None, false);
}

fn log_post_result(text: &str, b: &PriorityBreakdown, scores: Option<&MLScores>, accepted: bool) {
    let clean = text.replace('\n', " ");
    let preview = truncate_text(&clean, 300);
    println!();
    separator();
    let status_fmt = if accepted {
        "ACCEPTED".green().bold()
    } else {
        "REJECTED".yellow().bold()
    };
    println!(
        "{} [{}] {} {}",
        status_fmt,
        b.confidence.to_string().bold(),
        b.topic_label.dimmed(),
        format!("[{} chars]", text.len()).dimmed()
    );
    let preview_fmt = if accepted {
        format!("\"{}\"", preview).cyan()
    } else {
        format!("\"{}\"", preview).dimmed()
    };
    println!("{}\n", preview_fmt);

    if accepted {
        println!(
            "  {} {:.2} {}",
            "Priority:".bold(),
            b.final_priority,
            format!(
                "(topic:{} content:{} engagement:{})",
                pct(b.topic_score),
                signed_pct(b.content_modifier),
                signed_pct(b.engagement_boost)
            )
            .dimmed()
        );
        if !b.boost_reasons.is_empty() {
            println!("{} {}", "Boosts:".green(), b.boost_reasons.join(", "));
        }
        if !b.penalty_reasons.is_empty() {
            println!("{} {}", "Penalties:".yellow(), b.penalty_reasons.join(", "));
        }
        if let Some(s) = scores {
            let best_ref = REFERENCE_POSTS.get(s.best_reference_idx).unwrap_or(&"?");
            let ref_short: String = best_ref.chars().take(40).collect();
            println!(
                "  {}",
                format!("ML: {} semantic match=\"{}...\"", s.best_label, ref_short).dimmed()
            );
        }
    } else {
        println!(
            "  {} {:.2} {}",
            "Priority:".dimmed(),
            b.final_priority,
            "(below threshold)".dimmed()
        );
        if !b.penalty_reasons.is_empty() {
            println!(
                "  {}",
                format!("Reason: {}", b.penalty_reasons.join(", ")).dimmed()
            );
        }
    }
}
