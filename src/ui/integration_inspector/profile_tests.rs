use super::tests::{draw, key, setup_v3 as setup};
use super::*;
use crate::api::schema::{IntegrationViewInformation, ViewOwnerParams, ViewReplaceParams};
use crossterm::event::KeyModifiers;
use std::time::Duration;

fn publish(
    app: &mut AppState,
    owner: &ViewOwnerParams,
    revision: u64,
    now: Instant,
    info: IntegrationViewInformation,
) {
    app.integration_views
        .replace(
            &ViewReplaceParams {
                source: owner.source.clone(),
                lease: owner.lease.clone(),
                epoch: owner.epoch.clone(),
                revision,
                items: vec![IntegrationViewItem {
                    id: "same-0".into(),
                    label: "Unique owner name".into(),
                    detail: "private diagnostic needle ".repeat(16),
                    presentation: None,
                    information: Some(info),
                }],
            },
            now,
        )
        .unwrap();
}

#[test]
fn structured_empty_sections_are_honest_name_once_and_diagnostics_collapsed() {
    let (mut app, owners, now) = setup();
    publish(
        &mut app,
        &owners[1],
        2,
        now,
        IntegrationViewInformation::default(),
    );
    draw(&mut app, now);
    let c = app.integration_inspector.displayed.as_ref().unwrap();
    let lines = c.detail_lines.join("");
    assert!(lines.contains("AttentionNo attention information supplied."));
    assert!(lines.contains("Current activityNo activity information supplied."));
    assert!(lines.contains("Working contextNo working context supplied."));
    assert!(lines.contains("▸ Diagnostics [d]"));
    assert!(!lines.contains("Unique owner name"));
    assert!(!lines.contains("private diagnostic"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 24)).unwrap();
    terminal.draw(|f| render(&app, f)).unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert_eq!(rendered.matches("Unique owner name").count(), 1);
    key(&mut app, KeyCode::Char('d')); // Items focus does not toggle.
    assert!(!app.integration_inspector.expanded);
    key(&mut app, KeyCode::Tab);
    draw(&mut app, now);
    key(&mut app, KeyCode::Char('d'));
    assert!(app.integration_inspector.expanded);
    assert!(app.inspector_current_capture(now).is_none());
    draw(&mut app, now);
    assert!(app
        .integration_inspector
        .displayed
        .as_ref()
        .unwrap()
        .detail_lines
        .join("")
        .contains("private diagnostic needle"));
    key(&mut app, KeyCode::Enter);
    assert!(app
        .integration_views
        .poll(&owners[1], now)
        .unwrap()
        .intents
        .is_empty());
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Char('d'));
    assert_eq!(app.integration_inspector.query, "d");
    assert!(app.integration_inspector.expanded);
}

#[test]
fn structured_age_revisions_require_draw_preserve_focus_expansion_and_clamp() {
    let (mut app, owners, now) = setup();
    for revision in 2..=5 {
        let time = now + Duration::from_secs(revision - 2);
        publish(
            &mut app,
            &owners[1],
            revision,
            time,
            IntegrationViewInformation {
                activity: vec![format!("Evidence age: {}s", revision - 2)],
                ..Default::default()
            },
        );
        assert!(app.inspector_current_capture(time).is_none());
        key(&mut app, KeyCode::Char('d')); // Unseen revision cannot toggle.
        assert_eq!(app.integration_inspector.expanded, revision > 2);
        draw(&mut app, time);
        if revision == 2 {
            key(&mut app, KeyCode::Tab);
            draw(&mut app, time);
            key(&mut app, KeyCode::Char('d'));
        } else {
            key(&mut app, KeyCode::Char('d'));
            assert!(!app.integration_inspector.expanded);
            draw(&mut app, time);
            key(&mut app, KeyCode::Char('d'));
        }
        assert_eq!(app.integration_inspector.focus, 2);
        assert!(app.integration_inspector.expanded);
        draw(&mut app, time);
        let saved = app.save_sidebar_presentation();
        app.compute_integration_inspector(Rect::new(0, 0, 1, 1), time);
        app.restore_sidebar_presentation(saved);
        assert!(app.integration_inspector.expanded);
    }
    app.integration_inspector.detail_scroll = usize::MAX;
    draw(&mut app, now + Duration::from_secs(3));
    assert!(app.integration_inspector.detail_scroll > 0);
    key(&mut app, KeyCode::Char('d'));
    draw(&mut app, now + Duration::from_secs(3));
    let c = app.integration_inspector.displayed.as_ref().unwrap();
    assert!(
        c.scrolls[2]
            <= c.detail_lines
                .len()
                .saturating_sub(c.details.height as usize)
    );
    app.open_integration_inspector();
    assert!(!app.integration_inspector.expanded);
}

