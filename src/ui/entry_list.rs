//! Entry list with infinite scroll auto-loading.

use crate::storage::repository::Entry;
use crate::util::text::format_time;
use egui::Color32;

pub struct EntryListState {
    pub selected_index: usize,
    pub loading: bool,
}

impl Default for EntryListState {
    fn default() -> Self {
        Self { selected_index: 0, loading: false }
    }
}

/// Render the scrollable entry list with auto-load trigger.
pub fn entry_list(
    ui: &mut egui::Ui,
    entries: &[Entry],
    state: &mut EntryListState,
    has_more: bool,
    on_load_more: &mut dyn FnMut(),
) {
    let available_height = ui.available_height();

    // In egui 0.35, ScrollArea uses id_source for identity
    egui::ScrollArea::vertical()
        .id_salt("entry-list")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_min_height(available_height);

            if entries.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label("暂无剪贴板记录");
                });
                return;
            }

            for (i, entry) in entries.iter().enumerate() {
                render_entry_item(ui, entry, i == state.selected_index);
            }

            if has_more && !state.loading {
                let available = ui.available_height();
                if available < 120.0 {
                    state.loading = true;
                    on_load_more();
                }
            }

            if state.loading {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label("加载中...");
                });
            }
        });
}

fn render_entry_item(ui: &mut egui::Ui, entry: &Entry, is_selected: bool) {
    let item_height = 48.0;
    let (id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), item_height));

    let hovered = ui.rect_contains_pointer(rect);
    let fill = if hovered || is_selected {
        if is_selected { Color32::from_rgb(50, 47, 60) } else { Color32::from_rgb(45, 42, 55) }
    } else {
        Color32::from_rgb(38, 36, 46)
    };

    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(4), fill);

        let icon = if entry.tag_mask & 4 != 0 { "📷" }
                   else if entry.tag_mask & 16 != 0 { "📁" }
                   else { "📄" };
        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            icon,
            egui::FontId::proportional(16.0),
            Color32::WHITE,
        );

        let preview = if entry.content.len() > 50 {
            format!("{}…", &entry.content[..50])
        } else {
            entry.content.clone()
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 30.0, rect.min.y + 3.0),
            egui::Align2::LEFT_TOP,
            &preview,
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 218, 230),
        );

        if entry.is_favorite {
            ui.painter().text(
                egui::pos2(rect.right() - 20.0, rect.min.y + 4.0),
                egui::Align2::RIGHT_TOP,
                "⭐",
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
        }

        let source = entry.source_exe
            .rsplit(&['/', '\\'][..])
            .next()
            .unwrap_or(&entry.source_exe);
        let time = format_time(&entry.updated_at);
        ui.painter().text(
            egui::pos2(rect.min.x + 30.0, rect.min.y + 22.0),
            egui::Align2::LEFT_TOP,
            format!("{} · {}", source, time),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(130, 127, 145),
        );
    }

    let _response = ui.interact(rect, id, egui::Sense::click());
}
