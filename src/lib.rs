use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintKind {
    Tab,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintItem {
    pub label: String,
    pub kind: HintKind,
    pub target_id: String,
    pub display_name: String,
    pub status: String,
    pub focused: bool,
    pub context: Option<String>,
    pub group: Option<String>,
    pub title: Option<String>,
}

#[derive(Deserialize)]
struct CliResponse {
    result: ResultPayload,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResultPayload {
    WorkspaceList { workspaces: Vec<WorkspaceInfo> },
    TabList { tabs: Vec<TabInfo> },
    AgentList { agents: Vec<AgentInfo> },
}

#[derive(Deserialize)]
struct WorkspaceInfo {
    workspace_id: String,
    label: String,
}

#[derive(Deserialize)]
struct TabInfo {
    tab_id: String,
    label: String,
    workspace_id: String,
    focused: bool,
}

#[derive(Deserialize)]
struct AgentInfo {
    pane_id: String,
    name: Option<String>,
    agent: Option<String>,
    agent_status: String,
    cwd: Option<String>,
    focused: bool,
    terminal_title_stripped: Option<String>,
}

pub fn parse_workspace_labels(json: &str) -> Vec<(String, String)> {
    let resp: CliResponse = serde_json::from_str(json).expect("failed to parse workspace list");
    match resp.result {
        ResultPayload::WorkspaceList { workspaces } => workspaces
            .into_iter()
            .map(|w| (w.workspace_id, w.label))
            .collect(),
        _ => panic!("expected workspace_list"),
    }
}

pub fn parse_tabs(json: &str, workspace_labels: &[(String, String)]) -> Vec<HintItem> {
    let resp: CliResponse = serde_json::from_str(json).expect("failed to parse tab list");
    match resp.result {
        ResultPayload::TabList { tabs } => tabs
            .into_iter()
            .map(|t| {
                let group = workspace_labels
                    .iter()
                    .find(|(id, _)| id == &t.workspace_id)
                    .map(|(_, label)| label.clone());
                HintItem {
                    label: String::new(),
                    kind: HintKind::Tab,
                    target_id: t.tab_id,
                    display_name: t.label,
                    status: String::new(),
                    focused: t.focused,
                    context: None,
                    group,
                    title: None,
                }
            })
            .collect(),
        _ => panic!("expected tab_list"),
    }
}

pub fn parse_agents(json: &str, resolve_context: &dyn Fn(&str) -> Option<String>) -> Vec<HintItem> {
    let resp: CliResponse = serde_json::from_str(json).expect("failed to parse agent list");
    match resp.result {
        ResultPayload::AgentList { agents } => agents
            .into_iter()
            .map(|a| {
                let display_name = a
                    .name
                    .or(a.agent)
                    .unwrap_or_else(|| a.pane_id.clone());
                let context = a.cwd.as_deref().and_then(|cwd| resolve_context(cwd));
                HintItem {
                    label: String::new(),
                    kind: HintKind::Agent,
                    target_id: a.pane_id,
                    display_name,
                    status: a.agent_status,
                    focused: a.focused,
                    context,
                    group: None,
                    title: a.terminal_title_stripped,
                }
            })
            .collect(),
        _ => panic!("expected agent_list"),
    }
}

fn repo_name_from_paths(toplevel: &str, common_dir: Option<&str>) -> Option<String> {
    if let Some(cd) = common_dir {
        if cd.starts_with('/') {
            if let Some(name) = std::path::Path::new(cd)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            {
                return Some(name.to_string());
            }
        }
    }
    std::path::Path::new(toplevel)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

pub fn git_context(cwd: &str) -> Option<String> {
    use std::process::Command;
    let toplevel = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

    let common_dir = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let repo_name = repo_name_from_paths(&toplevel, common_dir.as_deref())?;

    let branch = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

    Some(format!("{repo_name}:{branch}"))
}

fn single_label(i: usize) -> String {
    String::from((b'a' + i as u8) as char)
}

fn double_label(i: usize) -> String {
    let first = (b'a' + (i / 26) as u8) as char;
    let second = (b'a' + (i % 26) as u8) as char;
    format!("{first}{second}")
}

pub fn assign_labels(tabs: Vec<HintItem>, agents: Vec<HintItem>) -> Vec<HintItem> {
    let all: Vec<_> = tabs.into_iter().chain(agents).collect();
    let use_double = all.len() > 26;
    all.into_iter()
        .enumerate()
        .map(|(i, mut item)| {
            item.label = if use_double {
                double_label(i)
            } else {
                single_label(i)
            };
            item
        })
        .collect()
}

fn tab_cell(item: &HintItem) -> String {
    let marker = if item.focused { "*" } else { " " };
    format!("{marker} [{label}]  {name}", label = item.label, name = item.display_name)
}

pub fn render(items: &[HintItem]) -> String {
    let mut out = String::new();

    let tabs: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Tab).collect();
    let agents: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Agent).collect();

    if !tabs.is_empty() {
        let mut groups: Vec<(&str, Vec<&HintItem>)> = Vec::new();
        for item in &tabs {
            let group = item.group.as_deref().unwrap_or("?");
            if let Some(last) = groups.last_mut() {
                if last.0 == group {
                    last.1.push(item);
                    continue;
                }
            }
            groups.push((group, vec![item]));
        }

        for (i, (group, group_tabs)) in groups.iter().enumerate() {
            if i > 0 {
                out.push_str("\r\n");
            }
            out.push_str(&format!(" {group}\r\n"));

            let col_width = group_tabs.iter()
                .map(|item| tab_cell(item).len())
                .max()
                .unwrap_or(0) + 2;

            for chunk in group_tabs.chunks(2) {
                let cell1 = tab_cell(chunk[0]);
                if let Some(second) = chunk.get(1) {
                    let cell2 = tab_cell(second);
                    out.push_str(&format!("   {cell1:<col_width$}{cell2}\r\n"));
                } else {
                    out.push_str(&format!("   {cell1}\r\n"));
                }
            }
        }
        out.push_str("\r\n");
    }

    if !agents.is_empty() {
        out.push_str(" Agents\r\n");
        let status_width = agents.iter().map(|i| i.status.len()).max().unwrap_or(0);
        for item in &agents {
            let marker = if item.focused { "*" } else { " " };
            let ctx = item.context.as_deref().unwrap_or("");
            let title = item.title.as_deref().unwrap_or("");
            let dim_title = if title.is_empty() {
                String::new()
            } else {
                format!("\x1b[2m{title}\x1b[0m")
            };
            out.push_str(&format!("   {marker} [{label}]  {status:<status_width$}  {ctx}  {dim_title}\r\n",
                status = item.status,
                label = item.label,
            ));
        }
    }

    out
}

pub fn resolve_input<'a>(items: &'a [HintItem], input: &str) -> Option<&'a HintItem> {
    items.iter().find(|item| item.label == input)
}

