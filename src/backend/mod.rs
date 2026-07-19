mod herdr;
mod tmux;

pub use herdr::HerdrBackend;
pub use tmux::TmuxBackend;

use crate::model::{BackendKind, Direction, PaneTemplate, TabCapture, WorkspaceCapture};
use anyhow::{Result, bail};

pub trait Backend {
    fn kind(&self) -> BackendKind;
    fn doctor(&self) -> Result<DoctorReport>;
    fn capture_all_workspaces(&self) -> Result<Vec<WorkspaceCapture>>;
    fn capture_current_workspace(&self, name: Option<String>) -> Result<WorkspaceCapture>;
    fn capture_current_tab(&self, name: Option<String>) -> Result<TabCapture>;
    fn capture_current_pane(&self, name: Option<String>) -> Result<PaneTemplate>;
    fn restore_workspace(&self, workspace: &WorkspaceCapture, dry_run: bool) -> Result<()>;
    fn apply_tab(&self, tab: &TabCapture, workspace: Option<&str>, dry_run: bool) -> Result<()>;
    fn smart_nav(&self, direction: Direction, key: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub backend: BackendKind,
    pub detected: bool,
    pub detail: String,
    pub features: Vec<(&'static str, bool)>,
}

pub fn detect_backend(requested: Option<BackendKind>) -> Result<Box<dyn Backend>> {
    match requested {
        Some(BackendKind::Herdr) => Ok(Box::new(HerdrBackend::new())),
        Some(BackendKind::Tmux) => Ok(Box::new(TmuxBackend::new())),
        None => {
            if HerdrBackend::is_detected() {
                Ok(Box::new(HerdrBackend::new()))
            } else if TmuxBackend::is_detected() {
                Ok(Box::new(TmuxBackend::new()))
            } else {
                bail!("no supported multiplexer detected; try --backend herdr or --backend tmux")
            }
        }
    }
}
