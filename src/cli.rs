use crate::model::{BackendKind, Direction};
use crate::store::ItemKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "kit", about = "Kitsune: composable multiplexer kits")]
pub struct Cli {
    #[arg(long, global = true, env = "KITSUNE_STORE")]
    pub store: Option<PathBuf>,

    #[arg(long, global = true, value_enum)]
    pub backend: Option<BackendArg>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BackendArg {
    Herdr,
    Tmux,
}

impl From<BackendArg> for BackendKind {
    fn from(value: BackendArg) -> Self {
        match value {
            BackendArg::Herdr => BackendKind::Herdr,
            BackendArg::Tmux => BackendKind::Tmux,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Detect the active multiplexer backend and report feature support.
    Doctor,
    /// Capture the current backend state into a named reusable template.
    Capture(CaptureArgs),
    /// Restore a saved workspace template.
    Restore(RestoreArgs),
    /// List saved templates.
    List(ListArgs),
    /// Show a saved template as YAML.
    Show(ShowArgs),
    /// Smart pane navigation that passes keys through to Vim/fzf-like apps.
    Nav(NavArgs),
    /// Open the interactive selector/browser.
    Tui,
    /// Print store path and create expected directories.
    Init,
}

#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Logical name for the captured workspace.
    pub name: Option<String>,

    /// Scope to capture. Only current-workspace is implemented for now.
    #[arg(long, value_enum, default_value_t = CaptureScope::Workspace)]
    pub scope: CaptureScope,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CaptureScope {
    Workspace,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Workspace template name.
    pub name: String,

    /// Print Herdr/tmux commands without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Recreate panes/layout but do not rerun captured commands.
    #[arg(long)]
    pub skip_commands: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(value_enum, default_value_t = KindArg::All)]
    pub kind: KindArg,

    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KindArg {
    All,
    Workspaces,
    Tabs,
    Panes,
    Stacks,
    Snapshots,
}

impl KindArg {
    pub fn item_kind(self) -> Option<ItemKind> {
        match self {
            KindArg::All => None,
            KindArg::Workspaces => Some(ItemKind::Workspace),
            KindArg::Tabs => Some(ItemKind::Tab),
            KindArg::Panes => Some(ItemKind::Pane),
            KindArg::Stacks => Some(ItemKind::Stack),
            KindArg::Snapshots => Some(ItemKind::Snapshot),
        }
    }
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(value_enum)]
    pub kind: KindArgRequired,
    pub name: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KindArgRequired {
    Workspace,
    Tab,
    Pane,
    Stack,
    Snapshot,
}

impl From<KindArgRequired> for ItemKind {
    fn from(value: KindArgRequired) -> Self {
        match value {
            KindArgRequired::Workspace => ItemKind::Workspace,
            KindArgRequired::Tab => ItemKind::Tab,
            KindArgRequired::Pane => ItemKind::Pane,
            KindArgRequired::Stack => ItemKind::Stack,
            KindArgRequired::Snapshot => ItemKind::Snapshot,
        }
    }
}

#[derive(Debug, Args)]
pub struct NavArgs {
    #[arg(value_enum)]
    pub direction: DirectionArg,
    pub key: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DirectionArg {
    Left,
    Down,
    Up,
    Right,
}

impl From<DirectionArg> for Direction {
    fn from(value: DirectionArg) -> Self {
        match value {
            DirectionArg::Left => Direction::Left,
            DirectionArg::Down => Direction::Down,
            DirectionArg::Up => Direction::Up,
            DirectionArg::Right => Direction::Right,
        }
    }
}
