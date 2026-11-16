use iced::border::Radius;
use iced::widget::button;
use iced::widget::container;
use iced::widget::pick_list;
use iced::widget::slider;
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

pub fn settings_window_style(_theme: &Theme) -> container::Style {
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

pub fn clean_slider_style(_theme: &Theme, _status: slider::Status) -> slider::Style {
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(COLOR_TEXT),           // Crisp white for active/filled rail!
                Background::Color(COLOR_SURFACE_MUTED),  // Dark zinc for unfilled rail!
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
            background: Background::Color(Color::WHITE),
            border_width: 1.0,
            border_color: COLOR_BORDER,
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

#[allow(dead_code)]
pub fn album_expanded_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => COLOR_ACTIVE_BG,
        _ => COLOR_SURFACE_MUTED,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: COLOR_TEXT,
        border: Border {
            color: COLOR_BORDER_ACTIVE,
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn thumbnail_border_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG)),
        border: Border {
            color: COLOR_BORDER,
            width: 1.0,
            radius: Radius {
                top_left: 2.0,
                top_right: 2.0,
                bottom_right: 2.0,
                bottom_left: 2.0,
            },
        },
        ..Default::default()
    }
}

pub fn media_tile_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.06, 0.06, 0.07))),
        border: Border {
            color: Color::from_rgb(0.12, 0.12, 0.14),
            width: 0.5,
            radius: Radius::default(),
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn media_menu_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.09, 0.09, 0.11, 0.98))),
        border: Border {
            color: COLOR_BORDER_ACTIVE,
            width: 1.0,
            radius: RADIUS_SM,
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        text_color: Some(COLOR_TEXT),
        ..Default::default()
    }
}

pub fn dots_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(0.25, 0.25, 0.28, 0.95)
        }
        _ => Color::from_rgba(0.08, 0.08, 0.10, 0.85),
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

pub fn media_menu_btn_style(is_danger: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_color, border_color) = if is_danger {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    Color::from_rgb(0.95, 0.22, 0.22),
                    Color::WHITE,
                    Color::WHITE,
                ),
                _ => (
                    Color::from_rgb(0.85, 0.20, 0.20),
                    Color::WHITE,
                    Color::TRANSPARENT,
                ),
            }
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => (
                    Color::from_rgb(0.24, 0.24, 0.28),
                    Color::WHITE,
                    Color::from_rgb(0.6, 0.6, 0.65),
                ),
                _ => (
                    Color::from_rgb(0.12, 0.12, 0.15),
                    Color::WHITE,
                    Color::TRANSPARENT,
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
        background: Some(Background::Color(COLOR_BG)),
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
