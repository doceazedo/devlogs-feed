use crate::scoring::{
    Bonus, ConfidenceTier, Filter, MLScores, PromoPenaltyBreakdown, ScoreBreakdown, ScoringSignals,
    MIN_FINAL_SCORE, REFERENCE_POSTS, WEIGHT_CLASSIFICATION, WEIGHT_HASHTAG, WEIGHT_KEYWORD,
    WEIGHT_SEMANTIC,
};
use colored::Colorize;

const CHECK: &str = "✓";
const CROSS: &str = "✗";
const ARROW: &str = "→";
const DOT: &str = "•";

pub fn log_header(text: &str) {
    println!("{}", text.bold());
}

pub fn log_separator() {
    println!(
        "{}",
        "─────────────────────────────────────────────────────────".dimmed()
    );
}

pub fn log_double_separator() {
    println!(
        "{}",
        "═════════════════════════════════════════════════════════".dimmed()
    );
}

pub fn log_value(label: &str, val: &str) {
    println!("  {} {}", format!("{}:", label).dimmed(), val);
}

pub fn log_success(text: &str) {
    println!("{} {}", CHECK.green(), text.green());
}

pub fn log_error(text: &str) {
    println!("{} {}", CROSS.red(), text.red());
}

pub fn log_warning(text: &str) {
    println!("{} {}", DOT.yellow(), text.yellow());
}

pub fn log_score(name: &str, val: f32) {
    let pct = (val * 100.0) as usize;
    let bar_len = pct / 5;
    let bar: String = "█".repeat(bar_len);
    let empty: String = "░".repeat(20 - bar_len);

    let colored_bar = if val >= 0.70 {
        bar.green()
    } else if val >= 0.60 {
        bar.yellow()
    } else {
        bar.red()
    };

    println!(
        "  {:<20} {} {}{} {:>3}%",
        name,
        ARROW.dimmed(),
        colored_bar,
        empty.dimmed(),
        pct
    );
}

pub fn log_startup_config(did: &str, host: &str, port: u16, db: &str, limit: usize) {
    println!("{}", "Game Dev Progress Feed".green().bold());
    println!("════════════════════════════════════════");
    println!("{} DID: {}", "[CONFIG]".cyan(), did);
    println!("{} Hostname: {}", "[CONFIG]".cyan(), host);
    println!("{} Port: {}", "[CONFIG]".cyan(), port);
    println!("{} Firehose limit: {}", "[CONFIG]".cyan(), limit);
    println!("{} Database: {}", "[CONFIG]".cyan(), db);
    println!();
}

pub fn log_ml_loading(step: &str) {
    println!("{} {}", "[ML]".yellow(), step);
}

pub fn log_ml_ready() {
    println!("{} Ready to score posts!", "[ML]".green());
}

pub fn log_ml_model_loaded(name: &str, duration_secs: f32) {
    println!(
        "{} {name} loaded {}",
        "[ML]".yellow(),
        format!("({duration_secs:.1}s)").dimmed()
    );
}

pub fn log_ml_step(message: &str) {
    println!("{} {message}", "[ML]".yellow());
}

pub fn log_ml_step_done(message: &str, detail: &str) {
    println!(
        "{} {message} {}",
        "[ML]".yellow(),
        format!("({detail})").dimmed()
    );
}

pub fn log_ml_shutdown() {
    println!("{} Worker shutting down", "[ML]".yellow());
}

pub fn log_ml_error(message: &str) {
    eprintln!("{} {message}", "[ML ERROR]".red());
}

pub fn log_db_status(message: &str) {
    println!("{} {}", "[DB]".cyan(), message);
}

pub fn log_db_ready() {
    println!("{} Connection pool ready", "[DB]".green());
}

pub fn log_server_starting(port: u16) {
    println!();
    println!(
        "{} Starting on port {}",
        "[SERVER]".green(),
        port.to_string().bold()
    );
    println!("════════════════════════════════════════");
    println!();
}

