use super::{Backend, DoctorReport, nav_passthrough_pattern};
use crate::model::{
    BackendKind, Direction, LayoutTemplate, PaneTemplate, Rect, SplitDirection, SplitTemplate,
    TabCapture, TabTemplate, WorkspaceCapture, WorkspaceTemplate,
};
use crate::store::slug;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct HerdrBackend {
    bin: String,
}

impl HerdrBackend {
    pub fn new() -> Self {
        Self {
            bin: std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into()),
        }
    }

    pub fn is_detected() -> bool {
        std::env::var("HERDR_ENV").as_deref() == Ok("1") || which::which("herdr").is_ok()
    }

    fn json(&self, args: &[&str]) -> Result<Value> {
        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| format!("running {} {}", self.bin, args.join(" ")))?;

        if !output.status.success() {
            bail!(
                "{} {} failed: {}",
                self.bin,
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        serde_json::from_slice(&output.stdout)
            .with_context(|| format!("parsing JSON from {} {}", self.bin, args.join(" ")))
    }

    fn run(&self, args: &[String], dry_run: bool) -> Result<Option<Value>> {
        if dry_run {
            println!("{} {}", self.bin, args.join(" "));
            return Ok(None);
        }

        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| format!("running {} {}", self.bin, args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "{} {} failed: {}",
                self.bin,
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if output.stdout.is_empty() {
            Ok(None)
        } else {
            Ok(Some(serde_json::from_slice(&output.stdout)?))
        }
    }

    fn workspace_list(&self) -> Result<Value> {
        self.json(&["workspace", "list"])
    }

    fn capture_workspace_from_value(
        &self,
        workspace: &Value,
        name: Option<String>,
        prefix_components: bool,
    ) -> Result<WorkspaceCapture> {
        let workspace_id = workspace["workspace_id"]
            .as_str()
            .context("workspace missing workspace_id")?;
        let workspace_label = workspace["label"].as_str().map(str::to_string);
        let template_name =
            name.unwrap_or_else(|| slug(workspace_label.as_deref().unwrap_or(workspace_id)));

        let tab_list = self.json(&["tab", "list", "--workspace", workspace_id])?;
        let pane_list = self.json(&["pane", "list", "--workspace", workspace_id])?;
        let panes = pane_list["result"]["panes"]
            .as_array()
            .context("missing result.panes")?;

        let mut tabs = Vec::new();
        let mut workspace_cwd = None;
        let mut tab_names_seen = HashMap::<String, usize>::new();
        let mut pane_names_seen = HashMap::<String, usize>::new();
        for tab in tab_list["result"]["tabs"]
            .as_array()
            .context("missing result.tabs")?
        {
            let tab_id = tab["tab_id"].as_str().context("tab missing tab_id")?;
            let tab_label = tab["label"].as_str().map(str::to_string);
            let tab_panes: Vec<&Value> = panes
                .iter()
                .filter(|p| p["tab_id"].as_str() == Some(tab_id))
                .collect();

            let first_pane_id = tab_panes
                .first()
                .and_then(|p| p["pane_id"].as_str())
                .unwrap_or("");
            let raw_layout = if first_pane_id.is_empty() {
                None
            } else {
                Some(
                    self.json(&["pane", "layout", "--pane", first_pane_id])?["result"]["layout"]
                        .clone(),
                )
            };
            let layout = parse_layout(raw_layout.as_ref());

            let tab_base = slug(tab_label.as_deref().unwrap_or(tab_id));
            let tab_base = if prefix_components {
                format!("{}-{}", template_name, tab_base)
            } else {
                tab_base
            };
            let tab_name = unique_name(&tab_base, &mut tab_names_seen);

            let mut pane_templates = Vec::new();
            for pane in tab_panes {
                let pane_id = pane["pane_id"].as_str().context("pane missing pane_id")?;
                let process = self
                    .json(&["pane", "process-info", "--pane", pane_id])
                    .unwrap_or(Value::Null);
                let observed_command = first_cmdline(&process);
                let cwd = pane["foreground_cwd"]
                    .as_str()
                    .or_else(|| pane["cwd"].as_str())
                    .map(PathBuf::from);
                if workspace_cwd.is_none() {
                    workspace_cwd = cwd.clone();
                }
                let label = pane["label"].as_str().map(str::to_string);
                let pane_base = slug(label.as_deref().unwrap_or(pane_id));
                let pane_base = if prefix_components {
                    format!("{}-{}", tab_name, pane_base)
                } else {
                    pane_base
                };
                let name = unique_name(&pane_base, &mut pane_names_seen);
                let rect = layout_rect_for_pane(raw_layout.as_ref(), pane_id);
                pane_templates.push(PaneTemplate {
                    name,
                    label,
                    identity: None,
                    cwd,
                    command: None,
                    observed: observed_command.map(|foreground_command| {
                        crate::model::ObservedState {
                            foreground_command: Some(foreground_command),
                        }
                    }),
                    agent: pane["agent"].as_str().map(str::to_string),
                    rect,
                    raw: Some(pane.clone()),
                });
            }

            let tab_cwd = pane_templates.first().and_then(|p| p.cwd.clone());
            tabs.push(TabCapture {
                tab: TabTemplate {
                    name: tab_name,
                    label: tab_label,
                    identity: None,
                    cwd: tab_cwd,
                    panes: vec![],
                    layout,
                    raw: Some(tab.clone()),
                },
                panes: pane_templates,
            });
        }

        Ok(WorkspaceCapture {
            workspace: WorkspaceTemplate {
                schema: "kitsune.workspace.v1".into(),
                name: template_name,
                label: workspace_label,
                identity: None,
                backend: BackendKind::Herdr,
                cwd: workspace_cwd,
                captured_at: Utc::now(),
                tabs: vec![],
                raw: Some(workspace.clone()),
            },
            tabs,
        })
    }

    fn resolve_workspace_id(&self, selector: Option<&str>) -> Result<String> {
        let workspace_list = self.workspace_list()?;
        let workspaces = workspace_list["result"]["workspaces"]
            .as_array()
            .context("missing result.workspaces")?;

        let workspace = match selector {
            None => workspaces
                .iter()
                .find(|workspace| workspace["focused"].as_bool() == Some(true))
                .or_else(|| workspaces.first())
                .context("no Herdr workspaces found")?,
            Some(selector) => {
                let normalized = slug(selector);
                workspaces
                    .iter()
                    .find(|workspace| {
                        workspace["workspace_id"].as_str() == Some(selector)
                            || workspace["number"].as_i64().map(|n| n.to_string())
                                == Some(selector.to_string())
                            || workspace["label"].as_str().map(slug) == Some(normalized.clone())
                    })
                    .with_context(|| format!("no live Herdr workspace matches '{selector}'"))?
            }
        };

        Ok(workspace["workspace_id"]
            .as_str()
            .context("workspace missing workspace_id")?
            .to_string())
    }

    fn workspace_label_exists(&self, label: &str) -> Result<bool> {
        let normalized = slug(label);
        let workspace_list = self.workspace_list()?;
        let workspaces = workspace_list["result"]["workspaces"]
            .as_array()
            .context("missing result.workspaces")?;
        Ok(workspaces
            .iter()
            .any(|workspace| workspace["label"].as_str().map(slug) == Some(normalized.clone())))
    }

    fn tab_label_exists(&self, workspace_id: &str, label: &str) -> Result<bool> {
        let normalized = slug(label);
        let tab_list = self.json(&["tab", "list", "--workspace", workspace_id])?;
        let tabs = tab_list["result"]["tabs"]
            .as_array()
            .context("missing result.tabs")?;
        Ok(tabs
            .iter()
            .any(|tab| tab["label"].as_str().map(slug) == Some(normalized.clone())))
    }
}

impl Backend for HerdrBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Herdr
    }

    fn doctor(&self) -> Result<DoctorReport> {
        Ok(DoctorReport {
            backend: BackendKind::Herdr,
            detected: Self::is_detected(),
            detail: format!(
                "HERDR_ENV={}, bin={}",
                std::env::var("HERDR_ENV").unwrap_or_else(|_| "unset".into()),
                self.bin
            ),
            features: vec![
                ("workspaces", true),
                ("tabs", true),
                ("panes", true),
                ("layout-capture", true),
                ("layout-restore", true),
                ("smart-nav", true),
            ],
        })
    }

    fn capture_all_workspaces(&self) -> Result<Vec<WorkspaceCapture>> {
        let workspace_list = self.workspace_list()?;
        let workspaces = workspace_list["result"]["workspaces"]
            .as_array()
            .context("missing result.workspaces")?;
        workspaces
            .iter()
            .map(|workspace| self.capture_workspace_from_value(workspace, None, true))
            .collect()
    }

    fn capture_current_workspace(&self, name: Option<String>) -> Result<WorkspaceCapture> {
        let workspace_list = self.workspace_list()?;
        let workspaces = workspace_list["result"]["workspaces"]
            .as_array()
            .context("missing result.workspaces")?;
        let workspace = workspaces
            .iter()
            .find(|w| w["focused"].as_bool() == Some(true))
            .or_else(|| workspaces.first())
            .context("no herdr workspaces found")?;
        self.capture_workspace_from_value(workspace, name, false)
    }

    fn capture_current_tab(&self, name: Option<String>) -> Result<TabCapture> {
        let workspace = self.capture_current_workspace(None)?;
        let mut tab = workspace
            .tabs
            .into_iter()
            .find(|tab| {
                tab.tab
                    .raw
                    .as_ref()
                    .and_then(|raw| raw.get("focused"))
                    .and_then(Value::as_bool)
                    == Some(true)
            })
            .context("no focused Herdr tab found")?;

        if let Some(name) = name {
            tab.tab.name = slug(&name);
        }
        Ok(tab)
    }

    fn capture_current_pane(&self, name: Option<String>) -> Result<PaneTemplate> {
        let workspace = self.capture_current_workspace(None)?;
        let mut pane = workspace
            .tabs
            .into_iter()
            .flat_map(|tab| tab.panes.into_iter())
            .find(|pane| {
                pane.raw
                    .as_ref()
                    .and_then(|raw| raw.get("focused"))
                    .and_then(Value::as_bool)
                    == Some(true)
            })
            .context("no focused Herdr pane found")?;

        if let Some(name) = name {
            pane.name = slug(&name);
        }
        Ok(pane)
    }

    fn restore_workspace(
        &self,
        workspace: &WorkspaceCapture,
        dry_run: bool,
        force: bool,
    ) -> Result<()> {
        if workspace.workspace.backend != BackendKind::Herdr {
            bail!(
                "workspace was captured for {}, not herdr",
                workspace.workspace.backend
            );
        }

        if !force && self.workspace_label_exists(workspace.workspace.label_or_name())? {
            bail!(
                "live Herdr workspace '{}' already exists; use --force to create another",
                workspace.workspace.label_or_name()
            );
        }

        let mut args = vec!["workspace".into(), "create".into()];
        if let Some(cwd) = &workspace.workspace.cwd {
            args.extend(["--cwd".into(), cwd.display().to_string()]);
        }
        args.extend(["--label".into(), workspace.workspace.label_or_name().into()]);

        let created = self
            .run(&args, dry_run)?
            .unwrap_or_else(|| fake_create_response("dry-workspace", "dry-tab", "dry-pane"));
        let workspace_id = created["result"]["workspace"]["workspace_id"]
            .as_str()
            .unwrap_or("dry-workspace")
            .to_string();
        let mut current_tab_id = created["result"]["tab"]["tab_id"]
            .as_str()
            .unwrap_or("dry-tab")
            .to_string();
        let mut current_root_pane = created["result"]["root_pane"]["pane_id"]
            .as_str()
            .unwrap_or("dry-pane")
            .to_string();

        for (idx, tab_capture) in workspace.tabs.iter().enumerate() {
            let tab = &tab_capture.tab;
            if idx == 0 {
                let rename = vec![
                    "tab".into(),
                    "rename".into(),
                    current_tab_id.clone(),
                    tab.label_or_name().into(),
                ];
                self.run(&rename, dry_run)?;
            } else {
                let mut tab_args = vec![
                    "tab".into(),
                    "create".into(),
                    "--workspace".into(),
                    workspace_id.clone(),
                ];
                if let Some(cwd) = &tab.cwd {
                    tab_args.extend(["--cwd".into(), cwd.display().to_string()]);
                }
                tab_args.extend(["--label".into(), tab.label_or_name().into()]);
                let created_tab = self
                    .run(&tab_args, dry_run)?
                    .unwrap_or_else(|| fake_create_response(&workspace_id, "dry-tab", "dry-pane"));
                current_tab_id = created_tab["result"]["tab"]["tab_id"]
                    .as_str()
                    .unwrap_or("dry-tab")
                    .to_string();
                current_root_pane = created_tab["result"]["root_pane"]["pane_id"]
                    .as_str()
                    .unwrap_or("dry-pane")
                    .to_string();
            }

            restore_tab(self, tab, &tab_capture.panes, &current_root_pane, dry_run)
                .with_context(|| format!("restoring tab {} ({})", idx + 1, tab.label_or_name()))?;
        }
        Ok(())
    }

    fn apply_tab(
        &self,
        tab: &TabCapture,
        workspace: Option<&str>,
        dry_run: bool,
        force: bool,
    ) -> Result<()> {
        let workspace_id = self.resolve_workspace_id(workspace)?;
        if !force && self.tab_label_exists(&workspace_id, tab.tab.label_or_name())? {
            bail!(
                "live Herdr workspace already has tab '{}'; use --force to create another",
                tab.tab.label_or_name()
            );
        }
        let mut tab_args = vec![
            "tab".into(),
            "create".into(),
            "--workspace".into(),
            workspace_id,
        ];
        if let Some(cwd) = &tab.tab.cwd {
            tab_args.extend(["--cwd".into(), cwd.display().to_string()]);
        }
        tab_args.extend(["--label".into(), tab.tab.label_or_name().into()]);

        let created_tab = self
            .run(&tab_args, dry_run)?
            .unwrap_or_else(|| fake_create_response("dry-workspace", "dry-tab", "dry-pane"));
        let root_pane = created_tab["result"]["root_pane"]["pane_id"]
            .as_str()
            .unwrap_or("dry-pane")
            .to_string();
        restore_tab(self, &tab.tab, &tab.panes, &root_pane, dry_run)
    }

    fn smart_nav(&self, direction: Direction, key: &str) -> Result<()> {
        let pane_id = match std::env::var("HERDR_ACTIVE_PANE_ID")
            .or_else(|_| std::env::var("HERDR_PANE_ID"))
        {
            Ok(id) if !id.is_empty() => id,
            _ => self.json(&["pane", "current", "--current"])?["result"]["pane"]["pane_id"]
                .as_str()
                .context("missing current pane id")?
                .to_string(),
        };

        let process = self
            .json(&["pane", "process-info", "--pane", &pane_id])
            .unwrap_or(Value::Null);
        if foreground_matches_passthrough(&process)? {
            let args = vec!["pane".into(), "send-keys".into(), pane_id, key.into()];
            self.run(&args, false)?;
        } else {
            let args = vec![
                "pane".into(),
                "focus".into(),
                "--direction".into(),
                direction.as_herdr().into(),
                "--pane".into(),
                pane_id,
            ];
            self.run(&args, false)?;
        }
        Ok(())
    }
}