#[test]
fn structured_search_hint_and_visible_mouse_expander_are_fenced_and_clipped() {
    let (mut app, owners, now) = setup();
    publish(
        &mut app,
        &owners[1],
        2,
        now,
        IntegrationViewInformation::default(),
    );
    app.integration_inspector.query = "needle".into();
    draw(&mut app, now);
    let c = app.integration_inspector.displayed.clone().unwrap();
    assert!(c
        .detail_lines
        .join("")
        .contains("Match in Diagnostics — expand to view."));
    assert!(!c.detail_lines.join("").contains("needle"));
    app.integration_inspector.detail_scroll = c.detail_lines.len();
    draw(&mut app, now);
    let c = app.integration_inspector.displayed.clone().unwrap();
    let hit = c.expander_hit.unwrap();
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.x,
        row: hit.y,
        modifiers: KeyModifiers::NONE,
    };
    app.integration_inspector_mouse(MouseEvent {
        column: c.area.right(),
        ..click
    });
    assert!(!app.integration_inspector.expanded);
    app.integration_inspector.displayed = None;
    app.integration_inspector_mouse(click);
    assert!(!app.integration_inspector.expanded);
    draw(&mut app, now);
    app.integration_inspector_mouse(click);
    assert!(app.integration_inspector.expanded);
    assert!(app
        .integration_views
        .poll(&owners[1], now)
        .unwrap()
        .intents
        .is_empty());
    draw(&mut app, now);
    app.integration_inspector.detail_scroll = 0;
    app.compute_integration_inspector(Rect::new(0, 0, 40, 12), now);
    assert!(app
        .integration_inspector
        .computed
        .as_ref()
        .unwrap()
        .expander_hit
        .is_none());
    for width in 0..=12 {
        app.compute_integration_inspector(Rect::new(0, 0, width, 12), now);
        let c = app.integration_inspector.computed.as_ref().unwrap();
        assert!(c
            .detail_lines
            .iter()
            .all(|s| s.width() <= c.details.width as usize));
        assert!(c
            .expander_hit
            .is_none_or(|r| r.width > 0 && r.intersection(c.details) == r));
    }
}

#[test]
fn structured_identity_removal_and_epoch_replacement_collapse_without_rebinding() {
    let (mut app, owners, now) = setup();
    publish(
        &mut app,
        &owners[1],
        2,
        now,
        IntegrationViewInformation::default(),
    );
    draw(&mut app, now);
    key(&mut app, KeyCode::Tab);
    draw(&mut app, now);
    key(&mut app, KeyCode::Char('d'));
    assert!(app.integration_inspector.expanded);
    app.integration_views
        .replace(
            &ViewReplaceParams {
                source: owners[1].source.clone(),
                lease: owners[1].lease.clone(),
                epoch: owners[1].epoch.clone(),
                revision: 3,
                items: vec![],
            },
            now,
        )
        .unwrap();
    draw(&mut app, now);
    assert!(!app.integration_inspector.expanded);
    assert!(app.integration_inspector.item.is_none());
    publish(
        &mut app,
        &owners[1],
        4,
        now,
        IntegrationViewInformation::default(),
    );
    app.inspector_select_source(now);
    draw(&mut app, now);
    key(&mut app, KeyCode::Char('d'));
    assert!(app.integration_inspector.expanded);
    app.integration_views.close(&owners[1], now).unwrap();
    app.integration_views
        .open(
            &crate::api::schema::ViewOpenParams {
                source: owners[1].source.clone(),
                version: 3,
            },
            now,
        )
        .unwrap();
    draw(&mut app, now);
    assert!(!app.integration_inspector.expanded);
    assert!(app.integration_inspector.item.is_none());
}

#[test]
fn structured_owner_arrays_are_ordered_wrapped_searchable_without_policy() {
    let (mut app, owners, now) = setup();
    publish(
        &mut app,
        &owners[1],
        2,
        now,
        IntegrationViewInformation {
            attention: vec!["Owner attention 界".into(), "Second attention".into()],
            activity: vec!["Owner activity 👩‍💻".into()],
            context: vec!["Owner context".into()],
        },
    );
    draw(&mut app, now);
    let c = app.integration_inspector.displayed.as_ref().unwrap();
    assert_eq!(
        c.detail_lines[..7],
        [
            "Attention",
            "Owner attention 界",
            "Second attention",
            "Current activity",
            "Owner activity 👩‍💻",
            "Working context",
            "Owner context"
        ]
    );
    let item = &c.sources.iter().find(|s| s.id == "coven").unwrap().items[0];
    for query in [
        "owner attention",
        "SECOND ATTENTION",
        "owner activity",
        "owner context",
        "needle",
        "Unique owner name",
    ] {
        assert!(matches_query(item, query));
    }
    for width in 0..=12 {
        app.compute_integration_inspector(Rect::new(0, 0, width, 24), now);
        let c = app.integration_inspector.computed.as_ref().unwrap();
        assert!(c
            .detail_lines
            .iter()
            .all(|line| line.width() <= c.details.width as usize));
    }
}