pub fn log_filter_failed(filter: Filter, details: Option<&str>) {
    let reason = match filter {
        Filter::MinLength => "Text too short",
        Filter::EnglishOnly => "Non-English content",
        Filter::HasGamedevSignals => "No gamedev keywords or hashtags",
        Filter::NegativeClassification => "Negative classification",
        Filter::MinScore => "Score below threshold",
    };

    if let Some(d) = details {
        println!(
            "{} {} {}",
            CROSS.red(),
            reason.red(),
            format!("({})", d).dimmed()
        );
    } else {
        println!("{} {}", CROSS.red(), reason.red());
    }
}

pub fn log_bonus_applied(bonus: Bonus, val: f32) {
    let label = match bonus {
        Bonus::Keyword => "Keywords",
        Bonus::Hashtag => "Hashtags",
        Bonus::Semantic => "Semantic",
        Bonus::Classification => "Classification",
        Bonus::FirstPerson => "First person",
        Bonus::HasMedia => "Has media",
        Bonus::PromoPenalty => "Promo penalty",
    };

    let sign = if val >= 0.0 { "+" } else { "" };
    let formatted = format!("{}{:.0}%", sign, val * 100.0);

    let colored = if val > 0.0 {
        formatted.green()
    } else if val < 0.0 {
        formatted.red()
    } else {
        formatted.normal()
    };

    println!("  {} {} {}", DOT.dimmed(), label, colored);
}

pub fn log_scoring_result(breakdown: &ScoreBreakdown) {
    log_double_separator();

    let keyword_str = if breakdown.has_keywords {
        format!("yes({})", breakdown.keyword_count)
            .green()
            .to_string()
    } else {
        "no".red().to_string()
    };
    let hashtag_str = if breakdown.has_hashtags {
        format!("yes({})", breakdown.hashtag_count)
            .green()
            .to_string()
    } else {
        "no".red().to_string()
    };

    log_header("Relevance signals:");
    println!(
        "  {} Keywords:       {} {}",
        "├─".dimmed(),
        keyword_str,
        format!("(weight: {:.0}%)", WEIGHT_KEYWORD * 100.0).dimmed()
    );
    println!(
        "  {} Hashtags:       {} {}",
        "├─".dimmed(),
        hashtag_str,
        format!("(weight: {:.0}%)", WEIGHT_HASHTAG * 100.0).dimmed()
    );
    println!(
        "  {} Semantic:       {:.0}% {}",
        "├─".dimmed(),
        breakdown.semantic_score * 100.0,
        format!("(weight: {:.0}%)", WEIGHT_SEMANTIC * 100.0).dimmed()
    );
    println!(
        "  {} Classification: {:.0}% {}",
        "└─".dimmed(),
        breakdown.classification_score * 100.0,
        format!("(weight: {:.0}%)", WEIGHT_CLASSIFICATION * 100.0).dimmed()
    );
    println!();

    log_header("Authenticity signals:");
    println!(
        "  {} First person:   {} {}",
        "├─".dimmed(),
        if breakdown.is_first_person {
            "yes".green().to_string()
        } else {
            "no".red().to_string()
        },
        "(+15% if yes)".dimmed()
    );
    println!(
        "  {} Has media:      {} {}",
        "├─".dimmed(),
        if breakdown.has_media {
            "yes".green().to_string()
        } else {
            "no".yellow().to_string()
        },
        "(+10% if yes)".dimmed()
    );
    println!(
        "  {} Promo penalty:  {}",
        "└─".dimmed(),
        if breakdown.promo_penalty > 0.0 {
            format!("-{:.0}%", breakdown.promo_penalty * 50.0)
                .yellow()
                .to_string()
        } else {
            "-0%".to_string()
        }
    );
    println!();

    log_header("Final score:");
    println!(
        "  {} Relevance:     {:.0}%",
        "├─".dimmed(),
        breakdown.relevance_score * 100.0
    );
    println!(
        "  {} Authenticity: {:+.0}%",
        "├─".dimmed(),
        breakdown.authenticity_modifier * 100.0
    );
    println!(
        "  {} Base score:    {:.0}%",
        "├─".dimmed(),
        breakdown.base_score * 100.0
    );
    println!(
        "  {} Type:          {} {}",
        "├─".dimmed(),
        breakdown.classification_label,
        format!("(x{:.1})", breakdown.label_multiplier).dimmed()
    );

    let score_str = format!("{:.0}%", breakdown.final_score * 100.0);
    let colored_score = match breakdown.confidence {
        ConfidenceTier::Strong | ConfidenceTier::High => score_str.green(),
        ConfidenceTier::Moderate => score_str.yellow(),
        ConfidenceTier::Low => score_str.red(),
    };
    println!(
        "  {} {} {} {}",
        "├─".dimmed(),
        "Final score:".bold(),
        colored_score.bold(),
        format!("(threshold: {:.0}%)", MIN_FINAL_SCORE * 100.0).dimmed()
    );
    println!(
        "  {} {} {:.2}",
        "└─".dimmed(),
        "Priority:".bold(),
        breakdown.priority
    );
}

