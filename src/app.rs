use std::collections::HashSet;
use std::path::PathBuf;

use chrono::NaiveDateTime;
use eframe::egui;

use crate::model::{Category, Corner, LogEntry, ParsedLog};
use crate::parser::parse_log;

const TIME_HINT: &str = "YYYY-MM-DD HH:MM:SS";

/// definition of the log viewer apps 
pub struct LogViewerApp {
    parsed: Option<ParsedLog>,
    file_path: Option<PathBuf>,
    status: String,

    // --- filter state ---
    search_text: String,
    matchid_filter: String,
    disabled_categories: HashSet<Category>,
    corner_filter: Option<Corner>,
    start_time_text: String,
    end_time_text: String,

    // --- export state ---
    export_matchid: Option<String>,
    last_export_note: Option<String>,

    // --- cached filtered output ---
    filters_dirty: bool,
    filtered_text: String,
    filtered_count: usize,
}

/// implementation of a default method to provide a quick return of LogViewerApp object with default
/// properties
impl Default for LogViewerApp {
    fn default() -> Self {
        Self {
            parsed: None,
            file_path: None,
            status: "No file loaded yet.".to_string(),
            search_text: String::new(),
            matchid_filter: String::new(),
            disabled_categories: HashSet::new(),
            corner_filter: None,
            start_time_text: String::new(),
            end_time_text: String::new(),
            export_matchid: None,
            last_export_note: None,
            filters_dirty: true,
            filtered_text: String::new(),
            filtered_count: 0,
        }
    }
}

