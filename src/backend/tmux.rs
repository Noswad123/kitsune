use super::{Backend, DoctorReport};
use crate::model::{BackendKind, Direction, PaneTemplate, TabCapture, WorkspaceCapture};
use anyhow::{Result, bail};
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

    fn capture_current_workspace(&self, _name: Option<String>) -> Result<WorkspaceCapture> {
        bail!("tmux capture is not implemented yet; herdr is the first backend")
    }

    fn capture_all_workspaces(&self) -> Result<Vec<WorkspaceCapture>> {
        bail!("tmux capture all is not implemented yet; herdr is the first backend")
    }

    fn capture_current_tab(&self, _name: Option<String>) -> Result<TabCapture> {
        bail!("tmux tab capture is not implemented yet; herdr is the first backend")
    }

    fn capture_current_pane(&self, _name: Option<String>) -> Result<PaneTemplate> {
        bail!("tmux pane capture is not implemented yet; herdr is the first backend")
    }

    fn restore_workspace(
        &self,
        _workspace: &WorkspaceCapture,
        _dry_run: bool,
        _skip_commands: bool,
    ) -> Result<()> {
        bail!("tmux restore is not implemented yet; herdr is the first backend")
    }

    fn apply_tab(
        &self,
        _tab: &TabCapture,
        _workspace: Option<&str>,
        _dry_run: bool,
        _skip_commands: bool,
    ) -> Result<()> {
        bail!("tmux apply tab is not implemented yet; herdr is the first backend")
    }

    fn smart_nav(&self, direction: Direction, key: &str) -> Result<()> {
        let pane_current_command = Command::new("tmux")
            .args(["display-message", "-p", "#{pane_current_command}"])
            .output()?;
        let current = String::from_utf8_lossy(&pane_current_command.stdout);
        let passthrough = ["vim", "nvim", "view", "fzf"]
            .iter()
            .any(|name| current.trim().ends_with(name));
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
