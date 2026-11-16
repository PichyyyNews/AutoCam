mod camera;
mod theme;

use camera::{CameraControl, CameraDeviceInfo, CameraManager, VideoResolution};
use iced::keyboard::{self, key, Key};
use iced::mouse;
use iced::widget::{
    button, column, container, image, mouse_area, pick_list, row, stack, text, Space,
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
    ToggleFlip,
    ToggleFreeze,
    ToggleFullscreen,
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

        let status_text = if selected_device.is_some() {
            "Camera stream active".to_string()
        } else {
            "No camera detected - connect camera to start".to_string()
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
                        // Device hot-plugged while no camera was active! Auto-connect without restart
                        if let Some(first_dev) = self.available_devices.first().cloned() {
                            self.status_text = format!("Camera connected: {}", first_dev.name);
                            return self.update(Message::DeviceSelected(first_dev));
                        }
                    } else if let Some(ref current) = self.selected_device {
                        let still_exists =
                            self.available_devices.iter().any(|d| d.name == current.name);
                        if !still_exists {
                            // Currently selected camera was unplugged!
                            if let Some(next_dev) = self.available_devices.first().cloned() {
                                self.status_text = format!("Switched to {}", next_dev.name);
                                return self.update(Message::DeviceSelected(next_dev));
                            } else {
                                self.selected_device = None;
                                self.camera_manager = None;
                                self.image_handle = None;
                                self.status_text =
                                    "Camera disconnected - waiting for device...".to_string();
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
                        Key::Character(ref ch) => {
                            if ch.eq_ignore_ascii_case("f") {
                                return self.update(Message::ToggleFlip);
                            } else if ch.eq_ignore_ascii_case("r") {
                                return self.update(Message::RotateNext);
                            } else if ch.eq_ignore_ascii_case("p") {
                                return self.update(Message::TogglePip);
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
                self.resolution = match self.resolution {
                    VideoResolution::Hd720p => VideoResolution::Fhd1080p,
                    VideoResolution::Fhd1080p => VideoResolution::Hd720p,
                };
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchResolution(self.resolution));
                }
                self.status_text = format!("Resolution: {}", self.resolution.label());
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
                self.status_text = if self.is_frozen {
                    "Frame Frozen [Space to Resume]".to_string()
                } else {
                    "Live Streaming".to_string()
                };
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

                if self.is_pip {
                    // Enter compact floating PiP (16:9 ratio, AlwaysOnTop)
                    Task::batch(vec![
                        window::latest().and_then(|id| {
                            window::resize(id, iced::Size::new(384.0, 216.0))
                        }),
                        window::latest().and_then(|id| {
                            window::set_level(id, window::Level::AlwaysOnTop)
                        }),
                    ])
                } else {
                    // Restore standard window
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

        // 1. Draggable Top Title Bar (Flush with window edges, strict monochrome)
        let title_bar = mouse_area(
            container(
                row![
                    app_logo,
                    text("AutoCam").size(13).color(theme::COLOR_TEXT),
                    text("Teaching").size(11).color(theme::COLOR_TEXT_MUTED),
                    Space::new().width(Length::Fixed(10.0)),
                    // Camera device picker
                    pick_list(
                        self.available_devices.clone(),
                        self.selected_device.clone(),
                        Message::DeviceSelected,
                    )
                    .text_size(12)
                    .padding([3, 8]),
                    // Resolution Toggle
                    button(text(self.resolution.label()).size(11))
                        .on_press(Message::ToggleResolution)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 10]),
                    // Refresh Device Button (Icon-focused)
                    button(text("⟳").size(12))
                        .on_press(Message::RefreshDevices)
                        .style(theme::clean_button_style(false, false))
                        .padding([3, 8]),
                    Space::new().width(Fill),
                    // PiP Switcher Button
                    button(text("◱ PiP [P]").size(11))
                        .on_press(Message::TogglePip)
                        .style(theme::clean_button_style(self.is_pip, false))
                        .padding([4, 10]),
                    // Fullscreen Button
                    button(text(if self.is_fullscreen { "🗗" } else { "⛶" }).size(12))
                        .on_press(Message::ToggleFullscreen)
                        .style(theme::clean_button_style(self.is_fullscreen, false))
                        .padding([3, 8]),
                    // Minimize and Close
                    button(text("–").size(12))
                        .on_press(Message::MinimizeWindow)
                        .style(theme::clean_button_style(false, false))
                        .padding([3, 8]),
                    button(text("✕").size(12))
                        .on_press(Message::CloseWindow)
                        .style(theme::clean_button_style(false, true))
                        .padding([3, 8]),
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
                    text("Plug in or select camera to start streaming")
                        .size(12)
                        .color(theme::COLOR_TEXT_MUTED),
                ]
                .spacing(6)
                .align_x(Alignment::Center),
            )
            .center(Fill)
            .into()
        };

        let video_viewport = container(video_content)
            .style(theme::video_viewport_style)
            .width(Fill)
            .height(Fill);

        // 3. Circuit Teaching Controls Toolbar (Icon-focused, strict monochrome)
        let toolbar = container(
            row![
                // Flip / Mirror
                button(text("⇄ Mirror [F]").size(12))
                    .on_press(Message::ToggleFlip)
                    .style(theme::clean_button_style(self.is_flip, false))
                    .padding([5, 12]),
                // Freeze Frame
                button(text(if self.is_frozen { "▶ Resume [Space]" } else { "⏸ Freeze [Space]" }).size(12))
                    .on_press(Message::ToggleFreeze)
                    .style(theme::clean_button_style(self.is_frozen, false))
                    .padding([5, 12]),
                // Rotate
                button(text(format!("↻ {}° [R]", self.rotation_deg)).size(12))
                    .on_press(Message::RotateNext)
                    .style(theme::clean_button_style(self.rotation_deg > 0, false))
                    .padding([5, 12]),
                // Zoom
                row![
                    button(text("−").size(12))
                        .on_press(Message::ZoomOut)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 8]),
                    text(format!("{:.1}x", self.zoom_level))
                        .size(12)
                        .color(theme::COLOR_TEXT),
                    button(text("+").size(12))
                        .on_press(Message::ZoomIn)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 8]),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                // Fullscreen toggle in toolbar
                button(text(if self.is_fullscreen { "🗗 Exit Fullscreen [F11]" } else { "⛶ Fullscreen [F11]" }).size(12))
                    .on_press(Message::ToggleFullscreen)
                    .style(theme::clean_button_style(self.is_fullscreen, false))
                    .padding([5, 12]),
                Space::new().width(Fill),
                // Status message
                text(&self.status_text)
                    .size(11)
                    .color(theme::COLOR_TEXT_MUTED),
                // Corner resize affordance indicator
                text("◿").size(10).color(theme::COLOR_BORDER_ACTIVE),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding([6, 12]),
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

    /// Picture-in-Picture Mode: Compact, borderless, AlwaysOnTop.
    /// Uses stack layout so hovering overlay controls NEVER shrinks or shifts the video feed.
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

        // Overlay header floats on top of the video without modifying video geometry
        if self.pip_hovered {
            let overlay_header: Element<'_, Message> = mouse_area(
                container(
                    row![
                        pip_logo,
                        text("AutoCam").size(11).color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        button(text(if self.is_frozen { "▶" } else { "⏸" }).size(11))
                            .on_press(Message::ToggleFreeze)
                            .style(theme::clean_button_style(self.is_frozen, false))
                            .padding([2, 6]),
                        button(text("⇄").size(11))
                            .on_press(Message::ToggleFlip)
                            .style(theme::clean_button_style(self.is_flip, false))
                            .padding([2, 6]),
                        button(text("Exit PiP").size(11))
                            .on_press(Message::TogglePip)
                            .style(theme::clean_button_style(false, false))
                            .padding([2, 8]),
                        button(text("✕").size(11))
                            .on_press(Message::CloseWindow)
                            .style(theme::clean_button_style(false, true))
                            .padding([2, 6]),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center)
                    .padding([4, 8]),
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
