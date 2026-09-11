//! Turning a parsed match into external formats.
//!
//! This module is the single place that knows how to serialize a
//! [`ParsedLog`] match into something outside the app: the raw
//! chronological log text (for copy/paste or a `.txt` save), and a
//! referee-facing Markdown scoresheet that gets rendered to PDF via
//! [`markdown2pdf`]. Keeping this separate from `app.rs` means the egui
//! UI code never has to know what a scoresheet looks like, and this code
//! never has to know about `egui`.

use std::path::Path;

use markdown2pdf::config::ConfigSource;

use crate::model::{short_id, Category, LogEntry, MatchSummary, ParsedLog};

/// Builds the raw, chronological log text for one match: a small header
/// (matchid/ring/round/competitors) followed by every raw log line that
/// belongs to it, in file order. This is what "Copy to clipboard" and
/// "Save as .txt" hand to the user.
pub fn plain_text_for(parsed: &ParsedLog, matchid: &str) -> String {
    let mut out = String::new();
    if let Some(summary) = parsed.find_match(matchid) {
        out.push_str(&format!("MATCHID: {}\n", summary.matchid));
        if let Some(r) = &summary.ring {
            out.push_str(&format!("Ring: {r}\n"));
        }
        if let Some(b) = &summary.bout_label {
            out.push_str(&format!("Bout: {b}\n"));
        }
        if let Some(rn) = &summary.round_name {
            out.push_str(&format!("Round: {rn}\n"));
        }
        if let Some(red) = &summary.red_name {
            out.push_str(&format!("RED:  {red}\n"));
        }
        if let Some(blue) = &summary.blue_name {
            out.push_str(&format!("BLUE: {blue}\n"));
        }
        out.push_str(&"-".repeat(60));
        out.push('\n');
    }
    for entry in &parsed.entries {
        if entry.matchid.as_deref() == Some(matchid) {
            out.push_str(&entry.raw);
            out.push('\n');
        }
    }
    out
}

/// Renders a referee-facing PDF scoresheet for the given match to `path`,
/// via a Markdown intermediate (header, round-by-round score table, plain-
/// English event timeline) fed through `markdown2pdf`.
pub fn render_pdf_for(parsed: &ParsedLog, matchid: &str, path: &Path) -> Result<(), String> {
    let markdown = markdown_report_for(parsed, matchid);
    markdown2pdf::parse_into_file(markdown, path, ConfigSource::Theme("academic"), None)
        .map_err(|e| e.to_string())
}

/// A suggested filename for the PDF export of the given match.
pub fn pdf_file_name(matchid: &str) -> String {
    format!("match_{}.pdf", short_id(matchid))
}

/// Builds the Markdown source for the referee scoresheet: a header block,
/// a round-by-round judge score table, and a plain-English event timeline.
fn markdown_report_for(parsed: &ParsedLog, matchid: &str) -> String {
    let mut md = String::new();
    let summary = parsed.find_match(matchid);

    write_header(&mut md, summary);
    write_score_table(&mut md, parsed, matchid);
    write_timeline(&mut md, parsed, matchid);

    md
}

fn write_header(md: &mut String, summary: Option<&MatchSummary>) {
    let title = summary
        .and_then(|s| match (&s.red_name, &s.blue_name) {
            (Some(r), Some(b)) => Some(format!("{r} vs {b}")),
            _ => None,
        })
        .unwrap_or_else(|| "Match Scoresheet".to_string());
    md.push_str(&format!("# {title}\n\n"));

    let Some(s) = summary else {
        return;
    };
    if let Some(ring) = &s.ring {
        md.push_str(&format!("**Ring:** {ring}  \n"));
    }
    if let Some(round) = &s.round_name {
        md.push_str(&format!("**Round:** {round}  \n"));
    }
    if let Some(bout) = &s.bout_label {
        md.push_str(&format!("**Bout:** {bout}  \n"));
    }
    if let Some(red) = &s.red_name {
        md.push_str(&format!("**RED:** {red}  \n"));
    }
    if let Some(blue) = &s.blue_name {
        md.push_str(&format!("**BLUE:** {blue}  \n"));
    }
    md.push_str(&format!("**Match ID:** {}\n\n", s.matchid));
}

