use std::io::{self, Write};
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
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

    terminal::enable_raw_mode().expect("failed to enable raw mode");

    print!("\x1b[?25l\x1b[2J\x1b[H");
    print!("{output}");
    io::stdout().flush().unwrap();

    let double = uses_double_labels(&items);

    let selected = loop {
        if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
            match code {
                KeyCode::Char(first) if double => {
                    if let Ok(Event::Key(KeyEvent { code: KeyCode::Char(second), .. })) = event::read() {
                        let input = format!("{first}{second}");
                        break resolve_input(&items, &input).cloned();
                    } else {
                        break None;
                    }
                }
                KeyCode::Char(ch) => {
                    let input = String::from(ch);
                    break resolve_input(&items, &input).cloned();
                }
                KeyCode::Esc => break None,
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
