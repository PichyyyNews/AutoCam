use iced::border::Radius;
use iced::overlay::menu;
use iced::widget::button;
use iced::widget::container;
use iced::widget::pick_list;
use iced::widget::slider;
use iced::{Background, Border, Color, Shadow, Theme};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppThemeMode {
    Dark,
    Light,
    System,
}

impl AppThemeMode {
    pub fn label(&self) -> &'static str {
        match self {
            AppThemeMode::Dark => "Dark",
            AppThemeMode::Light => "Light",
            AppThemeMode::System => "System",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentColor {
    Zinc,
    Blue,
    Emerald,
    Amber,
    Rose,
    Violet,
    Cyan,
}

impl AccentColor {
    pub fn label(&self) -> &'static str {
        match self {
            AccentColor::Zinc => "Zinc (Neutral)",
            AccentColor::Blue => "Studio Blue",
            AccentColor::Emerald => "Emerald Green",
            AccentColor::Amber => "Sunset Amber",
            AccentColor::Rose => "Crimson Rose",
            AccentColor::Violet => "Electric Violet",
            AccentColor::Cyan => "Cyber Cyan",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            AccentColor::Zinc => Color::from_rgb(0.45, 0.45, 0.50),
            AccentColor::Blue => Color::from_rgb(0.145, 0.388, 0.922),
            AccentColor::Emerald => Color::from_rgb(0.020, 0.588, 0.412),
            AccentColor::Amber => Color::from_rgb(0.851, 0.467, 0.024),
            AccentColor::Rose => Color::from_rgb(0.882, 0.114, 0.282),
            AccentColor::Violet => Color::from_rgb(0.486, 0.227, 0.929),
            AccentColor::Cyan => Color::from_rgb(0.031, 0.569, 0.698),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiFontScale {
    Compact, // 90%
    Normal,  // 100%
    Large,   // 115%
}

impl UiFontScale {
    pub fn label(&self) -> &'static str {
        match self {
            UiFontScale::Compact => "Compact (90%)",
            UiFontScale::Normal => "Standard (100%)",
            UiFontScale::Large => "Large (115%)",
        }
    }
}

// Atomic Global Theme State for instant styling across all windows
static THEME_MODE: AtomicU8 = AtomicU8::new(0); // 0: Dark, 1: Light, 2: System
static ACCENT_COLOR: AtomicU8 = AtomicU8::new(0); // 0: Zinc, 1: Blue, 2: Emerald, 3: Amber, 4: Rose, 5: Violet, 6: Cyan
static FONT_SCALE: AtomicU8 = AtomicU8::new(1); // 0: Compact, 1: Normal, 2: Large

pub fn set_theme_mode(mode: AppThemeMode) {
    let val = match mode {
        AppThemeMode::Dark => 0,
        AppThemeMode::Light => 1,
        AppThemeMode::System => 2,
    };
    THEME_MODE.store(val, Ordering::Relaxed);
}

pub fn get_theme_mode() -> AppThemeMode {
    match THEME_MODE.load(Ordering::Relaxed) {
        1 => AppThemeMode::Light,
        2 => AppThemeMode::System,
        _ => AppThemeMode::Dark,
    }
}

pub fn set_accent_color(accent: AccentColor) {
    let val = match accent {
        AccentColor::Zinc => 0,
        AccentColor::Blue => 1,
        AccentColor::Emerald => 2,
        AccentColor::Amber => 3,
        AccentColor::Rose => 4,
        AccentColor::Violet => 5,
        AccentColor::Cyan => 6,
    };
    ACCENT_COLOR.store(val, Ordering::Relaxed);
}

pub fn get_accent_color() -> AccentColor {
    match ACCENT_COLOR.load(Ordering::Relaxed) {
        1 => AccentColor::Blue,
        2 => AccentColor::Emerald,
        3 => AccentColor::Amber,
        4 => AccentColor::Rose,
        5 => AccentColor::Violet,
        6 => AccentColor::Cyan,
        _ => AccentColor::Zinc,
    }
}

pub fn set_font_scale(scale: UiFontScale) {
    let val = match scale {
        UiFontScale::Compact => 0,
        UiFontScale::Normal => 1,
        UiFontScale::Large => 2,
    };
    FONT_SCALE.store(val, Ordering::Relaxed);
}

pub fn get_font_scale() -> UiFontScale {
    match FONT_SCALE.load(Ordering::Relaxed) {
        0 => UiFontScale::Compact,
        2 => UiFontScale::Large,
        _ => UiFontScale::Normal,
    }
}

pub fn is_dark() -> bool {
    get_theme_mode() != AppThemeMode::Light
}

// Fallback constants for backward compatibility
#[allow(dead_code)]
pub const COLOR_BG: Color = Color::from_rgb(0.040, 0.040, 0.045);
#[allow(dead_code)]
pub const COLOR_SURFACE: Color = Color::from_rgb(0.080, 0.080, 0.090);
#[allow(dead_code)]
pub const COLOR_SURFACE_MUTED: Color = Color::from_rgb(0.140, 0.140, 0.155);
#[allow(dead_code)]
pub const COLOR_ACTIVE_BG: Color = Color::from_rgb(0.200, 0.200, 0.220);
#[allow(dead_code)]
pub const COLOR_BORDER: Color = Color::from_rgb(0.180, 0.180, 0.200);
#[allow(dead_code)]
pub const COLOR_BORDER_ACTIVE: Color = Color::from_rgb(0.380, 0.380, 0.410);
pub const COLOR_TEXT: Color = Color::from_rgb(0.960, 0.960, 0.970);
pub const COLOR_TEXT_MUTED: Color = Color::from_rgb(0.620, 0.620, 0.660);
pub const COLOR_DISABLED: Color = Color::from_rgba(0.55, 0.55, 0.60, 0.35);
pub const COLOR_DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267);

// Dynamic Theme Colors according to Clean UI Design System tokens
pub fn color_bg() -> Color {
    if is_dark() {
        Color::from_rgb(0.040, 0.040, 0.045) // #0a0a0b (Zinc 950 deep)
    } else {
        Color::from_rgb(0.960, 0.960, 0.965) // #f5f5f7 (Light zinc surface)
    }
}

pub fn color_surface() -> Color {
    if is_dark() {
        Color::from_rgb(0.080, 0.080, 0.090) // #141417 (Zinc 900)
    } else {
        Color::from_rgb(1.0, 1.0, 1.0)       // #ffffff (Pure clean white)
    }
}

pub fn color_surface_muted() -> Color {
    if is_dark() {
        Color::from_rgb(0.140, 0.140, 0.155) // #242428 (Zinc 800)
    } else {
        Color::from_rgb(0.910, 0.910, 0.925) // #e8e8ec (Light zinc subtle)
    }
}

pub fn color_active_bg() -> Color {
    if is_dark() {
        Color::from_rgb(0.200, 0.200, 0.220) // #333338 (Zinc 700)
    } else {
        Color::from_rgb(0.840, 0.840, 0.865) // #d6d6dd
    }
}

pub fn color_border() -> Color {
    if is_dark() {
        Color::from_rgb(0.180, 0.180, 0.200) // #2e2e33 (Subtle 1px border)
    } else {
        Color::from_rgb(0.850, 0.850, 0.875) // #d9d9df (Clean light border)
    }
}

pub fn color_border_active() -> Color {
    if is_dark() {
        Color::from_rgb(0.380, 0.380, 0.410) // #616169 (Bright dark border)
    } else {
        Color::from_rgb(0.600, 0.600, 0.650) // #9999a6
    }
}

pub fn color_text() -> Color {
    if is_dark() {
        Color::from_rgb(0.960, 0.960, 0.970) // #f5f5f7 (Crisp white text)
    } else {
        Color::from_rgb(0.080, 0.080, 0.090) // #141417 (Deep dark text)
    }
}

pub fn color_text_muted() -> Color {
    if is_dark() {
        Color::from_rgb(0.620, 0.620, 0.660) // #9e9ea8 (Subtle zinc text)
    } else {
        Color::from_rgb(0.420, 0.420, 0.460) // #6b6b75 (Subtle light text)
    }
}

pub fn color_accent() -> Color {
    get_accent_color().color()
}

// Font Size Scaling Token
pub fn font_size(base: u16) -> f32 {
    let b = base as f32;
    match get_font_scale() {
        UiFontScale::Compact => (b * 0.90).round(),
        UiFontScale::Normal => b,
        UiFontScale::Large => (b * 1.15).round(),
    }
}

// Radius Tokens
pub const RADIUS_SM: Radius = Radius {
    top_left: 4.0,
    top_right: 4.0,
    bottom_right: 4.0,
    bottom_left: 4.0,
};

pub const RADIUS_MD: Radius = Radius {
    top_left: 6.0,
    top_right: 6.0,
    bottom_right: 6.0,
    bottom_left: 6.0,
};

pub const RADIUS_FULL: Radius = Radius {
    top_left: 999.0,
    top_right: 999.0,
    bottom_right: 999.0,
    bottom_left: 999.0,
};

// Root Window Style
pub fn root_window_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_bg())),
        border: Border {
            color: color_border(),
            width: 1.0,
            radius: Radius::default(),
        },
        shadow: Shadow::default(),
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn title_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_surface())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::default(),
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn pip_overlay_title_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.06, 0.06, 0.07, 0.88,
        ))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::default(),
        },
        text_color: Some(Color::WHITE),
        ..Default::default()
    }
}

