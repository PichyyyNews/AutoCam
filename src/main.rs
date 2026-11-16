mod camera;
mod icons;
mod theme;

use camera::{
    AntiFlickerMode, CameraControl, CameraDeviceInfo, CameraManager, FramePayload, VideoResolution,
};
use iced::keyboard::{self, key, Key};
use iced::mouse;
use iced::widget::{
    button, column, container, image, mouse_area, pick_list, row, slider, stack, svg, text, tooltip,
    Space,
};
use iced::window;
use iced::{Alignment, Background, Color, ContentFit, Element, Fill, Length, Subscription, Task};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const APP_ICON_BYTES: &[u8] = include_bytes!("../public/icon.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    Off,
    RuleOfThirds,
    CenterCrosshair,
}

impl GridMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Off => Self::RuleOfThirds,
            Self::RuleOfThirds => Self::CenterCrosshair,
            Self::CenterCrosshair => Self::Off,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Grid: Off [G]",
            Self::RuleOfThirds => "Grid: 3x3 Rule of Thirds [G]",
            Self::CenterCrosshair => "Grid: Center Crosshair [G]",
        }
    }
}

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

    // Camera Engine Controls
    grid_mode: GridMode,
    is_loading_camera: bool,
    auto_focus: bool,
    manual_focus: u32,
    auto_ev: bool,
    ev_bias: f32,
    brightness: i32,
    contrast: f32,
    focus_peaking: bool,
    anti_flicker: AntiFlickerMode,
    latest_frame: Option<FramePayload>,
    snapshot_toast: Option<(String, Instant)>,
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

    // Advanced Camera Engine Messages
    CycleGridMode,
    ToggleAutoFocus,
    SetManualFocus(u32),
    ToggleAutoEv,
    SetEvBias(f32),
    SetBrightness(i32),
    SetContrast(f32),
    ToggleFocusPeaking,
    SetAntiFlicker(AntiFlickerMode),
    CaptureSnapshot,
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

                grid_mode: GridMode::Off,
                is_loading_camera: false,
                auto_focus: true,
                manual_focus: 50,
                auto_ev: true,
                ev_bias: 0.0,
                brightness: 0,
                contrast: 1.0,
                focus_peaking: false,
                anti_flicker: AntiFlickerMode::Off,
                latest_frame: None,
                snapshot_toast: None,
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

    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                if let Some(ref mgr) = self.camera_manager {
                    if let Some(frame) = mgr.take_latest_frame() {
                        self.frame_count += 1;
                        self.is_loading_camera = false; // Transition smoothly from loader to live
                        self.image_handle = Some(image::Handle::from_rgba(
                            frame.width,
                            frame.height,
                            frame.rgba.clone(),
                        ));
                        self.latest_frame = Some(frame);
                    }
                }
                // Check if snapshot toast expired (after 3 seconds)
                if let Some((_, created_at)) = self.snapshot_toast {
                    if created_at.elapsed() > Duration::from_secs(3) {
                        self.snapshot_toast = None;
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
                        Key::Character(c) if c.eq_ignore_ascii_case("f") => {
                            return self.update(Message::ToggleFlip)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                            return self.update(Message::RotateNext)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("p") => {
                            return self.update(Message::TogglePip)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                            return self.update(Message::ToggleSettings)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("g") => {
                            return self.update(Message::CycleGridMode)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                            return self.update(Message::ToggleAutoFocus)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                            return self.update(Message::ToggleAutoEv)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                            return self.update(Message::CaptureSnapshot)
                        }
                        Key::Character(c) if c == "-" || c == "_" => {
                            return self.update(Message::ZoomOut)
                        }
                        Key::Character(c) if c == "=" || c == "+" => {
                            return self.update(Message::ZoomIn)
                        }
                        Key::Named(key::Named::F11) => {
                            return self.update(Message::ToggleFullscreen)
                        }
                        _ => {}
                    }
                }
                Task::none()
            }

            Message::DeviceSelected(dev) => {
                self.selected_device = Some(dev.clone());
                self.status_text = format!("Connected: {}", dev.name);
                self.is_loading_camera = true; // Show smooth freeze-to-live spinner

                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchDevice(dev.index));
                } else {
                    let (mgr, _) = CameraManager::start(dev.index, self.resolution);
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                    mgr.send(CameraControl::SetAutoEv(self.auto_ev));
                    mgr.send(CameraControl::SetEvBias(self.ev_bias));
                    mgr.send(CameraControl::SetBrightness(self.brightness));
                    mgr.send(CameraControl::SetContrast(self.contrast));
                    mgr.send(CameraControl::SetFocusPeaking(self.focus_peaking));
                    self.camera_manager = Some(mgr);
                }
                Task::none()
            }

            Message::ToggleResolution => {
                let next_res = match self.resolution {
                    VideoResolution::Hd720p => VideoResolution::Fhd1080p,
                    VideoResolution::Fhd1080p => VideoResolution::Hd720p,
                };
                self.update(Message::SetResolution(next_res))
            }

            Message::SetResolution(res) => {
                self.resolution = res;
                self.is_loading_camera = true;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchResolution(res));
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
                self.zoom_level = (self.zoom_level + 0.25).min(3.0);
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                }
                Task::none()
            }

            Message::ZoomOut => {
                self.zoom_level = (self.zoom_level - 0.25).max(1.0);
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                }
                Task::none()
            }

            Message::CycleGridMode => {
                self.grid_mode = self.grid_mode.next();
                Task::none()
            }

            Message::ToggleAutoFocus => {
                self.auto_focus = !self.auto_focus;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAutoFocus(self.auto_focus));
                }
                Task::none()
            }

            Message::SetManualFocus(val) => {
                self.manual_focus = val;
                self.auto_focus = false;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetManualFocus(val));
                }
                Task::none()
            }

            Message::ToggleAutoEv => {
                self.auto_ev = !self.auto_ev;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAutoEv(self.auto_ev));
                }
                Task::none()
            }

            Message::SetEvBias(val) => {
                self.ev_bias = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetEvBias(val));
                }
                Task::none()
            }

            Message::SetBrightness(val) => {
                self.brightness = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetBrightness(val));
                }
                Task::none()
            }

            Message::SetContrast(val) => {
                self.contrast = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetContrast(val));
                }
                Task::none()
            }

            Message::ToggleFocusPeaking => {
                self.focus_peaking = !self.focus_peaking;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFocusPeaking(self.focus_peaking));
                }
                Task::none()
            }

            Message::SetAntiFlicker(mode) => {
                self.anti_flicker = mode;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAntiFlicker(mode));
                }
                Task::none()
            }

            Message::CaptureSnapshot => {
                if let Some(ref frame) = self.latest_frame {
                    let _ = std::fs::create_dir_all("captures");
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let filename = format!("captures/autocam_{}.png", timestamp);

                    if ::image::save_buffer(
                        &filename,
                        &frame.rgba,
                        frame.width,
                        frame.height,
                        ::image::ExtendedColorType::Rgba8,
                    )
                    .is_ok()
                    {
                        self.snapshot_toast = Some((filename, Instant::now()));
                    }
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

            Message::TogglePip => {
                self.is_pip = !self.is_pip;
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
            .width(Length::Fixed(23.0))
            .height(Length::Fixed(23.0));

        // 1. Draggable Top Title Bar
        let title_bar = mouse_area(
            container(
                row![
                    app_logo,
                    text("AutoCam").size(13).color(theme::COLOR_TEXT),
                    Space::new().width(Length::Fixed(10.0)),
                    // Quick camera device selector
                    pick_list(
                        self.available_devices.clone(),
                        self.selected_device.clone(),
                        Message::DeviceSelected,
                    )
                    .style(theme::pick_list_style)
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

        // 2a. Grid Overlay for Circuit Alignment / Framing
        match self.grid_mode {
            GridMode::Off => {}
            GridMode::RuleOfThirds => {
                let grid_lines = container(
                    stack![
                        // Horizontal 3x3 lines
                        column![
                            Space::new().height(Fill),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().height(Fill),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().height(Fill),
                        ]
                        .width(Fill)
                        .height(Fill),
                        // Vertical 3x3 lines
                        row![
                            Space::new().width(Fill),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().width(Fill),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().width(Fill),
                        ]
                        .width(Fill)
                        .height(Fill),
                    ]
                    .width(Fill)
                    .height(Fill),
                )
                .width(Fill)
                .height(Fill)
                .into();

                viewport_stack.push(grid_lines);
            }
            GridMode::CenterCrosshair => {
                let crosshair = container(
                    stack![
                        // Center horizontal line
                        column![
                            Space::new().height(Fill),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.28))),
                                    ..Default::default()
                                }),
                            Space::new().height(Fill),
                        ]
                        .width(Fill)
                        .height(Fill),
                        // Center vertical line
                        row![
                            Space::new().width(Fill),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.28))),
                                    ..Default::default()
                                }),
                            Space::new().width(Fill),
                        ]
                        .width(Fill)
                        .height(Fill),
                    ]
                    .width(Fill)
                    .height(Fill),
                )
                .center(Fill)
                .into();

                viewport_stack.push(crosshair);
            }
        }

        // 2b. Freeze-to-Live Micro Loading Overlay
        if self.is_loading_camera {
            let loader = container(
                container(
                    row![
                        render_svg(icons::ICON_SPINNER, 16.0, true, false),
                        text("Connecting camera...").size(12).color(theme::COLOR_TEXT),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(theme::tooltip_style),
            )
            .center(Fill)
            .into();

            viewport_stack.push(loader);
        }

        // 2c. Snapshot Toast Notification
        if let Some((ref filename, _)) = self.snapshot_toast {
            let toast = container(
                container(
                    row![
                        render_svg(icons::ICON_CAMERA, 14.0, true, false),
                        text(format!("Snapshot saved to {}", filename))
                            .size(11)
                            .color(theme::COLOR_TEXT),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(theme::tooltip_style),
            )
            .align_x(Alignment::Center)
            .padding([12, 0])
            .into();

            viewport_stack.push(toast);
        }

        // 2d. Settings Modal
        if self.show_settings {
            let settings_modal = container(
                container(self.view_settings_panel())
                    .style(theme::settings_dialog_style)
                    .width(Length::Fixed(460.0)),
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
                // Mirror / Flip
                render_icon_btn(
                    icons::ICON_FLIP,
                    "Mirror / Flip [F]",
                    Message::ToggleFlip,
                    self.is_flip,
                    false,
                    tooltip::Position::Top,
                ),
                // Freeze Frame
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
                // Auto Focus Quick Toggle
                render_icon_btn(
                    icons::ICON_FOCUS,
                    if self.auto_focus {
                        "Auto Focus: ON [A]"
                    } else {
                        "Auto Focus: OFF [A]"
                    },
                    Message::ToggleAutoFocus,
                    self.auto_focus,
                    false,
                    tooltip::Position::Top,
                ),
                // Auto EV Quick Toggle
                render_icon_btn(
                    icons::ICON_EV,
                    if self.auto_ev {
                        "Auto EV: ON [E]"
                    } else {
                        "Auto EV: OFF [E]"
                    },
                    Message::ToggleAutoEv,
                    self.auto_ev,
                    false,
                    tooltip::Position::Top,
                ),
                // Grid Alignment Overlay
                render_icon_btn(
                    icons::ICON_GRID,
                    self.grid_mode.label(),
                    Message::CycleGridMode,
                    self.grid_mode != GridMode::Off,
                    false,
                    tooltip::Position::Top,
                ),
                // Snapshot Still Capture
                render_icon_btn(
                    icons::ICON_CAMERA,
                    "Take Snapshot [C]",
                    Message::CaptureSnapshot,
                    false,
                    false,
                    tooltip::Position::Top,
                ),
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
                // Bottom-right Settings Gear Icon Button (Zero clutter, no text)
                render_icon_btn(
                    icons::ICON_SETTINGS,
                    "Settings [S]",
                    Message::ToggleSettings,
                    self.show_settings,
                    false,
                    tooltip::Position::Top,
                ),
            ]
            .spacing(6)
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

    /// Settings Modal View: Complete Camera Engine & Hardware Driver Controls
    fn view_settings_panel(&self) -> Element<'_, Message> {
        let header = row![
            render_svg(icons::ICON_SETTINGS, 16.0, true, false),
            text("Camera Engine & Settings")
                .size(13)
                .color(theme::COLOR_TEXT),
            Space::new().width(Fill),
            render_icon_btn(
                icons::ICON_CLOSE,
                "Close Settings",
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
            .style(theme::pick_list_style)
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

        // Hardware & Software Camera Tuning
        let tuning_section = column![
            text("Software Driver & DSP Tuning")
                .size(11)
                .color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    // Focus Peaking
                    row![
                        render_svg(icons::ICON_PEAKING, 14.0, self.focus_peaking, false),
                        text("Focus Peaking (Highlight in-focus PCB traces)")
                            .size(11)
                            .color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text(if self.focus_peaking { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleFocusPeaking)
                            .style(theme::clean_button_style(self.focus_peaking, false))
                            .padding([2, 8]),
                    ]
                    .align_y(Alignment::Center),
                    // EV Bias Slider
                    row![
                        text("EV Bias:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{:.1} EV", self.ev_bias))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(-20..=20, (self.ev_bias * 10.0) as i32, |val| {
                            Message::SetEvBias(val as f32 / 10.0)
                        })
                        .width(Length::Fixed(140.0)),
                    ]
                    .align_y(Alignment::Center),
                    // Brightness Slider
                    row![
                        text("Brightness:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{}", self.brightness))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(-50..=50, self.brightness, Message::SetBrightness)
                            .width(Length::Fixed(140.0)),
                    ]
                    .align_y(Alignment::Center),
                    // Contrast Slider
                    row![
                        text("Contrast:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{:.1}x", self.contrast))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(5..=20, (self.contrast * 10.0) as i32, |val| {
                            Message::SetContrast(val as f32 / 10.0)
                        })
                        .width(Length::Fixed(140.0)),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        let shortcuts_guide = column![
            text("Shortcuts").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    shortcut_row("Space", "Freeze / Resume live feed"),
                    shortcut_row("F", "Mirror / Flip camera"),
                    shortcut_row("R", "Rotate 90 degrees"),
                    shortcut_row("G", "Cycle Grid (Off -> 3x3 -> Crosshair)"),
                    shortcut_row("A", "Toggle Auto Focus"),
                    shortcut_row("E", "Toggle Auto EV (Exposure)"),
                    shortcut_row("C", "Capture high-res snapshot to captures/"),
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
            tuning_section,
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

        if self.pip_hovered {
            let overlay_header = mouse_area(
                container(
                    row![
                        pip_logo,
                        text("AutoCam").size(11).color(theme::COLOR_TEXT),
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
    position: tooltip::Position,
) -> Element<'a, Message> {
    let btn = button(render_svg(svg_str, 15.0, is_active, is_danger))
        .on_press(on_press)
        .style(theme::clean_button_style(is_active, is_danger))
        .padding([5, 7]);

    tooltip(
        btn,
        container(text(hint).size(11))
            .padding([3, 7])
            .style(theme::tooltip_style),
        position,
    )
    .into()
}

/// Borderless Window Responsive Resize Handles
fn with_resize_handles<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    let border_thickness = 4.0;
    let corner_size = 20.0;

    // Top border
    let top = mouse_area(
        container(Space::new().width(Fill).height(Length::Fixed(border_thickness)))
            .width(Fill),
    )
    .interaction(mouse::Interaction::ResizingVertically)
    .on_press(Message::ResizeWindow(window::Direction::North));

    // Bottom border
    let bottom = container(
        mouse_area(
            container(Space::new().width(Fill).height(Length::Fixed(border_thickness)))
                .width(Fill),
        )
        .interaction(mouse::Interaction::ResizingVertically)
        .on_press(Message::ResizeWindow(window::Direction::South)),
    )
    .align_bottom(Fill);

    // Left border
    let left = mouse_area(
        container(Space::new().width(Length::Fixed(border_thickness)).height(Fill))
            .height(Fill),
    )
    .interaction(mouse::Interaction::ResizingHorizontally)
    .on_press(Message::ResizeWindow(window::Direction::West));

    // Right border
    let right = container(
        mouse_area(
            container(Space::new().width(Length::Fixed(border_thickness)).height(Fill))
                .height(Fill),
        )
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
        .theme(AutoCamApp::theme)
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
