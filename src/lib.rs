use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintKind {
    Tab,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintItem {
    pub label: char,
    pub kind: HintKind,
    pub target_id: String,
    pub display_name: String,
    pub status: String,
    pub focused: bool,
    pub context: Option<String>,
    pub group: Option<String>,
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
    terminal_id: String,
    name: Option<String>,
    agent: Option<String>,
    agent_status: String,
    cwd: Option<String>,
    focused: bool,
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
                    label: ' ',
                    kind: HintKind::Tab,
                    target_id: t.tab_id,
                    display_name: t.label,
                    status: String::new(),
                    focused: t.focused,
                    context: None,
                    group,
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
                    .unwrap_or_else(|| a.terminal_id.clone());
                let context = a.cwd.as_deref().and_then(|cwd| resolve_context(cwd));
                HintItem {
                    label: ' ',
                    kind: HintKind::Agent,
                    target_id: a.terminal_id,
                    display_name,
                    status: a.agent_status,
                    focused: a.focused,
                    context,
                    group: None,
                }
            })
            .collect(),
        _ => panic!("expected agent_list"),
    }
}

pub fn git_context(cwd: &str) -> Option<String> {
    use std::process::Command;
    let repo = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

    let repo_name = std::path::Path::new(&repo).file_name()?.to_str()?;

    let branch = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;

    Some(format!("{repo_name}:{branch}"))
}

pub fn assign_labels(tabs: Vec<HintItem>, agents: Vec<HintItem>) -> Vec<HintItem> {
    tabs.into_iter()
        .chain(agents)
        .take(26)
        .enumerate()
        .map(|(i, mut item)| {
            item.label = (b'a' + i as u8) as char;
            item
        })
        .collect()
}

pub fn render(items: &[HintItem]) -> String {
    let mut out = String::new();

    let tabs: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Tab).collect();
    let agents: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Agent).collect();

    if !tabs.is_empty() {
        let mut current_group: Option<&str> = None;
        for item in &tabs {
            let group = item.group.as_deref().unwrap_or("?");
            if current_group != Some(group) {
                if current_group.is_some() {
                    out.push_str("\r\n");
                }
                out.push_str(&format!(" {group}\r\n"));
                current_group = Some(group);
            }
            let marker = if item.focused { "*" } else { " " };
            out.push_str(&format!("   {marker} [{label}]  {name}\r\n",
                label = item.label,
                name = item.display_name,
            ));
        }
        out.push_str("\r\n");
    }

    if !agents.is_empty() {
        out.push_str(" Agents\r\n");
        for item in &agents {
            let marker = if item.focused { "*" } else { " " };
            let ctx = item.context.as_deref().unwrap_or("");
            out.push_str(&format!("   {marker} [{label}]  {name}  {ctx}  ({status})\r\n",
                label = item.label,
                name = item.display_name,
                status = item.status,
            ));
        }
    }

    out
}

pub fn resolve_input(items: &[HintItem], ch: char) -> Option<&HintItem> {
    items.iter().find(|item| item.label == ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(target_id: &str, name: &str, group: &str, focused: bool) -> HintItem {
        HintItem {
            label: ' ',
            kind: HintKind::Tab,
            target_id: target_id.into(),
            display_name: name.into(),
            status: String::new(),
            focused,
            context: None,
            group: Some(group.into()),
        }
    }

    fn agent(target_id: &str, name: &str, status: &str, context: Option<&str>) -> HintItem {
        HintItem {
            label: ' ',
            kind: HintKind::Agent,
            target_id: target_id.into(),
            display_name: name.into(),
            status: status.into(),
            focused: false,
            context: context.map(Into::into),
            group: None,
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
        assert_eq!(items[0].label, 'a');
        assert_eq!(items[0].kind, HintKind::Tab);
        assert_eq!(items[1].label, 'b');
        assert_eq!(items[1].kind, HintKind::Agent);
        assert_eq!(items[2].label, 'c');
    }

    #[test]
    fn assign_labels_caps_at_26() {
        let tabs: Vec<HintItem> = (0..30)
            .map(|i| tab(&format!("t-{i}"), &format!("tab-{i}"), "ws", false))
            .collect();

        let items = assign_labels(tabs, vec![]);

        assert_eq!(items.len(), 26);
        assert_eq!(items[0].label, 'a');
        assert_eq!(items[25].label, 'z');
    }

    #[test]
    fn resolve_input_finds_matching_item() {
        let items = vec![
            HintItem { label: 'a', ..tab("w7:t1", "1", "herdr", true) },
            HintItem { label: 'b', ..agent("term-1", "claude", "idle", None) },
        ];

        let found = resolve_input(&items, 'b');
        assert_eq!(found.unwrap().target_id, "term-1");
    }

    #[test]
    fn resolve_input_returns_none_for_unknown_key() {
        let items = vec![HintItem { label: 'a', ..tab("w7:t1", "1", "herdr", false) }];
        assert!(resolve_input(&items, 'z').is_none());
    }

    #[test]
    fn render_groups_tabs_by_workspace() {
        let items = vec![
            HintItem { label: 'a', ..tab("w7:t1", "1", "herdr", true) },
            HintItem { label: 'b', ..tab("w7:t2", "2", "herdr", false) },
            HintItem { label: 'c', ..tab("w9:t1", "1", "ga-pms", false) },
            HintItem { label: 'd', ..agent("term-1", "claude", "working", Some("herdr:main")) },
        ];

        let output = render(&items);

        assert!(output.contains(" herdr\r\n"));
        assert!(output.contains("[a]  1"));
        assert!(output.contains("[b]  2"));
        assert!(output.contains(" ga-pms\r\n"));
        assert!(output.contains("[c]  1"));
        assert!(output.contains("Agents"));
        assert!(output.contains("     [d]  claude  herdr:main  (working)"));
    }

    #[test]
    fn parse_agent_list_with_context() {
        let ag_json = r#"{
            "id": "cli:agent:list",
            "result": {
                "type": "agent_list",
                "agents": [
                    { "terminal_id": "term-1", "name": "my-agent", "agent": "claude-code", "agent_status": "working", "cwd": "/home/user/repo-a", "workspace_id": "ws-1", "tab_id": "t1", "pane_id": "p1", "focused": true, "screen_detection_skipped": false, "revision": 1 },
                    { "terminal_id": "term-2", "name": null, "agent": "claude", "agent_status": "idle", "cwd": "/home/user/repo-b", "workspace_id": "ws-1", "tab_id": "t1", "pane_id": "p2", "focused": false, "screen_detection_skipped": false, "revision": 2 }
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
        assert_eq!(items[0].display_name, "my-agent");
        assert_eq!(items[0].context, Some("repo-a:main".into()));
        assert_eq!(items[1].display_name, "claude");
        assert_eq!(items[1].context, Some("repo-b:feature".into()));
    }

    #[test]
    fn render_agents_shows_context() {
        let items = vec![
            HintItem { label: 'a', ..agent("term-1", "claude", "idle", Some("herdr:main")) },
            HintItem { label: 'b', ..agent("term-2", "claude", "idle", Some("ga-pms:feature")) },
        ];

        let output = render(&items);

        assert!(output.contains("claude  herdr:main"));
        assert!(output.contains("claude  ga-pms:feature"));
    }
}