pub fn video_viewport_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_bg())),
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn controls_panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_surface())),
        border: Border {
            color: color_border(),
            width: 1.0,
            radius: Radius::default(),
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn tooltip_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if is_dark() {
            Color::from_rgba(0.12, 0.12, 0.14, 0.96)
        } else {
            Color::from_rgba(0.98, 0.98, 1.0, 0.98)
        })),
        border: Border {
            color: color_border_active(),
            width: 1.0,
            radius: RADIUS_SM,
        },
        text_color: Some(color_text()),
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn settings_window_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_bg())),
        border: Border {
            color: color_border(),
            width: 1.0,
            radius: Radius::default(),
        },
        shadow: Shadow::default(),
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn settings_inner_box_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_surface())),
        border: Border {
            color: color_border(),
            width: 1.0,
            radius: RADIUS_MD,
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

// Clean Sidebar Styles matching Desktop Settings Standards
pub fn sidebar_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if is_dark() {
            Color::from_rgb(0.060, 0.060, 0.070)
        } else {
            Color::from_rgb(0.935, 0.935, 0.945)
        })),
        border: Border {
            color: color_border(),
            width: 1.0,
            radius: Radius::default(),
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn sidebar_item_style(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_col) = if is_active {
            if get_accent_color() == AccentColor::Zinc {
                (color_active_bg(), color_text())
            } else {
                let acc = color_accent();
                let alpha = if is_dark() { 0.24 } else { 0.16 };
                (Color::from_rgba(acc.r, acc.g, acc.b, alpha), color_text())
            }
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    color_surface_muted(),
                    color_text(),
                ),
                _ => (
                    Color::TRANSPARENT,
                    color_text_muted(),
                ),
            }
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text_col,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS_MD,
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}

