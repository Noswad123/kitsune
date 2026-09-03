use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::status::state_label;
use super::text::truncate_end;
use super::widgets::{panel_contrast_fg, render_panel_shell};
use crate::app::AppState;

fn prefix_rhs_label(bindings: &crate::config::ActionKeybinds) -> String {
    bindings
        .prefix_rhs_label()
        .unwrap_or_else(|| "unset".to_string())
}

fn render_bottom_bar(frame: &mut Frame, area: Rect, line: Line<'_>, bg: ratatui::style::Color) {
    frame.render_widget(Clear, area);
    let buf = frame.buffer_mut();
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_style(Style::default().bg(bg));
    }
    frame.render_widget(Paragraph::new(line), area);
}

fn render_bottom_lines(
    frame: &mut Frame,
    area: Rect,
    lines: Vec<Line<'_>>,
    bg: ratatui::style::Color,
) {
    if area.width == 0 || area.height == 0 || lines.is_empty() {
        return;
    }
    let height = (lines.len() as u16).min(area.height);
    let y = area.y + area.height.saturating_sub(height);
    for (offset, line) in lines
        .into_iter()
        .rev()
        .take(height as usize)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
    {
        render_bottom_bar(
            frame,
            Rect::new(area.x, y + offset as u16, area.width, 1),
            line,
            bg,
        );
    }
}

pub(super) fn render_prefix_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let key = Style::default()
        .fg(app.palette.accent)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(app.palette.overlay0);
    let mode_style = Style::default()
        .fg(panel_contrast_fg(&app.palette))
        .bg(app.palette.accent)
        .add_modifier(Modifier::BOLD);

    let workspace_picker = prefix_rhs_label(&app.keybinds.workspace_picker);
    let help = prefix_rhs_label(&app.keybinds.help);
    let prefix = crate::config::format_key_combo((app.prefix_code, app.prefix_mods));

    let line = Line::from(vec![
        Span::styled(" PREFIX ", mode_style),
        Span::raw(" "),
        Span::styled("esc", key),
        Span::styled(" cancel  ", dim),
        Span::styled(prefix, key),
        Span::styled(" send prefix  ", dim),
        Span::styled(workspace_picker, key),
        Span::styled(" pane maint  ", dim),
        Span::styled(help, key),
        Span::styled(" keybinds", dim),
    ]);

    let overlay_y = area.y + area.height.saturating_sub(1);
    let overlay_area = Rect::new(area.x, overlay_y, area.width, 1);
    render_bottom_bar(frame, overlay_area, line, app.palette.panel_bg);
}

pub(super) fn render_copy_mode_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let key = Style::default()
        .fg(app.palette.accent)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(app.palette.overlay0);
    let mode_style = Style::default()
        .fg(panel_contrast_fg(&app.palette))
        .bg(app.palette.accent)
        .add_modifier(Modifier::BOLD);

    let Some(copy_mode) = app.copy_mode.as_ref() else {
        return;
    };
    let line = if let Some(prompt) = copy_mode.search.prompt.as_ref() {
        let marker = match prompt.direction {
            crate::app::state::CopyModeSearchDirection::Forward => "/",
            crate::app::state::CopyModeSearchDirection::Backward => "?",
        };
        Line::from(vec![
            Span::styled(" COPY ", mode_style),
            Span::raw(" "),
            Span::styled(marker, key),
            Span::styled(prompt.query.clone(), Style::default().fg(app.palette.text)),
            Span::styled("█", key),
            Span::styled("  enter search  esc cancel", dim),
        ])
    } else {
        let select = if copy_mode.selection.is_some() {
            "selecting"
        } else {
            "select"
        };
        let match_status = copy_mode
            .search
            .current
            .map(|current| format!(" {}/{}", current + 1, copy_mode.search.matches.len()))
            .or_else(|| (!copy_mode.search.query.is_empty()).then(|| " 0/0".to_string()))
            .unwrap_or_default();
        let (exit_keys, exit_label) =
            if copy_mode.search.query.is_empty() && copy_mode.selection.is_none() {
                ("q/esc", " exit")
            } else {
                ("esc", " clear  q exit")
            };
        Line::from(vec![
            Span::styled(" COPY ", mode_style),
            Span::raw(" "),
            Span::styled("h/j/k/l w/b/e { }", key),
            Span::styled(" move  ", dim),
            Span::styled("/ ?", key),
            Span::styled(" search  ", dim),
            Span::styled("n/N", key),
            Span::styled(format!(" repeat{match_status}  "), dim),
            Span::styled("v/space", key),
            Span::styled(format!(" {select}  "), dim),
            Span::styled("y/enter", key),
            Span::styled(" copy  ", dim),
            Span::styled(exit_keys, key),
            Span::styled(exit_label, dim),
        ])
    };

    let overlay_y = area.y + area.height.saturating_sub(1);
    let overlay_area = Rect::new(area.x, overlay_y, area.width, 1);
    render_bottom_bar(frame, overlay_area, line, app.palette.panel_bg);
}

