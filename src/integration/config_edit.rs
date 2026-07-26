use std::io;
use std::path::Path;

use serde_json::{json, Map, Value};

use super::command::{hook_command, legacy_bash_hook_command};
#[cfg(windows)]
use super::file_ops::legacy_bash_hook_path;

pub(crate) fn ensure_hooks_object<'a>(
    settings: &'a mut Value,
    settings_path: &Path,
    root_description: &str,
    hooks_description: &str,
) -> io::Result<&'a mut Map<String, Value>> {
    let root = settings.as_object_mut().ok_or_else(|| {
        io::Error::other(format!(
            "{root_description} at {} must be a JSON object",
            settings_path.display()
        ))
    })?;

    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    hooks.as_object_mut().ok_or_else(|| {
        io::Error::other(format!(
            "{hooks_description} at {} must be a JSON object",
            settings_path.display()
        ))
    })
}

pub(crate) fn hooks_object_if_present<'a>(
    settings: &'a mut Value,
    settings_path: &Path,
    root_description: &str,
    hooks_description: &str,
) -> io::Result<Option<&'a mut Map<String, Value>>> {
    let root = settings.as_object_mut().ok_or_else(|| {
        io::Error::other(format!(
            "{root_description} at {} must be a JSON object",
            settings_path.display()
        ))
    })?;

    let Some(hooks) = root.get_mut("hooks") else {
        return Ok(None);
    };

    hooks.as_object_mut().map(Some).ok_or_else(|| {
        io::Error::other(format!(
            "{hooks_description} at {} must be a JSON object",
            settings_path.display()
        ))
    })
}

pub(crate) fn ensure_command_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: String,
    timeout: u64,
    matcher: Option<&str>,
) -> io::Result<()> {
    let entries = hooks
        .entry(event.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| io::Error::other(format!("hook entries for {event} must be an array")))?;

    let already_installed = entries.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hook_entries| {
                hook_entries.iter().any(|hook| {
                    hook.get("type").and_then(Value::as_str) == Some("command")
                        && hook.get("command").and_then(Value::as_str) == Some(command.as_str())
                })
            })
    });
    if already_installed {
        return Ok(());
    }

    let mut entry = Map::new();
    if let Some(matcher) = matcher {
        entry.insert("matcher".to_string(), Value::String(matcher.to_string()));
    }
    entry.insert(
        "hooks".to_string(),
        json!([
            {
                "type": "command",
                "command": command,
                "timeout": timeout,
            }
        ]),
    );

    entries.push(Value::Object(entry));
    Ok(())
}

pub(crate) fn ensure_direct_command_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: String,
    timeout_sec: u64,
    matcher: Option<&str>,
) -> io::Result<()> {
    let entries = hooks
        .entry(event.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| io::Error::other(format!("hook entries for {event} must be an array")))?;

    let command_field = direct_command_field();
    if let Some(entry) = entries.iter_mut().find(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("command")
            && is_matching_direct_command_entry(entry, command.as_str())
    }) {
        let Some(entry_object) = entry.as_object_mut() else {
            return Ok(());
        };
        entry_object.remove("command");
        entry_object.remove("bash");
        entry_object.remove("powershell");
        entry_object.insert(command_field.to_string(), Value::String(command.clone()));
        entry_object.insert("timeoutSec".to_string(), Value::Number(timeout_sec.into()));
        match matcher {
            Some(matcher) => {
                entry_object.insert("matcher".to_string(), Value::String(matcher.to_string()));
            }
            None => {
                entry_object.remove("matcher");
            }
        }
        return Ok(());
    }

    let mut entry = Map::new();
    entry.insert("type".to_string(), Value::String("command".to_string()));
    if let Some(matcher) = matcher {
        entry.insert("matcher".to_string(), Value::String(matcher.to_string()));
    }
    entry.insert(command_field.to_string(), Value::String(command));
    entry.insert("timeoutSec".to_string(), Value::Number(timeout_sec.into()));
    entries.push(Value::Object(entry));
    Ok(())
}

pub(crate) fn direct_command_field() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "bash"
    }
}

pub(crate) fn is_matching_direct_command_entry(entry: &Value, command: &str) -> bool {
    entry.get("command").and_then(Value::as_str) == Some(command)
        || entry.get("bash").and_then(Value::as_str) == Some(command)
        || entry.get("powershell").and_then(Value::as_str) == Some(command)
}

pub(crate) fn remove_command_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: &str,
) -> io::Result<bool> {
    let Some(entries_value) = hooks.get_mut(event) else {
        return Ok(false);
    };

    let entries = entries_value
        .as_array_mut()
        .ok_or_else(|| io::Error::other(format!("hook entries for {event} must be an array")))?;

    let mut removed = false;
    entries.retain_mut(|entry| {
        let Some(entry_object) = entry.as_object_mut() else {
            return true;
        };
        let Some(hook_entries) = entry_object.get_mut("hooks") else {
            return true;
        };
        let Some(hook_entries) = hook_entries.as_array_mut() else {
            return true;
        };

        let before = hook_entries.len();
        hook_entries.retain(|hook| !is_matching_command_hook(hook, command));
        if hook_entries.len() != before {
            removed = true;
        }

        !hook_entries.is_empty()
    });

    if entries.is_empty() {
        hooks.remove(event);
    }

    Ok(removed)
}

