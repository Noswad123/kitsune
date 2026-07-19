use crate::model::{BackendKind, Direction};
use crate::store::ItemKind;
use anyhow::{Result, bail};
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
    Doctor(DoctorArgs),
    /// Save the current backend state into a named reusable template.
    Save(SaveArgs),
    /// Restore a saved workspace template.
    Restore(RestoreArgs),
    /// Apply saved templates to the live multiplexer without editing YAML.
    Apply(ApplyArgs),
    /// Run an explicitly configured saved action.
    Run(RunArgs),
    /// Compose templates by adding refs between components.
    Add(AddArgs),
    /// List saved templates.
    List(ListArgs),
    /// Show a saved template as YAML.
    Show(ShowArgs),
    /// Show a human-readable template/ref tree.
    Tree(TreeArgs),
    /// Validate the Kitsune store and saved templates.
    Validate(ValidateArgs),
    /// Smart pane navigation that passes keys through to Vim/fzf-like apps.
    Nav(NavArgs),
    /// Create and manage workspace stacks.
    Stack(StackArgs),
    /// Inspect and initialize Kitsune's template store.
    Store(StoreArgs),
    /// Open the interactive selector/browser.
    Tui,
    /// Print store path and create expected directories.
    Init,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[command(subcommand)]
    pub command: Option<DoctorCommand>,
}

#[derive(Debug, Subcommand)]
pub enum DoctorCommand {
    /// Inspect configured actions and live pane resolution.
    Actions,
}

#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Scope (`workspace`, `tab`, `pane`) or workspace name shorthand.
    pub scope_or_name: Option<String>,

    /// Logical name for the saved item when a scope is provided.
    pub name: Option<String>,

    /// Explicit scope to save.
    #[arg(long, value_enum)]
    pub scope: Option<SaveScope>,

    /// Explicitly save the currently focused workspace. Optional positional name overrides the saved name.
    #[arg(long)]
    pub current: bool,

    /// Preview what would be saved/reused without writing files.
    #[arg(long)]
    pub plan: bool,

    /// Do not reuse existing components with matching fingerprints.
    #[arg(long)]
    pub no_reuse: bool,

    /// Also write a timestamped point-in-time snapshot under snapshots/.
    #[arg(long)]
    pub append_snapshot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SaveScope {
    All,
    Workspace,
    Tab,
    Pane,
}

