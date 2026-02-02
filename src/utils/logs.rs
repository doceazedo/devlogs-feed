use crate::scoring::{
    ConfidenceTier, MLScores, ScoreBreakdown, ScoringSignals, BONUS_FIRST_PERSON, BONUS_MEDIA,
    BONUS_VIDEO, MANY_IMAGES_THRESHOLD, MIN_FINAL_SCORE, PENALTY_MANY_IMAGES, REFERENCE_POSTS,
    WEIGHT_CLASSIFICATION, WEIGHT_HASHTAG, WEIGHT_KEYWORD, WEIGHT_SEMANTIC,
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

pub fn log_backfill_start() {
    println!("{} Searching for recent posts to backfill...", "[BACKFILL]".yellow());
}
pub fn log_backfill_done(count: usize) {
    if count > 0 {
        println!("{} Backfilled {} posts", "[BACKFILL]".green(), count);
    } else {
        println!("{} No new posts to backfill", "[BACKFILL]".green());
    }
}
pub fn log_backfill_error(message: &str) {
    eprintln!("{} {}", "[BACKFILL ERROR]".red(), message);
}

pub fn log_server_starting(port: u16) {
    println!(
        "\n{} Starting on port {}\n════════════════════════════════════════\n",
        "[SERVER]".green(),
        port.to_string().bold()
    );
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
pub fn log_generic_error(tag: &str, message: &str) {
    println!("{} {message}", tag.red());
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
            tree(true).dimmed(),
            pct(scores.best_label_score).red(),
            format!("(label: \"{}\")", scores.best_label).dimmed(),
            marker
        );
    } else {
        println!(
            "{} Classification: {} {}",
            tree(true).dimmed(),
            pct(scores.classification_score),
            format!(
                "(label: \"{}\" at {})",
                scores.best_label,
                pct(scores.best_label_score)
            )
            .dimmed()
        );
    }
}

pub fn log_relevance_signals(s: &ScoringSignals) {
    let kw = if s.has_keywords { 1.0 } else { 0.0 };
    let ht = if s.has_hashtags { 1.0 } else { 0.0 };
    let total = (kw * WEIGHT_KEYWORD
        + ht * WEIGHT_HASHTAG
        + s.semantic_score * WEIGHT_SEMANTIC
        + s.classification_score * WEIGHT_CLASSIFICATION)
        .min(1.0);
    let cnt = |has, c| {
        if has {
            format!("{} {}", yes_no(true), format!("({c})").dimmed())
        } else {
            yes_no(false).to_string()
        }
    };

    println!("  {}", "Relevance".bold());
    println!(
        "  {} Keywords:       {} {}",
        tree(false).dimmed(),
        cnt(s.has_keywords, s.keyword_count),
        format!("(×{})", pct(WEIGHT_KEYWORD)).dimmed()
    );
    println!(
        "  {} Hashtags:       {} {}",
        tree(false).dimmed(),
        cnt(s.has_hashtags, s.hashtag_count),
        format!("(×{})", pct(WEIGHT_HASHTAG)).dimmed()
    );
    println!(
        "  {} Semantic:       {} {}",
        tree(false).dimmed(),
        pct(s.semantic_score),
        format!("(×{})", pct(WEIGHT_SEMANTIC)).dimmed()
    );
    println!(
        "  {} Classification: {} {}",
        tree(false).dimmed(),
        pct(s.classification_score),
        format!("(×{})", pct(WEIGHT_CLASSIFICATION)).dimmed()
    );
    println!(
        "  {} {}            {}",
        tree(true).dimmed(),
        "Total:".bold(),
        pct(total)
    );
}

pub fn log_authenticity_signals(s: &ScoringSignals) {
    let auth = if s.is_first_person {
        BONUS_FIRST_PERSON
    } else {
        0.0
    } + if s.has_media { BONUS_MEDIA } else { 0.0 };
    println!("  {}", "Authenticity".bold());
    println!(
        "  {} First person:    {} {}",
        tree(false).dimmed(),
        yes_no(s.is_first_person),
        format!("(+{})", pct(BONUS_FIRST_PERSON)).dimmed()
    );
    println!(
        "  {} Has media:       {} {}",
        tree(false).dimmed(),
        yes_no_opt(s.has_media),
        format!("(+{})", pct(BONUS_MEDIA)).dimmed()
    );
    println!(
        "  {} {}            {}",
        tree(true).dimmed(),
        "Total:".bold(),
        signed_pct(auth)
    );
}

