use std::io::{self, Write};
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal;

use herdr_hint::{assign_labels, parse_agents, parse_workspaces, render, resolve_input, HintKind};

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

    let workspaces = fetch_json(&herdr, &["workspace", "list"])
        .map(|json| parse_workspaces(&json))
        .unwrap_or_default();

    let agents = fetch_json(&herdr, &["agent", "list"])
        .map(|json| parse_agents(&json))
        .unwrap_or_default();

    let items = assign_labels(workspaces, agents);

    if items.is_empty() {
        return;
    }

    let output = render(&items);

    terminal::enable_raw_mode().expect("failed to enable raw mode");

    print!("\x1b[2J\x1b[H");
    print!("{output}");
    io::stdout().flush().unwrap();

    let selected = loop {
        if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
            match code {
                KeyCode::Char(ch) => break resolve_input(&items, ch).cloned(),
                KeyCode::Esc => break None,
                _ => {}
            }
        }
    };

    terminal::disable_raw_mode().expect("failed to disable raw mode");

    if let Some(item) = selected {
        let (cmd, target) = match item.kind {
            HintKind::Workspace => ("workspace", item.target_id.as_str()),
            HintKind::Agent => ("agent", item.target_id.as_str()),
        };
        let _ = Command::new(&herdr)
            .args([cmd, "focus", target])
            .status();
    }
}
