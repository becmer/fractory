# Fractory Roadmap

Fractory is a Rust-based tool for mosaic thinkers. It tracks file diffs over time, detects development sessions, and summarizes progress via GPT. The goal is to support nonlinear, intent-driven workflows by preserving flow and reconstructing context.

---

## I. Enhancing the Core Functionality

### Internal Baseline & Diff Storage
- [ ] `fractory init` creates a clean baseline (`.fractory/baseline/`)
- [ ] All diffs are computed against this baseline, not Git
- [ ] On every tracked change, the baseline is updated to reflect the new state
- [ ] Diffs are stored as patches in `.fractory/sessions/{timestamp}/diff.patch`
- [ ] The baseline is always aligned — it is the single source of truth

### Configurable Logging Granularity
- [ ] Respect `.gitignore` to skip noisy files/directories
- [ ] Ignore changes below a threshold (e.g., whitespace-only)
- [ ] Support multiple logging levels (e.g., per interval, on save, on manual trigger)

### Smarter Session Detection
- [ ] Idle time-based detection with customizable timeout
- [ ] Allow manual `fractory start-session` / `end-session` boundaries
- [ ] Merge/split sessions manually via CLI or config file

### Richer Context for GPT Summaries
- [ ] Include file paths, branch names, commit messages
- [ ] Add comments made during the session as input signals
- [ ] Experiment with prompt variations (e.g., ask GPT for "why", not just "what")
- [ ] GPT integration must be isolated and easy to disable globally

---

## II. Improving Workflow and Usability

### Powerful Command-Line Interface (CLI)
- [ ] `fractory status` – show current session or activity state
- [ ] `fractory summary [session_id|today]` – generate/update summary
- [ ] `fractory log "note"` – log manual annotations
- [ ] `fractory resume` – restore last touched files + summary context
- [ ] `fractory search [keyword|file|date]` – grep-like lookup across sessions

### IDE / Editor Integration
- [ ] Basic VS Code extension (change tracking indicator, quick commands)
- [ ] Trigger summaries and add notes from editor
- [ ] Session timeline view next to code

### Low-Friction Intent Annotation
- [ ] `fractory tag "my intent"` command
- [ ] Git commit hook to extract intent from messages
- [ ] Prompt user for intent after long idle or commit

---

## III. Realizing the Future Vision

### Incremental Graph Building
- [ ] Link sessions by shared files or common tags
- [ ] Manual linking: `fractory link sessionA sessionB`
- [ ] Use embeddings to suggest related sessions/fragments

### Timeline Visualization Filters
- [ ] Visual filters by tag, file type, branch, or directory
- [ ] Highlight intent tags or major turning points in development

### Integration with PKM Tools
- [ ] Export summaries to Obsidian/Logseq/Markdown
- [ ] Auto-link related sessions via backlinks or tags

---

## IV. Practical Considerations

### Performance & Storage Optimization
- [ ] Compressed diff storage
- [ ] Use time-series or append-only DB (e.g., `sled`, SQLite)
- [ ] Archive old sessions for long-term performance

### Privacy-Conscious AI
- [ ] GPT summarization opt-in by default
- [ ] Local model support (optional, long-term)
- [ ] Show user what is sent before it’s sent

### Onboarding & Documentation
- [ ] Explain mosaic thinking and how Fractory supports it
- [ ] Provide usage examples and typical workflows
- [ ] Visual tour of session tracking and summaries

---

## Immediate Priorities

- [ ] Create internal project baseline on first run (`fractory init`)
- [ ] Record and store diff against baseline per interval
- [ ] `.gitignore`-aware file watcher

---

# Appendix: Tools worth checking-out

- `cargo xtask`
- `justfile`