pub fn uses_double_labels(items: &[HintItem]) -> bool {
    items.first().map_or(false, |item| item.label.len() > 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(target_id: &str, name: &str, group: &str, focused: bool) -> HintItem {
        HintItem {
            label: String::new(),
            kind: HintKind::Tab,
            target_id: target_id.into(),
            display_name: name.into(),
            status: String::new(),
            focused,
            context: None,
            group: Some(group.into()),
            title: None,
        }
    }

    fn agent(target_id: &str, name: &str, status: &str, context: Option<&str>) -> HintItem {
        HintItem {
            label: String::new(),
            kind: HintKind::Agent,
            target_id: target_id.into(),
            display_name: name.into(),
            status: status.into(),
            focused: false,
            context: context.map(Into::into),
            group: None,
            title: None,
        }
    }

    #[test]
    fn parse_tab_list() {
        let ws_json = r#"{
            "id": "cli:workspace:list",
            "result": {
                "type": "workspace_list",
                "workspaces": [
                    { "workspace_id": "w7", "number": 1, "label": "herdr", "focused": true, "pane_count": 2, "tab_count": 2, "active_tab_id": "w7:t1", "agent_status": "working" },
                    { "workspace_id": "w9", "number": 2, "label": "ga-pms", "focused": false, "pane_count": 1, "tab_count": 1, "active_tab_id": "w9:t1", "agent_status": "idle" }
                ]
            }
        }"#;
        let tab_json = r#"{
            "id": "cli:tab:list",
            "result": {
                "type": "tab_list",
                "tabs": [
                    { "tab_id": "w7:t1", "label": "1", "number": 1, "workspace_id": "w7", "focused": true, "pane_count": 2, "agent_status": "working" },
                    { "tab_id": "w7:t2", "label": "2", "number": 2, "workspace_id": "w7", "focused": false, "pane_count": 1, "agent_status": "unknown" },
                    { "tab_id": "w9:t1", "label": "1", "number": 1, "workspace_id": "w9", "focused": false, "pane_count": 1, "agent_status": "idle" }
                ]
            }
        }"#;

        let ws_labels = parse_workspace_labels(ws_json);
        let items = parse_tabs(tab_json, &ws_labels);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, HintKind::Tab);
        assert_eq!(items[0].target_id, "w7:t1");
        assert_eq!(items[0].display_name, "1");
        assert_eq!(items[0].group, Some("herdr".into()));
        assert!(items[0].focused);
        assert_eq!(items[2].group, Some("ga-pms".into()));
    }

    #[test]
    fn assign_labels_tabs_then_agents() {
        let tabs = vec![tab("w7:t1", "1", "herdr", true)];
        let agents = vec![
            agent("term-1", "claude", "idle", None),
            agent("term-2", "codex", "working", None),
        ];

        let items = assign_labels(tabs, agents);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].label, "a");
        assert_eq!(items[0].kind, HintKind::Tab);
        assert_eq!(items[1].label, "b");
        assert_eq!(items[1].kind, HintKind::Agent);
        assert_eq!(items[2].label, "c");
    }

    #[test]
    fn assign_labels_double_when_over_26() {
        let tabs: Vec<HintItem> = (0..30)
            .map(|i| tab(&format!("t-{i}"), &format!("tab-{i}"), "ws", false))
            .collect();

        let items = assign_labels(tabs, vec![]);

        assert_eq!(items.len(), 30);
        assert_eq!(items[0].label, "aa");
        assert_eq!(items[1].label, "ab");
        assert_eq!(items[25].label, "az");
        assert_eq!(items[26].label, "ba");
        assert_eq!(items[29].label, "bd");
    }

    #[test]
    fn assign_labels_all_double_when_agents_push_over_26() {
        let tabs: Vec<HintItem> = (0..26)
            .map(|i| tab(&format!("t-{i}"), &format!("{i}"), "ws", false))
            .collect();
        let agents = vec![agent("term-1", "claude", "idle", None)];

        let items = assign_labels(tabs, agents);

        assert_eq!(items.len(), 27);
        assert_eq!(items[0].label, "aa");
        assert_eq!(items[26].label, "ba");
        assert_eq!(items[26].kind, HintKind::Agent);
    }

    #[test]
    fn resolve_input_finds_matching_item() {
        let items = vec![
            HintItem { label: "a".into(), ..tab("w7:t1", "1", "herdr", true) },
            HintItem { label: "b".into(), ..agent("term-1", "claude", "idle", None) },
        ];

        let found = resolve_input(&items, "b");
        assert_eq!(found.unwrap().target_id, "term-1");
    }

    #[test]
    fn resolve_input_returns_none_for_unknown_key() {
        let items = vec![HintItem { label: "a".into(), ..tab("w7:t1", "1", "herdr", false) }];
        assert!(resolve_input(&items, "z").is_none());
    }

    #[test]
    fn render_groups_tabs_by_workspace() {
        let items = vec![
            HintItem { label: "a".into(), ..tab("w7:t1", "1", "herdr", true) },
            HintItem { label: "b".into(), ..tab("w7:t2", "2", "herdr", false) },
            HintItem { label: "c".into(), ..tab("w9:t1", "1", "ga-pms", false) },
            HintItem { label: "d".into(), ..agent("term-1", "claude", "working", Some("herdr:main")) },
        ];

        let output = render(&items);

        assert!(output.contains(" herdr\r\n"));
        assert!(output.contains("[a]  1"));
        assert!(output.contains("[b]  2"));
        assert!(output.contains(" ga-pms\r\n"));
        assert!(output.contains("[c]  1"));
        assert!(output.contains("Agents"));
        assert!(output.contains("working"));
        assert!(output.contains("[d]"));
        assert!(output.contains("herdr:main"));
    }

    #[test]
    fn parse_agent_list_with_context() {
        let ag_json = r#"{
            "id": "cli:agent:list",
            "result": {
                "type": "agent_list",
                "agents": [
                    { "terminal_id": "term-1", "name": "my-agent", "agent": "claude-code", "agent_status": "working", "cwd": "/home/user/repo-a", "workspace_id": "ws-1", "tab_id": "t1", "pane_id": "ws-1:p1", "focused": true, "screen_detection_skipped": false, "revision": 1, "terminal_title_stripped": "Fix login bug" },
                    { "terminal_id": "term-2", "name": null, "agent": "claude", "agent_status": "idle", "cwd": "/home/user/repo-b", "workspace_id": "ws-1", "tab_id": "t1", "pane_id": "ws-1:p2", "focused": false, "screen_detection_skipped": false, "revision": 2 }
                ]
            }
        }"#;

        let resolver = |cwd: &str| -> Option<String> {
            match cwd {
                "/home/user/repo-a" => Some("repo-a:main".into()),
                "/home/user/repo-b" => Some("repo-b:feature".into()),
                _ => None,
            }
        };
        let items = parse_agents(ag_json, &resolver);

        assert_eq!(items.len(), 2);

        assert_eq!(items[0].target_id, "ws-1:p1");
        assert_eq!(items[0].display_name, "my-agent");
        assert_eq!(items[0].context, Some("repo-a:main".into()));
        assert_eq!(items[0].title, Some("Fix login bug".into()));
        assert_eq!(items[1].target_id, "ws-1:p2");
        assert_eq!(items[1].display_name, "claude");
        assert_eq!(items[1].context, Some("repo-b:feature".into()));
        assert_eq!(items[1].title, None);
    }

    #[test]
    fn render_tabs_in_two_columns() {
        let items = vec![
            HintItem { label: "a".into(), ..tab("w7:t1", "1", "herdr", true) },
            HintItem { label: "b".into(), ..tab("w7:t2", "2", "herdr", false) },
            HintItem { label: "c".into(), ..tab("w7:t3", "3", "herdr", false) },
            HintItem { label: "d".into(), ..tab("w7:t4", "4", "herdr", false) },
        ];

        let output = render(&items);
        let lines: Vec<&str> = output.split("\r\n").collect();

        let row1 = lines.iter().find(|l| l.contains("[a]")).unwrap();
        assert!(row1.contains("[b]"), "First row should contain both [a] and [b]");
        let row2 = lines.iter().find(|l| l.contains("[c]")).unwrap();
        assert!(row2.contains("[d]"), "Second row should contain both [c] and [d]");
    }

    #[test]
    fn render_tabs_two_columns_odd_count() {
        let items = vec![
            HintItem { label: "a".into(), ..tab("w7:t1", "1", "herdr", false) },
            HintItem { label: "b".into(), ..tab("w7:t2", "2", "herdr", false) },
            HintItem { label: "c".into(), ..tab("w7:t3", "3", "herdr", false) },
        ];

        let output = render(&items);
        let lines: Vec<&str> = output.split("\r\n").collect();

        let row1 = lines.iter().find(|l| l.contains("[a]")).unwrap();
        assert!(row1.contains("[b]"), "First row should contain both [a] and [b]");
        let row2 = lines.iter().find(|l| l.contains("[c]")).unwrap();
        assert!(!row2.contains("[a]") && !row2.contains("[b]"), "[c] should be alone");
    }

    #[test]
    fn repo_name_from_paths_normal_repo() {
        let name = repo_name_from_paths("/home/user/ga-pms", Some(".git"));
        assert_eq!(name, Some("ga-pms".to_string()));
    }

    #[test]
    fn repo_name_from_paths_worktree() {
        let name = repo_name_from_paths(
            "/tmp/worktrees/research",
            Some("/home/user/ga-pms/.git"),
        );
        assert_eq!(name, Some("ga-pms".to_string()));
    }

    #[test]
    fn repo_name_from_paths_no_common_dir() {
        let name = repo_name_from_paths("/home/user/my-repo", None);
        assert_eq!(name, Some("my-repo".to_string()));
    }

    #[test]
    fn render_agents_single_line_with_status_and_title() {
        let mut a1 = agent("term-1", "claude", "idle", Some("herdr:main"));
        a1.title = Some("Fix login bug".into());
        let mut a2 = agent("term-2", "claude", "working", Some("ga-pms:feat"));
        a2.title = Some("Add feature".into());
        a2.focused = true;
        let items = vec![
            HintItem { label: "a".into(), ..a1 },
            HintItem { label: "b".into(), ..a2 },
        ];

        let output = render(&items);
        let line_a = output.split("\r\n").find(|l| l.contains("[a]")).unwrap();
        let line_b = output.split("\r\n").find(|l| l.contains("[b]")).unwrap();

        assert!(line_a.contains("idle"));
        assert!(line_a.contains("herdr:main"));
        assert!(line_a.contains("Fix login bug"));
        assert!(!line_a.contains("claude"));

        assert!(line_b.contains("working"));
        assert!(line_b.contains("ga-pms:feat"));
        assert!(line_b.contains("Add feature"));
    }
}
