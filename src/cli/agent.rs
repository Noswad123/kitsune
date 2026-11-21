use crate::api::schema::legacy_agent_retirement_error;

pub(super) fn run_agent_command(args: &[String]) -> std::io::Result<i32> {
    let mut parse_args = args.to_vec();
    if args.first().is_some_and(|command| command == "prompt") && args.len() > 2 {
        // The legacy parser treated the second positional argument as literal text,
        // even when it was a recognized flag such as --wait or --help.
        parse_args[2] = "TEXT".into();
    }
    let command = super::spec::command()
        .find_subcommand("agent")
        .expect("agent command is registered")
        .clone()
        .bin_name(crate::product::command("agent"))
        .disable_help_subcommand(false)
        .subcommand_required(true);
    let matches = match command.try_get_matches_from(
        std::iter::once(crate::product::cli_name().to_string()).chain(parse_args),
    ) {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code();
            error.print()?;
            return Ok(code);
        }
    };
    let Some((subcommand, parsed)) = matches.subcommand() else {
        return Ok(2);
    };
    if subcommand == "explain" {
        return agent_explain(&args[1..]);
    }
    // Validate syntax only. Retired commands must never resolve targets or probe a peer.
    if matches!(subcommand, "start" | "wait" | "prompt") {
        if let Some(timeout) = parsed.get_one::<String>("timeout") {
            if let Err(error) = super::parse_u64_flag("--timeout", timeout) {
                eprintln!("{error}");
                return Ok(2);
            }
        }
    }
    if subcommand == "read" {
        if let Some(lines) = parsed.get_one::<String>("lines") {
            if let Err(error) = super::parse_u32_flag("--lines", lines) {
                eprintln!("{error}");
                return Ok(2);
            }
        }
    }
    let method = match subcommand {
        "list" => "agent.list",
        "get" => "agent.get",
        "read" => "agent.read",
        "send-keys" => "agent.send_keys",
        "prompt" => "agent.prompt",
        "rename" => "agent.rename",
        "focus" => "agent.focus",
        "wait" => "agent.wait",
        "start" => "agent.start",
        // Attach historically resolved its target through agent.get.
        "attach" => "agent.get",
        _ => return Ok(2),
    };
    print_retired(method)
}

fn print_retired(method: &'static str) -> std::io::Result<i32> {
    super::print_response(&serde_json::json!({
        "id": "cli:agent:retired",
        "error": legacy_agent_retirement_error(method),
    }))
}

fn agent_explain(args: &[String]) -> std::io::Result<i32> {
    let mut file = None;
    let mut agent = None;
    let mut json = false;
    let mut verbose = false;
    let mut target = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--file" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --file");
                    return Ok(2);
                };
                file = Some(value.clone());
                index += 2;
            }
            "--agent" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --agent");
                    return Ok(2);
                };
                agent = Some(value.clone());
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--format" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --format");
                    return Ok(2);
                };
                match value.as_str() {
                    "json" => json = true,
                    "text" => json = false,
                    other => {
                        eprintln!("invalid --format: {other} (expected text or json)");
                        return Ok(2);
                    }
                }
                index += 2;
            }
            "--verbose" | "-v" => {
                verbose = true;
                index += 1;
            }
            "help" | "--help" | "-h" => {
                eprintln!(
                    "{}",
                    crate::product::usage("agent explain <target> [--json|--verbose]")
                );
                eprintln!(
                    "{}",
                    crate::product::usage(
                        "agent explain --file PATH --agent LABEL [--json|--verbose]"
                    )
                );
                return Ok(0);
            }
            value if value.starts_with('-') => {
                eprintln!("unknown option: {value}");
                return Ok(2);
            }
            value => {
                if target.is_some() {
                    eprintln!(
                        "{}",
                        crate::product::usage("agent explain <target> [--json]")
                    );
                    return Ok(2);
                }
                target = Some(value.to_string());
                index += 1;
            }
        }
    }

    let explain = if let Some(path) = file {
        if target.is_some() {
            eprintln!(
                "{}",
                crate::product::usage("agent explain --file PATH --agent LABEL [--json]")
            );
            return Ok(2);
        }
        let Some(agent_label) = agent else {
            eprintln!(
                "{} requires --agent LABEL",
                crate::product::command("agent explain --file")
            );
            return Ok(2);
        };
        let content = std::fs::read_to_string(path)?;
        crate::detect::manifest::explain_to_json_value(&crate::detect::manifest::explain_for_label(
            &agent_label,
            &content,
        ))
    } else {
        if target.is_none() {
            eprintln!(
                "{}",
                crate::product::usage("agent explain <target> [--json]")
            );
            eprintln!(
                "{}",
                crate::product::usage("agent explain --file PATH --agent LABEL [--json]")
            );
            return Ok(2);
        }
        if agent.is_some() {
            eprintln!("--agent is only valid with --file");
            return Ok(2);
        }
        return print_retired("agent.explain");
    };

    if json {
        println!("{explain}");
    } else {
        print_agent_explain_text(&explain, verbose);
    }
    Ok(0)
}

