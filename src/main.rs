mod camera;
mod icons;
mod theme;

use camera::{CameraControl, CameraDeviceInfo, CameraManager, VideoResolution};
use iced::keyboard::{self, key, Key};
use iced::mouse;
use iced::widget::{
    button, column, container, image, mouse_area, pick_list, row, stack, svg, text, tooltip, Space,
};
use iced::window;
use iced::{Alignment, ContentFit, Element, Fill, Length, Subscription, Task};
use std::time::Duration;

const APP_ICON_BYTES: &[u8] = include_bytes!("../public/icon.png");

pub struct AutoCamApp {
    camera_manager: Option<CameraManager>,
    available_devices: Vec<CameraDeviceInfo>,
    selected_device: Option<CameraDeviceInfo>,
    resolution: VideoResolution,
    is_pip: bool,
    pip_hovered: bool,
    is_flip: bool,
    is_frozen: bool,
    is_fullscreen: bool,
    show_settings: bool,
    rotation_deg: u16,
    zoom_level: f32,
    image_handle: Option<image::Handle>,
    app_icon_handle: image::Handle,
    frame_count: u64,
    status_text: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    PollDevices,
    DevicesPolled(Vec<CameraDeviceInfo>),
    KeyboardEvent(keyboard::Event),
    DeviceSelected(CameraDeviceInfo),
    ToggleResolution,
    SetResolution(VideoResolution),
    ToggleFlip,
    ToggleFreeze,
    ToggleFullscreen,
    ToggleSettings,
    RotateNext,
    ZoomIn,
    ZoomOut,
    TogglePip,
    PipMouseEnter,
    PipMouseExit,
    DragWindow,
    ResizeWindow(window::Direction),
    MinimizeWindow,
    CloseWindow,
    RefreshDevices,
}

impl AutoCamApp {
    pub fn new() -> (Self, Task<Message>) {
        let devices = CameraManager::query_available_devices();
        let selected_device = devices.first().cloned();
        let resolution = VideoResolution::Hd720p;

        let camera_manager = if let Some(ref dev) = selected_device {
            let (mgr, _) = CameraManager::start(dev.index, resolution);
            mgr.send(CameraControl::SetFlip(true));
            Some(mgr)
        } else {
            None
        };

        let status_text = if let Some(ref dev) = selected_device {
            format!("Connected: {}", dev.name)
        } else {
            "No camera detected".to_string()
        };

        (
            Self {
                camera_manager,
                available_devices: devices,
                selected_device,
                resolution,
                is_pip: false,
                pip_hovered: false,
                is_flip: true, // Enabled by default to eliminate orientation confusion
                is_frozen: false,
                is_fullscreen: false,
                show_settings: false,
                rotation_deg: 0,
                zoom_level: 1.0,
                image_handle: None,
                app_icon_handle: image::Handle::from_bytes(APP_ICON_BYTES),
                frame_count: 0,
                status_text,
            },
            Task::none(),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick),
            iced::time::every(Duration::from_millis(1500)).map(|_| Message::PollDevices),
            keyboard::listen().map(Message::KeyboardEvent),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                if let Some(ref mgr) = self.camera_manager {
                    if let Some(frame) = mgr.take_latest_frame() {
                        self.frame_count += 1;
                        self.image_handle = Some(image::Handle::from_rgba(
                            frame.width,
                            frame.height,
                            frame.rgba,
                        ));
                    }
                }
                Task::none()
            }

            Message::PollDevices => Task::perform(
                async { CameraManager::query_available_devices() },
                Message::DevicesPolled,
            ),

            Message::DevicesPolled(new_devices) => {
                if new_devices != self.available_devices {
                    let was_empty =
                        self.selected_device.is_none() || self.available_devices.is_empty();
                    self.available_devices = new_devices.clone();

                    if was_empty {
                        if let Some(first_dev) = self.available_devices.first().cloned() {
                            self.status_text = format!("Connected: {}", first_dev.name);
                            return self.update(Message::DeviceSelected(first_dev));
                        }
                    } else if let Some(ref current) = self.selected_device {
                        let still_exists =
                            self.available_devices.iter().any(|d| d.name == current.name);
                        if !still_exists {
                            if let Some(next_dev) = self.available_devices.first().cloned() {
                                self.status_text = format!("Connected: {}", next_dev.name);
                                return self.update(Message::DeviceSelected(next_dev));
                            } else {
                                self.selected_device = None;
                                self.camera_manager = None;
                                self.image_handle = None;
                                self.status_text = "Camera disconnected".to_string();
                            }
                        }
                    }
                }
                Task::none()
            }

