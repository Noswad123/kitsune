use crate::model::{PaneTemplate, TabCapture, TabTemplate, WorkspaceCapture, WorkspaceTemplate};
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Workspace,
    Tab,
    Pane,
    Stack,
    Snapshot,
}

impl ItemKind {
    pub fn dir_name(self) -> &'static str {
        match self {
            ItemKind::Workspace => "workspaces",
            ItemKind::Tab => "tabs",
            ItemKind::Pane => "panes",
            ItemKind::Stack => "stacks",
            ItemKind::Snapshot => "snapshots",
        }
    }

    pub fn singular_name(self) -> &'static str {
        match self {
            ItemKind::Workspace => "workspace",
            ItemKind::Tab => "tab",
            ItemKind::Pane => "pane",
            ItemKind::Stack => "stack",
            ItemKind::Snapshot => "snapshot",
        }
    }
}

impl std::str::FromStr for ItemKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "workspace" | "workspaces" => Ok(ItemKind::Workspace),
            "tab" | "tabs" => Ok(ItemKind::Tab),
            "pane" | "panes" => Ok(ItemKind::Pane),
            "stack" | "stacks" => Ok(ItemKind::Stack),
            "snapshot" | "snapshots" => Ok(ItemKind::Snapshot),
            _ => anyhow::bail!("unknown kind: {s}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: Option<PathBuf>) -> Result<Self> {
        let root = match root {
            Some(root) => expand_home(root),
            None => default_root()?,
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure(&self) -> Result<()> {
        for kind in [
            ItemKind::Workspace,
            ItemKind::Tab,
            ItemKind::Pane,
            ItemKind::Stack,
            ItemKind::Snapshot,
        ] {
            fs::create_dir_all(self.root.join(kind.dir_name()))
                .with_context(|| format!("creating {}", kind.dir_name()))?;
        }
        Ok(())
    }

    pub fn list(&self, kind: ItemKind) -> Result<Vec<String>> {
        let dir = self.root.join(kind.dir_name());
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut names = vec![];
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn path(&self, kind: ItemKind, name: &str) -> PathBuf {
        self.root
            .join(kind.dir_name())
            .join(format!("{}.yaml", slug(name)))
    }

    pub fn save_workspace(&self, workspace: &WorkspaceTemplate) -> Result<PathBuf> {
        self.ensure()?;
        let path = self.path(ItemKind::Workspace, &workspace.name);
        let yaml = serde_yaml::to_string(workspace)?;
        fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn save_workspace_capture(&self, capture: &WorkspaceCapture) -> Result<Vec<PathBuf>> {
        let mut paths = vec![];
        for tab in &capture.tabs {
            paths.extend(self.save_tab_capture(tab)?);
        }
        paths.push(self.save_workspace(&capture.workspace)?);
        Ok(paths)
    }

    pub fn save_tab(&self, tab: &TabTemplate) -> Result<PathBuf> {
        self.ensure()?;
        let path = self.path(ItemKind::Tab, &tab.name);
        let yaml = serde_yaml::to_string(tab)?;
        fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn save_tab_capture(&self, capture: &TabCapture) -> Result<Vec<PathBuf>> {
        let mut paths = vec![];
        for pane in &capture.panes {
            paths.push(self.save_pane(pane)?);
        }
        paths.push(self.save_tab(&capture.tab)?);
        Ok(paths)
    }

    pub fn save_pane(&self, pane: &PaneTemplate) -> Result<PathBuf> {
        self.ensure()?;
        let path = self.path(ItemKind::Pane, &pane.name);
        let yaml = serde_yaml::to_string(pane)?;
        fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn load_workspace(&self, name: &str) -> Result<WorkspaceTemplate> {
        let path = self.path(ItemKind::Workspace, name);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading workspace template {}", path.display()))?;
        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn load_workspace_capture(&self, name: &str) -> Result<WorkspaceCapture> {
        let workspace = self.load_workspace(name)?;
        let tabs = workspace
            .tabs
            .iter()
            .map(|reference| self.load_tab_capture(&reference.name))
            .collect::<Result<Vec<_>>>()?;
        Ok(WorkspaceCapture { workspace, tabs })
    }

    pub fn load_tab(&self, name: &str) -> Result<TabTemplate> {
        let path = self.path(ItemKind::Tab, name);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading tab template {}", path.display()))?;
        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn load_tab_capture(&self, name: &str) -> Result<TabCapture> {
        let tab = self.load_tab(name)?;
        let panes = tab
            .panes
            .iter()
            .map(|reference| self.load_pane(&reference.name))
            .collect::<Result<Vec<_>>>()?;
        Ok(TabCapture { tab, panes })
    }

    pub fn load_pane(&self, name: &str) -> Result<PaneTemplate> {
        let path = self.path(ItemKind::Pane, name);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading pane template {}", path.display()))?;
        Ok(serde_yaml::from_str(&contents)?)
    }

    pub fn show(&self, kind: ItemKind, name: &str) -> Result<String> {
        let path = self.path(kind, name);
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    }

    pub fn doctor(&self) -> StoreDoctor {
        let root_exists = self.root.exists();
        let root_is_symlink = fs::symlink_metadata(&self.root)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        let real_root = fs::canonicalize(&self.root).ok();
        let directories = ItemKind::all()
            .into_iter()
            .map(|kind| StoreDirectoryStatus {
                name: kind.dir_name(),
                path: self.root.join(kind.dir_name()),
                exists: self.root.join(kind.dir_name()).is_dir(),
            })
            .collect();

        StoreDoctor {
            root: self.root.clone(),
            real_root,
            root_exists,
            root_is_symlink,
            directories,
        }
    }
}

impl ItemKind {
    pub fn all() -> [ItemKind; 5] {
        [
            ItemKind::Workspace,
            ItemKind::Tab,
            ItemKind::Pane,
            ItemKind::Stack,
            ItemKind::Snapshot,
        ]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreDoctor {
    pub root: PathBuf,
    pub real_root: Option<PathBuf>,
    pub root_exists: bool,
    pub root_is_symlink: bool,
    pub directories: Vec<StoreDirectoryStatus>,
}

impl StoreDoctor {
    pub fn ok(&self) -> bool {
        self.root_exists && self.directories.iter().all(|dir| dir.exists)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreDirectoryStatus {
    pub name: &'static str,
    pub path: PathBuf,
    pub exists: bool,
}

pub fn slug(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '/' || ch == ':' || ch == '.' {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "unnamed".into()
    } else {
        out
    }
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path;
    };
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path
}

fn default_root() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("kitsune"));
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config/kitsune"))
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugifies_names() {
        assert_eq!(slug("Darkness API"), "darkness-api");
        assert_eq!(slug("w18:t2"), "w18-t2");
    }
}
