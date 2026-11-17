use crate::api::schema::{EmptyParams, Method, Request, ServerLiveHandoffParams};

pub(super) fn run_server_command(args: &[String]) -> std::io::Result<Option<i32>> {
    let Some(subcommand) = args.first().map(|arg| arg.as_str()) else {
        return Ok(None);
    };
    match subcommand {
        "stop" => server_stop(&args[1..]).map(Some),
        "live-handoff" => server_live_handoff(&args[1..]).map(Some),
        "--handoff-import" => Ok(None),
        "reload-config" => server_reload_config(&args[1..]).map(Some),
        "agent-manifests" | "update-agent-manifests" | "reload-agent-manifests" => {
            retired_agent_manifests(subcommand, &args[1..]).map(Some)
        }
        "help" | "--help" | "-h" => {
            print_server_help();
            Ok(Some(0))
        }
        _ => {
            print_server_help();
            Ok(Some(2))
        }
    }
}

fn server_stop(args: &[String]) -> std::io::Result<i32> {
    if !args.is_empty() {
        eprintln!("{}", crate::product::usage("server stop"));
        return Ok(2);
    }
    match crate::session::stop_active_server() {
        Ok(()) => Ok(0),
        Err(err) => {
            eprintln!("{err}");
            Ok(1)
        }
    }
}

fn server_reload_config(args: &[String]) -> std::io::Result<i32> {
    if !args.is_empty() {
        eprintln!("{}", crate::product::usage("server reload-config"));
        return Ok(2);
    }
    super::print_response(&super::send_request(&Request {
        id: "cli:server:reload-config".into(),
        method: Method::ServerReloadConfig(EmptyParams::default()),
    })?)
}

fn retired_agent_manifests(command: &str, args: &[String]) -> std::io::Result<i32> {
    let allows_json = command != "reload-agent-manifests";
    let json = match args {
        [] => false,
        [flag] if allows_json && flag == "--json" => true,
        _ => {
            let suffix = if allows_json { " [--json]" } else { "" };
            eprintln!(
                "{}",
                crate::product::usage(&format!("server {command}{suffix}"))
            );
            return Ok(2);
        }
    };
    if json {
        super::print_response(&serde_json::json!({
            "id": format!("cli:server:{command}"),
            "error": {
                "code": "feature_retired",
                "message": crate::api::schema::DETECTOR_MANIFEST_RETIREMENT_MESSAGE,
            }
        }))
    } else {
        eprintln!(
            "feature_retired: {}",
            crate::api::schema::DETECTOR_MANIFEST_RETIREMENT_MESSAGE
        );
        Ok(1)
    }
}

fn server_live_handoff(args: &[String]) -> std::io::Result<i32> {
    let Some(params) = parse_live_handoff_params(args) else {
        eprintln!(
            "{}",
            crate::product::usage("server live-handoff [--import-exe <path>] [--expected-protocol <n>] [--expected-version <version>]")
        );
        return Ok(2);
    };

    // Live handoff is itself a protocol-mismatch recovery path, so it must
    // reach the running server without the normal CLI compatibility guard.
    let response = super::send_request_unchecked(&Request {
        id: "cli:server:live-handoff".into(),
        method: Method::ServerLiveHandoff(params),
    })?;
    if response.get("error").is_some() {
        let rendered = serde_json::to_string(&response).unwrap_or_else(|err| {
            format!(
                "{{\"error\":{{\"code\":\"render_failed\",\"message\":\"failed to render error response: {err}\"}}}}"
            )
        });
        eprintln!("{rendered}");
        return Ok(1);
    }

    eprintln!(
        "live handoff complete; server log: {}",
        crate::session::data_dir()
            .join(crate::product::SERVER_LOG_FILE_NAME)
            .display()
    );
    Ok(0)
}

fn parse_live_handoff_params(args: &[String]) -> Option<ServerLiveHandoffParams> {
    let mut params = ServerLiveHandoffParams::default();
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        let (flag, value) = if let Some((flag, value)) = arg.split_once('=') {
            (flag, Some(value.to_string()))
        } else {
            let value = args.get(idx + 1).cloned();
            idx += 1;
            (arg.as_str(), value)
        };
        let value = value?;
        match flag {
            "--import-exe" => params.import_exe = Some(value),
            "--expected-protocol" => {
                params.expected_protocol = Some(value.parse().ok()?);
            }
            "--expected-version" => params.expected_version = Some(value),
            _ => return None,
        }
        idx += 1;
    }
    Some(params)
}

fn print_server_help() {
    eprintln!("{} server commands:", crate::product::cli_name());
    for (command, description) in [
        ("server", "run as headless server"),
        ("server stop", "stop the running server via the API socket"),
        (
            "server live-handoff",
            "hand off live panes to a new local server",
        ),
        (
            "server reload-config",
            "reload config.toml in the running server",
        ),
        (
            "server agent-manifests [--json]",
            "retired detector-manifest management",
        ),
        (
            "server update-agent-manifests [--json]",
            "retired detector-manifest management",
        ),
        (
            "server reload-agent-manifests",
            "retired detector-manifest management",
        ),
    ] {
        eprintln!("  {}  {description}", crate::product::command(command));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_manifest_commands_fail_locally() {
        for command in [
            "agent-manifests",
            "update-agent-manifests",
            "reload-agent-manifests",
        ] {
            assert_eq!(retired_agent_manifests(command, &[]).unwrap(), 1);
            if command != "reload-agent-manifests" {
                assert_eq!(
                    retired_agent_manifests(command, &["--json".into()]).unwrap(),
                    1
                );
            }
            assert_eq!(
                retired_agent_manifests(command, &["--invalid".into()]).unwrap(),
                2
            );
        }
    }

    #[test]
    fn live_handoff_params_parse_remote_update_fields() {
        let args = vec![
            "--import-exe".to_string(),
            "/home/me/.local/bin/kitsune".to_string(),
            "--expected-protocol=9".to_string(),
            "--expected-version".to_string(),
            "0.6.2".to_string(),
        ];
        let params = parse_live_handoff_params(&args).expect("params");
        assert_eq!(
            params.import_exe.as_deref(),
            Some("/home/me/.local/bin/kitsune")
        );
        assert_eq!(params.expected_protocol, Some(9));
        assert_eq!(params.expected_version.as_deref(), Some("0.6.2"));
    }
}
