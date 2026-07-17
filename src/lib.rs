use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintKind {
    Workspace,
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
}

#[derive(Deserialize)]
struct CliResponse {
    result: ResultPayload,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResultPayload {
    WorkspaceList { workspaces: Vec<WorkspaceInfo> },
    AgentList { agents: Vec<AgentInfo> },
}

#[derive(Deserialize)]
struct WorkspaceInfo {
    workspace_id: String,
    label: String,
    agent_status: String,
    focused: bool,
}

#[derive(Deserialize)]
struct AgentInfo {
    terminal_id: String,
    name: Option<String>,
    agent: Option<String>,
    agent_status: String,
    focused: bool,
}

pub fn parse_workspaces(json: &str) -> Vec<HintItem> {
    let resp: CliResponse = serde_json::from_str(json).expect("failed to parse workspace list");
    match resp.result {
        ResultPayload::WorkspaceList { workspaces } => workspaces
            .into_iter()
            .map(|w| HintItem {
                label: ' ',
                kind: HintKind::Workspace,
                target_id: w.workspace_id,
                display_name: w.label,
                status: w.agent_status,
                focused: w.focused,
            })
            .collect(),
        _ => panic!("expected workspace_list"),
    }
}

pub fn parse_agents(json: &str) -> Vec<HintItem> {
    let resp: CliResponse = serde_json::from_str(json).expect("failed to parse agent list");
    match resp.result {
        ResultPayload::AgentList { agents } => agents
            .into_iter()
            .map(|a| {
                let display_name = a
                    .name
                    .or(a.agent)
                    .unwrap_or_else(|| a.terminal_id.clone());
                HintItem {
                    label: ' ',
                    kind: HintKind::Agent,
                    target_id: a.terminal_id,
                    display_name,
                    status: a.agent_status,
                    focused: a.focused,
                }
            })
            .collect(),
        _ => panic!("expected agent_list"),
    }
}

pub fn assign_labels(workspaces: Vec<HintItem>, agents: Vec<HintItem>) -> Vec<HintItem> {
    workspaces
        .into_iter()
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

    let workspaces: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Workspace).collect();
    let agents: Vec<_> = items.iter().filter(|i| i.kind == HintKind::Agent).collect();

    if !workspaces.is_empty() {
        out.push_str(" Workspaces\r\n\r\n");
        for item in &workspaces {
            let marker = if item.focused { "*" } else { " " };
            out.push_str(&format!(" {marker} [{label}]  {name}  ({status})\r\n",
                label = item.label,
                name = item.display_name,
                status = item.status,
            ));
        }
        out.push_str("\r\n");
    }

    if !agents.is_empty() {
        out.push_str(" Agents\r\n\r\n");
        for item in &agents {
            let marker = if item.focused { "*" } else { " " };
            out.push_str(&format!(" {marker} [{label}]  {name}  ({status})\r\n",
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

    #[test]
    fn parse_workspace_list() {
        let json = r#"{
            "id": "cli:workspace:list",
            "result": {
                "type": "workspace_list",
                "workspaces": [
                    {
                        "workspace_id": "ws-1",
                        "number": 1,
                        "label": "herdr",
                        "focused": true,
                        "pane_count": 2,
                        "tab_count": 1,
                        "active_tab_id": "tab-1",
                        "agent_status": "working"
                    },
                    {
                        "workspace_id": "ws-2",
                        "number": 2,
                        "label": "api-server",
                        "focused": false,
                        "pane_count": 1,
                        "tab_count": 1,
                        "active_tab_id": "tab-2",
                        "agent_status": "idle"
                    }
                ]
            }
        }"#;

        let items = parse_workspaces(json);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, HintKind::Workspace);
        assert_eq!(items[0].target_id, "ws-1");
        assert_eq!(items[0].display_name, "herdr");
        assert_eq!(items[0].status, "working");
        assert!(items[0].focused);
        assert_eq!(items[1].target_id, "ws-2");
        assert_eq!(items[1].display_name, "api-server");
        assert!(!items[1].focused);
    }

    #[test]
    fn assign_labels_to_items() {
        let workspaces = vec![
            HintItem {
                label: ' ',
                kind: HintKind::Workspace,
                target_id: "ws-1".into(),
                display_name: "herdr".into(),
                status: "working".into(),
                focused: true,
            },
        ];
        let agents = vec![
            HintItem {
                label: ' ',
                kind: HintKind::Agent,
                target_id: "term-1".into(),
                display_name: "claude".into(),
                status: "idle".into(),
                focused: false,
            },
            HintItem {
                label: ' ',
                kind: HintKind::Agent,
                target_id: "term-2".into(),
                display_name: "codex".into(),
                status: "working".into(),
                focused: false,
            },
        ];

        let items = assign_labels(workspaces, agents);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].label, 'a');
        assert_eq!(items[0].kind, HintKind::Workspace);
        assert_eq!(items[1].label, 'b');
        assert_eq!(items[1].kind, HintKind::Agent);
        assert_eq!(items[2].label, 'c');
    }

