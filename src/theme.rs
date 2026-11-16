use iced::border::Radius;
use iced::widget::button;
use iced::widget::container;
use iced::{Background, Border, Color, Shadow, Theme, Vector};

// Semantic Color Tokens from clean-ui-design
pub const COLOR_BG: Color = Color::from_rgb(0.070, 0.070, 0.078); // #121214
pub const COLOR_SURFACE: Color = Color::from_rgb(0.102, 0.102, 0.114); // #1a1a1d
pub const COLOR_SURFACE_MUTED: Color = Color::from_rgb(0.137, 0.137, 0.153); // #232327
pub const COLOR_BORDER: Color = Color::from_rgb(0.180, 0.180, 0.200); // #2e2e33
pub const COLOR_TEXT: Color = Color::from_rgb(0.925, 0.925, 0.937); // #ececef
pub const COLOR_TEXT_MUTED: Color = Color::from_rgb(0.631, 0.631, 0.667); // #a1a1aa
pub const COLOR_PRIMARY: Color = Color::from_rgb(0.231, 0.510, 0.965); // #3b82f6
pub const COLOR_PRIMARY_BG: Color = Color::from_rgb(0.118, 0.227, 0.450);
pub const COLOR_DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267); // #ef4444

// Radius Tokens
pub const RADIUS_SM: Radius = Radius {
    top_left: 6.0,
    top_right: 6.0,
    bottom_right: 6.0,
    bottom_left: 6.0,
};

pub const RADIUS_MD: Radius = Radius {
    top_left: 8.0,
    top_right: 8.0,
    bottom_right: 8.0,
    bottom_left: 8.0,
};

pub const RADIUS_LG: Radius = Radius {
    top_left: 12.0,
    top_right: 12.0,
    bottom_right: 12.0,
    bottom_left: 12.0,
};

// Clean UI Styles
pub fn root_window_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: RADIUS_LG,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn title_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_SURFACE)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: Radius {
                top_left: 11.0,
                top_right: 11.0,
                bottom_left: 0.0,
                bottom_right: 0.0,
            },
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn pip_overlay_title_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.07, 0.07, 0.08, 0.85,
        ))),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: RADIUS_MD,
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn video_viewport_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::BLACK)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: RADIUS_MD,
        },
        ..Default::default()
    }
}

pub fn controls_panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_SURFACE)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: RADIUS_MD,
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn clean_button_style(
    is_active: bool,
    is_danger: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, border_color, text_color) = if is_danger {
            match status {
                button::Status::Hovered => (
                    COLOR_DANGER,
                    COLOR_DANGER,
                    COLOR_TEXT,
                ),
                _ => (
                    Color::TRANSPARENT,
                    COLOR_BORDER,
                    COLOR_DANGER,
                ),
            }
        } else if is_active {
            match status {
                button::Status::Hovered => (
                    COLOR_PRIMARY,
                    COLOR_PRIMARY,
                    Color::WHITE,
                ),
                _ => (
                    COLOR_PRIMARY_BG,
                    COLOR_PRIMARY,
                    COLOR_TEXT,
                ),
            }
        } else {
            match status {
                button::Status::Hovered => (
                    COLOR_SURFACE_MUTED,
                    COLOR_BORDER,
                    COLOR_TEXT,
                ),
                _ => (
                    Color::TRANSPARENT,
                    COLOR_BORDER,
                    COLOR_TEXT_MUTED,
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
