//! Search bar with keyword input, regex toggle, sort dropdown, and stats.

pub struct SearchBarState {
    pub query: String,
    pub regex_mode: bool,
    pub sort_field: String,
    pub sort_order: String,
    pub total_count: i64,
    pub filtered_count: i64,
}

impl Default for SearchBarState {
    fn default() -> Self {
        Self {
            query: String::new(),
            regex_mode: false,
            sort_field: "updated_at".into(),
            sort_order: "desc".into(),
            total_count: 0,
            filtered_count: 0,
        }
    }
}

/// Render the search bar. Returns true if any filter/search changed.
pub fn search_bar(ui: &mut egui::Ui, state: &mut SearchBarState) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        // Search input
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("搜索内容...")
                .desired_width(200.0),
        );
        if resp.changed() {
            changed = true;
        }

        // Ctrl+L to focus
        if ui.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            resp.request_focus();
        }

        // Regex toggle
        let regex_resp = ui.selectable_label(state.regex_mode, ".*");
        if regex_resp.clicked() {
            state.regex_mode = !state.regex_mode;
            changed = true;
        }

        // Sort dropdown
        let sort_label = match (state.sort_field.as_str(), state.sort_order.as_str()) {
            ("updated_at", "desc") => "最新",
            ("content_length", "asc") => "最短",
            _ => "排序",
        };

        egui::ComboBox::from_id_salt("sort")
            .selected_text(sort_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(
                    state.sort_field == "updated_at" && state.sort_order == "desc",
                    "最新",
                ).clicked() {
                    state.sort_field = "updated_at".into();
                    state.sort_order = "desc".into();
                    changed = true;
                }
                if ui.selectable_label(
                    state.sort_field == "content_length" && state.sort_order == "asc",
                    "最短",
                ).clicked() {
                    state.sort_field = "content_length".into();
                    state.sort_order = "asc".into();
                    changed = true;
                }
            });

        // Stats
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.colored_label(
                egui::Color32::from_rgb(130, 127, 145),
                format!("{}/{}", state.filtered_count, state.total_count),
            );
        });
    });

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_bar_default() {
        let s = SearchBarState::default();
        assert!(s.query.is_empty());
        assert!(!s.regex_mode);
        assert_eq!(s.sort_field, "updated_at");
    }
}
