use super::{Backend, DoctorReport};
use crate::model::{
    BackendKind, BackendRef, Direction, LayoutTemplate, ObservedState, PaneTemplate, Rect,
    TabCapture, TabTemplate, WorkspaceCapture, WorkspaceTemplate,
};
use crate::store::slug;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TmuxBackend;

impl TmuxBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn is_detected() -> bool {
        std::env::var_os("TMUX").is_some() || which::which("tmux").is_ok()
    }

    fn output(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("tmux")
            .args(args)
            .output()
            .with_context(|| format!("running tmux {}", args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "tmux {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    }

    fn current_session_name(&self) -> Result<String> {
        self.output(&["display-message", "-p", "#{session_name}"])
    }

    fn current_window_id(&self) -> Result<String> {
        self.output(&["display-message", "-p", "#{window_id}"])
    }

    fn current_pane_id(&self) -> Result<String> {
        self.output(&["display-message", "-p", "#{pane_id}"])
    }

    fn session_names(&self) -> Result<Vec<String>> {
        Ok(self
            .output(&["list-sessions", "-F", "#{session_name}"])?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect())
    }

    fn capture_session(
        &self,
        session_name: &str,
        name: Option<String>,
    ) -> Result<WorkspaceCapture> {
        let template_name = name.unwrap_or_else(|| slug(session_name));
        let windows = self.output(&[
            "list-windows",
            "-t",
            session_name,
            "-F",
            "#{window_id}\t#{window_index}\t#{window_name}\t#{window_active}",
        ])?;

        let mut tabs = vec![];
        let mut workspace_cwd = None;
        let mut tab_names_seen = HashMap::<String, usize>::new();
        let mut pane_names_seen = HashMap::<String, usize>::new();
        for line in windows.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            let window_id = field(&fields, 0, "window_id")?;
            let window_index = field(&fields, 1, "window_index")?;
            let window_name = field(&fields, 2, "window_name")?;
            let window_active = field(&fields, 3, "window_active").unwrap_or("0") == "1";
            let tab_base = prefixed_name(&template_name, &slug(window_name));
            let tab_name = unique_name(&tab_base, &mut tab_names_seen);
            let panes = self.capture_window_panes(
                session_name,
                window_index,
                window_id,
                &tab_name,
                &mut pane_names_seen,
            )?;
            if workspace_cwd.is_none() {
                workspace_cwd = panes.first().and_then(|pane| pane.cwd.clone());
            }
            let area = window_area(&panes);
            let tab_cwd = panes.first().and_then(|pane| pane.cwd.clone());
            tabs.push(TabCapture {
                tab: TabTemplate {
                    name: tab_name,
                    label: (window_name != window_index).then(|| window_name.to_string()),
                    identity: None,
                    cwd: tab_cwd,
                    panes: vec![],
                    layout: LayoutTemplate {
                        area,
                        splits: vec![],
                    },
                    actions: Default::default(),
                    backend_ref: Some(BackendRef {
                        workspace_id: Some(session_name.to_string()),
                        tab_id: Some(window_id.to_string()),
                        pane_id: None,
                        focused: window_active,
                    }),
                },
                panes,
            });
        }

        Ok(WorkspaceCapture {
            workspace: WorkspaceTemplate {
                schema: "kitsune.workspace.v1".into(),
                name: template_name,
                label: None,
                identity: None,
                backend: BackendKind::Tmux,
                cwd: workspace_cwd,
                saved_at: Utc::now(),
                tabs: vec![],
                actions: Default::default(),
                backend_ref: Some(BackendRef {
                    workspace_id: Some(session_name.to_string()),
                    tab_id: None,
                    pane_id: None,
                    focused: false,
                }),
            },
            tabs,
        })
    }

    fn capture_window_panes(
        &self,
        session_name: &str,
        window_index: &str,
        window_id: &str,
        tab_name: &str,
        pane_names_seen: &mut HashMap<String, usize>,
    ) -> Result<Vec<PaneTemplate>> {
        let target = format!("{session_name}:{window_index}");
        let output = self.output(&[
            "list-panes",
            "-t",
            &target,
            "-F",
            "#{pane_id}\t#{pane_index}\t#{pane_title}\t#{pane_current_path}\t#{pane_current_command}\t#{pane_active}\t#{pane_left}\t#{pane_top}\t#{pane_width}\t#{pane_height}",
        ])?;

        let mut panes = vec![];
        for line in output.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            let pane_id = field(&fields, 0, "pane_id")?;
            let pane_index = field(&fields, 1, "pane_index")?;
            let pane_title = field(&fields, 2, "pane_title").unwrap_or("");
            let cwd = field(&fields, 3, "pane_current_path")
                .ok()
                .map(PathBuf::from);
            let current_command = field(&fields, 4, "pane_current_command")
                .ok()
                .map(str::to_string);
            let active = field(&fields, 5, "pane_active").unwrap_or("0") == "1";
            let rect = tmux_rect(&fields);
            let label = meaningful_tmux_pane_label(pane_title);
            let base = label
                .as_deref()
                .map(|label| prefixed_name(tab_name, &slug(label)))
                .unwrap_or_else(|| format!("{tab_name}-pane-{pane_index}"));
            let name = unique_name(&base, pane_names_seen);
            panes.push(PaneTemplate {
                name,
                label,
                identity: None,
                cwd,
                command: None,
                observed: current_command.map(|foreground_command| ObservedState {
                    foreground_command: Some(foreground_command),
                }),
                agent: None,
                rect,
                actions: Default::default(),
                backend_ref: Some(BackendRef {
                    workspace_id: Some(session_name.to_string()),
                    tab_id: Some(window_id.to_string()),
                    pane_id: Some(pane_id.to_string()),
                    focused: active,
                }),
            });
        }
        Ok(panes)
    }
}