            Message::KeyboardEvent(event) => {
                if let keyboard::Event::KeyPressed { key, .. } = event {
                    match key {
                        Key::Named(key::Named::Space) => return self.update(Message::ToggleFreeze),
                        Key::Named(key::Named::F11) => return self.update(Message::ToggleFullscreen),
                        Key::Named(key::Named::Escape) => {
                            if self.show_settings {
                                self.show_settings = false;
                            } else if self.is_fullscreen {
                                return self.update(Message::ToggleFullscreen);
                            }
                        }
                        Key::Character(ref ch) => {
                            if ch.eq_ignore_ascii_case("f") {
                                return self.update(Message::ToggleFlip);
                            } else if ch.eq_ignore_ascii_case("r") {
                                return self.update(Message::RotateNext);
                            } else if ch.eq_ignore_ascii_case("p") {
                                return self.update(Message::TogglePip);
                            } else if ch.eq_ignore_ascii_case("s") {
                                return self.update(Message::ToggleSettings);
                            } else if ch == "+" || ch == "=" {
                                return self.update(Message::ZoomIn);
                            } else if ch == "-" {
                                return self.update(Message::ZoomOut);
                            }
                        }
                        _ => {}
                    }
                }
                Task::none()
            }

            Message::DeviceSelected(device) => {
                self.selected_device = Some(device.clone());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchDevice(device.index));
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                } else {
                    let (mgr, _) = CameraManager::start(device.index, self.resolution);
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                    self.camera_manager = Some(mgr);
                }
                self.status_text = format!("Connected: {}", device.name);
                Task::none()
            }

            Message::ToggleResolution => {
                let next = match self.resolution {
                    VideoResolution::Hd720p => VideoResolution::Fhd1080p,
                    VideoResolution::Fhd1080p => VideoResolution::Hd720p,
                };
                self.update(Message::SetResolution(next))
            }

            Message::SetResolution(res) => {
                self.resolution = res;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchResolution(self.resolution));
                }
                Task::none()
            }

            Message::ToggleFlip => {
                self.is_flip = !self.is_flip;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                }
                Task::none()
            }

            Message::ToggleFreeze => {
                self.is_frozen = !self.is_frozen;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFreeze(self.is_frozen));
                }
                Task::none()
            }

            Message::ToggleFullscreen => {
                self.is_fullscreen = !self.is_fullscreen;
                let mode = if self.is_fullscreen {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                };
                window::latest().and_then(move |id| window::set_mode(id, mode))
            }

            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Task::none()
            }

            Message::RotateNext => {
                self.rotation_deg = match self.rotation_deg {
                    0 => 90,
                    90 => 180,
                    180 => 270,
                    _ => 0,
                };
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                }
                Task::none()
            }

            Message::ZoomIn => {
                if self.zoom_level < 2.5 {
                    self.zoom_level = (self.zoom_level + 0.5).min(2.5);
                    if let Some(ref mgr) = self.camera_manager {
                        mgr.send(CameraControl::SetZoom(self.zoom_level));
                    }
                }
                Task::none()
            }

            Message::ZoomOut => {
                if self.zoom_level > 1.0 {
                    self.zoom_level = (self.zoom_level - 0.5).max(1.0);
                    if let Some(ref mgr) = self.camera_manager {
                        mgr.send(CameraControl::SetZoom(self.zoom_level));
                    }
                }
                Task::none()
            }

            Message::TogglePip => {
                self.is_pip = !self.is_pip;
                self.pip_hovered = false;
                self.show_settings = false;

                if self.is_pip {
                    Task::batch(vec![
                        window::latest().and_then(|id| {
                            window::resize(id, iced::Size::new(384.0, 216.0))
                        }),
                        window::latest().and_then(|id| {
                            window::set_level(id, window::Level::AlwaysOnTop)
                        }),
                    ])
                } else {
                    Task::batch(vec![
                        window::latest().and_then(|id| {
                            window::resize(id, iced::Size::new(960.0, 600.0))
                        }),
                        window::latest().and_then(|id| {
                            window::set_level(id, window::Level::Normal)
                        }),
                    ])
                }
            }

            Message::PipMouseEnter => {
                self.pip_hovered = true;
                Task::none()
            }

            Message::PipMouseExit => {
                self.pip_hovered = false;
                Task::none()
            }

            Message::DragWindow => window::latest().and_then(window::drag),
            Message::ResizeWindow(direction) => {
                window::latest().and_then(move |id| window::drag_resize(id, direction))
            }
            Message::MinimizeWindow => window::latest().and_then(|id| window::minimize(id, true)),
            Message::CloseWindow => window::latest().and_then(window::close),

            Message::RefreshDevices => {
                self.available_devices = CameraManager::query_available_devices();
                if self.selected_device.is_none() && !self.available_devices.is_empty() {
                    let dev = self.available_devices[0].clone();
                    return self.update(Message::DeviceSelected(dev));
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = if self.is_pip {
            self.view_pip()
        } else {
            self.view_normal()
        };

        if self.is_fullscreen {
            content
        } else {
            with_resize_handles(content)
        }
    }

    /// Full Normal Mode with Custom Draggable Title Bar and Circuit Teaching Tools
    fn view_normal(&self) -> Element<'_, Message> {
        let app_logo = image(self.app_icon_handle.clone())
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0));

        // 1. Draggable Top Title Bar
        let title_bar = mouse_area(
            container(
                row![
                    app_logo,
                    text("AutoCam").size(13).color(theme::COLOR_TEXT),
                    text("Teaching").size(11).color(theme::COLOR_TEXT_MUTED),
                    Space::new().width(Length::Fixed(10.0)),
                    // Quick camera device selector
                    pick_list(
                        self.available_devices.clone(),
                        self.selected_device.clone(),
                        Message::DeviceSelected,
                    )
                    .text_size(12)
                    .padding([3, 8]),
                    // Resolution Toggle Button
                    tooltip(
                        button(text(self.resolution.label()).size(11))
                            .on_press(Message::ToggleResolution)
                            .style(theme::clean_button_style(false, false))
                            .padding([4, 8]),
                        container(text("Toggle 720p / 1080p").size(11))
                            .padding([3, 7])
                            .style(theme::tooltip_style),
                        tooltip::Position::Bottom,
                    ),
                    // Refresh Device Button
                    render_icon_btn(
                        icons::ICON_REFRESH,
                        "Refresh Devices",
                        Message::RefreshDevices,
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    Space::new().width(Fill),
                    // PiP Switcher Button
                    render_icon_btn(
                        icons::ICON_PIP,
                        "Picture in Picture [P]",
                        Message::TogglePip,
                        self.is_pip,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    // Fullscreen Button
                    render_icon_btn(
                        if self.is_fullscreen {
                            icons::ICON_MINIMIZE_2
                        } else {
                            icons::ICON_MAXIMIZE
                        },
                        if self.is_fullscreen {
                            "Exit Fullscreen [F11]"
                        } else {
                            "Fullscreen [F11]"
                        },
                        Message::ToggleFullscreen,
                        self.is_fullscreen,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    // Minimize and Close
                    render_icon_btn(
                        icons::ICON_MINUS,
                        "Minimize",
                        Message::MinimizeWindow,
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_CLOSE,
                        "Close",
                        Message::CloseWindow,
                        false,
                        true,
                        tooltip::Position::Bottom,
                    ),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .padding([6, 12]),
            )
            .style(theme::title_bar_style)
            .width(Fill),
        )
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleFullscreen);

        // 2. Video Viewport
        let video_content: Element<'_, Message> = if let Some(ref handle) = self.image_handle {
            image(handle)
                .content_fit(ContentFit::Contain)
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            container(
                column![
                    text("Waiting for camera feed...").size(14),
                    text("Plug in or select a camera to start streaming")
                        .size(12)
                        .color(theme::COLOR_TEXT_MUTED),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            )
            .center(Fill)
            .into()
        };

        let mut viewport_stack = vec![video_content];

        // If Settings view is open, show modal overlay centered over viewport
        if self.show_settings {
            let settings_modal = container(
                container(self.view_settings_panel())
                    .style(theme::settings_dialog_style)
                    .width(Length::Fixed(440.0)),
            )
            .center(Fill)
            .into();

            viewport_stack.push(settings_modal);
        }

        let video_viewport = container(stack(viewport_stack).width(Fill).height(Fill))
            .style(theme::video_viewport_style)
            .width(Fill)
            .height(Fill);

        // 3. Bottom Controls Toolbar: Icon-only buttons with hover tooltips
        let toolbar = container(
            row![
                // Mirror / Flip Icon Button
                render_icon_btn(
                    icons::ICON_FLIP,
                    "Mirror / Flip [F]",
                    Message::ToggleFlip,
                    self.is_flip,
                    false,
                    tooltip::Position::Top,
                ),
                // Freeze Frame Icon Button
                render_icon_btn(
                    if self.is_frozen {
                        icons::ICON_PLAY
                    } else {
                        icons::ICON_PAUSE
                    },
                    if self.is_frozen {
                        "Resume Live [Space]"
                    } else {
                        "Freeze Frame [Space]"
                    },
                    Message::ToggleFreeze,
                    self.is_frozen,
                    false,
                    tooltip::Position::Top,
                ),
                // Rotate Icon Button with Angle Badge
                tooltip(
                    button(
                        row![
                            render_svg(icons::ICON_ROTATE, 15.0, self.rotation_deg > 0, false),
                            text(format!("{}°", self.rotation_deg)).size(11),
                        ]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::RotateNext)
                    .style(theme::clean_button_style(self.rotation_deg > 0, false))
                    .padding([5, 8]),
                    container(text("Rotate 90° [R]").size(11))
                        .padding([3, 7])
                        .style(theme::tooltip_style),
                    tooltip::Position::Top,
                ),
                // Zoom Controls
                row![
                    render_icon_btn(
                        icons::ICON_ZOOM_OUT,
                        "Zoom Out [-]",
                        Message::ZoomOut,
                        false,
                        false,
                        tooltip::Position::Top,
                    ),
                    text(format!("{:.1}x", self.zoom_level))
                        .size(11)
                        .color(theme::COLOR_TEXT),
                    render_icon_btn(
                        icons::ICON_ZOOM_IN,
                        "Zoom In [=]",
                        Message::ZoomIn,
                        false,
                        false,
                        tooltip::Position::Top,
                    ),
                ]
                .spacing(3)
                .align_y(Alignment::Center),
                // Fullscreen Icon Button
                render_icon_btn(
                    if self.is_fullscreen {
                        icons::ICON_MINIMIZE_2
                    } else {
                        icons::ICON_MAXIMIZE
                    },
                    if self.is_fullscreen {
                        "Exit Fullscreen [F11]"
                    } else {
                        "Fullscreen [F11]"
                    },
                    Message::ToggleFullscreen,
                    self.is_fullscreen,
                    false,
                    tooltip::Position::Top,
                ),
                Space::new().width(Fill),
                // Status message
                text(&self.status_text)
                    .size(11)
                    .color(theme::COLOR_TEXT_MUTED),
                // Bottom-right Settings Gear Icon Button
                render_icon_btn(
                    icons::ICON_SETTINGS,
                    "Settings [S]",
                    Message::ToggleSettings,
                    self.show_settings,
                    false,
                    tooltip::Position::Top,
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([5, 10]),
        )
        .style(theme::controls_panel_style)
        .width(Fill);

        let content = column![title_bar, video_viewport, toolbar,]
            .width(Fill)
            .height(Fill);

        container(content)
            .style(theme::root_window_style)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// Settings modal panel
    fn view_settings_panel(&self) -> Element<'_, Message> {
        let header = row![
            render_svg(icons::ICON_SETTINGS, 16.0, true, false),
            text("Settings").size(14).color(theme::COLOR_TEXT),
            Space::new().width(Fill),
            render_icon_btn(
                icons::ICON_CLOSE,
                "Close Settings [Esc]",
                Message::ToggleSettings,
                false,
                true,
                tooltip::Position::Bottom,
            ),
        ]
        .align_y(Alignment::Center);

        let dev_section = column![
            text("Camera Device").size(11).color(theme::COLOR_TEXT_MUTED),
            pick_list(
                self.available_devices.clone(),
                self.selected_device.clone(),
                Message::DeviceSelected,
            )
            .text_size(12)
            .padding([4, 8])
            .width(Fill),
        ]
        .spacing(4);

        let res_section = column![
            text("Resolution").size(11).color(theme::COLOR_TEXT_MUTED),
            row![
                button(text("720p HD").size(11))
                    .on_press(Message::SetResolution(VideoResolution::Hd720p))
                    .style(theme::clean_button_style(
                        self.resolution == VideoResolution::Hd720p,
                        false
                    ))
                    .padding([4, 12]),
                button(text("1080p FHD").size(11))
                    .on_press(Message::SetResolution(VideoResolution::Fhd1080p))
                    .style(theme::clean_button_style(
                        self.resolution == VideoResolution::Fhd1080p,
                        false
                    ))
                    .padding([4, 12]),
            ]
            .spacing(8),
        ]
        .spacing(4);

        let shortcuts_guide = column![
            text("Shortcuts").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    shortcut_row("Space", "Freeze / Resume live feed"),
                    shortcut_row("F", "Mirror / Flip camera"),
                    shortcut_row("R", "Rotate 90 degrees"),
                    shortcut_row("P", "Toggle Picture-in-Picture"),
                    shortcut_row("F11", "Toggle Fullscreen"),
                    shortcut_row("+ / -", "Zoom in / Zoom out"),
                    shortcut_row("S", "Toggle Settings"),
                ]
                .spacing(5),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        let footer = row![
            text(format!("Engine: tiny-skia • Status: {}", self.status_text))
                .size(10)
                .color(theme::COLOR_TEXT_MUTED),
        ];

        column![
            header,
            Space::new().height(Length::Fixed(6.0)),
            dev_section,
            res_section,
            shortcuts_guide,
            Space::new().height(Length::Fixed(4.0)),
            footer,
        ]
        .spacing(10)
        .padding(16)
        .into()
    }

    /// Picture-in-Picture Mode: Compact, borderless, AlwaysOnTop.
    fn view_pip(&self) -> Element<'_, Message> {
        let video_content: Element<'_, Message> = if let Some(ref handle) = self.image_handle {
            image(handle)
                .content_fit(ContentFit::Contain)
                .width(Fill)
                .height(Fill)
                .into()
        } else {
            container(text("PiP Camera").size(12))
                .center(Fill)
                .into()
        };

        let pip_logo = image(self.app_icon_handle.clone())
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0));

        let mut layers: Vec<Element<'_, Message>> = vec![video_content];

        // Floating header appears on hover without displacing video feed
        if self.pip_hovered {
            let overlay_header: Element<'_, Message> = mouse_area(
                container(
                    row![
                        pip_logo,
                        text("AutoCam").size(11).color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        render_icon_btn(
                            if self.is_frozen {
                                icons::ICON_PLAY
                            } else {
                                icons::ICON_PAUSE
                            },
                            if self.is_frozen {
                                "Resume"
                            } else {
                                "Freeze"
                            },
                            Message::ToggleFreeze,
                            self.is_frozen,
                            false,
                            tooltip::Position::Bottom,
                        ),
                        render_icon_btn(
                            icons::ICON_FLIP,
                            "Flip",
                            Message::ToggleFlip,
                            self.is_flip,
                            false,
                            tooltip::Position::Bottom,
                        ),
                        render_icon_btn(
                            icons::ICON_PIP,
                            "Exit PiP",
                            Message::TogglePip,
                            false,
                            false,
                            tooltip::Position::Bottom,
                        ),
                        render_icon_btn(
                            icons::ICON_CLOSE,
                            "Close",
                            Message::CloseWindow,
                            false,
                            true,
                            tooltip::Position::Bottom,
                        ),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center)
                    .padding([3, 6]),
                )
                .style(theme::pip_overlay_title_bar_style)
                .width(Fill),
            )
            .on_press(Message::DragWindow)
            .into();

            layers.push(overlay_header);
        }

        let pip_stack = stack(layers).width(Fill).height(Fill);

        mouse_area(
            container(pip_stack)
                .style(theme::root_window_style)
                .width(Fill)
                .height(Fill),
        )
        .on_enter(Message::PipMouseEnter)
        .on_exit(Message::PipMouseExit)
        .into()
    }
}