impl SaveArgs {
    pub fn resolve(&self) -> Result<(SaveScope, Option<String>)> {
        if self.current {
            if self.scope.is_some() {
                bail!("save --current cannot be combined with --scope");
            }
            if self.name.is_some() {
                bail!("save --current takes at most one optional name");
            }
            if self.scope_or_name.as_deref() == Some("all") {
                bail!("save --current cannot be combined with all");
            }
            return Ok((SaveScope::Workspace, self.scope_or_name.clone()));
        }

        if let Some(scope) = self.scope {
            if self.name.is_some() {
                bail!("save name was provided twice");
            }
            return Ok((scope, self.scope_or_name.clone()));
        }

        match self.scope_or_name.as_deref() {
            None => Ok((SaveScope::Workspace, None)),
            Some("all") => {
                if self.name.is_some() {
                    bail!("save all does not take a name");
                }
                Ok((SaveScope::All, None))
            }
            Some("workspace" | "workspaces") => Ok((SaveScope::Workspace, self.name.clone())),
            Some("tab" | "tabs") => Ok((SaveScope::Tab, self.name.clone())),
            Some("pane" | "panes") => Ok((SaveScope::Pane, self.name.clone())),
            Some(name) => {
                if self.name.is_some() {
                    bail!("unknown save scope '{name}'; expected all, workspace, tab, or pane");
                }
                Ok((SaveScope::Workspace, Some(name.to_string())))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_current_resolves_to_workspace() {
        let args = SaveArgs {
            scope_or_name: None,
            name: None,
            scope: None,
            current: true,
            plan: false,
            no_reuse: false,
            append_snapshot: false,
        };
        assert_eq!(args.resolve().unwrap().0, SaveScope::Workspace);
    }

    #[test]
    fn save_current_accepts_optional_name() {
        let args = SaveArgs {
            scope_or_name: Some("darkness".into()),
            name: None,
            scope: None,
            current: true,
            plan: false,
            no_reuse: false,
            append_snapshot: false,
        };
        assert_eq!(
            args.resolve().unwrap(),
            (SaveScope::Workspace, Some("darkness".into()))
        );
    }
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Template kind (`workspace` or `stack`) or workspace name shorthand.
    pub target_or_name: String,

    /// Template name when kind is provided.
    pub name: Option<String>,

    /// Print Herdr/tmux commands without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before applying live restore changes.
    #[arg(long)]
    pub confirm: bool,

    /// Allow creating duplicate live workspace/tab labels.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[command(subcommand)]
    pub command: AddCommand,
}

#[derive(Debug, Subcommand)]
pub enum AddCommand {
    /// Add a tab ref to a workspace template.
    Tab(AddTabArgs),
}

#[derive(Debug, Args)]
pub struct AddTabArgs {
    /// Existing tab template name to reference.
    pub name: String,

    /// Existing workspace template to modify. Defaults to the focused workspace.
    #[arg(long)]
    pub to: Option<String>,

    /// Also apply the tab to the live multiplexer.
    #[arg(long)]
    pub apply: bool,

    /// Print backend commands without executing live changes.
    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before applying live changes.
    #[arg(long)]
    pub confirm: bool,

    /// Allow creating duplicate live tab labels.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    #[command(subcommand)]
    pub command: ApplyCommand,
}

#[derive(Debug, Subcommand)]
pub enum ApplyCommand {
    /// Apply a saved tab to a live workspace. Defaults to focused workspace.
    Tab(ApplyTabArgs),
    /// Apply/restore a saved workspace as a new live workspace.
    Workspace(ApplyWorkspaceArgs),
    /// Apply/restore every workspace in a saved stack.
    Stack(ApplyStackArgs),
}

#[derive(Debug, Args)]
pub struct ApplyTabArgs {
    pub name: String,

    /// Live workspace selector. Defaults to focused workspace.
    #[arg(long)]
    pub to: Option<String>,

    /// Print backend commands without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before applying live changes.
    #[arg(long)]
    pub confirm: bool,

    /// Allow creating duplicate live tab labels.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ApplyWorkspaceArgs {
    pub name: String,

    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before applying live changes.
    #[arg(long)]
    pub confirm: bool,

    /// Allow creating duplicate live workspace labels.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ApplyStackArgs {
    pub name: String,

    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before applying live changes.
    #[arg(long)]
    pub confirm: bool,

    /// Allow creating duplicate live workspace labels.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Named action to run, such as start, stop, test, or dev.
    pub action: String,

    /// Workspace/tab/pane template name. Lookup prefers workspace, then tab, then pane.
    pub target: String,

    /// Restrict target lookup to one template kind.
    #[arg(long, value_enum)]
    pub kind: Option<RunKind>,

    /// Print commands without executing them.
    #[arg(long)]
    pub dry_run: bool,

    /// Prompt before executing commands.
    #[arg(long)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunKind {
    Workspace,
    Tab,
    Pane,
}

impl From<RunKind> for ItemKind {
    fn from(value: RunKind) -> Self {
        match value {
            RunKind::Workspace => ItemKind::Workspace,
            RunKind::Tab => ItemKind::Tab,
            RunKind::Pane => ItemKind::Pane,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreTarget {
    Workspace,
    Stack,
}

impl RestoreArgs {
    pub fn resolve(&self) -> Result<(RestoreTarget, String)> {
        match self.target_or_name.as_str() {
            "workspace" | "workspaces" => {
                let name = self
                    .name
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("restore workspace requires a name"))?;
                Ok((RestoreTarget::Workspace, name))
            }
            "stack" | "stacks" => {
                let name = self
                    .name
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("restore stack requires a name"))?;
                Ok((RestoreTarget::Stack, name))
            }
            name => {
                if self.name.is_some() {
                    bail!("unknown restore target '{name}'; expected workspace or stack");
                }
                Ok((RestoreTarget::Workspace, name.to_string()))
            }
        }
    }
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

#[derive(Debug, Args)]
pub struct TreeArgs {
    #[arg(value_enum)]
    pub kind: KindArgRequired,
    pub name: String,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Treat warnings as validation failures.
    #[arg(long)]
    pub strict: bool,
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

#[derive(Debug, Args)]
pub struct StackArgs {
    #[command(subcommand)]
    pub command: StackCommand,
}

#[derive(Debug, Subcommand)]
pub enum StackCommand {
    /// Create a stack from existing workspace template refs.
    Create(StackCreateArgs),
}

#[derive(Debug, Args)]
pub struct StackCreateArgs {
    pub name: String,
    pub workspaces: Vec<String>,
}

#[derive(Debug, Args)]
pub struct StoreArgs {
    #[command(subcommand)]
    pub command: StoreCommand,
}

#[derive(Debug, Subcommand)]
pub enum StoreCommand {
    /// Print the configured store path.
    Path(StorePathArgs),
    /// Check the configured store path and expected directories.
    Doctor(StoreDoctorArgs),
    /// Create the configured store and expected directories.
    Init,
}

#[derive(Debug, Args)]
pub struct StorePathArgs {
    /// Resolve symlinks and print the real path when possible.
    #[arg(long)]
    pub real: bool,
}

#[derive(Debug, Args)]
pub struct StoreDoctorArgs {
    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
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