/// One parsed row of a `Scoretable` block, e.g. the `R1 | J1 0:0 | ... |
/// false |` line, decoded into its round label, per-judge red:blue scores,
/// and whether the round is marked closed.
struct ScoreRow {
    round: String,
    judges: Vec<(String, Option<(i32, i32)>)>,
}

fn parse_scoretable_row(raw: &str) -> Option<ScoreRow> {
    let cells: Vec<&str> = raw.split('|').map(|c| c.trim()).collect();
    let round = *cells.first()?;
    if !matches!(round, "R1" | "R2" | "R3" | "TOTAL") {
        return None;
    }

    let mut judges = Vec::new();
    for cell in &cells[1..] {
        if cell.is_empty() || *cell == "true" || *cell == "false" {
            continue;
        }
        let mut parts = cell.splitn(2, char::is_whitespace);
        let label = parts.next().unwrap_or("").trim();
        if label.is_empty() {
            continue;
        }
        let score = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|s| s.split_once(':'))
            .and_then(|(a, b)| Some((a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?)));
        judges.push((label.to_string(), score));
    }
    Some(ScoreRow {
        round: round.to_string(),
        judges,
    })
}

fn write_score_table(md: &mut String, parsed: &ParsedLog, matchid: &str) {
    // Later snapshots overwrite earlier ones for the same round, so the
    // final entry per round label reflects the match's end state.
    let mut rows: std::collections::HashMap<String, ScoreRow> = std::collections::HashMap::new();
    let mut judge_order: Vec<String> = Vec::new();

    for entry in &parsed.entries {
        if entry.matchid.as_deref() != Some(matchid) || entry.category != Category::Scoretable {
            continue;
        }
        if let Some(row) = parse_scoretable_row(&entry.message) {
            for (label, _) in &row.judges {
                if !judge_order.contains(label) {
                    judge_order.push(label.clone());
                }
            }
            rows.insert(row.round.clone(), row);
        }
    }

    if rows.is_empty() {
        return;
    }
    judge_order.sort();

    md.push_str("## Round-by-round score\n\n");
    md.push_str("| Round |");
    for j in &judge_order {
        md.push_str(&format!(" {j} (RED:BLUE) |"));
    }
    md.push('\n');
    md.push_str("|---|");
    for _ in &judge_order {
        md.push_str("---|");
    }
    md.push('\n');

    for round in ["R1", "R2", "R3", "TOTAL"] {
        let Some(row) = rows.get(round) else { continue };
        let label = if round == "TOTAL" { "**Total**" } else { round };
        md.push_str(&format!("| {label} |"));
        for j in &judge_order {
            let cell = row
                .judges
                .iter()
                .find(|(l, _)| l == j)
                .and_then(|(_, score)| *score)
                .map(|(r, b)| format!("{r}:{b}"))
                .unwrap_or_else(|| "–".to_string());
            md.push_str(&format!(" {cell} |"));
        }
        md.push('\n');
    }
    md.push('\n');
}

fn write_timeline(md: &mut String, parsed: &ParsedLog, matchid: &str) {
    md.push_str("## Event timeline\n\n");
    let mut any = false;
    for entry in &parsed.entries {
        if entry.matchid.as_deref() != Some(matchid) {
            continue;
        }
        if matches!(entry.category, Category::Header | Category::Scoretable) {
            continue;
        }
        let Some(line) = humanize_entry(entry) else {
            continue;
        };
        any = true;
        md.push_str(&format!("- {line}\n"));
    }
    if !any {
        md.push_str("_No events recorded for this match._\n");
    }
    md.push('\n');
}