    #[test]
    fn assign_labels_caps_at_26() {
        let workspaces: Vec<HintItem> = (0..30)
            .map(|i| HintItem {
                label: ' ',
                kind: HintKind::Workspace,
                target_id: format!("ws-{i}"),
                display_name: format!("workspace-{i}"),
                status: "idle".into(),
                focused: false,
            })
            .collect();

        let items = assign_labels(workspaces, vec![]);

        assert_eq!(items.len(), 26);
        assert_eq!(items[0].label, 'a');
        assert_eq!(items[25].label, 'z');
    }

    #[test]
    fn resolve_input_finds_matching_item() {
        let items = vec![
            HintItem {
                label: 'a',
                kind: HintKind::Workspace,
                target_id: "ws-1".into(),
                display_name: "herdr".into(),
                status: "working".into(),
                focused: true,
            },
            HintItem {
                label: 'b',
                kind: HintKind::Agent,
                target_id: "term-1".into(),
                display_name: "claude".into(),
                status: "idle".into(),
                focused: false,
            },
        ];

        let found = resolve_input(&items, 'b');
        assert_eq!(found.unwrap().target_id, "term-1");
    }

    #[test]
    fn resolve_input_returns_none_for_unknown_key() {
        let items = vec![HintItem {
            label: 'a',
            kind: HintKind::Workspace,
            target_id: "ws-1".into(),
            display_name: "herdr".into(),
            status: "idle".into(),
            focused: false,
        }];

        assert!(resolve_input(&items, 'z').is_none());
    }

    #[test]
    fn render_items_produces_lines() {
        let items = vec![
            HintItem {
                label: 'a',
                kind: HintKind::Workspace,
                target_id: "ws-1".into(),
                display_name: "herdr".into(),
                status: "working".into(),
                focused: true,
            },
            HintItem {
                label: 'b',
                kind: HintKind::Agent,
                target_id: "term-1".into(),
                display_name: "claude".into(),
                status: "idle".into(),
                focused: false,
            },
        ];

        let output = render(&items);

        assert!(output.contains("[a]"));
        assert!(output.contains("herdr"));
        assert!(output.contains("working"));
        assert!(output.contains("[b]"));
        assert!(output.contains("claude"));
    }

    #[test]
    fn parse_agent_list() {
        let json = r#"{
            "id": "cli:agent:list",
            "result": {
                "type": "agent_list",
                "agents": [
                    {
                        "terminal_id": "term-1",
                        "name": "my-agent",
                        "agent": "claude-code",
                        "agent_status": "working",
                        "workspace_id": "ws-1",
                        "tab_id": "tab-1",
                        "pane_id": "pane-1",
                        "focused": true,
                        "screen_detection_skipped": false,
                        "revision": 1
                    },
                    {
                        "terminal_id": "term-2",
                        "name": null,
                        "agent": "codex",
                        "agent_status": "idle",
                        "workspace_id": "ws-1",
                        "tab_id": "tab-1",
                        "pane_id": "pane-2",
                        "focused": false,
                        "screen_detection_skipped": false,
                        "revision": 2
                    },
                    {
                        "terminal_id": "term-3",
                        "name": null,
                        "agent": null,
                        "agent_status": "unknown",
                        "workspace_id": "ws-2",
                        "tab_id": "tab-2",
                        "pane_id": "pane-3",
                        "focused": false,
                        "screen_detection_skipped": false,
                        "revision": 3
                    }
                ]
            }
        }"#;

        let items = parse_agents(json);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].display_name, "my-agent");
        assert_eq!(items[1].display_name, "codex");
        assert_eq!(items[2].display_name, "term-3");
    }
}
