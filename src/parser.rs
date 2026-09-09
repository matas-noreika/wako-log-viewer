use std::collections::HashMap;

use chrono::NaiveDateTime;
use regex::Regex;

use crate::model::{Category, Corner, LogEntry, MatchSummary, ParsedLog};

/// Matches the common line prefix:
/// `2026-08-28 17:03:44 [INFO] [MATCHID 00000...] [EVENT 163] <message>`
/// The `[EVENT n]` group is optional (Match-Info / Match / Scoretable header
/// lines don't have one), and MATCHID can be empty (`[MATCHID ]`).
fn line_regex() -> Regex {
    Regex::new(
        r"^(?P<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+\[(?P<level>\w+)\]\s+\[MATCHID\s*(?P<matchid>\d*)\]\s*(?:\[EVENT\s+(?P<event>\d+)\]\s*)?(?P<msg>.*)$",
    )
    .expect("static regex is valid")
}

/// Parses `Match: Ring04 - 06 LK 381 S F -60 kg | QUARTER-FINAL #1433 -> RED... : BLUE...`
/// into (ring, bout_label, round_name, red_name, blue_name). Best-effort: any
/// piece that doesn't fit the expected shape is left as `None` rather than
/// causing the whole line to be discarded.
fn parse_match_header(msg: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) {
    let body = msg.trim_start_matches("Match:").trim();
    let (left, right) = match body.split_once("->") {
        Some((l, r)) => (l.trim(), Some(r.trim())),
        None => (body, None),
    };

    let (ring_bout, round_name) = match left.split_once('|') {
        Some((rb, rn)) => (rb.trim(), Some(rn.trim().to_string())),
        None => (left, None),
    };

    let (ring, bout_label) = match ring_bout.split_once('-') {
        Some((r, b)) => (Some(r.trim().to_string()), Some(b.trim().to_string())),
        None => (Some(ring_bout.to_string()), None),
    };

    let (red_name, blue_name) = match right.and_then(|r| r.split_once(':')) {
        Some((r, b)) => (Some(r.trim().to_string()), Some(b.trim().to_string())),
        None => (None, None),
    };

    (ring, bout_label, round_name, red_name, blue_name)
}

/// Parse the raw text of a WAKO point-panel log file.
pub fn parse_log(text: &str) -> ParsedLog {
    let re = line_regex();
    let ts_fmt = "%Y-%m-%d %H:%M:%S";

    let mut entries: Vec<LogEntry> = Vec::new();
    let mut match_order: Vec<String> = Vec::new();
    let mut match_index: HashMap<String, MatchSummary> = HashMap::new();

    let mut last_matchid: Option<String> = None;

    for (i, raw_line) in text.lines().enumerate() {
        let line_no = i + 1;
        if raw_line.trim().is_empty() {
            continue;
        }

        if let Some(caps) = re.captures(raw_line) {
            let ts = caps.name("ts").and_then(|m| NaiveDateTime::parse_from_str(m.as_str(), ts_fmt).ok());
            let matchid_raw = caps.name("matchid").map(|m| m.as_str().to_string()).unwrap_or_default();
            let matchid = if matchid_raw.is_empty() { None } else { Some(matchid_raw) };
            let event_code = caps.name("event").and_then(|m| m.as_str().parse::<u32>().ok());
            let message = caps.name("msg").map(|m| m.as_str().to_string()).unwrap_or_default();
            let category = Category::classify(event_code, &message);
            let corner = Corner::detect(&message);

            if let Some(mid) = &matchid {
                last_matchid = Some(mid.clone());
                let summary = match_index.entry(mid.clone()).or_insert_with(|| {
                    match_order.push(mid.clone());
                    MatchSummary {
                        matchid: mid.clone(),
                        first_line_no: line_no,
                        ..Default::default()
                    }
                });
                summary.last_line_no = line_no;
                summary.line_count += 1;

                if category == Category::Header && message.trim_start().starts_with("Match:") {
                    let (ring, bout_label, round_name, red, blue) = parse_match_header(&message);
                    summary.ring = ring;
                    summary.bout_label = bout_label;
                    summary.round_name = round_name;
                    summary.red_name = red;
                    summary.blue_name = blue;
                }
            }

            entries.push(LogEntry {
                line_no,
                raw: raw_line.to_string(),
                timestamp: ts,
                matchid,
                event_code,
                message,
                category,
                corner,
            });
        } else {
            // Continuation line: a Scoretable row (`R1 | ...`, `TOTAL | ...`)
            // that has no timestamp/MATCHID of its own. Attach it to whatever
            // match id we last saw so it still filters/exports correctly.
            let matchid = last_matchid.clone();
            if let Some(mid) = &matchid {
                if let Some(summary) = match_index.get_mut(mid) {
                    summary.last_line_no = line_no;
                    summary.line_count += 1;
                }
            }
            entries.push(LogEntry {
                line_no,
                raw: raw_line.to_string(),
                timestamp: None,
                matchid,
                event_code: None,
                message: raw_line.trim().to_string(),
                category: Category::Scoretable,
                corner: None,
            });
        }
    }

    let matches = match_order
        .into_iter()
        .filter_map(|id| match_index.remove(&id))
        .collect();

    ParsedLog { entries, matches }
}
