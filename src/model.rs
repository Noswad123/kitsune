use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Herdr,
    Tmux,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Herdr => write!(f, "herdr"),
            BackendKind::Tmux => write!(f, "tmux"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

impl Direction {
    pub fn as_herdr(self) -> &'static str {
        match self {
            Direction::Left => "left",
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::Right => "right",
        }
    }

    pub fn as_tmux(self) -> &'static str {
        match self {
            Direction::Left => "L",
            Direction::Down => "D",
            Direction::Up => "U",
            Direction::Right => "R",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceTemplate {
    pub schema: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<WorkspaceIdentity>,
    pub backend: BackendKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    pub saved_at: DateTime<Utc>,
    #[serde(default)]
    pub tabs: Vec<ComponentRef>,
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub actions: BTreeMap<String, ActionTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_ref: Option<BackendRef>,
}

impl WorkspaceTemplate {
    pub fn label_or_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabTemplate {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<TabIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub panes: Vec<ComponentRef>,
    pub layout: LayoutTemplate,
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub actions: BTreeMap<String, ActionTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_ref: Option<BackendRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentRef {
    #[serde(rename = "ref")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCapture {
    pub workspace: WorkspaceTemplate,
    pub tabs: Vec<TabCapture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabCapture {
    pub tab: TabTemplate,
    pub panes: Vec<PaneTemplate>,
}

impl TabTemplate {
    pub fn label_or_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneTemplate {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<PaneIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<ObservedState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    #[serde(default, skip_serializing_if = "is_empty_map")]
    pub actions: BTreeMap<String, ActionTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_ref: Option<BackendRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub focused: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_empty_map<K, V>(value: &BTreeMap<K, V>) -> bool {
    value.is_empty()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIdentity {
    pub label: Option<String>,
    pub cwd: Option<PathBuf>,
    pub git_root: Option<PathBuf>,
    pub tab_fingerprints: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabIdentity {
    pub label: Option<String>,
    pub pane_fingerprints: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneIdentity {
    pub label: Option<String>,
    pub cwd: Option<PathBuf>,
    pub git_root: Option<PathBuf>,
    pub agent: Option<String>,
    pub fingerprint: String,
}

impl PaneTemplate {
    pub fn label_or_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutTemplate {
    pub area: Option<Rect>,
    #[serde(default)]
    pub splits: Vec<SplitTemplate>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

impl Rect {
    pub fn distance(&self, other: &Rect) -> i64 {
        (self.x - other.x).abs()
            + (self.y - other.y).abs()
            + (self.width - other.width).abs()
            + (self.height - other.height).abs()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitTemplate {
    pub direction: SplitDirection,
    pub ratio: f64,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SplitDirection {
    Right,
    Down,
}

impl SplitDirection {
    pub fn as_herdr(self) -> &'static str {
        match self {
            SplitDirection::Right => "right",
            SplitDirection::Down => "down",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackTemplate {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub workspaces: Vec<ComponentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSnapshot {
    pub schema: String,
    pub name: String,
    pub saved_at: DateTime<Utc>,
    pub backend: BackendKind,
    pub scope: SnapshotScope,
    pub payload: serde_yaml::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotScope {
    All,
    Workspace,
    Tab,
    Pane,
}

impl SnapshotScope {
    pub fn as_str(self) -> &'static str {
        match self {
            SnapshotScope::All => "all",
            SnapshotScope::Workspace => "workspace",
            SnapshotScope::Tab => "tab",
            SnapshotScope::Pane => "pane",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KitsuneConfig {
    pub schema: String,
    pub nav: NavConfig,
}

impl Default for KitsuneConfig {
    fn default() -> Self {
        Self {
            schema: "kitsune.config.v1".into(),
            nav: NavConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NavConfig {
    pub passthrough_regex: Option<String>,
}
