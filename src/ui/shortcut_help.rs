//! Keyboard shortcuts help modal.

use egui::Color32;

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+L",         "聚焦搜索框"),
    ("Ctrl+E",         "在编辑器中打开"),
    ("Ctrl+C",         "复制选中条目"),
    ("Ctrl+1~9",       "复制第 1~9 条"),
    ("↑ / ↓",         "移动条目焦点"),
    ("Enter",          "复制焦点条目"),
    ("Delete",         "删除焦点条目"),
    ("Space",          "切换收藏"),
    ("Home / End",     "滚动顶部 / 底部"),
    ("PageUp / Down",  "翻页"),
    ("Esc",            "清空搜索 / 隐藏窗口"),
    ("Alt+V",          "全局热键 - 切换窗口"),
    ("?",              "显示此帮助"),
];

/// Show the shortcuts help modal window.
pub fn shortcut_help_window(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("快捷键帮助")
        .open(open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.colored_label(Color32::from_rgb(156, 128, 255), "jPaste 快捷键");
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for &(key, desc) in SHORTCUTS {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::from_rgb(156, 128, 255), key);
                            ui.label(desc);
                        });
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcuts_list() {
        assert!(SHORTCUTS.len() > 5);
        let has_ctrl_l = SHORTCUTS.iter().any(|(k, _)| *k == "Ctrl+L");
        assert!(has_ctrl_l);
    }
}
