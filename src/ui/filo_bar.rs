//! Bottom bar for FiloStack mode toggle and queue visualization.

use egui::{Color32, Id};

pub struct FiloBarState {
    pub mode: String,
    pub queue_items: Vec<String>,
}

impl FiloBarState {
    pub fn new(mode: &str) -> Self {
        Self { mode: mode.into(), queue_items: vec![] }
    }
}

/// Render the bottom FiloStack control bar.
pub fn filo_bar(
    ui: &mut egui::Ui,
    state: &mut FiloBarState,
    on_toggle: &mut dyn FnMut(&str),
) {
    let mut toggled = false;

    ui.horizontal(|ui| {
        let normal_selected = state.mode == "normal";
        if ui.selectable_label(normal_selected, "正常").clicked() && !normal_selected {
            state.mode = "normal".into();
            on_toggle("normal");
            toggled = true;
        }

        let queue_selected = state.mode == "queue";
        let label = format!("队列 ({})", state.queue_items.len());
        if ui.selectable_label(queue_selected, label).clicked() && !queue_selected {
            state.mode = "queue".into();
            on_toggle("queue");
            toggled = true;
        }

        // Queue popup on hover
        if state.mode == "queue" && !state.queue_items.is_empty() {
            let resp = ui.colored_label(
                Color32::from_rgb(130, 127, 145),
                format!("{} 个待粘贴", state.queue_items.len()),
            );

            if resp.hovered() {
                egui::Area::new(Id::new("queue-popup"))
                    .interactable(true)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_width(200.0);
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label("队列 (FIFO):");
                            ui.separator();
                            for (i, item) in state.queue_items.iter().enumerate() {
                                let prefix = if i == 0 { "▶ " } else { "  " };
                                let preview = if item.len() > 40 {
                                    format!("{}...", &item[..40])
                                } else {
                                    item.clone()
                                };
                                ui.label(format!("{}{}", prefix, preview));
                            }
                        });
                    });
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.colored_label(Color32::from_rgb(130, 127, 145), "Ctrl+V 自队列弹出");
        });
    });

    let _ = toggled;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filo_state_default() {
        let s = FiloBarState::new("normal");
        assert_eq!(s.mode, "normal");
        assert!(s.queue_items.is_empty());
    }
}
