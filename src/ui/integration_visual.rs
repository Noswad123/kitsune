//! Host-owned visuals for declared integration categories, never inferred text.
use crate::{
    api::schema::{IntegrationViewItem, IntegrationViewStatus},
    app::state::Palette,
};
use ratatui::style::Color;

pub(crate) fn item_label(item: &IntegrationViewItem) -> String {
    match item.presentation.as_ref() {
        Some(p) if !p.label.is_empty() => format!("{}  {}", p.label, item.label),
        _ => item.label.clone(),
    }
}

pub(crate) fn visual(
    status: Option<IntegrationViewStatus>,
    tick: u32,
    animations: bool,
    palette: &Palette,
) -> (&'static str, Color) {
    use IntegrationViewStatus::*;
    match status {
        Some(Running) => (
            if animations {
                super::spinner_frame(tick)
            } else {
                "◌"
            },
            palette.accent,
        ),
        Some(Pending) => ("◌", palette.subtext0),
        Some(Attention) => ("!", palette.yellow),
        Some(Error) => ("×", palette.red),
        Some(Success) => ("✓", palette.green),
        Some(Unknown) => ("?", palette.overlay0),
        None => ("·", palette.overlay0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_labels_preserve_optional_publisher_captions_without_id_fallback() {
        use crate::api::schema::IntegrationViewPresentation;
        let mut item = IntegrationViewItem {
            information: None,
            id: "never-display-session-hash".into(),
            label: "Djinn".into(),
            detail: String::new(),
            presentation: None,
        };
        assert_eq!(item_label(&item), "Djinn");
        item.presentation = Some(IntegrationViewPresentation {
            label: String::new(),
            status: Some(IntegrationViewStatus::Running),
        });
        assert_eq!(item_label(&item), "Djinn");
        item.presentation.as_mut().unwrap().label = "Building".into();
        assert_eq!(item_label(&item), "Building  Djinn");
        item.label.clear();
        item.presentation.as_mut().unwrap().label.clear();
        assert_eq!(item_label(&item), "");
    }

    #[test]
    fn declared_categories_have_generic_host_visuals_and_neutral_is_not_success() {
        use IntegrationViewStatus::*;
        let p = Palette::catppuccin();
        for (status, glyph, color) in [
            (None, "·", p.overlay0),
            (Some(Pending), "◌", p.subtext0),
            (Some(Attention), "!", p.yellow),
            (Some(Error), "×", p.red),
            (Some(Success), "✓", p.green),
            (Some(Unknown), "?", p.overlay0),
        ] {
            assert_eq!(visual(status, 0, true, &p), (glyph, color));
            assert_eq!(visual(status, 8, false, &p), (glyph, color));
        }
        assert_ne!(
            visual(Some(Running), 0, true, &p),
            visual(Some(Running), 8, true, &p)
        );
        assert_eq!(
            visual(Some(Running), 0, false, &p),
            visual(Some(Running), 8, false, &p)
        );
    }
}
