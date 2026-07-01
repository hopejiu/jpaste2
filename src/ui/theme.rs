//! Fixed "梦幻浅紫" theme for jPaste.

use std::sync::OnceLock;

use egui::{Color32, FontData, FontDefinitions, Style, Visuals};

const PRIMARY: Color32 = Color32::from_rgb(156, 128, 255);
const BG_WINDOW: Color32 = Color32::from_rgb(28, 27, 34);
const BG_ITEM: Color32 = Color32::from_rgb(38, 36, 46);
const BG_HOVER: Color32 = Color32::from_rgb(50, 47, 60);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 218, 230);
const TEXT_MUTED: Color32 = Color32::from_rgb(130, 127, 145);
const BORDER: Color32 = Color32::from_rgb(50, 48, 58);

/// Load MS YaHei from system font dir, build FontDefinitions once.
fn make_fonts() -> &'static FontDefinitions {
    static FONTS: OnceLock<FontDefinitions> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut fonts = FontDefinitions::default();

        // Try to load Microsoft YaHei from C:\Windows\Fonts
        let font_paths = [
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\msyh.ttf",
            r"C:\Windows\Fonts\msyhbd.ttc",
        ];

        for path in &font_paths {
            if let Ok(data) = std::fs::read(path) {
                let name = "Microsoft YaHei".to_owned();
                fonts
                    .font_data
                    .insert(name.clone(), FontData::from_owned(data).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, name.clone());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, name);
                break;
            }
        }

        fonts
    })
}

/// Apply the jPaste custom theme + fonts to an egui context.
pub fn apply_theme(ctx: &egui::Context) {
    // ── Fonts: once-initialized with MS YaHei for CJK ──
    ctx.set_fonts(make_fonts().clone());

    // ── Visual style ──
    let mut style = Style::default();
    style.visuals = Visuals {
        dark_mode: true,
        override_text_color: Some(TEXT_PRIMARY),
        window_fill: BG_WINDOW,
        panel_fill: BG_WINDOW,
        faint_bg_color: BG_ITEM,
        extreme_bg_color: Color32::from_rgb(18, 17, 22),
        code_bg_color: BG_ITEM,
        warn_fg_color: Color32::from_rgb(255, 200, 80),
        error_fg_color: Color32::from_rgb(255, 100, 100),
        hyperlink_color: PRIMARY,
        selection: egui::style::Selection {
            bg_fill: PRIMARY.gamma_multiply(0.3),
            stroke: egui::Stroke::new(1.0, PRIMARY),
        },
        widgets: egui::style::Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: BG_ITEM,
                weak_bg_fill: BG_ITEM,
                bg_stroke: egui::Stroke::new(1.0, BORDER),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(1.0, TEXT_MUTED),
                expansion: 0.0,
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: BG_ITEM,
                weak_bg_fill: BG_ITEM,
                bg_stroke: egui::Stroke::new(1.0, BORDER),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(1.0, TEXT_PRIMARY),
                expansion: 0.0,
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: BG_HOVER,
                weak_bg_fill: BG_HOVER,
                bg_stroke: egui::Stroke::new(1.0, PRIMARY),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(2.0, TEXT_PRIMARY),
                expansion: 0.0,
            },
            active: egui::style::WidgetVisuals {
                bg_fill: PRIMARY.gamma_multiply(0.2),
                weak_bg_fill: PRIMARY.gamma_multiply(0.2),
                bg_stroke: egui::Stroke::new(1.0, PRIMARY),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(2.0, PRIMARY),
                expansion: 0.0,
            },
            open: egui::style::WidgetVisuals {
                bg_fill: BG_ITEM,
                weak_bg_fill: BG_ITEM,
                bg_stroke: egui::Stroke::new(1.0, PRIMARY),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(1.5, TEXT_PRIMARY),
                expansion: 0.0,
            },
        },
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 16.0;

    ctx.set_style_of(egui::Theme::from_dark_mode(true), style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify theme can be applied after Context::run() — the real startup sequence.
    #[test]
    fn test_apply_theme_after_run() {
        let ctx = egui::Context::default();
        // run() initializes fonts; apply_theme afterwards should not panic.
        let _ = ctx.run_ui(egui::RawInput::default(), |_ctx| {});
        apply_theme(&ctx);
    }
}