fn restore_tab(
    backend: &HerdrBackend,
    tab: &TabTemplate,
    panes: &[PaneTemplate],
    root_pane: &str,
    dry_run: bool,
) -> Result<()> {
    let mut leaves = vec![Leaf {
        pane_id: root_pane.to_string(),
        rect: tab.layout.area.unwrap_or(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        }),
    }];

    for split in &tab.layout.splits {
        let target_idx = leaves
            .iter()
            .enumerate()
            .min_by_key(|(_, leaf)| leaf.rect.distance(&split.rect))
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let target = leaves[target_idx].clone();
        let args = vec![
            "pane".into(),
            "split".into(),
            target.pane_id.clone(),
            "--direction".into(),
            split.direction.as_herdr().into(),
            "--ratio".into(),
            format!("{:.4}", split.ratio),
            "--no-focus".into(),
        ];
        let created = backend
            .run(&args, dry_run)?
            .unwrap_or_else(|| fake_pane_split("dry-pane"));
        let new_pane = created["result"]["pane"]["pane_id"]
            .as_str()
            .unwrap_or("dry-pane")
            .to_string();
        let (first, second) = split_rect(target.rect, split.direction, split.ratio);
        leaves[target_idx] = Leaf {
            pane_id: target.pane_id,
            rect: first,
        };
        leaves.push(Leaf {
            pane_id: new_pane,
            rect: second,
        });
    }

    for pane in panes {
        let pane_id = match pane.rect {
            Some(rect) => leaves
                .iter()
                .min_by_key(|leaf| leaf.rect.distance(&rect))
                .map(|leaf| leaf.pane_id.clone()),
            None => leaves.first().map(|leaf| leaf.pane_id.clone()),
        }
        .context("no restored pane available")?;

        if pane.label.is_some() || pane.name != "unnamed" {
            let args = vec![
                "pane".into(),
                "rename".into(),
                pane_id.clone(),
                pane.label_or_name().into(),
            ];
            backend.run(&args, dry_run)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Leaf {
    pane_id: String,
    rect: Rect,
}

fn split_rect(rect: Rect, direction: SplitDirection, ratio: f64) -> (Rect, Rect) {
    match direction {
        SplitDirection::Right => {
            let first_width = ((rect.width as f64) * ratio).round() as i64;
            (
                Rect {
                    width: first_width,
                    ..rect
                },
                Rect {
                    x: rect.x + first_width,
                    width: rect.width - first_width,
                    ..rect
                },
            )
        }
        SplitDirection::Down => {
            let first_height = ((rect.height as f64) * ratio).round() as i64;
            (
                Rect {
                    height: first_height,
                    ..rect
                },
                Rect {
                    y: rect.y + first_height,
                    height: rect.height - first_height,
                    ..rect
                },
            )
        }
    }
}

fn parse_layout(raw: Option<&Value>) -> LayoutTemplate {
    let Some(raw) = raw else {
        return LayoutTemplate::default();
    };
    let area = parse_rect(raw.get("area"));
    let splits = raw
        .get("splits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|split| {
            let direction = match split.get("direction")?.as_str()? {
                "right" => SplitDirection::Right,
                "down" => SplitDirection::Down,
                _ => return None,
            };
            Some(SplitTemplate {
                direction,
                ratio: split.get("ratio").and_then(Value::as_f64).unwrap_or(0.5),
                rect: parse_rect(split.get("rect")).unwrap_or(Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                }),
            })
        })
        .collect();
    LayoutTemplate { area, splits }
}

