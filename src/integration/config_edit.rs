use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use super::command::{hook_command, legacy_bash_hook_command};
#[cfg(windows)]
use super::file_ops::legacy_bash_hook_path;

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