pub fn log_final_result(accepted: bool, breakdown: &ScoreBreakdown) {
    println!();
    log_separator();

    if accepted {
        println!(
            "{} [{}] {}",
            "ACCEPTED".green().bold(),
            breakdown.confidence.label().bold(),
            breakdown.classification_label.dimmed()
        );
        println!(
            "  {}",
            format!(
                "Final score {:.0}% >= {:.0}% threshold",
                breakdown.final_score * 100.0,
                MIN_FINAL_SCORE * 100.0
            )
            .dimmed()
        );

        if !breakdown.boost_reasons.is_empty() {
            println!(
                "{} {}",
                "Boosts:".green(),
                breakdown.boost_reasons.join(", ")
            );
        }
    } else {
        println!(
            "{} [{}] {}",
            "REJECTED".red().bold(),
            breakdown.confidence.label().bold(),
            breakdown.classification_label.dimmed()
        );

        if breakdown.negative_rejection {
            println!(
                "  {}",
                format!(
                    "Negative classification: \"{}\" at {:.0}%",
                    breakdown.negative_label,
                    breakdown.negative_label_score * 100.0
                )
                .red()
            );
        } else {
            println!(
                "  {}",
                format!(
                    "Final score {:.0}% < {:.0}% threshold",
                    breakdown.final_score * 100.0,
                    MIN_FINAL_SCORE * 100.0
                )
                .dimmed()
            );
        }

        if !breakdown.nerf_reasons.is_empty() {
            println!(
                "  {}",
                format!("Reasons: {}", breakdown.nerf_reasons.join(", ")).dimmed()
            );
        }
    }
}

