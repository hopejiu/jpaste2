//! Settings page — rendered as an egui Window.

use crate::settings::Config;

pub struct SettingsPageState {
    pub open: bool,
    /// Directly holds the Config — no more field-by-field duplication.
    pub config: Config,
    pub stats: (i64, i64),
    pub clear_all_clicked: bool,
}

impl SettingsPageState {
    pub fn from_config(config: &Config) -> Self {
        Self {
            open: false,
            config: config.clone(),
            stats: (0, 0),
            clear_all_clicked: false,
        }
    }

    pub fn apply_to(&self, config: &mut Config) {
        *config = self.config.clone();
    }
}

/// Simple hotkey capture: listens for modifier+key combo when text field is focused.
fn hotkey_capture(ui: &mut egui::Ui, buf: &mut String) {
    let resp = ui.add(
        egui::TextEdit::singleline(buf)
            .desired_width(120.0)
            .hint_text("Alt+V"),
    );
    if resp.has_focus() {
        let input = ui.input(|i| i.clone());
        let alt = input.modifiers.alt;
        let ctrl = input.modifiers.ctrl;
        let shift = input.modifiers.shift;
        let logo = input.modifiers.mac_cmd;

        // Build the modifier prefix
        let mods = [("Alt", alt), ("Ctrl", ctrl), ("Shift", shift), ("Win", logo)];
        let has_mod = mods.iter().any(|(_, v)| *v);

        if has_mod {
            for event in &input.events {
                if let egui::Event::Key { key, pressed: true, .. } = event {
                    let key_name = format!("{:?}", key);
                    // Filter out modifier-only keys
                    let is_mod_key = matches!(key,
                        egui::Key::AltLeft | egui::Key::AltRight |
                        egui::Key::ShiftRight | egui::Key::ShiftLeft |
                        egui::Key::ControlRight | egui::Key::ControlLeft |
                        egui::Key::SuperLeft | egui::Key::SuperRight
                    );
                    if !is_mod_key {
                        let mut parts: Vec<&str> = vec![];
                        for (name, active) in &mods {
                            if *active { parts.push(name); }
                        }
                        parts.push(key_name.as_str());
                        *buf = parts.join("+");
                        return;
                    }
                }
            }
        }
    }
}

pub fn settings_page(
    ctx: &egui::Context,
    state: &mut SettingsPageState,
    on_clear_all: &mut dyn FnMut(),
) {
    egui::Window::new("设置")
        .open(&mut state.open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .default_width(400.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Global Hotkey
                    ui.horizontal(|ui| {
                        ui.label("全局热键:");
                        hotkey_capture(ui, &mut state.config.hotkey);
                    });

                    ui.separator();

                    // Retain days
                    ui.horizontal(|ui| {
                        ui.label("保留天数:");
                        ui.add(egui::Slider::new(&mut state.config.retain_days, 1..=365).suffix(" 天"));
                    });

                    ui.checkbox(&mut state.config.auto_start, "开机自启");
                    ui.checkbox(&mut state.config.start_minimized, "启动时最小化");

                    ui.separator();

                    ui.checkbox(&mut state.config.notify_enabled, "剪贴板变化时通知");
                    if state.config.notify_enabled {
                        ui.horizontal(|ui| {
                            ui.label("通知透明度:");
                            ui.add(egui::Slider::new(&mut state.config.notify_opacity, 0..=100).suffix("%"));
                        });
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("粘贴顺序:");
                        egui::ComboBox::from_id_salt("paste_order")
                            .selected_text(&state.config.paste_order)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut state.config.paste_order, "normal".into(), "正常");
                                ui.selectable_value(&mut state.config.paste_order, "queue".into(), "队列 (FIFO)");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("窗口位置:");
                        egui::ComboBox::from_id_salt("window_pos")
                            .selected_text(&state.config.window_position)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut state.config.window_position, "center".into(), "居中");
                                ui.selectable_value(&mut state.config.window_position, "remember".into(), "记忆位置");
                            });
                    });

                    ui.separator();

                    ui.checkbox(&mut state.config.auto_clear_search, "窗口显示时自动清空搜索");
                    if state.config.auto_clear_search {
                        ui.horizontal(|ui| {
                            ui.label("自动清空阈值 (0=总是):");
                            ui.add(egui::Slider::new(&mut state.config.auto_clear_seconds, 0..=300).suffix(" 秒"));
                        });
                    }

                    ui.separator();

                    // Stats
                    let kb = state.stats.1 as f64 / 1024.0;
                    ui.colored_label(
                        egui::Color32::from_rgb(130, 127, 145),
                        format!("条目: {} | 总大小: {:.1} KB", state.stats.0, kb),
                    );

                    ui.separator();

                    if ui.button("清空全部数据").clicked() {
                        state.clear_all_clicked = true;
                        on_clear_all();
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_state_from_config() {
        let config = Config::default();
        let state = SettingsPageState::from_config(&config);
        assert_eq!(state.config.retain_days, 30);
        assert_eq!(state.config.hotkey, "Alt+V");
    }

    #[test]
    fn test_apply_to_config() {
        let mut config = Config::default();
        let _state = SettingsPageState::from_config(&config);
        let mut state2 = SettingsPageState::from_config(&config);
        state2.config.hotkey = "Ctrl+Shift+H".into();
        state2.config.retain_days = 60;
        state2.apply_to(&mut config);
        assert_eq!(config.hotkey, "Ctrl+Shift+H");
        assert_eq!(config.retain_days, 60);
    }
}