fn parse_rect(value: Option<&Value>) -> Option<Rect> {
    let value = value?;
    Some(Rect {
        x: value.get("x")?.as_i64()?,
        y: value.get("y")?.as_i64()?,
        width: value.get("width")?.as_i64()?,
        height: value.get("height")?.as_i64()?,
    })
}

fn layout_rect_for_pane(raw: Option<&Value>, pane_id: &str) -> Option<Rect> {
    raw?.get("panes")?
        .as_array()?
        .iter()
        .find(|pane| pane["pane_id"].as_str() == Some(pane_id))
        .and_then(|pane| parse_rect(pane.get("rect")))
}

fn first_cmdline(process: &Value) -> Option<String> {
    process["result"]["process_info"]["foreground_processes"]
        .as_array()?
        .iter()
        .find_map(|proc_| proc_["cmdline"].as_str().map(str::to_string))
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

fn foreground_matches_passthrough(process: &Value) -> Result<bool> {
    let pattern = nav_passthrough_pattern();
    let re = Regex::new(&pattern).context("invalid KITSUNE_NAV_PASSTHROUGH regex")?;
    let Some(processes) = process["result"]["process_info"]["foreground_processes"].as_array()
    else {
        return Ok(false);
    };
    Ok(processes.iter().any(|proc_| {
        ["name", "argv0"]
            .iter()
            .filter_map(|key| proc_.get(*key).and_then(Value::as_str))
            .any(|name| re.is_match(name))
    }))
}

fn fake_create_response(workspace: &str, tab: &str, pane: &str) -> Value {
    serde_json::json!({
        "result": {
            "workspace": {"workspace_id": workspace},
            "tab": {"tab_id": tab},
            "root_pane": {"pane_id": pane}
        }
    })
}

fn fake_pane_split(pane: &str) -> Value {
    serde_json::json!({"result": {"pane": {"pane_id": pane}}})
}

#[cfg(test)]
mod tests {
    use super::{foreground_matches_passthrough, split_rect};
    use crate::model::{Rect, SplitDirection};

    #[test]
    fn split_rect_right() {
        let (left, right) = split_rect(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 50,
            },
            SplitDirection::Right,
            0.6,
        );
        assert_eq!(left.width, 60);
        assert_eq!(right.x, 60);
        assert_eq!(right.width, 40);
    }

    #[test]
    fn detects_nvim_passthrough() {
        let value = serde_json::json!({
            "result": {"process_info": {"foreground_processes": [{"name": "nvim", "argv0": "nvim"}]}}
        });
        assert!(foreground_matches_passthrough(&value).unwrap());
    }

    #[test]
    fn detects_helix_and_lazygit_passthrough() {
        let value = serde_json::json!({
            "result": {"process_info": {"foreground_processes": [
                {"name": "hx", "argv0": "hx"},
                {"name": "lazygit", "argv0": "lazygit"}
            ]}}
        });
        assert!(foreground_matches_passthrough(&value).unwrap());
    }
}