pub fn log_post_header(text: &str, has_media: bool) {
    let text_preview = truncate_text(text, 300);
    let text_clean = text_preview.replace('\n', " ");

    println!();
    log_double_separator();
    println!(
        "{} {}",
        "Post:".bold(),
        format!("\"{}\"", text_clean).cyan()
    );
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

pub fn log_ml_scores(scores: &MLScores) {
    let best_ref = REFERENCE_POSTS
        .get(scores.best_reference_idx)
        .unwrap_or(&"?");
    let best_ref_truncated: String = best_ref.chars().take(50).collect();

    log_header("ML scores:");
    println!(
        "{} Semantic:       {:.0}% {}",
        "├─".dimmed(),
        scores.semantic_score * 100.0,
        format!("(match: \"{}...\")", best_ref_truncated).dimmed()
    );

    if scores.is_negative_label {
        let rejection_marker = if scores.negative_rejection {
            " [REJECTED]".red().bold().to_string()
        } else {
            String::new()
        };

        println!(
            "{} Classification: {} {}{}",
            "└─".dimmed(),
            format!("{:.0}%", scores.best_label_score * 100.0).red(),
            format!("(label: \"{}\")", scores.best_label).dimmed(),
            rejection_marker
        );
    } else {
        println!(
            "{} Classification: {:.0}% {}",
            "└─".dimmed(),
            scores.classification_score * 100.0,
            format!(
                "(label: \"{}\" at {:.0}%)",
                scores.best_label,
                scores.best_label_score * 100.0
            )
            .dimmed()
        );
    }
}

pub fn log_relevance_signals(signals: &ScoringSignals) {
    let keyword_display = if signals.has_keywords {
        format!(
            "{} {}",
            "yes".green(),
            format!("({})", signals.keyword_count).dimmed()
        )
    } else {
        "no".red().to_string()
    };
    let hashtag_display = if signals.has_hashtags {
        format!(
            "{} {}",
            "yes".green(),
            format!("({})", signals.hashtag_count).dimmed()
        )
    } else {
        "no".red().to_string()
    };

    log_header("Relevance signals:");
    println!(
        "  {} Keywords:       {} {}",
        "├─".dimmed(),
        keyword_display,
        format!("(weight: {:.0}%)", WEIGHT_KEYWORD * 100.0).dimmed()
    );
    println!(
        "  {} Hashtags:       {} {}",
        "├─".dimmed(),
        hashtag_display,
        format!("(weight: {:.0}%)", WEIGHT_HASHTAG * 100.0).dimmed()
    );
    println!(
        "  {} Semantic:       {:.0}% {}",
        "├─".dimmed(),
        signals.semantic_score * 100.0,
        format!("(weight: {:.0}%)", WEIGHT_SEMANTIC * 100.0).dimmed()
    );
    println!(
        "  {} Classification: {:.0}% {}",
        "└─".dimmed(),
        signals.classification_score * 100.0,
        format!("(weight: {:.0}%)", WEIGHT_CLASSIFICATION * 100.0).dimmed()
    );
}

pub fn log_authenticity_signals(signals: &ScoringSignals) {
    log_header("Authenticity signals:");
    println!(
        "  {} First person:    {} {}",
        "├─".dimmed(),
        if signals.is_first_person {
            "yes".green().to_string()
        } else {
            "no".red().to_string()
        },
        "(+15% if yes)".dimmed()
    );
    println!(
        "  {} Has media:       {} {}",
        "├─".dimmed(),
        if signals.has_media {
            "yes".green().to_string()
        } else {
            "no".yellow().to_string()
        },
        "(+10% if yes)".dimmed()
    );

    let penalty_pct = signals.promo_penalty * 50.0;
    if signals.promo_penalty > 0.0 {
        println!(
            "  {} Promo penalty:  {}",
            "└─".dimmed(),
            format!("-{:.0}%", penalty_pct).yellow()
        );
        log_promo_breakdown(&signals.promo_breakdown);
    } else {
        println!("  {} Promo penalty:  -0%", "└─".dimmed());
    }
}

pub fn log_promo_breakdown(breakdown: &PromoPenaltyBreakdown) {
    if breakdown.keyword_count > 0 {
        println!(
            "       {} Keywords:    {}",
            "├─".dimmed(),
            breakdown.keyword_count.to_string().yellow()
        );
    } else {
        println!("       {} Keywords:   0", "├─".dimmed());
    }

    if breakdown.marketing_hashtag_count > 0 {
        let hashtags = breakdown.marketing_hashtags_found.join(", ");
        println!(
            "       {} Hashtags:   {} {}",
            "├─".dimmed(),
            breakdown.marketing_hashtag_count.to_string().yellow(),
            format!("({})", hashtags).dimmed()
        );
    } else {
        println!("       {} Hashtags:   0", "├─".dimmed());
    }

    if breakdown.has_link {
        let link_type = if breakdown.promo_link_count > 0 {
            format!(
                "promo ({} domain{})",
                breakdown.promo_link_count,
                if breakdown.promo_link_count > 1 {
                    "s"
                } else {
                    ""
                }
            )
            .red()
            .to_string()
        } else {
            "other".to_string()
        };
        println!(
            "       {} Link:       {} {}",
            "└─".dimmed(),
            "yes".yellow(),
            format!("({})", link_type).dimmed()
        );
    } else {
        println!("       {} Link:       no", "└─".dimmed());
    }
}

pub fn log_score_breakdown(breakdown: &ScoreBreakdown) {
    println!(
        "{} Relevance:     {:.0}%",
        "├─".dimmed(),
        breakdown.relevance_score * 100.0
    );
    println!(
        "{} Authenticity: {:+.0}%",
        "├─".dimmed(),
        breakdown.authenticity_modifier * 100.0
    );
    println!(
        "{} Base score:    {:.0}%",
        "├─".dimmed(),
        breakdown.base_score * 100.0
    );
    println!(
        "{} Type:          {} {}",
        "├─".dimmed(),
        breakdown.classification_label,
        format!("(x{:.1})", breakdown.label_multiplier).dimmed()
    );

    let score_str = format!("{:.0}%", breakdown.final_score * 100.0);
    let colored_score = match breakdown.confidence {
        ConfidenceTier::Strong | ConfidenceTier::High => score_str.green(),
        ConfidenceTier::Moderate => score_str.yellow(),
        ConfidenceTier::Low => score_str.red(),
    };
    println!(
        "{} {} {} {}",
        "├─".dimmed(),
        "Final score:".bold(),
        colored_score.bold(),
        format!("(threshold: {:.0}%)", MIN_FINAL_SCORE * 100.0).dimmed()
    );
    println!(
        "{} {} {:.2}",
        "└─".dimmed(),
        "Priority:".bold(),
        breakdown.priority
    );
}

pub fn log_boost_nerf_reasons(breakdown: &ScoreBreakdown) {
    if !breakdown.boost_reasons.is_empty() {
        println!(
            "{} {}",
            "Boosts:".green(),
            breakdown.boost_reasons.join(", ")
        );
    }
    if !breakdown.nerf_reasons.is_empty() {
        println!(
            "{} {}",
            "Nerfs:".yellow(),
            breakdown.nerf_reasons.join(", ")
        );
    }
}

pub fn truncate_text(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((idx, _)) => &text[..idx],
        None => text,
    }
}

