use std::collections::{HashMap, HashSet, VecDeque};

use super::{ForegroundJob, ForegroundProcess};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProcessEntry {
    pub(super) pid: u32,
    pub(super) parent_pid: u32,
    pub(super) name: String,
    pub(super) argv0: Option<String>,
    pub(super) argv: Option<Vec<String>>,
    pub(super) cmdline: Option<String>,
}

pub(super) fn select_pane_foreground_job<A: PartialEq>(
    shell_pid: u32,
    entries: &[ProcessEntry],
    mut classify: impl FnMut(&ForegroundJob) -> Option<A>,
) -> Option<ForegroundJob> {
    let shell = entries.iter().find(|entry| entry.pid == shell_pid)?;
    let shell_job = || ForegroundJob {
        process_group_id: shell_pid,
        processes: vec![foreground_process_from_entry(shell)],
    };

    let descendants = descendant_entries(shell_pid, entries);
    let mut candidates = Vec::new();
    for entry in &descendants {
        let process = foreground_process_from_entry(entry);
        let job = ForegroundJob {
            process_group_id: entry.pid,
            processes: vec![process],
        };
        if let Some(agent) = classify(&job) {
            candidates.push((*entry, agent));
        }
    }

    match candidates.len() {
        1 => candidates
            .pop()
            .map(|(entry, _)| foreground_job_from_entry(entry)),
        _ => select_single_agent_chain_candidate(&candidates, entries).map_or_else(
            || Some(shell_job()),
            |entry| Some(foreground_job_from_entry(entry)),
        ),
    }
}

fn foreground_job_from_entry(entry: &ProcessEntry) -> ForegroundJob {
    ForegroundJob {
        process_group_id: entry.pid,
        processes: vec![foreground_process_from_entry(entry)],
    }
}

fn select_single_agent_chain_candidate<'a, A: PartialEq>(
    candidates: &[(&'a ProcessEntry, A)],
    entries: &[ProcessEntry],
) -> Option<&'a ProcessEntry> {
    let (_, first_agent) = candidates.first()?;
    if !candidates.iter().all(|(_, agent)| agent == first_agent) {
        return None;
    }

    let parent_by_pid: HashMap<u32, u32> = entries
        .iter()
        .map(|entry| (entry.pid, entry.parent_pid))
        .collect();

    candidates.iter().map(|(entry, _)| *entry).find(|entry| {
        candidates.iter().all(|(other, _)| {
            entry.pid == other.pid || process_is_ancestor(entry.pid, other.pid, &parent_by_pid)
        })
    })
}

fn process_is_ancestor(
    ancestor_pid: u32,
    descendant_pid: u32,
    parent_by_pid: &HashMap<u32, u32>,
) -> bool {
    let mut current = descendant_pid;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let Some(parent) = parent_by_pid.get(&current).copied() else {
            return false;
        };
        if parent == ancestor_pid {
            return true;
        }
        if parent == 0 {
            return false;
        }
        current = parent;
    }

    false
}

pub(super) fn descendant_entries(root_pid: u32, entries: &[ProcessEntry]) -> Vec<&ProcessEntry> {
    let mut children: HashMap<u32, Vec<&ProcessEntry>> = HashMap::new();
    for entry in entries {
        children.entry(entry.parent_pid).or_default().push(entry);
    }

    let mut output = Vec::new();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    visited.insert(root_pid);
    if let Some(root_children) = children.get(&root_pid) {
        for entry in root_children.iter().copied() {
            if visited.insert(entry.pid) {
                queue.push_back(entry);
            }
        }
    }
    while let Some(entry) = queue.pop_front() {
        output.push(entry);
        if let Some(next) = children.get(&entry.pid) {
            for child in next.iter().copied() {
                if visited.insert(child.pid) {
                    queue.push_back(child);
                }
            }
        }
    }
    output
}

