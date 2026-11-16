use crate::api::schema::{
    EmptyParams, Method, Request, ResponseResult, ServerLiveHandoffParams, SuccessResponse,
};

pub(super) fn run_server_command(args: &[String]) -> std::io::Result<Option<i32>> {
    let Some(subcommand) = args.first().map(|arg| arg.as_str()) else {
        return Ok(None);
    };

    match subcommand {
        "stop" => server_stop(&args[1..]).map(Some),
        "live-handoff" => server_live_handoff(&args[1..]).map(Some),
        "--handoff-import" => Ok(None),
        "reload-config" => server_reload_config(&args[1..]).map(Some),
        "agent-manifests" => server_agent_manifests(&args[1..]).map(Some),
        "update-agent-manifests" => server_update_agent_manifests(&args[1..]).map(Some),
        "reload-agent-manifests" => server_reload_agent_manifests(&args[1..]).map(Some),
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

fn server_agent_manifests(args: &[String]) -> std::io::Result<i32> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => {
            eprintln!(
                "{}",
                crate::product::usage("server agent-manifests [--json]")
            );
            return Ok(2);
        }
    };

    let response = super::send_request(&Request {
        id: "cli:server:agent-manifests".into(),
        method: Method::ServerAgentManifests(EmptyParams::default()),
    })?;
    if json || response.get("error").is_some() {
        return super::print_response(&response);
    }

    print_agent_manifest_status(&response);
    Ok(0)
}

fn server_reload_agent_manifests(args: &[String]) -> std::io::Result<i32> {
    if !args.is_empty() {
        eprintln!("{}", crate::product::usage("server reload-agent-manifests"));
        return Ok(2);
    }

    super::print_response(&super::send_request(&Request {
        id: "cli:server:reload-agent-manifests".into(),
        method: Method::ServerReloadAgentManifests(EmptyParams::default()),
    })?)
}

fn server_update_agent_manifests(args: &[String]) -> std::io::Result<i32> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => {
            eprintln!(
                "{}",
                crate::product::usage("server update-agent-manifests [--json]")
            );
            return Ok(2);
        }
    };

    let response = match update_agent_manifest_status(super::send_request, || {
        crate::detect::manifest_update::check_and_update().map(|_| ())
    })? {
        Ok(response) => response,
        Err(err) => {
            if json {
                return super::print_response(&agent_manifest_update_error_response(&err));
            }
            eprintln!("failed to update agent detection manifests: {err}");
            return Ok(1);
        }
    };
    if json || response.get("error").is_some() {
        return super::print_response(&response);
    }

    print_agent_manifest_status(&response);
    Ok(0)
}

fn update_agent_manifest_status(
    mut send_request: impl FnMut(&Request) -> std::io::Result<serde_json::Value>,
    update_manifests: impl FnOnce() -> Result<(), String>,
) -> std::io::Result<Result<serde_json::Value, String>> {
    // The selected server is authoritative, not this CLI process's unset policy.
    // This is a point-in-time preflight, not an atomic transaction across a
    // concurrent server handoff. External servers keep detectors and queued
    // manifest events disabled even if ownership changes after this probe.
    let preflight_request = Request {
        id: "cli:server:agent-manifests".into(),
        method: Method::ServerAgentManifests(EmptyParams::default()),
    };
    let preflight = send_request(&preflight_request)?;
    if preflight.get("error").is_some() {
        return Ok(Ok(preflight));
    }
    if !matches!(
        serde_json::from_value::<SuccessResponse>(preflight),
        Ok(SuccessResponse {
            id,
            result: ResponseResult::AgentManifestStatus { .. },
        }) if id == preflight_request.id
    ) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "server returned invalid or uncorrelated agent manifest status for update preflight",
        ));
    }
    if let Err(err) = update_manifests() {
        return Ok(Err(err));
    }

    let reload_response = send_request(&Request {
        id: "cli:server:reload-agent-manifests".into(),
        method: Method::ServerReloadAgentManifests(EmptyParams::default()),
    })?;
    if reload_response.get("error").is_some() {
        return Ok(Ok(reload_response));
    }

    send_request(&Request {
        id: "cli:server:agent-manifests".into(),
        method: Method::ServerAgentManifests(EmptyParams::default()),
    })
    .map(Ok)
}

fn agent_manifest_update_error_response(err: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "cli:server:update-agent-manifests",
        "error": {
            "code": "agent_manifest_update_failed",
            "message": err,
        }
    })
}