/// implementation of all methods attached to LogViewerApp
impl LogViewerApp {
    /// method to read from referenced source file
    fn load_file(&mut self, path: PathBuf) {
        //essential a try catch block in rust
        match std::fs::read(&path) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let parsed = parse_log(&text);
                self.status = format!(
                    "Loaded {} ({} lines, {} matches).",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                    parsed.entries.len(),
                    parsed.matches.len()
                );
                self.export_matchid = parsed.matches.first().map(|m| m.matchid.clone());
                self.file_path = Some(path);
                self.parsed = Some(parsed);
                self.filters_dirty = true;
            }
            Err(e) => {
                self.status = format!("Failed to read file: {e}");
            }
        }
    }

    /// method to clear all filter options
    fn clear_filters(&mut self) {
        self.search_text.clear();
        self.matchid_filter.clear();
        self.disabled_categories.clear();
        self.corner_filter = None;
        self.start_time_text.clear();
        self.end_time_text.clear();
        self.filters_dirty = true;
    }


    fn entry_passes_filters(
        &self,
        entry: &LogEntry,
        search_lower: &str,
        matchid_lower: &str,
        start: Option<NaiveDateTime>,
        end: Option<NaiveDateTime>,
    ) -> bool {
        if self.disabled_categories.contains(&entry.category) {
            return false;
        }

        if let Some(wanted) = self.corner_filter {
            if entry.corner != Some(wanted) {
                return false;
            }
        }

        if !matchid_lower.is_empty() {
            match &entry.matchid {
                Some(mid) if mid.to_lowercase().contains(matchid_lower) => {}
                _ => return false,
            }
        }

        if !search_lower.is_empty() && !entry.raw.to_lowercase().contains(search_lower) {
            return false;
        }

        if let Some(ts) = entry.timestamp {
            if let Some(start) = start {
                if ts < start {
                    return false;
                }
            }
            if let Some(end) = end {
                if ts > end {
                    return false;
                }
            }
        }

        true
    }

    fn recompute_filtered(&mut self) {
        self.filters_dirty = false;
        let Some(parsed) = &self.parsed else {
            self.filtered_text.clear();
            self.filtered_count = 0;
            return;
        };

        let search_lower = self.search_text.trim().to_lowercase();
        let matchid_lower = self.matchid_filter.trim().to_lowercase();
        let start = parse_time(&self.start_time_text);
        let end = parse_time(&self.end_time_text);

        let mut out = String::new();
        let mut count = 0usize;
        for entry in &parsed.entries {
            if self.entry_passes_filters(entry, &search_lower, &matchid_lower, start, end) {
                out.push_str(&entry.raw);
                out.push('\n');
                count += 1;
            }
        }

        self.filtered_text = out;
        self.filtered_count = count;
    }

    fn export_text_for(&self, matchid: &str) -> String {
        let Some(parsed) = &self.parsed else {
            return String::new();
        };
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

    fn draw_filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Filters");
        ui.add_space(4.0);

        ui.label("Free text search");
        if ui.add(egui::TextEdit::singleline(&mut self.search_text).desired_width(ui.available_width())).changed() {
            self.filters_dirty = true;
        }

        ui.add_space(6.0);
        ui.label("Match ID contains");
        if ui.add(egui::TextEdit::singleline(&mut self.matchid_filter).desired_width(ui.available_width())).changed() {
            self.filters_dirty = true;
        }

        // Collect just the (id, label) pairs we need up front so the combo
        // box closure below only touches owned local data plus the specific
        // `self` fields it updates, not a lingering borrow of `self.parsed`.
        let match_picks: Vec<(String, String)> = self
            .parsed
            .as_ref()
            .map(|p| p.matches.iter().map(|m| (m.matchid.clone(), m.picker_label())).collect())
            .unwrap_or_default();

        if !match_picks.is_empty() {
            ui.add_space(4.0);
            let current_label = self
                .export_matchid
                .as_ref()
                .and_then(|id| match_picks.iter().find(|(mid, _)| mid == id))
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| "Select a match…".to_string());

            ui.label("Jump to match");
            egui::ComboBox::from_id_salt("jump_to_match")
                .selected_text(current_label)
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for (mid, label) in &match_picks {
                        let selected = self.export_matchid.as_deref() == Some(mid.as_str());
                        if ui.selectable_label(selected, label).clicked() {
                            self.export_matchid = Some(mid.clone());
                            self.matchid_filter = mid.clone();
                            self.filters_dirty = true;
                        }
                    }
                });
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Event category");
        for cat in Category::ALL {
            let mut enabled = !self.disabled_categories.contains(&cat);
            if ui.checkbox(&mut enabled, cat.label()).changed() {
                if enabled {
                    self.disabled_categories.remove(&cat);
                } else {
                    self.disabled_categories.insert(cat);
                }
                self.filters_dirty = true;
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Corner");
        ui.horizontal(|ui| {
            if ui.radio_value(&mut self.corner_filter, None, "Any").changed() {
                self.filters_dirty = true;
            }
            if ui
                .radio_value(&mut self.corner_filter, Some(Corner::Red), "RED")
                .changed()
            {
                self.filters_dirty = true;
            }
            if ui
                .radio_value(&mut self.corner_filter, Some(Corner::Blue), "BLUE")
                .changed()
            {
                self.filters_dirty = true;
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.label(format!("Time range ({TIME_HINT}, optional)"));
        if ui
            .add(egui::TextEdit::singleline(&mut self.start_time_text).hint_text("start"))
            .changed()
        {
            self.filters_dirty = true;
        }
        if ui
            .add(egui::TextEdit::singleline(&mut self.end_time_text).hint_text("end"))
            .changed()
        {
            self.filters_dirty = true;
        }
        if !self.start_time_text.trim().is_empty() && parse_time(&self.start_time_text).is_none() {
            ui.colored_label(egui::Color32::from_rgb(200, 80, 80), "Start time not understood, ignored.");
        }
        if !self.end_time_text.trim().is_empty() && parse_time(&self.end_time_text).is_none() {
            ui.colored_label(egui::Color32::from_rgb(200, 80, 80), "End time not understood, ignored.");
        }

        ui.add_space(12.0);
        if ui.button("Clear all filters").clicked() {
            self.clear_filters();
        }

        ui.add_space(16.0);
        ui.separator();
        ui.heading("Export whole match");
        ui.label("Uses the match picked above.");
        ui.horizontal(|ui| {
            let enabled = self.export_matchid.is_some();
            if ui
                .add_enabled(enabled, egui::Button::new("Copy to clipboard"))
                .clicked()
            {
                if let Some(mid) = self.export_matchid.clone() {
                    let text = self.export_text_for(&mid);
                    ui.ctx().copy_text(text);
                    self.last_export_note = Some("Match text copied to clipboard.".to_string());
                }
            }
            if ui.add_enabled(enabled, egui::Button::new("Save as .txt…")).clicked() {
                if let Some(mid) = self.export_matchid.clone() {
                    let text = self.export_text_for(&mid);
                    let default_name = format!("match_{}.txt", crate::model::short_id(&mid));
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name(&default_name)
                        .add_filter("Text file", &["txt"])
                        .save_file()
                    {
                        match std::fs::write(&path, text) {
                            Ok(()) => {
                                self.last_export_note =
                                    Some(format!("Saved to {}", path.display()));
                            }
                            Err(e) => {
                                self.last_export_note = Some(format!("Failed to save: {e}"));
                            }
                        }
                    }
                }
            }
        });
        if let Some(note) = &self.last_export_note {
            ui.label(note);
        }
    }
}

impl eframe::App for LogViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Note: as of egui 0.36, `SidePanel`/`TopBottomPanel` were merged into
        // a single `egui::Panel`, and every panel's `.show()` takes the parent
        // `&mut Ui` (from `App::ui`) rather than a `&Context`. Panels must be
        // added in order, with `CentralPanel` last.
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Open log file…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("WAKO log", &["log", "txt", "1"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                    {
                        self.load_file(path);
                    }
                }
                ui.separator();
                ui.label(&self.status);
            });
            ui.add_space(4.0);
        });

        egui::Panel::left("filters_panel")
            .resizable(true)
            .default_size(200.0)
            .size_range(120.0..=600.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.draw_filter_panel(ui);
                });
            });

        if self.filters_dirty {
            self.recompute_filtered();
        }

        // the filter log panel
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Log");
                ui.label(format!("({} matching lines)", self.filtered_count));
            });
            ui.separator();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                // A selectable Label (rather than a TextEdit) keeps this
                // panel strictly read-only while still letting the user
                // click-drag to select and copy any subset of lines.
                ui.add(
                    egui::Label::new(egui::RichText::new(&self.filtered_text).monospace())
                        .selectable(true)
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            });
        });
    }
}

/// Parse a `YYYY-MM-DD HH:MM:SS` timestamp from a filter text box, returning
/// `None` for blank or unparsable input rather than erroring.
fn parse_time(text: &str) -> Option<NaiveDateTime> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").ok()
}
