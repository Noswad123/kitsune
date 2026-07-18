use crate::model::{
    PaneIdentity, PaneTemplate, TabIdentity, TabTemplate, WorkspaceIdentity, WorkspaceTemplate,
};
use std::path::{Path, PathBuf};
use std::process::Command;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

pub fn annotate_workspace(workspace: &mut WorkspaceTemplate) {
    for tab in &mut workspace.tabs {
        annotate_tab(tab);
    }

    let mut tab_fingerprints: Vec<String> = workspace
        .tabs
        .iter()
        .filter_map(|tab| {
            tab.identity
                .as_ref()
                .map(|identity| identity.fingerprint.clone())
        })
        .collect();
    tab_fingerprints.sort();

    let git_root = workspace.cwd.as_deref().and_then(git_root_for);
    let key = stable_key(&[
        ("kind", Some("workspace".to_string())),
        ("label", workspace.label.as_deref().map(normalize_label)),
        (
            "root",
            stable_path(git_root.as_ref().or(workspace.cwd.as_ref())),
        ),
        ("tabs", Some(tab_fingerprints.join(","))),
    ]);

    workspace.identity = Some(WorkspaceIdentity {
        label: workspace.label.as_deref().map(normalize_label),
        cwd: workspace.cwd.clone(),
        git_root,
        tab_fingerprints,
        fingerprint: fingerprint(&key),
    });
}

pub fn annotate_tab(tab: &mut TabTemplate) {
    for pane in &mut tab.panes {
        annotate_pane(pane);
    }

    let mut pane_fingerprints: Vec<String> = tab
        .panes
        .iter()
        .filter_map(|pane| {
            pane.identity
                .as_ref()
                .map(|identity| identity.fingerprint.clone())
        })
        .collect();
    pane_fingerprints.sort();

    let key = stable_key(&[
        ("kind", Some("tab".to_string())),
        ("label", tab.label.as_deref().map(normalize_label)),
        ("panes", Some(pane_fingerprints.join(","))),
    ]);

    tab.identity = Some(TabIdentity {
        label: tab.label.as_deref().map(normalize_label),
        pane_fingerprints,
        fingerprint: fingerprint(&key),
    });
}

pub fn annotate_pane(pane: &mut PaneTemplate) {
    let git_root = pane.cwd.as_deref().and_then(git_root_for);
    let normalized_label = pane.label.as_deref().map(normalize_label);
    let normalized_agent = pane.agent.as_deref().map(normalize_label);

    let key = stable_key(&[
        ("kind", Some("pane".to_string())),
        ("label", normalized_label.clone()),
        ("root", stable_path(git_root.as_ref().or(pane.cwd.as_ref()))),
        ("agent", normalized_agent.clone()),
    ]);

    pane.identity = Some(PaneIdentity {
        label: normalized_label,
        cwd: pane.cwd.clone(),
        git_root,
        agent: normalized_agent,
        fingerprint: fingerprint(&key),
    });
}

fn normalize_label(input: &str) -> String {
    input
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn stable_path(path: Option<&PathBuf>) -> Option<String> {
    path.map(|path| path.to_string_lossy().trim_end_matches('/').to_string())
}

fn git_root_for(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["-C", path.to_str()?, "rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8(output.stdout).ok()?;
    let root = root.trim();
    if root.is_empty() {
        None
    } else {
        Some(PathBuf::from(root))
    }
}

fn stable_key(parts: &[(&str, Option<String>)]) -> String {
    parts
        .iter()
        .map(|(key, value)| format!("{key}={}", value.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("|")
}

fn fingerprint(key: &str) -> String {
    let mut hash = FNV_OFFSET;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("kit-fp-v1:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::{annotate_pane, normalize_label};
    use crate::model::PaneTemplate;
    use std::path::PathBuf;

    #[test]
    fn normalizes_labels() {
        assert_eq!(normalize_label(" Agent Pane "), "agent-pane");
    }

    #[test]
    fn pane_fingerprint_ignores_command_and_rect() {
        let mut first = PaneTemplate {
            name: "agent".into(),
            label: Some("Agent".into()),
            identity: None,
            cwd: Some(PathBuf::from("/tmp")),
            command: Some("vim src/main.rs".into()),
            agent: Some("opencode".into()),
            rect: None,
            raw: None,
        };
        let mut second = PaneTemplate {
            command: Some("cargo test".into()),
            ..first.clone()
        };

        annotate_pane(&mut first);
        annotate_pane(&mut second);

        assert_eq!(
            first.identity.unwrap().fingerprint,
            second.identity.unwrap().fingerprint
        );
    }
}
