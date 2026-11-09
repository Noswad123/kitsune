use super::*;
use crate::platform::{ForegroundJob, ForegroundProcess};
use std::io;
use std::path::{Path, PathBuf};

fn process(pid: u32, name: &str, argv: &[&str], cmdline: Option<&str>) -> ForegroundProcess {
    ForegroundProcess {
        pid,
        name: name.into(),
        argv0: None,
        argv: Some(argv.iter().map(|arg| (*arg).into()).collect()),
        cmdline: cmdline.map(str::to_string),
    }
}

fn refused() -> io::Result<PathBuf> {
    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

#[test]
fn lexical_matches_and_single_component_misses_never_resolve() {
    for (name, argv, expected) in [
        (
            "Claude",
            vec!["/unused/entry"],
            Some((Agent::Claude, "Claude")),
        ),
        ("claude-code", vec![], Some((Agent::Claude, "claude-code"))),
        (
            "opencode.exe",
            vec![],
            Some((Agent::OpenCode, "opencode.exe")),
        ),
        (
            "worker",
            vec!["/bin/ghcs"],
            Some((Agent::GithubCopilot, "copilot")),
        ),
        (
            "node",
            vec![
                "node",
                "/pkg/node_modules/@earendil-works/pi-coding-agent/dist/cli.js",
            ],
            Some((Agent::Pi, "pi")),
        ),
        ("worker", vec!["unknown"], None),
    ] {
        let job = ForegroundJob {
            process_group_id: 1,
            processes: vec![process(1, name, &argv, Some("unknown"))],
        };
        assert_eq!(
            identify_agent_in_job_with_resolver(&job, &mut |_| panic!("unexpected resolution")),
            expected.map(|(agent, name)| (agent, name.into())),
        );
    }
    assert_eq!(
        normalized_process_name(&process(1, "Claude", &[], None)),
        "Claude"
    );
}

#[test]
fn scripted_resolution_results_use_only_the_resolved_utf8_basename() {
    for (result, expected) in [
        (refused(), None),
        (
            Ok(PathBuf::from("/resolved/CURSOR-AGENT.EXE")),
            Some("cursor"),
        ),
        (Ok(PathBuf::from("/resolved/unknown")), None),
        (Ok(PathBuf::from("/")), None),
        (Ok(PathBuf::from("")), None),
    ] {
        let mut result = Some(result);
        let mut calls = vec![];
        assert_eq!(
            agent_name_from_path_token("\"/input/entry\"", &mut |path| {
                calls.push(path.to_path_buf());
                result.take().expect("only one resolution")
            }),
            expected.map(str::to_string),
        );
        assert_eq!(calls, [PathBuf::from("/input/entry")]);
        assert!(result.is_none());
    }
}

#[cfg(unix)]
#[test]
fn resolved_non_utf8_basename_is_not_lossily_matched() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(
        b"/resolved/codex\xff".to_vec(),
    ));
    let mut calls = 0;
    assert_eq!(
        agent_name_from_path_token("/input/entry", &mut |_| {
            calls += 1;
            Ok(path.clone())
        }),
        None
    );
    assert_eq!(calls, 1);
}

#[test]
fn native_component_gate_and_token_rejections_precede_resolution() {
    for token in ["", "''", "--flag", "unknown"] {
        assert_eq!(
            agent_name_from_path_token(token, &mut |_| panic!("unexpected resolution")),
            None
        );
    }
    let token = r"C:\tools\unknown";
    let mut calls = 0;
    assert_eq!(
        agent_name_from_path_token(token, &mut |_| {
            calls += 1;
            refused()
        }),
        None
    );
    assert_eq!(
        calls,
        usize::from(Path::new(token).components().count() >= 2)
    );
}

