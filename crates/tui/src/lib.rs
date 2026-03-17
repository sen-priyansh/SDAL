// crates/tui/src/lib.rs

// Interactive TUI merge conflict resolution tool for SDAL.
// Launched via `sdal mergetool`.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use sdal_core::Object;
use sdal_storage::{FilesystemStorage, Storage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ─── MergeState (mirrors sdal_core::merge::MergeState) ────────────────────

#[derive(Serialize, Deserialize, Debug)]
struct MergeState {
    ours: String,
    theirs: String,
    target_branch: String,
    pub conflicts: Vec<String>,
    #[serde(default)]
    conflict_details: HashMap<String, (String, String)>,
    merged_tree_hash: String,
}

// ─── Conflict Tracking ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
struct ConflictEntry {
    path: String,
    ours_blob: String,
    theirs_blob: String,
    resolved: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConflictIndex {
    entries: Vec<ConflictEntry>,
}

struct ConflictFile {
    path: String,
    ours_blob: String,
    theirs_blob: String,
    ours_lines: Vec<String>,
    theirs_lines: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Resolution {
    Ours,
    Theirs,
}

// ─── Blob Helper ───────────────────────────────────────────────────────────

fn read_blob_to_string(storage: &FilesystemStorage, hash: &str) -> String {
    if hash.is_empty() {
        return "".to_string();
    }
    let data = storage.get(hash).unwrap_or_default();
    let obj = match Object::from_bytes(&data) {
        Ok(o) => o,
        Err(_) => return "(invalid object)".to_string(),
    };

    if let Object::Blob(blob) = obj {
        let mut content = Vec::new();
        for chunk_entry in blob.chunks {
            if let Ok(chunk_data) = storage.get(&chunk_entry.hash) {
                content.extend_from_slice(&chunk_data);
            }
        }
        String::from_utf8(content).unwrap_or_else(|_| "(binary file)".to_string())
    } else {
        "(not a blob)".to_string()
    }
}

// ─── App state ─────────────────────────────────────────────────────────────

struct App {
    conflicts: Vec<ConflictFile>,
    current: usize,
    scroll_ours: u16,
    scroll_theirs: u16,
    resolutions: Vec<Option<Resolution>>,
    sdal_root: PathBuf,
    working_dir: PathBuf,
    quit: bool,
    applied: bool,
}

impl App {
    fn load(sdal_root: &Path, working_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let merge_state_path = sdal_root.join("MERGE_STATE");
        if !merge_state_path.exists() {
            return Err("No merge in progress. Run 'sdal merge <branch>' first.".into());
        }

        let content = fs::read_to_string(&merge_state_path)?;
        let state: MergeState = serde_json::from_str(&content)?;

        if state.conflicts.is_empty() {
            return Err("No conflicts to resolve.".into());
        }

        let storage = FilesystemStorage::new(sdal_root)?;

        let mut conflicts = Vec::new();
        for path in &state.conflicts {
            let (ours_hash, theirs_hash) = match state.conflict_details.get(path) {
                Some((o, t)) => (o.clone(), t.clone()),
                None => ("".to_string(), "".to_string()),
            };

            let ours_content = read_blob_to_string(&storage, &ours_hash);
            let theirs_content = read_blob_to_string(&storage, &theirs_hash);

            conflicts.push(ConflictFile {
                path: path.clone(),
                ours_blob: ours_hash,
                theirs_blob: theirs_hash,
                ours_lines: ours_content.lines().map(String::from).collect(),
                theirs_lines: theirs_content.lines().map(String::from).collect(),
            });
        }

        let n = conflicts.len();
        Ok(App {
            conflicts,
            current: 0,
            scroll_ours: 0,
            scroll_theirs: 0,
            resolutions: vec![None; n],
            sdal_root: sdal_root.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            quit: false,
            applied: false,
        })
    }

    fn current_conflict(&self) -> &ConflictFile {
        &self.conflicts[self.current]
    }

    fn max_scroll(&self) -> u16 {
        let c = self.current_conflict();
        let max_lines = c.ours_lines.len().max(c.theirs_lines.len());
        max_lines.saturating_sub(5) as u16
    }

    fn scroll_down(&mut self) {
        let max = self.max_scroll();
        if self.scroll_ours < max {
            self.scroll_ours += 1;
        }
        if self.scroll_theirs < max {
            self.scroll_theirs += 1;
        }
    }

    fn scroll_up(&mut self) {
        self.scroll_ours = self.scroll_ours.saturating_sub(1);
        self.scroll_theirs = self.scroll_theirs.saturating_sub(1);
    }

    fn next_conflict(&mut self) {
        if self.current + 1 < self.conflicts.len() {
            self.current += 1;
            self.scroll_ours = 0;
            self.scroll_theirs = 0;
        }
    }

    fn prev_conflict(&mut self) {
        if self.current > 0 {
            self.current -= 1;
            self.scroll_ours = 0;
            self.scroll_theirs = 0;
        }
    }

    fn select_ours(&mut self) {
        self.resolutions[self.current] = Some(Resolution::Ours);
    }

    fn select_theirs(&mut self) {
        self.resolutions[self.current] = Some(Resolution::Theirs);
    }

    fn all_resolved(&self) -> bool {
        self.resolutions.iter().all(|r| r.is_some())
    }

    fn resolved_count(&self) -> usize {
        self.resolutions.iter().filter(|r| r.is_some()).count()
    }

    fn apply(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let storage = FilesystemStorage::new(&self.sdal_root)?;

        for (i, conflict) in self.conflicts.iter().enumerate() {
            if let Some(resolution) = self.resolutions[i] {
                let chosen_hash = match resolution {
                    Resolution::Ours => &conflict.ours_blob,
                    Resolution::Theirs => &conflict.theirs_blob,
                };

                let target_path = self.working_dir.join(&conflict.path);

                // Write the chosen blob contents to the actual file
                sdal_core::checkout::restore_blob(chosen_hash, &storage, &target_path)?;
            }
        }

        self.applied = true;

        // Update CONFLICTS JSON — mark resolved entries, delete if all done
        let conflicts_path = self.sdal_root.join("CONFLICTS");
        if conflicts_path.exists() {
            let content = fs::read_to_string(&conflicts_path)?;
            let mut index: ConflictIndex = serde_json::from_str(&content)?;
            for conflict in &self.conflicts {
                for entry in &mut index.entries {
                    if entry.path == conflict.path {
                        entry.resolved = true;
                    }
                }
            }
            if index.entries.iter().all(|e| e.resolved) {
                fs::remove_file(&conflicts_path)?;
            } else {
                let updated = serde_json::to_string_pretty(&index)?;
                fs::write(&conflicts_path, updated)?;
            }
        }

        Ok(())
    }
}

// ─── Diff highlighting helpers ─────────────────────────────────────────────

fn diff_lines<'a>(
    ours: &'a [String],
    theirs: &'a [String],
) -> (Vec<(usize, &'a str, bool)>, Vec<(usize, &'a str, bool)>) {
    let max = ours.len().max(theirs.len());
    let mut ours_out = Vec::new();
    let mut theirs_out = Vec::new();

    for i in 0..max {
        let o = ours.get(i).map(|s| s.as_str()).unwrap_or("");
        let t = theirs.get(i).map(|s| s.as_str()).unwrap_or("");
        let changed = o != t;
        ours_out.push((i + 1, o, changed));
        theirs_out.push((i + 1, t, changed));
    }

    (ours_out, theirs_out)
}

// ─── TUI entry point ──────────────────────────────────────────────────────

pub fn run_tui(sdal_root: &Path, working_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let app = App::load(sdal_root, working_dir)?;
    color_eyre::install()?;

    let terminal = ratatui::init();
    let result = run_app(terminal, app);
    ratatui::restore();

    result
}

fn run_app(mut terminal: DefaultTerminal, mut app: App) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| render(&app, f))?;

        if let Event::Key(key) = event::read()? {
            handle_key(&mut app, key)?;
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}

// ─── Key handling ──────────────────────────────────────────────────────────

fn handle_key(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => {
            app.quit = true;
        }

        // Ctrl+C
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit = true;
        }

        // Scroll
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),

        // Navigate conflicts
        KeyCode::Tab => app.next_conflict(),
        KeyCode::BackTab => app.prev_conflict(),
        KeyCode::Char('n') => app.next_conflict(),
        KeyCode::Char('p') => app.prev_conflict(),

        // Select resolution
        KeyCode::Char('1') | KeyCode::Left => app.select_ours(),
        KeyCode::Char('2') | KeyCode::Right => app.select_theirs(),

        // Apply all resolutions
        KeyCode::Enter | KeyCode::Char('a') => {
            if app.all_resolved() {
                app.apply()?;
                app.quit = true;
            }
        }

        _ => {}
    }
    Ok(())
}

