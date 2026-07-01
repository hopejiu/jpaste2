//! Tag filter tab bar — All / Text / Image / URL / File / Favorites.

use egui::Color32;

/// A single tab entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabItem {
    pub label: &'static str,
    pub tag_mask: i32,
}

/// Built-in tabs. tag_mask=0 → show all.
pub const TABS: &[TabItem] = &[
    TabItem { label: "全部", tag_mask: 0 },
    TabItem { label: "文本", tag_mask: 1 },
    TabItem { label: "图片", tag_mask: 4 },
    TabItem { label: "网址", tag_mask: 8 },
    TabItem { label: "文件", tag_mask: 16 },
    TabItem { label: "收藏", tag_mask: 32 },
];

/// Render the tab bar. Returns the new selected tag_mask if changed.
pub fn tab_bar(ui: &mut egui::Ui, current_tag_mask: &mut i32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for tab in TABS {
            let is_selected = *current_tag_mask == tab.tag_mask;
            let text_color = if is_selected {
                Color32::from_rgb(156, 128, 255)
            } else {
                Color32::from_rgb(130, 127, 145)
            };

            // Create a colored label button
            let label = egui::RichText::new(tab.label).color(text_color);
            if ui.selectable_label(is_selected, label).clicked() && !is_selected {
                *current_tag_mask = tab.tag_mask;
                changed = true;
            }
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabs_count() {
        assert_eq!(TABS.len(), 6);
    }

    #[test]
    fn test_tab_tag_values() {
        assert_eq!(TABS[0].tag_mask, 0);
        assert_eq!(TABS[1].tag_mask, 1);
        assert_eq!(TABS[3].tag_mask, 8);
        assert_eq!(TABS[5].tag_mask, 32);
    }
}