pub(super) fn foreground_process_from_entry(entry: &ProcessEntry) -> ForegroundProcess {
    ForegroundProcess {
        pid: entry.pid,
        name: entry.name.clone(),
        argv0: entry.argv0.clone(),
        argv: entry.argv.clone(),
        cmdline: entry.cmdline.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ForegroundJob, ProcessEntry};

    fn select_pane_foreground_job(
        shell_pid: u32,
        entries: &[ProcessEntry],
    ) -> Option<ForegroundJob> {
        super::select_pane_foreground_job(shell_pid, entries, |job| {
            crate::detect::identify_agent_in_job(job).map(|(agent, _)| agent)
        })
    }

    fn test_entry(pid: u32, parent_pid: u32, name: &str, argv: &[&str]) -> ProcessEntry {
        ProcessEntry {
            pid,
            parent_pid,
            name: name.to_string(),
            argv0: argv.first().map(|value| (*value).to_string()),
            argv: Some(argv.iter().map(|value| (*value).to_string()).collect()),
            cmdline: Some(argv.join(" ")),
        }
    }

    #[test]
    fn callback_receives_breadth_first_singletons_with_all_fields() {
        // Neither Clone nor Eq is required for classifier identities.
        #[derive(PartialEq)]
        struct Identity(f32);

        let entries = vec![
            test_entry(10, 1, "root", &["root"]),
            ProcessEntry {
                pid: 40,
                parent_pid: 20,
                name: "display name".into(),
                argv0: Some("different argv0".into()),
                argv: Some(vec!["argument zero".into(), "".into(), "two words".into()]),
                cmdline: Some("independent raw command line".into()),
            },
            test_entry(30, 10, "first child", &[]),
            test_entry(20, 10, "second child", &["second"]),
            ProcessEntry {
                pid: 50,
                parent_pid: 30,
                name: "no command metadata".into(),
                argv0: None,
                argv: None,
                cmdline: None,
            },
        ];
        let expected = |index: usize| ForegroundJob {
            process_group_id: entries[index].pid,
            processes: vec![super::ForegroundProcess {
                pid: entries[index].pid,
                name: entries[index].name.clone(),
                argv0: entries[index].argv0.clone(),
                argv: entries[index].argv.clone(),
                cmdline: entries[index].cmdline.clone(),
            }],
        };
        let mut observed = Vec::new();
        let job = super::select_pane_foreground_job(10, &entries, |job| {
            observed.push(job.clone());
            (job.process_group_id == 40).then_some(Identity(1.0))
        });

        assert_eq!(
            observed,
            vec![expected(2), expected(3), expected(4), expected(1)]
        );
        assert_eq!(job, Some(expected(1)));
        assert_eq!(
            super::select_pane_foreground_job(10, &entries, |_| None::<Identity>),
            Some(expected(0))
        );
    }

    #[test]
    fn empty_or_missing_root_does_not_classify() {
        let entries = vec![test_entry(20, 10, "orphan", &[])];
        for entries in [&[][..], entries.as_slice()] {
            assert_eq!(
                super::select_pane_foreground_job(10, entries, |_| -> Option<()> {
                    panic!("missing root must not classify descendants")
                }),
                None
            );
        }
    }

    #[test]
    fn root_uses_first_match_and_is_not_classified() {
        let entries = vec![
            test_entry(10, 10, "first root", &[]),
            test_entry(10, 1, "second root", &["second"]),
        ];
        let job = super::select_pane_foreground_job(10, &entries, |_| -> Option<()> {
            panic!("root must not be classified")
        })
        .unwrap();
        assert_eq!(
            job,
            ForegroundJob {
                process_group_id: 10,
                processes: vec![super::foreground_process_from_entry(&entries[0])],
            }
        );
    }

    #[test]
    fn same_identity_ancestor_can_have_branching_candidates() {
        let entries = vec![
            test_entry(10, 1, "root", &[]),
            test_entry(20, 10, "ancestor", &[]),
            test_entry(30, 20, "left", &[]),
            test_entry(40, 20, "right", &[]),
        ];
        let job = super::select_pane_foreground_job(10, &entries, |_| Some("same")).unwrap();
        assert_eq!(job.process_group_id, 20);
        assert_eq!(
            job.processes,
            vec![super::foreground_process_from_entry(&entries[1])]
        );
    }

    #[test]
    fn duplicate_pid_traversal_uses_first_reachable_entry_on_enqueue() {
        let entries = vec![
            test_entry(10, 1, "root", &[]),
            test_entry(40, 20, "earlier source but deeper", &[]),
            test_entry(20, 10, "child", &[]),
            test_entry(40, 10, "first reachable", &[]),
            test_entry(40, 10, "later duplicate", &[]),
            test_entry(50, 40, "grandchild", &[]),
        ];
        assert_eq!(
            super::descendant_entries(10, &entries),
            vec![&entries[2], &entries[3], &entries[5]]
        );
        let mut observed = Vec::new();
        let job = super::select_pane_foreground_job(10, &entries, |job| {
            observed.push(job.process_group_id);
            (job.process_group_id == 40).then_some(())
        })
        .unwrap();
        assert_eq!(observed, vec![20, 40, 50]);
        assert_eq!(
            job.processes,
            vec![super::foreground_process_from_entry(&entries[3])]
        );
    }

    #[test]
    fn duplicate_pid_parent_map_uses_last_entry_even_if_unreachable() {
        let mut entries = vec![
            test_entry(10, 1, "root", &[]),
            test_entry(20, 10, "candidate ancestor", &[]),
            test_entry(30, 20, "candidate descendant", &[]),
            test_entry(30, 99, "unreachable duplicate", &[]),
        ];
        let classify = |_: &ForegroundJob| Some(());
        assert_eq!(
            super::select_pane_foreground_job(10, &entries, classify)
                .unwrap()
                .process_group_id,
            10
        );
        entries.swap(2, 3);
        assert_eq!(
            super::select_pane_foreground_job(10, &entries, classify)
                .unwrap()
                .process_group_id,
            20
        );
    }

    #[test]
    fn windows_process_tree_selects_direct_agent_descendant() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "codex.exe", &["codex.exe"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 20);
        assert_eq!(job.processes.len(), 1);
        assert_eq!(job.processes[0].name, "codex.exe");
    }

    #[test]
    fn windows_process_tree_selects_wrapped_agent_descendant() {
        let entries = vec![
            test_entry(10, 1, "cmd.exe", &["cmd.exe"]),
            test_entry(
                20,
                10,
                "node.exe",
                &[
                    "node.exe",
                    "C:\\Users\\kitsune\\AppData\\Roaming\\npm\\node_modules\\codex\\bin\\codex.js",
                ],
            ),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 20);
        assert_eq!(job.processes[0].name, "node.exe");
    }

    #[test]
    fn windows_process_tree_selects_cmd_wrapped_agent_descendant() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(
                20,
                10,
                "cmd.exe",
                &[
                    "cmd.exe",
                    "/D",
                    "/S",
                    "/C",
                    "C:\\Users\\kitsune\\AppData\\Roaming\\npm\\codex.cmd --model gpt-5",
                ],
            ),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 20);
        assert_eq!(job.processes[0].name, "cmd.exe");
    }

    #[test]
    fn windows_process_tree_selects_topmost_codex_process_in_single_agent_chain() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(
                20,
                10,
                "node.exe",
                &[
                    "node.exe",
                    "C:\\Users\\kitsune\\AppData\\Roaming\\npm\\node_modules\\@openai\\codex\\bin\\codex.js",
                ],
            ),
            test_entry(
                30,
                20,
                "codex.exe",
                &["C:\\Users\\kitsune\\AppData\\Roaming\\npm\\node_modules\\@openai\\codex\\node_modules\\@openai\\codex-win32-x64\\vendor\\x86_64-pc-windows-msvc\\bin\\codex.exe"],
            ),
            test_entry(40, 30, "node_repl.exe", &["node_repl.exe"]),
            test_entry(
                50,
                40,
                "codex.exe",
                &["codex.exe", "app-server", "--listen", "stdio://"],
            ),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 20);
        assert_eq!(job.processes[0].name, "node.exe");
    }

    #[test]
    fn windows_process_tree_selects_topmost_claude_process_in_single_agent_chain() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "claude.exe", &["claude.exe"]),
            test_entry(30, 20, "claude.exe", &["claude.exe", "mcp-server"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 20);
        assert_eq!(job.processes[0].name, "claude.exe");
    }

    #[test]
    fn windows_process_tree_returns_shell_for_same_agent_siblings() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "codex.exe", &["codex.exe"]),
            test_entry(30, 10, "codex.exe", &["codex.exe"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 10);
        assert_eq!(job.processes[0].name, "powershell.exe");
    }

    #[test]
    fn windows_process_tree_returns_shell_for_plain_descendant() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "git.exe", &["git.exe", "status"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 10);
        assert_eq!(job.processes[0].name, "powershell.exe");
    }

    #[test]
    fn windows_process_tree_returns_shell_for_multiple_agent_descendants() {
        let entries = vec![
            test_entry(10, 1, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "codex.exe", &["codex.exe"]),
            test_entry(30, 10, "claude.exe", &["claude.exe"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 10);
        assert_eq!(job.processes[0].name, "powershell.exe");
    }

    #[test]
    fn windows_process_tree_ignores_pid_reuse_cycles() {
        let entries = vec![
            test_entry(10, 30, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "codex.exe", &["codex.exe"]),
            test_entry(30, 20, "node.exe", &["node.exe"]),
        ];

        let descendants = super::descendant_entries(10, &entries);

        assert_eq!(
            descendants
                .iter()
                .map(|entry| entry.pid)
                .collect::<Vec<_>>(),
            vec![20, 30]
        );
    }

    #[test]
    fn windows_process_tree_returns_shell_when_candidate_parent_chain_cycles() {
        let entries = vec![
            test_entry(10, 40, "powershell.exe", &["powershell.exe"]),
            test_entry(20, 10, "codex.exe", &["codex.exe"]),
            test_entry(30, 10, "codex.exe", &["codex.exe"]),
            test_entry(40, 10, "node.exe", &["node.exe"]),
        ];

        let job = select_pane_foreground_job(10, &entries).unwrap();

        assert_eq!(job.process_group_id, 10);
        assert_eq!(job.processes[0].name, "powershell.exe");
    }
}