fn print_agent_manifest_status(response: &serde_json::Value) {
    let result = &response["result"];
    let last_check = result["last_check_unix"]
        .as_u64()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "never".to_string());
    let last_result = result["last_result"].as_str().unwrap_or("not checked");
    println!("last check: {last_check}");
    println!("result: {last_result}");
    println!();

    let Some(manifests) = result["manifests"].as_array() else {
        return;
    };
    for manifest in manifests {
        let agent = manifest["agent"].as_str().unwrap_or("-");
        let source = manifest["source_kind"].as_str().unwrap_or("-");
        let active_version = manifest["active_version"].as_str().unwrap_or("-");
        let remote_version = manifest["cached_remote_version"].as_str().unwrap_or("-");
        let remote_result = manifest["remote_update_result"]
            .as_str()
            .unwrap_or("not checked");
        let local_override_shadowing_remote = manifest["local_override_shadowing_remote"]
            .as_bool()
            .unwrap_or(false);
        let marker = if local_override_shadowing_remote {
            "!"
        } else if manifest["remote_update_error"].as_str().is_some() {
            "x"
        } else {
            " "
        };
        println!(
            "{marker} {agent:<9} {source:<14} active {active_version:<14} remote {remote_version:<14} {remote_result}"
        );
        if let Some(error) = manifest["remote_update_error"].as_str() {
            println!("  {error}");
        } else if local_override_shadowing_remote {
            println!("  local override shadows cached remote rules");
        } else if let Some(warning) = manifest["warning"].as_str() {
            println!("  {warning}");
        }
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
    eprintln!(
        "  {}                run as headless server",
        crate::product::command("server")
    );
    eprintln!(
        "  {}           stop the running server via the API socket",
        crate::product::command("server stop")
    );
    eprintln!(
        "  {}   hand off live panes to a new local server",
        crate::product::command("server live-handoff")
    );
    eprintln!(
        "  {}  reload config.toml in the running server",
        crate::product::command("server reload-config")
    );
    eprintln!(
        "  {}  show agent detection manifest status",
        crate::product::command("server agent-manifests [--json]")
    );
    eprintln!(
        "  {}  fetch and reload agent detection manifests",
        crate::product::command("server update-agent-manifests [--json]")
    );
    eprintln!(
        "  {}  reload agent detection manifests in the running server",
        crate::product::command("server reload-agent-manifests")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_manifest_preflight_refuses_before_local_update() {
        let response = update_agent_manifest_status(
            |request| {
                assert!(matches!(request.method, Method::ServerAgentManifests(_)));
                Ok(serde_json::json!({"id": request.id, "error": {"code": "external_agent_management", "message": "Use Coven."}}))
            },
            || panic!("local updater must not run"),
        ).unwrap().unwrap();
        assert_eq!(response["error"]["code"], "external_agent_management");
    }

    #[test]
    fn update_agent_manifest_status_fetches_reloads_then_reads_status() {
        let mut methods = Vec::new();
        let order = std::cell::RefCell::new(Vec::new());
        let response = update_agent_manifest_status(
            |request| {
                order.borrow_mut().push(match request.method {
                    Method::ServerAgentManifests(_) => "status",
                    Method::ServerReloadAgentManifests(_) => "reload",
                    _ => panic!("unexpected request"),
                });
                methods.push(request.method.clone());
                match &request.method {
                    Method::ServerReloadAgentManifests(_) => Ok(serde_json::json!({
                        "id": request.id,
                        "result": { "type": "agent_manifest_reload", "manifests": [] }
                    })),
                    Method::ServerAgentManifests(_) => Ok(serde_json::json!({
                        "id": request.id,
                        "result": {
                            "type": "agent_manifest_status",
                            "last_result": "checked",
                            "manifests": []
                        }
                    })),
                    _ => panic!("unexpected request"),
                }
            },
            || {
                order.borrow_mut().push("update");
                Ok(())
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(response["result"]["type"], "agent_manifest_status");
        assert_eq!(*order.borrow(), ["status", "update", "reload", "status"]);
        assert_eq!(
            methods,
            vec![
                Method::ServerAgentManifests(EmptyParams::default()),
                Method::ServerReloadAgentManifests(EmptyParams::default()),
                Method::ServerAgentManifests(EmptyParams::default())
            ]
        );
    }

    #[test]
    fn update_agent_manifest_status_probes_but_does_not_reload_when_fetch_fails() {
        let mut probes = 0;
        let response = update_agent_manifest_status(
            |request| {
                probes += 1;
                assert_eq!(probes, 1);
                assert!(matches!(request.method, Method::ServerAgentManifests(_)));
                Ok(serde_json::json!({"id": request.id, "result": {"type": "agent_manifest_status", "manifests": []}}))
            },
            || Err("network unavailable".to_string()),
        )
        .unwrap();

        assert_eq!(response, Err("network unavailable".to_string()));
        assert_eq!(probes, 1);
        assert_eq!(
            agent_manifest_update_error_response("network unavailable")["error"]["code"],
            "agent_manifest_update_failed"
        );
    }

    #[test]
    fn update_agent_manifest_status_stops_after_reload_error() {
        let mut methods = Vec::new();
        let response = update_agent_manifest_status(
            |request| {
                methods.push(request.method.clone());
                if matches!(request.method, Method::ServerAgentManifests(_)) {
                    return Ok(serde_json::json!({"id": request.id, "result": {"type": "agent_manifest_status", "manifests": []}}));
                }
                Ok(serde_json::json!({
                    "id": request.id,
                    "error": {
                        "code": "reload_failed",
                        "message": "reload failed"
                    }
                }))
            },
            || Ok(()),
        )
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], "reload_failed");
        assert_eq!(
            methods,
            vec![
                Method::ServerAgentManifests(EmptyParams::default()),
                Method::ServerReloadAgentManifests(EmptyParams::default()),
            ]
        );
    }

    #[test]
    fn external_manifest_failed_or_unknown_server_never_updates_locally() {
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::InvalidData,
        ] {
            let result = update_agent_manifest_status(
                |request| {
                    assert!(matches!(request.method, Method::ServerAgentManifests(_)));
                    Err(std::io::Error::new(kind, "fake selected-server failure"))
                },
                || panic!("local updater must not run"),
            );
            assert_eq!(result.unwrap_err().kind(), kind);
        }
        for response in [
            serde_json::json!({}),
            serde_json::json!({"result": {"type": "unknown"}}),
        ] {
            assert!(update_agent_manifest_status(
                |_| Ok(response.clone()),
                || panic!("local updater must not run"),
            )
            .is_err());
        }
    }

    #[test]
    fn manifest_preflight_rejects_malformed_or_uncorrelated_success_before_local_update() {
        let valid = serde_json::json!({
            "id": "cli:server:agent-manifests",
            "result": {"type": "agent_manifest_status", "manifests": []}
        });
        let manifest = serde_json::json!({
            "agent": "codex", "source": "bundled", "source_kind": "bundled",
            "local_override_shadowing_remote": false
        });
        let mut malformed = Vec::new();
        let mut missing_id = valid.clone();
        missing_id.as_object_mut().unwrap().remove("id");
        malformed.push(missing_id);
        for id in [
            serde_json::json!("wrong-id"),
            serde_json::json!(null),
            serde_json::json!(123),
        ] {
            let mut response = valid.clone();
            response["id"] = id;
            malformed.push(response);
        }
        let mut missing_manifests = valid.clone();
        missing_manifests["result"]
            .as_object_mut()
            .unwrap()
            .remove("manifests");
        malformed.push(missing_manifests);
        for manifests in [
            serde_json::json!(null),
            serde_json::json!({}),
            serde_json::json!([{}]),
        ] {
            let mut response = valid.clone();
            response["result"]["manifests"] = manifests;
            malformed.push(response);
        }
        for (field, value) in [
            ("type", serde_json::json!("unknown")),
            ("type", serde_json::json!("agent_manifest_reload")),
            ("last_check_unix", serde_json::json!("123")),
            ("last_result", serde_json::json!(false)),
        ] {
            let mut response = valid.clone();
            response["result"][field] = value;
            malformed.push(response);
        }
        for (field, value) in [
            ("agent", serde_json::json!(123)),
            ("source", serde_json::json!(null)),
            ("source_kind", serde_json::json!([])),
            (
                "local_override_shadowing_remote",
                serde_json::json!("false"),
            ),
            ("active_version", serde_json::json!(42)),
            ("remote_last_checked_unix", serde_json::json!(-1)),
        ] {
            let mut response = valid.clone();
            response["result"]["manifests"] = serde_json::json!([manifest.clone()]);
            response["result"]["manifests"][0][field] = value;
            malformed.push(response);
        }
        for response in malformed {
            let error = update_agent_manifest_status(
                |_| Ok(response.clone()),
                || panic!("malformed preflight must not authorize local updater"),
            )
            .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{response}");
        }
    }

    #[test]
    fn manifest_preflight_accepts_schema_optional_fields_missing_or_null() {
        for optional in [
            serde_json::json!({}),
            serde_json::json!({
                "last_check_unix": null, "last_result": null
            }),
        ] {
            let mut updated = false;
            let mut calls = 0;
            update_agent_manifest_status(
                |request| {
                    calls += 1;
                    if matches!(request.method, Method::ServerReloadAgentManifests(_)) {
                        return Ok(serde_json::json!({"id": request.id, "result": {
                            "type": "agent_manifest_reload", "manifests": []
                        }}));
                    }
                    let mut result = optional.clone();
                    result["type"] = serde_json::json!("agent_manifest_status");
                    result["manifests"] = serde_json::json!([{
                        "agent": "codex", "source": "bundled", "source_kind": "bundled",
                        "local_override_shadowing_remote": false, "active_version": null
                    }]);
                    Ok(serde_json::json!({"id": request.id, "result": result}))
                },
                || {
                    updated = true;
                    Ok(())
                },
            )
            .unwrap()
            .unwrap();
            assert!(updated);
            assert_eq!(calls, 3);
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