pub fn log_accepted_post(text: &str, breakdown: &ScoreBreakdown, scores: &MLScores) {
    let text_clean = text.replace('\n', " ");
    let text_preview = truncate_text(&text_clean, 300);
    let best_ref = REFERENCE_POSTS
        .get(scores.best_reference_idx)
        .unwrap_or(&"?");
    let best_ref_short: String = best_ref.chars().take(40).collect();

    let keyword_str = if breakdown.has_keywords {
        format!("yes({})", breakdown.keyword_count)
    } else {
        "no".to_string()
    };
    let hashtag_str = if breakdown.has_hashtags {
        format!("yes({})", breakdown.hashtag_count)
    } else {
        "no".to_string()
    };

    println!();
    log_separator();
    println!(
        "{} [{}] {} {}",
        "ACCEPTED".green().bold(),
        breakdown.confidence.label().bold(),
        breakdown.classification_label.dimmed(),
        format!("[{} chars]", text.len()).dimmed()
    );
    println!("{}", format!("\"{}\"", text_preview).cyan());
    println!();
    println!(
        "  {} {}",
        "Final Score:".bold(),
        format!("{:.0}%", breakdown.final_score * 100.0)
            .green()
            .bold()
    );
    println!(
        "  {}",
        format!(
            "├─ Relevance:     {:.0}% (keywords:{} hashtags:{} sem:{:.0}% cls:{:.0}%)",
            breakdown.relevance_score * 100.0,
            keyword_str,
            hashtag_str,
            breakdown.semantic_score * 100.0,
            breakdown.classification_score * 100.0
        )
        .dimmed()
    );
    println!(
        "  {}",
        format!(
            "├─ Authenticity: {:+.0}% (voice:{} media:{} promo:-{:.0}%)",
            breakdown.authenticity_modifier * 100.0,
            if breakdown.is_first_person {
                "yes"
            } else {
                "no"
            },
            if breakdown.has_media { "yes" } else { "no" },
            breakdown.promo_penalty * 50.0
        )
        .dimmed()
    );
    println!(
        "  {}",
        format!(
            "└─ Type Boost:   x{:.1} ({})",
            breakdown.label_multiplier, breakdown.classification_label
        )
        .dimmed()
    );
    println!();
    log_boost_nerf_reasons(breakdown);
    println!(
        "  {}",
        format!(
            "ML: classification={} semantic match=\"{}...\"",
            scores.best_label, best_ref_short
        )
        .dimmed()
    );
}

pub fn log_rejected_post(text: &str, breakdown: &ScoreBreakdown) {
    let text_clean = text.replace('\n', " ");
    let text_preview = truncate_text(&text_clean, 300);

    println!();
    log_separator();
    println!(
        "{} [{}] {} {}",
        "REJECTED".yellow().bold(),
        breakdown.confidence.label().bold(),
        breakdown.classification_label.dimmed(),
        format!("[{} chars]", text.len()).dimmed()
    );
    println!("{}", format!("\"{}\"", text_preview).dimmed());
    println!();
    println!(
        "  {} {} {}",
        "Final Score:".dimmed(),
        format!("{:.0}%", breakdown.final_score * 100.0)
            .red()
            .bold(),
        format!("(need {:.0}%)", MIN_FINAL_SCORE * 100.0).dimmed()
    );
    if !breakdown.nerf_reasons.is_empty() {
        println!(
            "  {}",
            format!("Reason: {}", breakdown.nerf_reasons.join(", ")).dimmed()
        );
    }
}

