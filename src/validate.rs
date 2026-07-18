use crate::fingerprint;
use crate::model::{BackendKind, PaneTemplate, StackTemplate, TabTemplate, WorkspaceTemplate};
use crate::store::{ItemKind, Store};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub store: PathBuf,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn new(store: PathBuf) -> Self {
        Self {
            store,
            issues: vec![],
        }
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Warning)
            .count()
    }

    pub fn passes(&self, strict: bool) -> bool {
        self.error_count() == 0 && (!strict || self.warning_count() == 0)
    }

    fn push(
        &mut self,
        severity: Severity,
        kind: impl Into<String>,
        path: impl Into<Option<PathBuf>>,
        message: impl Into<String>,
    ) {
        self.issues.push(ValidationIssue {
            severity,
            kind: kind.into(),
            path: path.into(),
            message: message.into(),
        });
    }

    fn error(
        &mut self,
        kind: impl Into<String>,
        path: impl Into<Option<PathBuf>>,
        message: impl Into<String>,
    ) {
        self.push(Severity::Error, kind, path, message);
    }

    fn warning(
        &mut self,
        kind: impl Into<String>,
        path: impl Into<Option<PathBuf>>,
        message: impl Into<String>,
    ) {
        self.push(Severity::Warning, kind, path, message);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub kind: String,
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Error,
    Warning,
}

pub fn validate_store(store: &Store) -> Result<ValidationReport> {
    let mut report = ValidationReport::new(store.root().to_path_buf());
    let doctor = store.doctor();

    if !doctor.root_exists {
        report.error(
            "store",
            Some(doctor.root.clone()),
            "store root does not exist; run `kit store init`",
        );
    }

    for dir in doctor.directories {
        if !dir.exists {
            report.error(
                "store-directory",
                Some(dir.path),
                format!("missing {} directory; run `kit store init`", dir.name),
            );
        }
    }

    validate_workspace_templates(store, &mut report)?;
    validate_stack_templates(store, &mut report)?;
    validate_tab_templates(store, &mut report)?;
    validate_pane_templates(store, &mut report)?;
    validate_generic_yaml_dir(store, ItemKind::Snapshot, &mut report)?;
    validate_duplicate_fingerprints(store, &mut report)?;

    Ok(report)
}

fn validate_workspace_templates(store: &Store, report: &mut ValidationReport) -> Result<()> {
    for path in yaml_files(store, ItemKind::Workspace)? {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading workspace template {}", path.display()))?;
        match serde_yaml::from_str::<WorkspaceTemplate>(&contents) {
            Ok(workspace) => validate_workspace(store, &path, &workspace, report),
            Err(err) => report.error(
                "workspace-template",
                Some(path),
                format!("invalid workspace template: {err}"),
            ),
        }
    }
    Ok(())
}

fn validate_workspace(
    store: &Store,
    path: &Path,
    workspace: &WorkspaceTemplate,
    report: &mut ValidationReport,
) {
    if workspace.schema != "kitsune.workspace.v1" {
        report.warning(
            "workspace-schema",
            Some(path.to_path_buf()),
            format!(
                "unexpected schema '{}'; expected kitsune.workspace.v1",
                workspace.schema
            ),
        );
    }
    if workspace.name.trim().is_empty() {
        report.error(
            "workspace-name",
            Some(path.to_path_buf()),
            "workspace name is empty",
        );
    }
    if workspace.tabs.is_empty() {
        report.warning(
            "workspace-tabs",
            Some(path.to_path_buf()),
            "workspace has no tabs",
        );
    }
    if workspace.backend == BackendKind::Tmux {
        report.warning(
            "backend-support",
            Some(path.to_path_buf()),
            "tmux workspace restore is not implemented yet",
        );
    }

    for tab in &workspace.tabs {
        if tab.name.trim().is_empty() {
            report.error(
                "tab-ref",
                Some(path.to_path_buf()),
                format!("workspace '{}' contains an empty tab ref", workspace.name),
            );
        }
        if !store.path(ItemKind::Tab, &tab.name).exists() {
            report.error(
                "broken-ref",
                Some(path.to_path_buf()),
                format!("workspace references missing tab '{}'", tab.name),
            );
        }
    }
}

fn validate_stack_templates(store: &Store, report: &mut ValidationReport) -> Result<()> {
    let workspace_names = workspace_names(store)?;
    for path in yaml_files(store, ItemKind::Stack)? {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading stack template {}", path.display()))?;
        match serde_yaml::from_str::<StackTemplate>(&contents) {
            Ok(stack) => validate_stack(&path, &stack, &workspace_names, report),
            Err(err) => report.error(
                "stack-template",
                Some(path),
                format!("invalid stack template: {err}"),
            ),
        }
    }
    Ok(())
}

fn validate_tab_templates(store: &Store, report: &mut ValidationReport) -> Result<()> {
    for path in yaml_files(store, ItemKind::Tab)? {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading tab template {}", path.display()))?;
        match serde_yaml::from_str::<TabTemplate>(&contents) {
            Ok(tab) => {
                if tab.name.trim().is_empty() {
                    report.error("tab-name", Some(path.clone()), "tab name is empty");
                }
                if tab.panes.is_empty() {
                    report.warning("tab-panes", Some(path.clone()), "tab has no pane refs");
                }
                for pane in &tab.panes {
                    if pane.name.trim().is_empty() {
                        report.error(
                            "pane-ref",
                            Some(path.clone()),
                            "tab contains an empty pane ref",
                        );
                    }
                    if !store.path(ItemKind::Pane, &pane.name).exists() {
                        report.error(
                            "broken-ref",
                            Some(path.clone()),
                            format!("tab references missing pane '{}'", pane.name),
                        );
                    }
                }
            }
            Err(err) => report.error(
                "tab-template",
                Some(path),
                format!("invalid tab template: {err}"),
            ),
        }
    }
    Ok(())
}

