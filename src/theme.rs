use iced::border::Radius;
use iced::widget::button;
use iced::widget::container;
use iced::widget::pick_list;
use iced::{Background, Border, Color, Shadow, Theme};

// Semantic Monochrome Tokens from clean-ui-design (Strict black & white / zinc)
pub const COLOR_BG: Color = Color::from_rgb(0.040, 0.040, 0.045);         // #0a0a0b (Zinc 950 deep)
pub const COLOR_SURFACE: Color = Color::from_rgb(0.080, 0.080, 0.090);    // #141417 (Zinc 900)
pub const COLOR_SURFACE_MUTED: Color = Color::from_rgb(0.140, 0.140, 0.155); // #242428 (Zinc 800)
pub const COLOR_ACTIVE_BG: Color = Color::from_rgb(0.200, 0.200, 0.220); // #333338 (Zinc 700)
pub const COLOR_BORDER: Color = Color::from_rgb(0.180, 0.180, 0.200);     // #2e2e33 (Subtle 1px border)
pub const COLOR_BORDER_ACTIVE: Color = Color::from_rgb(0.380, 0.380, 0.410); // #616169 (Bright border)
pub const COLOR_TEXT: Color = Color::from_rgb(0.960, 0.960, 0.970);       // #f5f5f7 (Crisp white)
pub const COLOR_TEXT_MUTED: Color = Color::from_rgb(0.620, 0.620, 0.660); // #9e9ea8 (Subtle zinc text)
pub const COLOR_DISABLED: Color = Color::from_rgba(0.55, 0.55, 0.60, 0.35); // #8c8c99 dimmed
pub const COLOR_DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267);     // #ef4444 (Close hover only)

// Radius Tokens
pub const RADIUS_SM: Radius = Radius {
    top_left: 4.0,
    top_right: 4.0,
    bottom_right: 4.0,
    bottom_left: 4.0,
};

pub const RADIUS_MD: Radius = Radius {
    top_left: 8.0,
    top_right: 8.0,
    bottom_right: 8.0,
    bottom_left: 8.0,
};

// Root Window Style - Zero white border, 100% flush rectangular dark frame
pub fn root_window_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: Radius::default(),
        },
        shadow: Shadow::default(),
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn title_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_SURFACE)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::default(),
        },
        text_color: Some(COLOR_TEXT),
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
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn video_viewport_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG)),
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn controls_panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_SURFACE)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::default(),
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn tooltip_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.12, 0.12, 0.14, 0.96,
        ))),
        border: Border {
            color: COLOR_BORDER_ACTIVE,
            width: 1.0,
            radius: RADIUS_SM,
        },
        text_color: Some(COLOR_TEXT),
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub fn settings_dialog_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.07, 0.07, 0.08, 0.98,
        ))),
        border: Border {
            color: COLOR_BORDER_ACTIVE,
            width: 1.0,
            radius: RADIUS_MD,
        },
        shadow: Shadow::default(),
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn settings_inner_box_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_SURFACE)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: RADIUS_SM,
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let bg = match status {
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => {
            Background::Color(COLOR_SURFACE_MUTED)
        }
        _ => Background::Color(COLOR_SURFACE),
    };

    pick_list::Style {
        text_color: COLOR_TEXT,
        placeholder_color: COLOR_TEXT_MUTED,
        handle_color: COLOR_TEXT,
        background: bg,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
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
                    COLOR_TEXT,
                ),
            }
        } else if is_active {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    COLOR_ACTIVE_BG,
                    Color::WHITE,
                ),
                _ => (
                    COLOR_SURFACE_MUTED,
                    COLOR_TEXT,
                ),
            }
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    COLOR_SURFACE_MUTED,
                    Color::WHITE,
                ),
                _ => (
                    Color::TRANSPARENT,
                    COLOR_TEXT,
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