pub fn log_score_breakdown(b: &ScoreBreakdown) {
    println!(
        "  {} {}",
        "Score".bold(),
        "(relevance×75% + (50%+auth)×25%)".dimmed()
    );
    println!(
        "  {} Relevance:     {} × 75% = {}",
        tree(false).dimmed(),
        pct(b.relevance_score),
        pct(b.relevance_score * 0.75)
    );
    println!(
        "  {} Authenticity:  (50%{}) × 25% = {}",
        tree(false).dimmed(),
        signed_pct(b.authenticity_modifier),
        pct((0.5 + b.authenticity_modifier) * 0.25)
    );

    let score_colored = match b.confidence {
        ConfidenceTier::Strong | ConfidenceTier::High => pct(b.final_score).green(),
        ConfidenceTier::Moderate => pct(b.final_score).yellow(),
        ConfidenceTier::Low => pct(b.final_score).red(),
    };
    println!(
        "  {} {}         {} {}\n",
        tree(true).dimmed(),
        "Score:".bold(),
        score_colored.bold(),
        format!("(threshold: {})", pct(MIN_FINAL_SCORE)).dimmed()
    );

    println!(
        "  {} {}",
        "Priority".bold(),
        "(score + modifiers, for ranking)".dimmed()
    );

    let color_signed =
        |v: f32, pos: fn(String) -> ColoredString, neg: fn(String) -> ColoredString| {
            let s = format!("{:+.0}%", v * 100.0);
            if v > 0.0 {
                pos(s)
            } else if v < 0.0 {
                neg(s)
            } else {
                s.normal()
            }
        };
    let video = if b.has_video { BONUS_VIDEO } else { 0.0 };
    let imgs = if b.image_count >= MANY_IMAGES_THRESHOLD {
        -PENALTY_MANY_IMAGES
    } else {
        0.0
    };
    let promo = -b.promo_penalty * 0.5;
    let label = b.label_multiplier - 1.0;

    println!(
        "  {} Video:         {}",
        tree(false).dimmed(),
        color_signed(video, |s| s.green(), |s| s.normal())
    );
    println!(
        "  {} Many images:   {}",
        tree(false).dimmed(),
        color_signed(imgs, |s| s.normal(), |s| s.yellow())
    );
    println!(
        "  {} Promo:         {}",
        tree(false).dimmed(),
        color_signed(promo, |s| s.normal(), |s| s.yellow())
    );
    println!(
        "  {} Label:         {}",
        tree(false).dimmed(),
        color_signed(label, |s| s.green(), |s| s.red())
    );
    println!(
        "  {} {}       {:.2}",
        tree(true).dimmed(),
        "Priority:".bold(),
        b.priority
    );
}

pub fn log_boost_nerf_reasons(b: &ScoreBreakdown) {
    if !b.boost_reasons.is_empty() {
        println!("{} {}", "Boosts:".green(), b.boost_reasons.join(", "));
    }
    if !b.nerf_reasons.is_empty() {
        println!("{} {}", "Nerfs:".yellow(), b.nerf_reasons.join(", "));
    }
}

pub fn log_final_result(accepted: bool, b: &ScoreBreakdown) {
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
        b.confidence.label().bold(),
        b.classification_label.dimmed()
    );

    if accepted {
        println!(
            "  {}",
            format!(
                "Score {} >= {} threshold (priority: {:.2})",
                pct(b.final_score),
                pct(MIN_FINAL_SCORE),
                b.priority
            )
            .dimmed()
        );
        if !b.boost_reasons.is_empty() {
            println!("{} {}", "Boosts:".green(), b.boost_reasons.join(", "));
        }
    } else if b.negative_rejection {
        println!(
            "  {}",
            format!(
                "Negative classification: \"{}\" at {}",
                b.negative_label,
                pct(b.negative_label_score)
            )
            .red()
        );
    } else {
        println!(
            "  {}",
            format!(
                "Score {} < {} threshold",
                pct(b.final_score),
                pct(MIN_FINAL_SCORE)
            )
            .dimmed()
        );
        if !b.nerf_reasons.is_empty() {
            println!(
                "  {}",
                format!("Reasons: {}", b.nerf_reasons.join(", ")).dimmed()
            );
        }
    }
}

fn log_post_result(text: &str, b: &ScoreBreakdown, scores: Option<&MLScores>, accepted: bool) {
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
        b.confidence.label().bold(),
        b.classification_label.dimmed(),
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
            "  {} {} {}",
            "Score:".bold(),
            pct(b.final_score).green().bold(),
            format!(
                "(rel:{} auth:{})",
                pct(b.relevance_score),
                signed_pct(b.authenticity_modifier)
            )
            .dimmed()
        );
        println!(
            "  {} {:.2} {}",
            "Priority:".bold(),
            b.priority,
            format!(
                "(video:{} imgs:{} promo:-{} label:{:+.0}%)",
                if b.has_video { "yes" } else { "no" },
                b.image_count,
                pct(b.promo_penalty * 0.5),
                (b.label_multiplier - 1.0) * 100.0
            )
            .dimmed()
        );
        log_boost_nerf_reasons(b);
        if let Some(s) = scores {
            let best_ref = REFERENCE_POSTS.get(s.best_reference_idx).unwrap_or(&"?");
            let ref_short: String = best_ref.chars().take(40).collect();
            println!(
                "  {}",
                format!(
                    "ML: classification={} semantic match=\"{}...\"",
                    s.best_label, ref_short
                )
                .dimmed()
            );
        }
    } else {
        println!(
            "  {} {} {}",
            "Score:".dimmed(),
            pct(b.final_score).red().bold(),
            format!("(need {})", pct(MIN_FINAL_SCORE)).dimmed()
        );
        if !b.nerf_reasons.is_empty() {
            println!(
                "  {}",
                format!("Reason: {}", b.nerf_reasons.join(", ")).dimmed()
            );
        }
    }
}

pub fn log_accepted_post(text: &str, b: &ScoreBreakdown, scores: &MLScores) {
    log_post_result(text, b, Some(scores), true);
}

pub fn log_rejected_post(text: &str, b: &ScoreBreakdown) {
    log_post_result(text, b, None, false);
}