impl Backend for TmuxBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Tmux
    }

    fn doctor(&self) -> Result<DoctorReport> {
        Ok(DoctorReport {
            backend: BackendKind::Tmux,
            detected: Self::is_detected(),
            detail: format!(
                "TMUX={}, bin={}",
                std::env::var("TMUX").unwrap_or_else(|_| "unset".into()),
                if which::which("tmux").is_ok() {
                    "available"
                } else {
                    "missing"
                }
            ),
            features: vec![
                ("sessions-as-workspaces", true),
                ("windows-as-tabs", true),
                ("panes", true),
                ("layout-capture", false),
                ("layout-restore", false),
                ("smart-nav", true),
            ],
        })
    }

    fn capture_current_workspace(&self, name: Option<String>) -> Result<WorkspaceCapture> {
        let session = self.current_session_name()?;
        self.capture_session(&session, name)
    }

    fn capture_all_workspaces(&self) -> Result<Vec<WorkspaceCapture>> {
        self.session_names()?
            .into_iter()
            .map(|session| self.capture_session(&session, None))
            .collect()
    }

    fn capture_current_tab(&self, name: Option<String>) -> Result<TabCapture> {
        let session = self.current_session_name()?;
        let window_id = self.current_window_id()?;
        let workspace = self.capture_session(&session, None)?;
        let mut tab = workspace
            .tabs
            .into_iter()
            .find(|tab| {
                tab.tab
                    .backend_ref
                    .as_ref()
                    .and_then(|ref_| ref_.tab_id.as_deref())
                    == Some(window_id.as_str())
            })
            .context("current tmux window was not found in captured session")?;
        if let Some(name) = name {
            tab.tab.name = slug(&name);
        }
        Ok(tab)
    }

    fn capture_current_pane(&self, name: Option<String>) -> Result<PaneTemplate> {
        let pane_id = self.current_pane_id()?;
        let tab = self.capture_current_tab(None)?;
        let mut pane = tab
            .panes
            .into_iter()
            .find(|pane| {
                pane.backend_ref
                    .as_ref()
                    .and_then(|ref_| ref_.pane_id.as_deref())
                    == Some(pane_id.as_str())
            })
            .context("current tmux pane was not found in captured window")?;
        if let Some(name) = name {
            pane.name = slug(&name);
        }
        Ok(pane)
    }

    fn restore_workspace(
        &self,
        _workspace: &WorkspaceCapture,
        _dry_run: bool,
        _force: bool,
    ) -> Result<()> {
        bail!("tmux restore is not implemented yet; herdr is the first backend")
    }

    fn apply_tab(
        &self,
        _tab: &TabCapture,
        _workspace: Option<&str>,
        _dry_run: bool,
        _force: bool,
    ) -> Result<()> {
        bail!("tmux apply tab is not implemented yet; herdr is the first backend")
    }

    fn apply_workspace_metadata(
        &self,
        _workspace: &WorkspaceTemplate,
        _dry_run: bool,
    ) -> Result<()> {
        bail!("tmux apply workspace metadata is not implemented yet; herdr is the first backend")
    }

    fn apply_tab_metadata(&self, _tab: &TabTemplate, _dry_run: bool) -> Result<()> {
        bail!("tmux apply tab metadata is not implemented yet; herdr is the first backend")
    }

    fn apply_pane_metadata(&self, _pane: &PaneTemplate, _dry_run: bool) -> Result<()> {
        bail!("tmux apply pane metadata is not implemented yet; herdr is the first backend")
    }

    fn smart_nav(&self, direction: Direction, key: &str, passthrough_pattern: &str) -> Result<()> {
        let pane_current_command = Command::new("tmux")
            .args(["display-message", "-p", "#{pane_current_command}"])
            .output()?;
        let current = String::from_utf8_lossy(&pane_current_command.stdout);
        let re = Regex::new(passthrough_pattern).context("invalid nav passthrough regex")?;
        let passthrough = re.is_match(current.trim());
        if passthrough {
            Command::new("tmux").args(["send-keys", key]).status()?;
        } else {
            Command::new("tmux")
                .args(["select-pane", &format!("-{}", direction.as_tmux())])
                .status()?;
        }
        Ok(())
    }
}

fn field<'a>(fields: &'a [&str], index: usize, name: &str) -> Result<&'a str> {
    fields
        .get(index)
        .copied()
        .context(format!("tmux output missing {name}"))
}

fn meaningful_tmux_pane_label(label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() || label == "default" {
        None
    } else {
        Some(label.to_string())
    }
}

fn tmux_rect(fields: &[&str]) -> Option<Rect> {
    Some(Rect {
        x: field(fields, 6, "pane_left").ok()?.parse().ok()?,
        y: field(fields, 7, "pane_top").ok()?.parse().ok()?,
        width: field(fields, 8, "pane_width").ok()?.parse().ok()?,
        height: field(fields, 9, "pane_height").ok()?.parse().ok()?,
    })
}

fn window_area(panes: &[PaneTemplate]) -> Option<Rect> {
    let mut rects = panes.iter().filter_map(|pane| pane.rect.as_ref());
    let first = rects.next()?;
    let (mut min_x, mut min_y) = (first.x, first.y);
    let (mut max_x, mut max_y) = (first.x + first.width, first.y + first.height);
    for rect in rects {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x + rect.width);
        max_y = max_y.max(rect.y + rect.height);
    }
    Some(Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

fn prefixed_name(parent: &str, child: &str) -> String {
    if child == parent || child.starts_with(&format!("{parent}-")) {
        child.to_string()
    } else {
        format!("{parent}-{child}")
    }
}

fn unique_name(base: &str, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(base.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base.to_string()
    } else {
        format!("{base}-{count}")
    }
}