// ─── Rendering ─────────────────────────────────────────────────────────────

fn render(app: &App, frame: &mut Frame) {
    let size = frame.area();

    // Main layout: header, body, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(5),    // body (diff panels)
            Constraint::Length(3), // footer
        ])
        .split(size);

    render_header(app, frame, main_chunks[0]);
    render_diff_panels(app, frame, main_chunks[1]);
    render_footer(app, frame, main_chunks[2]);
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let conflict = app.current_conflict();
    let resolved = app.resolved_count();
    let total = app.conflicts.len();

    let resolution_str = match app.resolutions[app.current] {
        Some(Resolution::Ours) => " ✓ OURS",
        Some(Resolution::Theirs) => " ✓ THEIRS",
        None => " ⚠ UNRESOLVED",
    };

    let header_text = format!(
        "  Conflict {}/{} │ {} │{}",
        app.current + 1,
        total,
        conflict.path,
        resolution_str,
    );

    let progress_text = format!(" ({}/{} resolved)", resolved, total);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            header_text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            progress_text,
            Style::default().fg(if resolved == total {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" SDAL Merge Tool ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(header, area);
}

fn render_diff_panels(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let conflict = app.current_conflict();
    let (ours_diff, theirs_diff) = diff_lines(&conflict.ours_lines, &conflict.theirs_lines);

    let is_ours_selected = app.resolutions[app.current] == Some(Resolution::Ours);
    let is_theirs_selected = app.resolutions[app.current] == Some(Resolution::Theirs);

    // Ours panel
    let ours_border_color = if is_ours_selected {
        Color::Green
    } else {
        Color::DarkGray
    };
    let ours_title = if is_ours_selected {
        " OURS (current) ✓ "
    } else {
        " OURS (current) "
    };

    let ours_lines: Vec<Line> = ours_diff
        .iter()
        .map(|(num, line, changed)| {
            let line_num = Span::styled(
                format!("{:>4} │ ", num),
                Style::default().fg(Color::DarkGray),
            );
            let content = if *changed {
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White).bg(Color::Rgb(40, 60, 40)),
                )
            } else {
                Span::styled(line.to_string(), Style::default().fg(Color::Gray))
            };
            Line::from(vec![line_num, content])
        })
        .collect();

    let ours_panel = Paragraph::new(ours_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(ours_title)
                .title_style(Style::default().fg(if is_ours_selected {
                    Color::Green
                } else {
                    Color::White
                }))
                .border_style(Style::default().fg(ours_border_color)),
        )
        .scroll((app.scroll_ours, 0));

    frame.render_widget(ours_panel, chunks[0]);

    // Theirs panel
    let theirs_border_color = if is_theirs_selected {
        Color::Green
    } else {
        Color::DarkGray
    };
    let theirs_title = if is_theirs_selected {
        " THEIRS (incoming) ✓ "
    } else {
        " THEIRS (incoming) "
    };

    let theirs_lines: Vec<Line> = theirs_diff
        .iter()
        .map(|(num, line, changed)| {
            let line_num = Span::styled(
                format!("{:>4} │ ", num),
                Style::default().fg(Color::DarkGray),
            );
            let content = if *changed {
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White).bg(Color::Rgb(60, 40, 40)),
                )
            } else {
                Span::styled(line.to_string(), Style::default().fg(Color::Gray))
            };
            Line::from(vec![line_num, content])
        })
        .collect();

    let theirs_panel = Paragraph::new(theirs_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(theirs_title)
                .title_style(Style::default().fg(if is_theirs_selected {
                    Color::Green
                } else {
                    Color::White
                }))
                .border_style(Style::default().fg(theirs_border_color)),
        )
        .scroll((app.scroll_theirs, 0));

    frame.render_widget(theirs_panel, chunks[1]);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let apply_style = if app.all_resolved() {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let keys = Line::from(vec![
        Span::styled(
            " 1/← ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Ours ", Style::default().fg(Color::Gray)),
        Span::styled(
            " 2/→ ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Theirs ", Style::default().fg(Color::Gray)),
        Span::styled(
            " Tab ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Next ", Style::default().fg(Color::Gray)),
        Span::styled(
            " S-Tab ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Prev ", Style::default().fg(Color::Gray)),
        Span::styled(
            " ↑/↓ ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Scroll ", Style::default().fg(Color::Gray)),
        Span::styled(" Enter ", apply_style),
        Span::styled(
            if app.all_resolved() {
                "Apply ✓"
            } else {
                "Apply (resolve all first)"
            },
            apply_style,
        ),
        Span::styled(
            " q ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Quit", Style::default().fg(Color::Gray)),
    ]);

    let footer = Paragraph::new(keys).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(footer, area);
}
