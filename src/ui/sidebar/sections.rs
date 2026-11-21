//! Client-only section identities and pure, shared sidebar geometry.
use std::collections::HashSet;

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SectionId {
    Spaces,
    Integration(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SectionLayout {
    pub menu: Rect,
    pub toggle: Rect,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Section {
    pub id: SectionId,
    pub header: Rect,
    pub body: Rect,
}

impl SectionLayout {
    pub fn body(&self, id: &SectionId) -> Rect {
        self.sections
            .iter()
            .find(|s| &s.id == id)
            .map_or(Rect::default(), |s| s.body)
    }
}

pub(crate) fn identities(sources: &[String]) -> Vec<SectionId> {
    std::iter::once(SectionId::Spaces)
        .chain(sources.iter().cloned().map(SectionId::Integration))
        .collect()
}

/// Tokens in native-Spaces + configured-source order. IDs are validated ASCII
/// and at most eight peers are configured. Reserve names before config order so
/// a literal `Coven`/`spaces` or earlier `co*` peer cannot impersonate either.
/// The caller adds the collapse caret only when at least two cells are available.
pub(crate) fn compact_labels(sources: &[String], width: u16) -> Vec<String> {
    let mut used = HashSet::from(["sp".to_owned(), "co".to_owned()]);
    let native = match width {
        0 => "",
        1 | 2 => "S",
        _ => "Sp",
    };
    std::iter::once(native.to_owned())
        .chain(sources.iter().enumerate().map(|(index, id)| {
            if width == 0 {
                return String::new();
            }
            if id == "coven" {
                return if width < 3 { "C" } else { "Co" }.to_owned();
            }
            if width < 3 {
                return (index + 1).to_string();
            }
            let preferred = id.chars().take(2).collect::<String>();
            if used.insert(preferred.to_ascii_lowercase()) {
                preferred
            } else {
                format!("#{}", index + 1)
            }
        }))
        .collect()
}

/// Overflow is a contiguous window starting at the client-owned offset. Each
/// visible expanded section costs a header and at least one body cell, except
/// at height one where the focused header alone remains reachable by wheel.
pub(crate) fn compute(
    area: Rect,
    ids: &[SectionId],
    collapsed: &HashSet<SectionId>,
    offset: usize,
) -> SectionLayout {
    let mut result = SectionLayout::default();
    let width = area.width.saturating_sub(1);
    if width == 0 || area.height == 0 {
        return result;
    }
    result.menu = Rect::new(area.x, area.y, width, 1);
    if area.height > 1 {
        result.toggle = Rect::new(area.x, area.bottom() - 1, width, 1);
    }
    let height = area.height.saturating_sub(2) as usize;
    if height == 0 || ids.is_empty() {
        return result;
    }
    let cost = |id: &SectionId| 1 + usize::from(!collapsed.contains(id));
    let total: usize = ids.iter().map(cost).sum();
    let start = if total <= height {
        0
    } else {
        offset.min(ids.len() - 1)
    };
    let mut end = start;
    let mut used = 0;
    while end < ids.len() && used + cost(&ids[end]) <= height {
        used += cost(&ids[end]);
        end += 1;
    }
    if end == start {
        end += 1;
    }
    let visible = &ids[start..end];
    let expanded = visible.iter().filter(|id| !collapsed.contains(id)).count();
    let budget = height.saturating_sub(visible.len());
    let quotient = budget.checked_div(expanded).unwrap_or(0);
    let remainder = budget.checked_rem(expanded).unwrap_or(0);
    let mut y = area.y + 1;
    let mut index = 0;
    for id in visible {
        let body_height = if collapsed.contains(id) {
            0
        } else {
            let h = quotient + usize::from(index < remainder);
            index += 1;
            h as u16
        };
        result.sections.push(Section {
            id: id.clone(),
            header: Rect::new(area.x, y, width, 1),
            body: Rect::new(area.x, y + 1, width, body_height),
        });
        y += 1 + body_height;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustive_geometry_is_bounded_fair_and_reachable() {
        for count in 1..=9 {
            let ids = identities(&(1..count).map(|i| format!("source{i}")).collect::<Vec<_>>());
            for mask in 0..1usize << count {
                let collapsed = ids
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, id)| id.clone())
                    .collect();
                for height in 0..=30 {
                    for width in [0, 1, 2, 36] {
                        let area = Rect::new(3, 4, width, height);
                        let mut reached = HashSet::new();
                        for offset in 0..count {
                            let layout = compute(area, &ids, &collapsed, offset);
                            if width > 1 && height > 0 {
                                assert_eq!(layout.menu, Rect::new(area.x, area.y, width - 1, 1));
                                if height > 1 {
                                    assert_eq!(layout.toggle.y, area.bottom() - 1);
                                } else {
                                    assert_eq!(layout.toggle, Rect::default());
                                }
                            }
                            let mut bottom = area.y + 1;
                            let mut heights = Vec::new();
                            for section in &layout.sections {
                                assert!(section.header.y >= bottom);
                                assert!(section.header.right() < area.right());
                                assert_eq!(section.body.y, section.header.bottom());
                                assert!(section.body.bottom() < area.bottom());
                                bottom = section.body.bottom();
                                if !collapsed.contains(&section.id) {
                                    heights.push(section.body.height);
                                } else {
                                    assert_eq!(section.body.height, 0);
                                }
                                reached.insert(section.id.clone());
                            }
                            if let (Some(min), Some(max)) =
                                (heights.iter().min(), heights.iter().max())
                            {
                                assert!(max - min <= 1);
                                assert!(heights.windows(2).all(|pair| pair[0] >= pair[1]));
                            }
                            if width > 1
                                && ids
                                    .iter()
                                    .map(|id| 1 + usize::from(!collapsed.contains(id)))
                                    .sum::<usize>()
                                    <= usize::from(height.saturating_sub(2))
                            {
                                assert_eq!(layout.sections.len(), count);
                                assert!(heights.iter().all(|h| *h >= 1));
                            }
                        }
                        if height >= 3 && width >= 2 {
                            assert_eq!(reached.len(), count);
                        } else {
                            assert!(reached.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn native_identity_cannot_collide_with_peer() {
        assert_ne!(SectionId::Spaces, SectionId::Integration("spaces".into()));
    }

    #[test]
    fn compact_tokens_reserve_native_and_coven_before_peers_and_resolve_prefix_collisions() {
        let sources = [
            "spaces",
            "Coven",
            "build",
            "BUILD",
            "spare",
            "coal",
            "coven",
            "build.two",
        ]
        .map(String::from);
        assert_eq!(
            compact_labels(&sources, 3),
            ["Sp", "#1", "#2", "bu", "#4", "#5", "#6", "Co", "#8"]
        );
        for width in [1, 2] {
            assert_eq!(
                compact_labels(&sources, width),
                ["S", "1", "2", "3", "4", "5", "6", "C", "8"]
            );
        }
        assert!(compact_labels(&sources, 0).iter().all(String::is_empty));
        for width in 1..=3 {
            for rotation in 0..sources.len() {
                let mut reordered = sources.clone();
                reordered.rotate_left(rotation);
                let labels = compact_labels(&reordered, width);
                assert_eq!(
                    labels
                        .iter()
                        .map(|s| s.to_ascii_lowercase())
                        .collect::<HashSet<_>>()
                        .len(),
                    9
                );
                assert_eq!(
                    labels[reordered.iter().position(|id| id == "coven").unwrap() + 1],
                    if width == 3 { "Co" } else { "C" }
                );
            }
        }
    }
}
