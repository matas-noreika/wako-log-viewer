use chrono::NaiveDateTime;

#[allow(dead_code)]
/// Broad, human-facing grouping of the numeric [EVENT n] codes found in the
/// log, so the UI can offer checkboxes instead of raw numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Clock,
    Point,
    Warning,
    KnockDown,
    KickCount,
    MinusPoint,
    Round,
    SideChange,
    Reset,
    TimeExpired,
    Header,     // Match-Info / Match lines
    Scoretable, // Scoretable block + its R1/R2/R3/TOTAL rows
    Other,
}

impl Category {
    /// All categories, in a stable display order for the filter panel.
    pub const ALL: [Category; 13] = [
        Category::Header,
        Category::Point,
        Category::MinusPoint,
        Category::Warning,
        Category::KnockDown,
        Category::KickCount,
        Category::Round,
        Category::Clock,
        Category::TimeExpired,
        Category::SideChange,
        Category::Reset,
        Category::Scoretable,
        Category::Other,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::Clock => "Clock (start/stop/reset)",
            Category::Point => "Points scored",
            Category::Warning => "Warnings",
            Category::KnockDown => "Knockdowns",
            Category::KickCount => "Kick count",
            Category::MinusPoint => "Minus points / penalties",
            Category::Round => "Round changes",
            Category::SideChange => "Corner/side change",
            Category::Reset => "Reset",
            Category::TimeExpired => "Time expired",
            Category::Header => "Match header (Match-Info / Match)",
            Category::Scoretable => "Scoretable snapshots",
            Category::Other => "Other / unclassified",
        }
    }

    /// Classify a parsed line from its EVENT code (if any) and its message text.
    pub fn classify(event_code: Option<u32>, message: &str) -> Category {
        if let Some(code) = event_code {
            return match code {
                46 | 47 | 48 => Category::Clock,
                49 | 50 | 51 | 52 | 159 | 160 | 163 | 164 | 167 | 168 => Category::Point,
                209 | 210 => Category::MinusPoint,
                140 | 142 => Category::Warning,
                175 | 176 | 177 => Category::KnockDown,
                184 | 186 => Category::KickCount,
                146 | 170 => Category::Round,
                98 | 126 => Category::SideChange,
                87 => Category::Reset,
                88 => Category::Clock,
                89 => Category::TimeExpired,
                _ => Category::Other,
            };
        }
        let trimmed = message.trim_start();
        if trimmed.starts_with("Match-Info") || trimmed.starts_with("Match:") {
            Category::Header
        } else if trimmed.starts_with("Scoretable")
            || trimmed.starts_with("R1 ")
            || trimmed.starts_with("R2 ")
            || trimmed.starts_with("R3 ")
            || trimmed.starts_with("TOTAL ")
        {
            Category::Scoretable
        } else {
            Category::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    Red,
    Blue,
}

impl Corner {
    /// Best-effort detection of which corner a message line refers to.
    pub fn detect(message: &str) -> Option<Corner> {
        let trimmed = message.trim_start();
        if trimmed.starts_with("RED") {
            Some(Corner::Red)
        } else if trimmed.starts_with("BLUE") {
            Some(Corner::Blue)
        } else {
            None
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Corner::Red => "RED",
            Corner::Blue => "BLUE",
        }
    }
}

/// One line of the raw log file, parsed as much as it usefully can be.
///
/// Rows that belong to a `Scoretable` block (the `R1 | ... | ...` /
/// `TOTAL | ...` lines) have no timestamp of their own; they inherit the
/// match id of the most recent tagged line and are tagged `Category::Scoretable`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub line_no: usize,
    pub raw: String,
    pub timestamp: Option<NaiveDateTime>,
    pub matchid: Option<String>,
    pub event_code: Option<u32>,
    pub message: String,
    pub category: Category,
    pub corner: Option<Corner>,
}

/// Metadata about one bout, gathered from its `Match-Info` / `Match:` header
/// lines, used to populate the "jump to match" / "export match" pickers.
#[derive(Debug, Clone, Default)]
pub struct MatchSummary {
    pub matchid: String,
    pub ring: Option<String>,
    pub bout_label: Option<String>, // e.g. "06 LK 381 S F -60 kg"
    pub round_name: Option<String>, // e.g. "QUARTER-FINAL #1433"
    pub red_name: Option<String>,
    pub blue_name: Option<String>,
    pub first_line_no: usize,
    pub last_line_no: usize,
    pub line_count: usize,
}

impl MatchSummary {
    /// A single-line label for use in combo boxes / lists.
    pub fn picker_label(&self) -> String {
        let who = match (&self.red_name, &self.blue_name) {
            (Some(r), Some(b)) => format!("{r} vs {b}"),
            _ => "(unknown competitors)".to_string(),
        };
        let round = self.round_name.as_deref().unwrap_or("");
        let ring = self.ring.as_deref().unwrap_or("?");
        format!("[{ring}] {round} — {who} ({})", short_id(&self.matchid))
    }
}

/// Shorten a 29-digit match id to its last 8 digits for compact display.
pub fn short_id(matchid: &str) -> String {
    if matchid.len() > 8 {
        format!("…{}", &matchid[matchid.len() - 8..])
    } else {
        matchid.to_string()
    }
}

/// Everything produced by parsing a log file.
#[derive(Debug, Clone, Default)]
pub struct ParsedLog {
    pub entries: Vec<LogEntry>,
    /// Matches in first-seen order, keyed by matchid for lookup.
    pub matches: Vec<MatchSummary>,
}

impl ParsedLog {
    pub fn find_match(&self, matchid: &str) -> Option<&MatchSummary> {
        self.matches.iter().find(|m| m.matchid == matchid)
    }
}