pub(crate) fn remove_direct_command_hook(
    hooks: &mut Map<String, Value>,
    event: &str,
    command: &str,
) -> io::Result<bool> {
    let Some(entries_value) = hooks.get_mut(event) else {
        return Ok(false);
    };

    let entries = entries_value
        .as_array_mut()
        .ok_or_else(|| io::Error::other(format!("hook entries for {event} must be an array")))?;

    let before = entries.len();
    entries.retain(|entry| {
        !(entry.get("type").and_then(Value::as_str) == Some("command")
            && is_matching_direct_command_entry(entry, command))
    });
    let removed = entries.len() != before;
    if entries.is_empty() {
        hooks.remove(event);
    }
    Ok(removed)
}

pub(crate) fn remove_hook_commands(
    hooks: &mut Map<String, Value>,
    event: &str,
    hook_path: &Path,
    action: Option<&str>,
) -> io::Result<bool> {
    let mut removed = false;
    for command in hook_command_variants(hook_path, action) {
        removed |= remove_command_hook(hooks, event, &command)?;
    }
    Ok(removed)
}

pub(crate) fn remove_direct_hook_commands(
    hooks: &mut Map<String, Value>,
    event: &str,
    hook_path: &Path,
    action: Option<&str>,
) -> io::Result<bool> {
    let mut removed = false;
    for command in hook_command_variants(hook_path, action) {
        removed |= remove_direct_command_hook(hooks, event, &command)?;
    }
    Ok(removed)
}

pub(crate) fn hook_command_variants(hook_path: &Path, action: Option<&str>) -> Vec<String> {
    let mut commands = vec![hook_command(hook_path, action)];
    push_unique_command(&mut commands, legacy_bash_hook_command(hook_path, action));

    #[cfg(windows)]
    {
        push_unique_command(
            &mut commands,
            legacy_bash_hook_command(&legacy_bash_hook_path(hook_path), action),
        );
    }

    commands
}

pub(crate) fn push_unique_command(commands: &mut Vec<String>, command: String) {
    if !commands.iter().any(|existing| existing == &command) {
        commands.push(command);
    }
}

pub(crate) fn is_matching_command_hook(hook: &Value, command: &str) -> bool {
    hook.get("type").and_then(Value::as_str) == Some("command")
        && hook.get("command").and_then(Value::as_str) == Some(command)
}

pub(crate) fn build_codex_config_with_hooks(content: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let trailing_newline = content.ends_with('\n');
    let mut in_top_level_features = false;
    let mut features_header_index = None;
    let mut hooks_index = None;
    let mut deprecated_hooks_indexes = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if let Some(header) = toml_table_header(line) {
            in_top_level_features = header == "[features]";
            if in_top_level_features && features_header_index.is_none() {
                features_header_index = Some(index);
            }
            continue;
        }

        if !in_top_level_features {
            continue;
        }

        if is_toml_key(line, "codex_hooks") {
            deprecated_hooks_indexes.push(index);
        } else if is_toml_key(line, "hooks") {
            hooks_index = Some(index);
        }
    }

    if let Some(index) = hooks_index {
        lines[index] = "hooks = true".to_string();
    }

    for index in deprecated_hooks_indexes.into_iter().rev() {
        lines.remove(index);
    }

    if hooks_index.is_none() {
        if let Some(index) = features_header_index {
            lines.insert(index + 1, "hooks = true".to_string());
            return join_toml_lines(lines, trailing_newline);
        }

        let mut result = content.trim_end_matches('\n').to_string();
        if !result.is_empty() {
            result.push('\n');
            result.push('\n');
        }
        result.push_str("[features]\nhooks = true\n");
        return result;
    }

    join_toml_lines(lines, trailing_newline)
}

pub(crate) fn join_toml_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut result = lines.join("\n");
    if trailing_newline || result.is_empty() {
        result.push('\n');
    }
    result
}

pub(crate) fn toml_table_header(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || !trimmed.starts_with('[') {
        return None;
    }

    let header_end = if trimmed.starts_with("[[") {
        trimmed.find("]]").map(|index| index + 2)?
    } else {
        trimmed.find(']').map(|index| index + 1)?
    };
    let header = &trimmed[..header_end];
    let rest = trimmed[header_end..].trim_start();
    if !rest.is_empty() && !rest.starts_with('#') {
        return None;
    }

    Some(header)
}

pub(crate) fn is_toml_key(line: &str, key: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || !trimmed.starts_with(key) {
        return false;
    }

    trimmed[key.len()..].trim_start().starts_with('=')
}