pub(super) fn render_navigate_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let key = Style::default()
        .fg(app.palette.accent)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(app.palette.overlay0);

    let mode_style = Style::default()
        .fg(panel_contrast_fg(&app.palette))
        .bg(app.palette.accent)
        .add_modifier(Modifier::BOLD);

    let info = navigate_hpm_info(app, area.width as usize);
    let status = app.navigate_status.as_deref().unwrap_or("ready");
    let summary = Line::from(vec![
        Span::styled(" NAVIGATE ", mode_style),
        Span::raw(" "),
        Span::styled(info, Style::default().fg(app.palette.text)),
        Span::styled("  status ", dim),
        Span::styled(truncate_end(status, 32), key),
    ]);
    let compact_keys = Line::from(vec![
        Span::styled(" h/j/k/l", key),
        Span::styled(" select  ", dim),
        Span::styled("H/J/K/L", key),
        Span::styled(" move  ", dim),
        Span::styled("y/o", key),
        Span::styled(" width  ", dim),
        Span::styled("u/i", key),
        Span::styled(" height  ", dim),
        Span::styled("v/-", key),
        Span::styled(" split  ", dim),
        Span::styled("tab/shift+tab", key),
        Span::styled(" move tab  ", dim),
        Span::styled("d", key),
        Span::styled(" close  ", dim),
        Span::styled("q/esc", key),
        Span::styled(" exit  ", dim),
        Span::styled("?", key),
        Span::styled(" help", dim),
    ]);

    let lines = if app.navigate_help_visible {
        vec![
            summary,
            compact_keys,
            Line::from(vec![
                Span::styled(" enter/1-9", key),
                Span::styled(" workspace  ", dim),
                Span::styled("arrows", key),
                Span::styled(" pane/workspace nav  ", dim),
                Span::styled("f", key),
                Span::styled(" navigator  ", dim),
                Span::styled("a", key),
                Span::styled(" agents  ", dim),
                Span::styled("r", key),
                Span::styled(" resize mode  ", dim),
                Span::styled("s", key),
                Span::styled(" settings", dim),
            ]),
        ]
    } else {
        vec![summary, compact_keys]
    };

    render_bottom_lines(frame, area, lines, app.palette.panel_bg);
}

