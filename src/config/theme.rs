use std::collections::BTreeMap;

use serde::Deserialize;
use tracing::warn;

/// Theme configuration: pick a built-in or override individual tokens.
///
/// ```toml
/// [theme]
/// name = "tokyo-night"  # built-in: catppuccin, terminal, dracula, nord, etc.
///
/// [theme.custom]        # override individual tokens on top of the base
/// accent = "#f5c2e7"
/// red = "#ff6188"
/// ```
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Built-in theme name. Default: "catppuccin".
    pub name: Option<String>,
    /// Follow host terminal light/dark appearance and switch between theme names.
    pub auto_switch: bool,
    /// Theme name used when `auto_switch` selects a dark appearance.
    pub dark_name: Option<String>,
    /// Theme name used when `auto_switch` selects a light appearance.
    pub light_name: Option<String>,
    /// Custom overrides — applied on top of the selected base theme.
    pub custom: Option<CustomThemeColors>,
    /// Pane-border overrides selected by the local OS hostname.
    pub hosts: BTreeMap<String, PaneBorderColors>,
}

/// Host-specific presentation overrides; other theme accents stay unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PaneBorderColors {
    pub pane_border_active: Option<String>,
    pub pane_border_inactive: Option<String>,
}

impl ThemeConfig {
    /// Select one host entry, full name before short name, ignoring ASCII case.
    /// A full-name entry is not merged with a short-name entry.
    pub fn borders_for_hostname(&self, hostname: Option<&str>) -> Option<&PaneBorderColors> {
        let hostname = hostname?.trim_end_matches('.');
        if hostname.is_empty() {
            return None;
        }
        let short = hostname.split('.').next()?;
        for candidate in [hostname, short] {
            let mut matches = self
                .hosts
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(candidate));
            if let Some((_, borders)) = matches.next() {
                if matches.next().is_some() {
                    warn!(
                        hostname = candidate,
                        "ambiguous theme.hosts entries; ignoring host border overrides"
                    );
                    return None;
                }
                return Some(borders);
            }
        }
        None
    }
}

/// Per-token color overrides. All fields optional — only set what you want to change.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CustomThemeColors {
    pub accent: Option<String>,
    pub pane_border_active: Option<String>,
    pub pane_border_inactive: Option<String>,
    pub panel_bg: Option<String>,
    pub surface0: Option<String>,
    pub surface1: Option<String>,
    pub surface_dim: Option<String>,
    pub overlay0: Option<String>,
    pub overlay1: Option<String>,
    pub text: Option<String>,
    pub subtext0: Option<String>,
    pub mauve: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub red: Option<String>,
    pub blue: Option<String>,
    pub teal: Option<String>,
    pub peach: Option<String>,
}

