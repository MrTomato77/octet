use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub primary: Color32,
    pub selection: Color32,
    pub accent: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: oklch(0.19, 0.0, 0.0),
                surface: oklch(0.23, 0.0, 0.0),
                ink: oklch(0.96, 0.0, 0.0),
                muted: oklch(0.76, 0.008, 3.4),
                border: oklch(0.40, 0.008, 3.4),
                primary: oklch(0.49, 0.18, 3.4),
                selection: oklch(0.33, 0.055, 3.4),
                accent: oklch(0.84, 0.06, 205.0),
            }
        } else {
            Self {
                background: oklch(1.0, 0.0, 0.0),
                surface: oklch(0.975, 0.0, 0.0),
                ink: oklch(0.24, 0.012, 3.4),
                muted: oklch(0.46, 0.01, 3.4),
                border: oklch(0.87, 0.006, 3.4),
                primary: oklch(0.49, 0.18, 3.4),
                selection: oklch(0.955, 0.025, 3.4),
                accent: oklch(0.33, 0.055, 205.0),
            }
        }
    }
}

/// Tokens stay in OKLCH; convert only at egui's sRGB rendering boundary.
fn oklch(l: f32, c: f32, hue: f32) -> Color32 {
    let a = c * hue.to_radians().cos();
    let b = c * hue.to_radians().sin();
    let ll = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let mm = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let ss = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let channel = |value: f32| {
        let value = if value <= 0.003_130_8 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color32::from_rgb(
        channel(4.076_741_7 * ll - 3.307_711_6 * mm + 0.230_969_94 * ss),
        channel(-1.268_438 * ll + 2.609_757_4 * mm - 0.341_319_4 * ss),
        channel(-0.004_196_086_3 * ll - 0.703_418_6 * mm + 1.707_614_7 * ss),
    )
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // Read installed fonts without embedding or redistributing Microsoft assets.
    if let Some(windir) = std::env::var_os("WINDIR") {
        let directory = std::path::PathBuf::from(windir).join("Fonts");
        for (name, file) in [("windows-ui", "segoeui.ttf")] {
            if let Ok(bytes) = std::fs::read(directory.join(file)) {
                fonts.font_data.insert(
                    name.into(),
                    std::sync::Arc::new(FontData::from_owned(bytes)),
                );
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, name.into());
            }
        }
    }
    fonts.font_data.insert(
        "noto-sans-thai".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSansThai.ttf"
        ))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto-sans-thai".into());
    fonts.font_data.insert(
        "phosphor".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Phosphor.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name("symbols".into()), vec!["phosphor".into()]);
    ctx.set_fonts(fonts);
}

pub fn install_style(ctx: &egui::Context, dark: bool) {
    let p = Palette::new(dark);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let mut style = egui::Style {
        visuals: if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        },
        ..Default::default()
    };
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(16.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(14.0));
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 28.0;
    style.animation_time = 0.0;
    style.visuals.override_text_color = Some(p.ink);
    style.visuals.panel_fill = p.background;
    style.visuals.window_fill = p.background;
    style.visuals.extreme_bg_color = p.background;
    style.visuals.faint_bg_color = p.surface;
    style.visuals.selection.bg_fill = p.selection;
    style.visuals.selection.stroke = Stroke::new(1.0, p.ink);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    style.visuals.widgets.inactive.weak_bg_fill = p.surface;
    style.visuals.widgets.inactive.bg_fill = p.surface;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.ink);
    style.visuals.widgets.hovered.weak_bg_fill = p.selection;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.primary);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ink);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.5, p.primary);
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f32 {
        let linear = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let a = luminance(a);
        let b = luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_and_action_colors_have_readable_contrast() {
        for dark in [false, true] {
            let p = Palette::new(dark);
            assert!(contrast(p.ink, p.background) >= 7.0);
            assert!(contrast(p.muted, p.background) >= 4.5);
            assert!(contrast(p.muted, p.surface) >= 4.5);
            assert!(contrast(Color32::WHITE, p.primary) >= 4.5);
            assert!(contrast(p.ink, p.selection) >= 4.5);
        }
    }
}