fn print_agent_explain_text(explain: &serde_json::Value, verbose: bool) {
    println!("agent: {}", explain["agent"].as_str().unwrap_or("unknown"));
    println!("state: {}", explain["state"].as_str().unwrap_or("unknown"));
    println!(
        "manifest: {} {}",
        explain["manifest_source"].as_str().unwrap_or("none"),
        explain["manifest_version"].as_str().unwrap_or("unknown")
    );
    if let Some(rule) = explain["matched_rule"].as_object() {
        let rule_id = rule
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("-");
        println!(
            "rule: {} (region={} priority={})",
            rule_id,
            rule.get("region")
                .and_then(|value| value.as_str())
                .unwrap_or("-"),
            rule.get("priority")
                .and_then(|value| value.as_i64())
                .unwrap_or(0),
        );
        if let Some(preview) = matched_rule_region_preview(explain, rule_id) {
            println!("evidence: {preview:?}");
        }
    } else {
        println!("rule: none");
    }
    for field in [
        "fallback_reason",
        "screen_detection_skip_reason",
        "skipped_update_reason",
        "warning",
    ] {
        if let Some(value) = explain[field].as_str() {
            println!("{field}: {value}");
        }
    }
    if !verbose {
        return;
    }
    println!(
        "visible: idle={} blocker={} working={}",
        explain["visible_idle"].as_bool().unwrap_or(false),
        explain["visible_blocker"].as_bool().unwrap_or(false),
        explain["visible_working"].as_bool().unwrap_or(false)
    );
    println!(
        "cached_remote_version: {}",
        explain["cached_remote_version"].as_str().unwrap_or("none")
    );
    println!(
        "local_override_shadowing_remote: {}",
        explain["local_override_shadowing_remote"]
            .as_bool()
            .unwrap_or(false)
    );
    for field in ["remote_update_status", "remote_update_error"] {
        if let Some(value) = explain[field].as_str() {
            println!("{field}: {value}");
        }
    }
    if let Some(evaluated_rules) = explain["evaluated_rules"]
        .as_array()
        .filter(|rules| !rules.is_empty())
    {
        println!("evaluated_rules:");
        for rule in evaluated_rules {
            println!(
                "  {} {} priority={} region={} state={}",
                if rule["matched"].as_bool().unwrap_or(false) {
                    "✓"
                } else {
                    "✗"
                },
                rule["id"].as_str().unwrap_or("-"),
                rule["priority"].as_i64().unwrap_or(0),
                rule["region"].as_str().unwrap_or("-"),
                rule["state"].as_str().unwrap_or("unknown")
            );
            let evidence = &rule["evidence"];
            println!(
                "    matchers: contains={:?} regex={:?} line_regex={:?} all={} any={} not={}",
                evidence["contains"],
                evidence["regex"],
                evidence["line_regex"],
                evidence["all_count"].as_u64().unwrap_or(0),
                evidence["any_count"].as_u64().unwrap_or(0),
                evidence["not_count"].as_u64().unwrap_or(0)
            );
            println!(
                "    region: bytes={} preview={:?}",
                evidence["region_bytes"].as_u64().unwrap_or(0),
                evidence["region_preview"].as_str().unwrap_or("")
            );
        }
    }
}

fn matched_rule_region_preview<'a>(
    explain: &'a serde_json::Value,
    rule_id: &str,
) -> Option<&'a str> {
    explain["evaluated_rules"]
        .as_array()?
        .iter()
        .find(|rule| rule["id"].as_str() == Some(rule_id))?["evidence"]["region_preview"]
        .as_str()
        .filter(|preview| !preview.is_empty())
}