#[test]
fn runtime_then_argv0_then_cmdline_are_lazy_and_preserve_repeated_paths() {
    for success_at in [Some(1), Some(2), Some(3), None] {
        let job = ForegroundJob {
            process_group_id: 99,
            processes: vec![process(
                1,
                "node",
                &["/input/runtime", "/input/runtime"],
                Some("/input/cmdline --flag"),
            )],
        };
        let mut calls = vec![];
        let found = identify_agent_in_job_with_resolver(&job, &mut |path| {
            calls.push(path.to_path_buf());
            if success_at == Some(calls.len()) {
                Ok(PathBuf::from("/resolved/codex"))
            } else {
                refused()
            }
        });
        assert_eq!(found, success_at.map(|_| (Agent::Codex, "codex".into())));
        let expected = ["/input/runtime", "/input/runtime", "/input/cmdline"];
        assert_eq!(
            calls,
            expected[..success_at.unwrap_or(3)]
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn runtime_wrapper_branches_share_the_injected_resolver() {
    for (runtime, argv) in [
        ("node", vec!["node", "--", "/input/entry"]),
        ("python3", vec!["python3", "/input/entry"]),
        ("bash", vec!["bash", "/input/entry"]),
        (
            "cmd.exe",
            vec!["cmd.exe", "/C", "call '/input/entry' --flag"],
        ),
        (
            "powershell.exe",
            vec!["powershell.exe", "-File", "/input/entry"],
        ),
        ("pwsh", vec!["pwsh", "-Command", "& '/input/entry' --flag"]),
        ("pwsh", vec!["pwsh", "./input/entry"]),
    ] {
        let job = ForegroundJob {
            process_group_id: 1,
            processes: vec![process(1, runtime, &argv, Some("/not/reached"))],
        };
        let mut calls = vec![];
        assert_eq!(
            identify_agent_in_job_with_resolver(&job, &mut |path| {
                calls.push(path.to_path_buf());
                Ok(PathBuf::from("/resolved/ghcs"))
            }),
            Some((Agent::GithubCopilot, "copilot".into()))
        );
        assert_eq!(
            calls,
            [PathBuf::from(if argv.len() == 2 && runtime == "pwsh" {
                "./input/entry"
            } else {
                "/input/entry"
            })]
        );
    }
}

#[test]
fn leader_is_revisited_without_caching_and_later_traversal_can_succeed() {
    let job = ForegroundJob {
        process_group_id: 7,
        processes: vec![
            process(7, "worker", &["/input/entry"], None),
            process(8, "claude", &[], None),
        ],
    };
    let mut calls = vec![];
    assert_eq!(
        identify_agent_in_job_with_resolver(&job, &mut |path| {
            calls.push(path.to_path_buf());
            if calls.len() == 1 {
                refused()
            } else {
                Ok(PathBuf::from("/resolved/codex"))
            }
        }),
        Some((Agent::Codex, "codex".into()))
    );
    assert_eq!(
        calls,
        [PathBuf::from("/input/entry"), PathBuf::from("/input/entry")]
    );
}

#[test]
fn selected_leader_stops_before_resolving_any_other_process() {
    let job = ForegroundJob {
        process_group_id: 7,
        processes: vec![
            process(8, "worker", &["/not/reached"], None),
            process(7, "worker", &["/input/leader"], None),
        ],
    };
    let mut calls = vec![];
    assert_eq!(
        identify_agent_in_job_with_resolver(&job, &mut |path| {
            calls.push(path.to_path_buf());
            Ok(PathBuf::from("/resolved/claude-code"))
        }),
        Some((Agent::Claude, "claude".into()))
    );
    assert_eq!(calls, [PathBuf::from("/input/leader")]);
}

#[test]
fn priority_and_first_equal_score_winner_are_unchanged() {
    for reverse in [false, true] {
        let mut wrapped = vec![
            process(2, "worker", &["/input/first"], None),
            process(3, "worker", &["/input/second"], None),
        ];
        if reverse {
            wrapped.reverse();
        }
        let mut processes = vec![process(1, "claude", &[], None)];
        processes.extend(wrapped);
        processes.push(process(4, "gemini", &[], None));
        let job = ForegroundJob {
            process_group_id: 99,
            processes,
        };
        let mut calls = vec![];
        assert_eq!(
            identify_agent_in_job_with_resolver(&job, &mut |path| {
                calls.push(path.to_path_buf());
                Ok(PathBuf::from(if path == Path::new("/input/first") {
                    "/resolved/codex"
                } else {
                    "/resolved/pi"
                }))
            }),
            Some(if reverse {
                (Agent::Pi, "pi".into())
            } else {
                (Agent::Codex, "codex".into())
            })
        );
        let mut expected = vec![
            PathBuf::from("/input/first"),
            PathBuf::from("/input/second"),
        ];
        if reverse {
            expected.reverse();
        }
        assert_eq!(calls, expected);
    }
}