fn navigate_hpm_info(app: &AppState, max_width: usize) -> String {
    let Some(ws_idx) = app.active else {
        return "selected: none".into();
    };
    let Some(ws) = app.workspaces.get(ws_idx) else {
        return "selected: none".into();
    };
    let Some(pane_id) = ws.focused_pane_id() else {
        return "selected: none".into();
    };
    let Some(tab_idx) = ws.find_tab_index_for_pane(pane_id) else {
        return "selected: none".into();
    };
    let pane_label = ws
        .public_pane_number(pane_id)
        .map(|number| format!("p{number}"))
        .unwrap_or_else(|| format!("pane {}", pane_id.raw()));
    let tab_label = ws
        .tab_display_name(tab_idx)
        .unwrap_or_else(|| (tab_idx + 1).to_string());
    let workspace = ws.display_name_from_terminals(&app.terminals);
    let selected_workspace = crate::ui::chrome_workspace_index(app)
        .filter(|idx| *idx != ws_idx)
        .and_then(|idx| app.workspaces.get(idx))
        .map(|ws| ws.display_name_from_terminals(&app.terminals));
    let terminal = ws.tabs[tab_idx]
        .terminal_id(pane_id)
        .and_then(|terminal_id| app.terminals.get(terminal_id));
    let agent = terminal
        .and_then(|terminal| terminal.effective_display_agent())
        .or_else(|| {
            terminal.and_then(|terminal| terminal.effective_agent_label().map(str::to_string))
        })
        .unwrap_or_else(|| "shell".into());
    let seen = ws
        .tabs
        .get(tab_idx)
        .and_then(|tab| tab.panes.get(&pane_id))
        .is_none_or(|pane| pane.seen);
    let state = terminal
        .map(|terminal| state_label(terminal.state, seen))
        .unwrap_or("unknown");
    let cwd = terminal
        .map(|terminal| terminal.cwd.display().to_string())
        .unwrap_or_default();
    let active_context = if cwd.is_empty() {
        format!("selected: {pane_label}  tab: {tab_label}  workspace: {workspace}  agent: {agent}/{state}")
    } else {
        format!("selected: {pane_label}  tab: {tab_label}  workspace: {workspace}  agent: {agent}/{state}  cwd: {cwd}")
    };
    let raw = if let Some(selected_workspace) = selected_workspace {
        format!("selected workspace: {selected_workspace}  {active_context}")
    } else {
        active_context
    };
    truncate_end(&raw, max_width.saturating_sub(32))
}

pub(super) fn render_global_launcher_menu(app: &AppState, frame: &mut Frame) {
    let rect = app.global_menu_rect();
    let Some(inner) = render_panel_shell(frame, rect, app.palette.accent, app.palette.panel_bg)
    else {
        return;
    };

    let items = app.global_menu_labels();
    for (idx, item) in items.iter().enumerate() {
        let y = inner.y + idx as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let selected = idx == app.global_menu.highlighted;
        let rect = Rect::new(inner.x, y, inner.width, 1);

        let selected_style = Style::default()
            .fg(panel_contrast_fg(&app.palette))
            .bg(app.palette.accent)
            .add_modifier(Modifier::BOLD);
        let item_style = if selected {
            selected_style
        } else {
            Style::default().fg(app.palette.text)
        };
        let badge_style = if selected {
            selected_style
        } else {
            Style::default()
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD)
        };

        let line = if app.global_menu_item_has_badge(item) {
            Line::from(vec![
                Span::styled(" ●", badge_style),
                Span::styled(format!(" {item} "), item_style),
            ])
        } else {
            Line::from(Span::styled(format!(" {item} "), item_style))
        };
        frame.render_widget(Paragraph::new(line).alignment(Alignment::Left), rect);
    }
}

pub(super) fn render_resize_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let key = Style::default()
        .fg(app.palette.accent)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(app.palette.overlay0);

    let mode_style = Style::default()
        .fg(panel_contrast_fg(&app.palette))
        .bg(app.palette.mauve)
        .add_modifier(Modifier::BOLD);

    let line = Line::from(vec![
        Span::styled(" RESIZE ", mode_style),
        Span::raw("  "),
        Span::styled("h/l", key),
        Span::styled(" width  ", dim),
        Span::styled("j/k", key),
        Span::styled(" height  ", dim),
        Span::styled("esc", key),
        Span::styled(" done", dim),
    ]);

    let overlay_y = area.y + area.height.saturating_sub(1);
    let overlay_area = Rect::new(area.x, overlay_y, area.width, 1);
    render_bottom_bar(frame, overlay_area, line, app.palette.panel_bg);
}

pub(super) fn render_context_menu(app: &AppState, frame: &mut Frame) {
    let Some(menu) = &app.context_menu else {
        return;
    };

    let p = &app.palette;
    let Some(menu_rect) = app.context_menu_rect() else {
        return;
    };
    let Some(inner) = render_panel_shell(frame, menu_rect, p.accent, p.panel_bg) else {
        return;
    };

    let items: Vec<ListItem> = menu
        .items()
        .iter()
        .map(|item| ListItem::new(Line::from(*item)))
        .collect();
    let list = List::new(items)
        .style(Style::default().fg(p.text))
        .highlight_style(
            Style::default()
                .bg(p.accent)
                .fg(panel_contrast_fg(p))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ");
    let mut state = ListState::default().with_selected(Some(menu.list.highlighted));
    frame.render_stateful_widget(list, inner, &mut state);
}