/// Helper function to render a clean shortcut guide row
fn shortcut_row<'a>(key_name: &'static str, description: &'static str) -> Element<'a, Message> {
    row![
        container(text(key_name).size(10).color(theme::COLOR_TEXT))
            .padding([2, 5])
            .style(theme::tooltip_style),
        text(description).size(11).color(theme::COLOR_TEXT_MUTED),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Helper function to render SVG icon element
fn render_svg<'a>(
    svg_str: &'static str,
    size: f32,
    is_active: bool,
    is_danger: bool,
) -> Element<'a, Message> {
    let icon_color = if is_danger {
        theme::COLOR_DANGER
    } else if is_active {
        theme::COLOR_TEXT
    } else {
        theme::COLOR_TEXT_MUTED
    };

    svg(svg::Handle::from_memory(svg_str.as_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme, _status| svg::Style {
            color: Some(icon_color),
        })
        .into()
}

/// Helper function to create an icon button with hover tooltip
fn render_icon_btn<'a>(
    svg_str: &'static str,
    hint: &'static str,
    on_press: Message,
    is_active: bool,
    is_danger: bool,
    tooltip_pos: tooltip::Position,
) -> Element<'a, Message> {
    let icon_el = render_svg(svg_str, 15.0, is_active, is_danger);

    let btn = button(icon_el)
        .on_press(on_press)
        .style(theme::clean_button_style(is_active, is_danger))
        .padding([5, 7]);

    tooltip(
        btn,
        container(text(hint).size(11))
            .padding([3, 7])
            .style(theme::tooltip_style),
        tooltip_pos,
    )
    .into()
}