pub fn log_feed_served(count: usize, total: usize) {
    if count > 0 {
        println!(
            "{} Served {} posts (from {} total)",
            "[FEED]".blue(),
            count,
            total
        );
    }
}

pub fn log_feed_error(message: &str) {
    eprintln!("{} {}", "[FEED]".blue(), message);
}

pub fn log_db_error(message: &str) {
    eprintln!("{} {}", "[DB ERROR]".red(), message);
}

pub fn log_cleanup_done(deleted: usize) {
    println!("{} Cleaned up {} old posts", "[DB]".cyan(), deleted);
}

pub fn log_newline() {
    println!();
}

pub fn log_prompt(text: &str) {
    println!("{}", text.cyan());
}

pub fn log_fetch_start(at_uri: &str) {
    println!();
    println!("{} Fetching post from Bluesky...", "[FETCH]".yellow());
    println!("  {} AT URI: {}", "├─".dimmed(), at_uri.dimmed());
}

pub fn log_fetch_success() {
    println!(
        "  {} {}",
        "└─".dimmed(),
        "Post fetched successfully".green()
    );
}

pub fn log_fetch_error(error: &str) {
    println!("  {} {}", "└─".dimmed(), format!("Error: {error}").red());
}

pub fn log_generic_error(tag: &str, message: &str) {
    println!("{} {message}", tag.red());
}

pub fn log_ml_loading_binary() {
    println!();
    println!("{} Loading models...", "[ML]".yellow());
    println!("  {}", "└─ This may take a while on first run".dimmed());
}

pub fn log_prefilter_length_fail(chars: usize, min: usize) {
    println!(
        "  {} Length:   {} {}",
        "├─".dimmed(),
        "TOO SHORT".red(),
        format!("({} chars < {})", chars, min).dimmed()
    );
}

pub fn log_prefilter_length_ok(chars: usize) {
    println!(
        "  {} Length:   {} {}",
        "├─".dimmed(),
        "OK".green(),
        format!("({} chars)", chars).dimmed()
    );
}

pub fn log_prefilter_no_signals() {
    println!("  {} Keywords: {}", "├─".dimmed(), "NONE".red().bold());
    println!("  {} Hashtags: {}", "└─".dimmed(), "NONE".red().bold());
    println!();
    log_separator();
    println!(
        "{} - No gamedev keywords or hashtags",
        "REJECTED".red().bold()
    );
}

pub fn log_prefilter_signals(
    found_keywords: bool,
    keyword_count: usize,
    found_hashtags: bool,
    hashtag_count: usize,
) {
    println!(
        "  {} Keywords: {}",
        "├─".dimmed(),
        if found_keywords {
            format!("{} {}", "OK".green(), format!("({keyword_count})").dimmed())
        } else {
            "NONE".red().bold().to_string()
        }
    );
    println!(
        "  {} Hashtags: {}",
        "└─".dimmed(),
        if found_hashtags {
            format!("{} {}", "OK".green(), format!("({hashtag_count})").dimmed())
        } else {
            "NONE".red().bold().to_string()
        }
    );
}

pub fn log_inference_start() {
    println!("  {}", "└─ Running inference...".dimmed());
}

pub fn log_dimmed(message: &str) {
    println!("  {}", message.dimmed());
}

pub fn log_prefilter_signal(prefix: &str, label: &str, found: bool, count: Option<usize>) {
    let value = if found {
        match count {
            Some(c) => format!("{} {}", "OK".green(), format!("({c})").dimmed()),
            None => "OK".green().to_string(),
        }
    } else {
        "NONE".red().bold().to_string()
    };
    println!("  {} {}: {}", prefix.dimmed(), label, value);
}

pub fn log_rejection_no_signals() {
    log_separator();
    println!(
        "{} - No gamedev keywords or hashtags",
        "REJECTED".red().bold()
    );
}
