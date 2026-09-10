# WAKO Point Panel Log Viewer

A small Rust + egui desktop GUI for browsing, filtering, and exporting
WAKO point-panel bout logs (the `point_panel_wakolc_log.*` files).

## Build & run

Requires a recent stable Rust toolchain (install via <https://rustup.rs> if
you don't have one — `cargo --version` should be 1.75+, ideally current
stable, since `eframe`/`egui` move fast).

```
cargo run --release
```

On Linux you'll also need the usual GUI dev packages for `eframe`/`rfd`
(X11/Wayland + file-dialog backends), e.g. on Debian/Ubuntu:

```
sudo apt install libgtk-3-dev libxkbcommon-dev libssl-dev
```

On macOS you may use

## Using it

1. **Open log file…** — pick your `.log` / `.txt` / `.1` log file.
2. **Filters** (left panel):
   - Free text search — substring match across the whole raw line.
   - Match ID contains — filter to one bout (or use "Jump to match" to
     pick a bout by ring/round/competitor names instead of typing the ID).
   - Event category checkboxes — toggle points, warnings, knockdowns,
     kick counts, minus points, round changes, clock events, side
     changes, resets, headers, and scoretable snapshots independently.
   - Corner — Any / RED / BLUE.
   - Time range — optional `YYYY-MM-DD HH:MM:SS` start/end bounds.
3. **Log panel** (center) shows every line matching the current filters,
   selectable/copyable as plain text.
4. **Export whole match** — pick a bout from "Jump to match", then either
   copy its complete raw log text to the clipboard or save it as a
   standalone `.txt` file (includes a small header with ring/round/
   competitor info followed by every raw log line for that bout, in
   order — clock, points, warnings, scoretables, everything).

## Project layout

- `src/model.rs` — data types: `LogEntry`, `Category`, `Corner`, `MatchSummary`.
- `src/parser.rs` — turns raw log text into `ParsedLog` (entries + per-match summaries).
- `src/app.rs` — the egui UI: filters, filtered view, match export.
- `src/main.rs` — entry point.

## Notes on the log format

See the event-code table this tool was built against — briefly: each
line is `<timestamp> [INFO] [MATCHID <id>] [EVENT <code>] <message>`;
`Match-Info:` / `Match:` lines carry bout metadata; `Scoretable` blocks
are a few un-tagged continuation rows (`R1 | ... | TOTAL | ...`) that
the parser attaches to the preceding match automatically.