/// Wraps window content with non-blocking edge/corner resize zones for borderless window dragging
fn with_resize_handles<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    let edge_thickness = 6.0;
    let corner_size = 20.0;

    // Top edge (slim to keep titlebar drag smooth)
    let top = mouse_area(Space::new().width(Fill).height(Length::Fixed(4.0)))
        .interaction(mouse::Interaction::ResizingVertically)
        .on_press(Message::ResizeWindow(window::Direction::North));

    // Bottom edge
    let bottom = container(
        mouse_area(Space::new().width(Fill).height(Length::Fixed(edge_thickness)))
            .interaction(mouse::Interaction::ResizingVertically)
            .on_press(Message::ResizeWindow(window::Direction::South)),
    )
    .align_bottom(Fill)
    .width(Fill);

    // Left edge
    let left = mouse_area(Space::new().width(Length::Fixed(edge_thickness)).height(Fill))
        .interaction(mouse::Interaction::ResizingHorizontally)
        .on_press(Message::ResizeWindow(window::Direction::West));

    // Right edge
    let right = container(
        mouse_area(Space::new().width(Length::Fixed(edge_thickness)).height(Fill))
            .interaction(mouse::Interaction::ResizingHorizontally)
            .on_press(Message::ResizeWindow(window::Direction::East)),
    )
    .align_right(Fill)
    .height(Fill);

    // Top-Left corner
    let top_left = mouse_area(
        Space::new()
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0)),
    )
    .interaction(mouse::Interaction::ResizingDiagonallyDown)
    .on_press(Message::ResizeWindow(window::Direction::NorthWest));

    // Bottom-Left corner
    let bottom_left = container(
        mouse_area(
            Space::new()
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0)),
        )
        .interaction(mouse::Interaction::ResizingDiagonallyUp)
        .on_press(Message::ResizeWindow(window::Direction::SouthWest)),
    )
    .align_bottom(Fill);

    // Bottom-Right corner (Primary resize handle - generous hit area)
    let bottom_right = container(
        mouse_area(
            Space::new()
                .width(Length::Fixed(corner_size))
                .height(Length::Fixed(corner_size)),
        )
        .interaction(mouse::Interaction::ResizingDiagonallyDown)
        .on_press(Message::ResizeWindow(window::Direction::SouthEast)),
    )
    .align_right(Fill)
    .align_bottom(Fill);

    stack![
        content,
        top,
        bottom,
        left,
        right,
        top_left,
        bottom_left,
        bottom_right,
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn main() -> iced::Result {
    let icon = window::icon::from_file_data(APP_ICON_BYTES, None).ok();

    iced::application(AutoCamApp::new, AutoCamApp::update, AutoCamApp::view)
        .subscription(AutoCamApp::subscription)
        .window(window::Settings {
            icon,
            decorations: false,
            transparent: true,
            size: iced::Size::new(960.0, 600.0),
            resizable: true,
            ..Default::default()
        })
        .run()
}