fn validate_pane_templates(store: &Store, report: &mut ValidationReport) -> Result<()> {
    for path in yaml_files(store, ItemKind::Pane)? {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading pane template {}", path.display()))?;
        match serde_yaml::from_str::<PaneTemplate>(&contents) {
            Ok(pane) => {
                if pane.name.trim().is_empty() {
                    report.error("pane-name", Some(path.clone()), "pane name is empty");
                }
                if let Some(cwd) = pane.cwd {
                    if !cwd.exists() {
                        report.warning(
                            "pane-cwd",
                            Some(path),
                            format!("pane cwd does not exist: {}", cwd.display()),
                        );
                    }
                }
            }
            Err(err) => report.error(
                "pane-template",
                Some(path),
                format!("invalid pane template: {err}"),
            ),
        }
    }
    Ok(())
}

fn validate_stack(
    path: &Path,
    stack: &StackTemplate,
    workspace_names: &HashSet<String>,
    report: &mut ValidationReport,
) {
    if stack.schema != "kitsune.stack.v1" {
        report.warning(
            "stack-schema",
            Some(path.to_path_buf()),
            format!(
                "unexpected schema '{}'; expected kitsune.stack.v1",
                stack.schema
            ),
        );
    }
    if stack.name.trim().is_empty() {
        report.error(
            "stack-name",
            Some(path.to_path_buf()),
            "stack name is empty",
        );
    }
    if stack.workspaces.is_empty() {
        report.warning(
            "stack-workspaces",
            Some(path.to_path_buf()),
            "stack does not reference any workspaces",
        );
    }
    for workspace in &stack.workspaces {
        if !workspace_names.contains(workspace) {
            report.error(
                "broken-ref",
                Some(path.to_path_buf()),
                format!("stack references missing workspace '{workspace}'"),
            );
        }
    }
}

fn validate_generic_yaml_dir(
    store: &Store,
    kind: ItemKind,
    report: &mut ValidationReport,
) -> Result<()> {
    for path in yaml_files(store, kind)? {
        let contents =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        if let Err(err) = serde_yaml::from_str::<serde_yaml::Value>(&contents) {
            report.error(
                format!("{}-yaml", kind.singular_name()),
                Some(path),
                format!("invalid YAML: {err}"),
            );
        }
    }
    Ok(())
}

fn validate_duplicate_fingerprints(store: &Store, report: &mut ValidationReport) -> Result<()> {
    warn_duplicate_fingerprints(store, ItemKind::Workspace, report)?;
    warn_duplicate_fingerprints(store, ItemKind::Tab, report)?;
    warn_duplicate_fingerprints(store, ItemKind::Pane, report)?;
    Ok(())
}

fn warn_duplicate_fingerprints(
    store: &Store,
    kind: ItemKind,
    report: &mut ValidationReport,
) -> Result<()> {
    let mut seen = std::collections::HashMap::<String, Vec<String>>::new();
    for path in yaml_files(store, kind)? {
        let Some(name) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let fingerprint = match kind {
            ItemKind::Workspace => {
                let Ok(mut workspace) = store.load_workspace_capture(&name) else {
                    continue;
                };
                if workspace.workspace.identity.is_none() {
                    fingerprint::annotate_workspace_capture(&mut workspace);
                }
                workspace
                    .workspace
                    .identity
                    .map(|identity| identity.fingerprint)
            }
            ItemKind::Tab => {
                let Ok(mut tab) = store.load_tab_capture(&name) else {
                    continue;
                };
                if tab.tab.identity.is_none() {
                    fingerprint::annotate_tab_capture(&mut tab);
                }
                tab.tab.identity.map(|identity| identity.fingerprint)
            }
            ItemKind::Pane => {
                let Ok(mut pane) = store.load_pane(&name) else {
                    continue;
                };
                if pane.identity.is_none() {
                    fingerprint::annotate_pane(&mut pane);
                }
                pane.identity.map(|identity| identity.fingerprint)
            }
            ItemKind::Stack | ItemKind::Snapshot => None,
        };

        if let Some(fingerprint) = fingerprint {
            seen.entry(fingerprint).or_default().push(name);
        }
    }

    for names in seen.values().filter(|names| names.len() > 1) {
        report.warning(
            "duplicate-fingerprint",
            None,
            format!(
                "{} templates appear to describe the same component: {}",
                kind.dir_name(),
                names.join(", ")
            ),
        );
    }

    Ok(())
}

fn workspace_names(store: &Store) -> Result<HashSet<String>> {
    Ok(yaml_files(store, ItemKind::Workspace)?
        .into_iter()
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect())
}

fn yaml_files(store: &Store, kind: ItemKind) -> Result<Vec<PathBuf>> {
    let dir = store.root().join(kind.dir_name());
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