pub fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let (bg, border_color) = if is_dark() {
        match status {
            pick_list::Status::Hovered | pick_list::Status::Opened { .. } => (
                Background::Color(Color::from_rgb(0.18, 0.18, 0.22)),
                Color::from_rgb(0.45, 0.45, 0.52),
            ),
            _ => (
                Background::Color(Color::from_rgb(0.11, 0.11, 0.13)),
                Color::from_rgb(0.24, 0.24, 0.28),
            ),
        }
    } else {
        match status {
            pick_list::Status::Hovered | pick_list::Status::Opened { .. } => (
                Background::Color(Color::from_rgb(0.92, 0.92, 0.95)),
                Color::from_rgb(0.55, 0.55, 0.65),
            ),
            _ => (
                Background::Color(Color::from_rgb(1.0, 1.0, 1.0)),
                Color::from_rgb(0.80, 0.80, 0.85),
            ),
        }
    };

    pick_list::Style {
        text_color: color_text(),
        placeholder_color: color_text_muted(),
        handle_color: color_text(),
        background: bg,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_SM,
        },
    }
}

pub fn disabled_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: COLOR_DISABLED,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn clean_button_style(
    is_active: bool,
    is_danger: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_color) = if is_danger {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    COLOR_DANGER,
                    Color::WHITE,
                ),
                _ => (
                    Color::TRANSPARENT,
                    color_text(),
                ),
            }
        } else if is_active {
            let active_bg = if get_accent_color() == AccentColor::Zinc {
                color_active_bg()
            } else {
                color_accent()
            };
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    active_bg,
                    Color::WHITE,
                ),
                _ => (
                    active_bg,
                    Color::WHITE,
                ),
            }
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    color_surface_muted(),
                    color_text(),
                ),
                _ => (
                    Color::TRANSPARENT,
                    color_text(),
                ),
            }
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS_SM,
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}