/// Parse a color string into a ratatui Color.
/// Supports: hex (#rrggbb, #rgb), named colors, rgb(r,g,b), and reset aliases.
pub fn parse_color(s: &str) -> ratatui::style::Color {
    use ratatui::style::Color;
    let s = s.trim().to_lowercase();

    match s.as_str() {
        "reset" | "default" | "none" | "transparent" => return Color::Reset,
        _ => {}
    }

    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        } else if hex.len() == 3 {
            let chars: Vec<u8> = hex
                .chars()
                .filter_map(|c| u8::from_str_radix(&c.to_string(), 16).ok())
                .collect();
            if chars.len() == 3 {
                return Color::Rgb(chars[0] * 17, chars[1] * 17, chars[2] * 17);
            }
        }
    }

    if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].trim().parse::<u8>(),
                parts[1].trim().parse::<u8>(),
                parts[2].trim().parse::<u8>(),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }

    match s.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" | "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        _ => {
            warn!(color = s, "unknown color, defaulting to cyan");
            Color::Cyan
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn theme_name_parses() {
        let toml = r#"
[theme]
name = "dracula"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.theme.name.as_deref(), Some("dracula"));
    }

    #[test]
    fn parse_color_accepts_reset_aliases() {
        use ratatui::style::Color;

        for value in ["reset", "default", "none", "transparent"] {
            assert_eq!(parse_color(value), Color::Reset, "value: {value}");
        }
    }

    #[test]
    fn theme_auto_switch_fields_parse() {
        let toml = r#"
[theme]
name = "catppuccin"
auto_switch = true
dark_name = "tokyo-night"
light_name = "catppuccin-latte"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.theme.name.as_deref(), Some("catppuccin"));
        assert!(config.theme.auto_switch);
        assert_eq!(config.theme.dark_name.as_deref(), Some("tokyo-night"));
        assert_eq!(config.theme.light_name.as_deref(), Some("catppuccin-latte"));
    }

    #[test]
    fn theme_custom_overrides_parse() {
        let toml = r##"
[theme]
name = "nord"

[theme.custom]
panel_bg = "#1e1e2e"
accent = "#ff79c6"
red = "rgb(255, 85, 85)"
"##;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.theme.name.as_deref(), Some("nord"));
        let custom = config.theme.custom.as_ref().unwrap();
        assert_eq!(custom.panel_bg.as_deref(), Some("#1e1e2e"));
        assert_eq!(custom.accent.as_deref(), Some("#ff79c6"));
        assert_eq!(custom.red.as_deref(), Some("rgb(255, 85, 85)"));
        assert!(custom.green.is_none());
    }

    #[test]
    fn theme_defaults_when_missing() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.theme.name.is_none());
        assert!(!config.theme.auto_switch);
        assert!(config.theme.dark_name.is_none());
        assert!(config.theme.light_name.is_none());
        assert!(config.theme.custom.is_none());
        assert!(config.theme.hosts.is_empty());
    }

    #[test]
    fn host_border_overrides_parse_and_match_without_changing_accents() {
        let config: Config = toml::from_str(
            r##"
[theme.custom]
accent = "#B4BEFE"
pane_border_inactive = "#555555"

[theme.hosts.bumblebee]
pane_border_active = "#FFD700"
pane_border_inactive = "#807000"

[theme.hosts.megatron]
pane_border_active = "#FF4040"
pane_border_inactive = "#802020"

[theme.hosts."bumblebee.example.net"]
pane_border_active = "#FFFFFF"
"##,
        )
        .unwrap();
        let theme = &config.theme;
        for name in ["bumblebee", "BUMBLEBEE", "Bumblebee.other.net."] {
            let borders = theme.borders_for_hostname(Some(name)).unwrap();
            assert_eq!(borders.pane_border_active.as_deref(), Some("#FFD700"));
            assert_eq!(borders.pane_border_inactive.as_deref(), Some("#807000"));
        }
        let full = theme
            .borders_for_hostname(Some("BUMBLEBEE.EXAMPLE.NET"))
            .unwrap();
        assert_eq!(full.pane_border_active.as_deref(), Some("#FFFFFF"));
        assert!(full.pane_border_inactive.is_none());
        let megatron = theme.borders_for_hostname(Some("megatron")).unwrap();
        assert_eq!(megatron.pane_border_active.as_deref(), Some("#FF4040"));
        assert_eq!(megatron.pane_border_inactive.as_deref(), Some("#802020"));
        for name in [None, Some("unknown"), Some(""), Some(".")] {
            assert!(theme.borders_for_hostname(name).is_none());
        }
        let custom = theme.custom.as_ref().unwrap();
        assert_eq!(custom.accent.as_deref(), Some("#B4BEFE"));
        assert_eq!(custom.pane_border_inactive.as_deref(), Some("#555555"));
    }

    #[test]
    fn saving_theme_selection_preserves_nested_host_border_tables() {
        let content = r##"
[theme.hosts.bumblebee]
pane_border_active = "#FFD700"
[theme.hosts."megatron.example.net"]
pane_border_active = "#FF4040"
"##;
        let content = crate::config::upsert_section_value(content, "theme", "name", "\"nord\"");
        let content = crate::config::upsert_section_bool(&content, "theme", "auto_switch", false);
        let config: Config = toml::from_str(&content).unwrap();
        assert_eq!(config.theme.name.as_deref(), Some("nord"));
        assert!(!config.theme.auto_switch);
        assert_eq!(config.theme.hosts.len(), 2);
        assert_eq!(
            config
                .theme
                .borders_for_hostname(Some("bumblebee"))
                .unwrap()
                .pane_border_active
                .as_deref(),
            Some("#FFD700")
        );
        assert_eq!(
            config
                .theme
                .borders_for_hostname(Some("megatron.example.net"))
                .unwrap()
                .pane_border_active
                .as_deref(),
            Some("#FF4040")
        );
    }

    #[test]
    fn case_colliding_host_entries_are_not_selected_arbitrarily() {
        let config: Config = toml::from_str(
            r#"
[theme.hosts.Bumblebee]
pane_border_active = "yellow"
[theme.hosts.bumblebee]
pane_border_active = "red"
"#,
        )
        .unwrap();
        assert!(config
            .theme
            .borders_for_hostname(Some("BUMBLEBEE"))
            .is_none());
    }
}
