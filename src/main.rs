use std::io::{self, Write};
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;

use herdr_hint::{
    assign_labels, git_context, parse_agents, parse_tabs, parse_workspace_labels, render,
    resolve_input, uses_double_labels, HintKind,
};

fn herdr_bin() -> String {
    std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into())
}

fn fetch_json(herdr: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(herdr).args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

fn main() {
    let herdr = herdr_bin();

    let ws_json = fetch_json(&herdr, &["workspace", "list"]);
    let workspace_labels = ws_json
        .as_deref()
        .map(parse_workspace_labels)
        .unwrap_or_default();

    let tabs = fetch_json(&herdr, &["tab", "list"])
        .map(|json| parse_tabs(&json, &workspace_labels))
        .unwrap_or_default();

    let agents = fetch_json(&herdr, &["agent", "list"])
        .map(|json| parse_agents(&json, &git_context))
        .unwrap_or_default();

    let items = assign_labels(tabs, agents);

    if items.is_empty() {
        return;
    }

    let output = render(&items);
    let lines: Vec<&str> = output.split("\r\n").collect();
    let (_, term_height) = terminal::size().unwrap_or((80, 24));
    let visible = (term_height as usize).saturating_sub(1);
    let max_offset = lines.len().saturating_sub(visible);
    let mut offset: usize = 0;

    let draw = |offset: usize| {
        print!("\x1b[2J\x1b[H");
        let end = (offset + visible).min(lines.len());
        for line in &lines[offset..end] {
            print!("{line}\r\n");
        }
        io::stdout().flush().unwrap();
    };

    terminal::enable_raw_mode().expect("failed to enable raw mode");
    print!("\x1b[?25l");
    io::stdout().flush().unwrap();
    draw(offset);

    let double = uses_double_labels(&items);

    let selected = loop {
        if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
            match (code, modifiers) {
                (KeyCode::Char('d'), m) if m.contains(KeyModifiers::CONTROL) => {
                    offset = (offset + visible / 2).min(max_offset);
                    draw(offset);
                }
                (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
                    offset = offset.saturating_sub(visible / 2);
                    draw(offset);
                }
                (KeyCode::Char(first), _) if double => {
                    if let Ok(Event::Key(KeyEvent { code: KeyCode::Char(second), .. })) = event::read() {
                        let input = format!("{first}{second}");
                        break resolve_input(&items, &input).cloned();
                    } else {
                        break None;
                    }
                }
                (KeyCode::Char(ch), _) => {
                    let input = String::from(ch);
                    break resolve_input(&items, &input).cloned();
                }
                (KeyCode::Esc, _) => break None,
                _ => {}
            }
        }
    };

    print!("\x1b[?25h");
    io::stdout().flush().unwrap();
    terminal::disable_raw_mode().expect("failed to disable raw mode");

    if let Some(item) = selected {
        let (cmd, target) = match item.kind {
            HintKind::Tab => ("tab", item.target_id.as_str()),
            HintKind::Agent => ("agent", item.target_id.as_str()),
        };
        let _ = Command::new(&herdr)
            .args([cmd, "focus", target])
            .status();
    }
}