/// Turns one log entry into a plain-English timeline bullet, e.g.
/// `17:03:44 — RED point (Judge 2), running score 1:1`. Returns `None` for
/// entries with no timestamp (nothing meaningful to show on a timeline).
fn humanize_entry(entry: &LogEntry) -> Option<String> {
    let ts = entry.timestamp?.format("%H:%M:%S").to_string();
    let corner = entry.corner.map(|c| c.label());
    let msg = entry.message.trim();

    let body = match entry.category {
        Category::Point => humanize_point(msg, corner),
        Category::Warning => humanize_counter(msg, corner, "warning"),
        Category::KnockDown => humanize_counter(msg, corner, "knockdown"),
        Category::KickCount => humanize_counter(msg, corner, "kick"),
        Category::MinusPoint => humanize_minus_point(msg, corner),
        Category::Round => humanize_round(msg),
        Category::Clock => humanize_clock(msg),
        Category::SideChange => "Corners swapped".to_string(),
        Category::Reset => "Scores reset".to_string(),
        Category::TimeExpired => "Time expired".to_string(),
        Category::Other => msg.to_string(),
        Category::Header | Category::Scoretable => return None,
    };

    Some(format!("`{ts}` — {body}"))
}

/// Parses `"<n> J<k> -> <total>"` out of a point-scoring message, if present.
fn extract_judge_point(msg: &str) -> Option<(String, String, String)> {
    // Expected shape once the corner word is stripped:
    // "add point: 1 J2 -> 1 [J1 1:0] ..." or "remove point: 1 J2 -> 0 [...]"
    let after_colon = msg.split_once(':')?.1.trim();
    let mut words = after_colon.split_whitespace();
    let count = words.next()?;
    let judge = words.next()?;
    let arrow = words.next()?;
    let total = words.next()?;
    if arrow != "->" || !judge.starts_with('J') {
        return None;
    }
    Some((count.to_string(), judge.to_string(), total.to_string()))
}

fn humanize_point(msg: &str, corner: Option<&str>) -> String {
    let corner = corner.unwrap_or("Corner");
    let verb = if msg.contains("remove") { "point removed" } else { "point" };
    match extract_judge_point(msg) {
        Some((count, judge, total)) => {
            format!("**{corner}** {verb} ({count}) — {judge}, running total {total}")
        }
        None => format!("**{corner}** {verb}"),
    }
}

/// Handles Warning / KnockDown / KickCount messages, all of which share the
/// shape `"<CORNER> add <Label>: 1 <Label>:<total>"`.
fn humanize_counter(msg: &str, corner: Option<&str>, noun: &str) -> String {
    let corner = corner.unwrap_or("Corner");
    let verb = if msg.contains("remove") { "removed" } else { "added" };
    let total = msg
        .rsplit_once(':')
        .map(|(_, t)| t.trim())
        .filter(|t| !t.is_empty());
    match total {
        Some(total) => format!("**{corner}** {noun} {verb} — total {total}"),
        None => format!("**{corner}** {noun} {verb}"),
    }
}

fn humanize_minus_point(msg: &str, corner: Option<&str>) -> String {
    let corner = corner.unwrap_or("Corner");
    let amount = msg
        .split_once("MP:")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .unwrap_or("1");
    format!("**{corner}** penalty (-{amount})")
}

fn humanize_round(msg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("New round") {
        let n = rest.trim();
        if !n.is_empty() && !n.starts_with('[') {
            return format!("Round {n} started");
        }
        return "Round score snapshot".to_string();
    }
    if msg.starts_with("Last round diff") {
        return "Round score difference recorded".to_string();
    }
    msg.to_string()
}

fn humanize_clock(msg: &str) -> String {
    if msg.starts_with("Start clock") {
        "Clock started".to_string()
    } else if msg.starts_with("Stop clock") {
        "Clock stopped".to_string()
    } else if msg.starts_with("Reset clock") {
        "Clock reset".to_string()
    } else if msg.starts_with("Time warn") {
        "Time warning".to_string()
    } else {
        msg.to_string()
    }
}