pub fn clean_slider_style(_theme: &Theme, _status: slider::Status) -> slider::Style {
    let accent = color_accent();
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(if get_accent_color() == AccentColor::Zinc { color_text() } else { accent }),
                Background::Color(color_surface_muted()),
            ),
            width: 3.5,
            border: Border {
                radius: RADIUS_SM,
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 6.5 },
            background: Background::Color(if is_dark() { Color::WHITE } else { color_text() }),
            border_width: 1.0,
            border_color: color_border(),
        },
    }
}

pub fn recording_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(0.94, 0.27, 0.27, 0.28)
        }
        _ => Color::from_rgba(0.94, 0.27, 0.27, 0.16),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::from_rgb(0.94, 0.27, 0.27),
        border: Border {
            color: Color::from_rgba(0.94, 0.27, 0.27, 0.4),
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn media_tile_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if is_dark() {
            Color::from_rgb(0.06, 0.06, 0.07)
        } else {
            Color::from_rgb(0.92, 0.92, 0.95)
        })),
        border: Border {
            color: color_border(),
            width: 0.5,
            radius: Radius::default(),
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn media_menu_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if is_dark() {
            Color::from_rgb(0.12, 0.12, 0.15)
        } else {
            Color::from_rgb(0.98, 0.98, 1.0)
        })),
        border: Border {
            color: color_border_active(),
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, if is_dark() { 0.85 } else { 0.15 }),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 14.0,
        },
        text_color: Some(color_text()),
        ..Default::default()
    }
}

pub fn dots_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(0.32, 0.32, 0.40, 0.95)
        }
        _ => Color::from_rgba(0.10, 0.10, 0.14, 0.90),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::WHITE,
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn danger_menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
    media_menu_btn_style(true)(theme, status)
}

pub fn menu_style(_theme: &Theme) -> menu::Style {
    menu::Style {
        text_color: color_text(),
        background: Background::Color(if is_dark() {
            Color::from_rgb(0.12, 0.12, 0.15)
        } else {
            Color::from_rgb(0.98, 0.98, 1.0)
        }),
        border: Border {
            color: color_border_active(),
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, if is_dark() { 0.8 } else { 0.12 }),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        selected_text_color: Color::WHITE,
        selected_background: Background::Color(if get_accent_color() == AccentColor::Zinc {
            color_active_bg()
        } else {
            color_accent()
        }),
    }
}

pub fn media_menu_btn_style(is_danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_color, border_color) = if is_danger {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    Color::from_rgb(0.85, 0.20, 0.20),
                    Color::WHITE,
                    Color::WHITE,
                ),
                _ => (
                    Color::from_rgb(0.35, 0.11, 0.11),
                    Color::from_rgb(0.98, 0.45, 0.45),
                    Color::from_rgb(0.60, 0.18, 0.18),
                ),
            }
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    Color::from_rgb(0.28, 0.28, 0.35),
                    Color::WHITE,
                    Color::WHITE,
                ),
                _ => (
                    if is_dark() { Color::from_rgb(0.16, 0.16, 0.20) } else { Color::from_rgb(0.92, 0.92, 0.96) },
                    color_text(),
                    color_border(),
                ),
            }
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: RADIUS_SM,
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}

pub fn tooltip_image_preview_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color_bg())),
        border: Border {
            color: Color::WHITE,
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.8),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    }
}
