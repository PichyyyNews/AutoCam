mod camera;
mod theme;

use camera::{CameraControl, CameraDeviceInfo, CameraManager, VideoResolution};
use iced::keyboard::{self, key, Key};
use iced::widget::{button, column, container, image, mouse_area, pick_list, row, text, Space};
use iced::window;
use iced::{Alignment, ContentFit, Element, Fill, Length, Subscription, Task};

pub struct AutoCamApp {
    camera_manager: Option<CameraManager>,
    available_devices: Vec<CameraDeviceInfo>,
    selected_device: Option<CameraDeviceInfo>,
    resolution: VideoResolution,
    is_pip: bool,
    pip_hovered: bool,
    is_flip: bool,
    is_frozen: bool,
    rotation_deg: u16,
    zoom_level: f32,
    image_handle: Option<image::Handle>,
    frame_count: u64,
    status_text: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    KeyboardEvent(keyboard::Event),
    DeviceSelected(CameraDeviceInfo),
    ToggleResolution,
    ToggleFlip,
    ToggleFreeze,
    RotateNext,
    ZoomIn,
    ZoomOut,
    TogglePip,
    PipMouseEnter,
    PipMouseExit,
    DragWindow,
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
            Some(mgr)
        } else {
            None
        };

        let status_text = if selected_device.is_some() {
            "Camera stream active".to_string()
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
                is_flip: false,
                is_frozen: false,
                rotation_deg: 0,
                zoom_level: 1.0,
                image_handle: None,
                frame_count: 0,
                status_text,
            },
            Task::none(),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            window::frames().map(|_| Message::Tick),
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

            Message::KeyboardEvent(event) => {
                if let keyboard::Event::KeyPressed { key, .. } = event {
                    match key {
                        Key::Named(key::Named::Space) => return self.update(Message::ToggleFreeze),
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
                } else {
                    let (mgr, _) = CameraManager::start(device.index, self.resolution);
                    self.camera_manager = Some(mgr);
                }
                self.status_text = format!("Switched to {}", device.name);
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
                self.status_text = format!("Resolution set to {}", self.resolution.label());
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
                    "Frame Frozen [Press Space to Resume]".to_string()
                } else {
                    "Live Streaming".to_string()
                };
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
        if self.is_pip {
            self.view_pip()
        } else {
            self.view_normal()
        }
    }

    /// Full Normal Mode with Custom Draggable Title Bar and Circuit Teaching Tools
    fn view_normal(&self) -> Element<'_, Message> {
        // 1. Draggable Top Title Bar
        let title_bar = mouse_area(
            container(
                row![
                    text("AutoCam").size(14),
                    text("Teaching").size(11).color(theme::COLOR_TEXT_MUTED),
                    Space::new().width(Length::Fixed(12.0)),
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
                    // Refresh Device Button
                    button(text("Refresh").size(11))
                        .on_press(Message::RefreshDevices)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 10]),
                    Space::new().width(Fill),
                    // PiP Switcher Button
                    button(text("PiP [P]").size(11))
                        .on_press(Message::TogglePip)
                        .style(theme::clean_button_style(true, false))
                        .padding([4, 12]),
                    // Minimize and Close
                    button(text("–").size(13))
                        .on_press(Message::MinimizeWindow)
                        .style(theme::clean_button_style(false, false))
                        .padding([2, 8]),
                    button(text("✕").size(13))
                        .on_press(Message::CloseWindow)
                        .style(theme::clean_button_style(false, true))
                        .padding([2, 8]),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([6, 12]),
            )
            .style(theme::title_bar_style)
            .width(Fill),
        )
        .on_press(Message::DragWindow);

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
                    text("Ensure camera is connected and not used by another app")
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
            .height(Fill)
            .padding(1);

        // 3. Circuit Teaching Controls Toolbar
        let toolbar = container(
            row![
                // Flip / Mirror
                button(text(if self.is_flip { "Flip: On [F]" } else { "Flip [F]" }).size(12))
                    .on_press(Message::ToggleFlip)
                    .style(theme::clean_button_style(self.is_flip, false))
                    .padding([6, 14]),
                // Freeze Frame
                button(text(if self.is_frozen { "Resume [Space]" } else { "Freeze [Space]" }).size(12))
                    .on_press(Message::ToggleFreeze)
                    .style(theme::clean_button_style(self.is_frozen, false))
                    .padding([6, 14]),
                // Rotate
                button(text(format!("Rotate: {}° [R]", self.rotation_deg)).size(12))
                    .on_press(Message::RotateNext)
                    .style(theme::clean_button_style(self.rotation_deg > 0, false))
                    .padding([6, 14]),
                // Zoom
                row![
                    button(text("–").size(12))
                        .on_press(Message::ZoomOut)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 10]),
                    text(format!("{:.1}x", self.zoom_level))
                        .size(12)
                        .color(theme::COLOR_TEXT),
                    button(text("+").size(12))
                        .on_press(Message::ZoomIn)
                        .style(theme::clean_button_style(false, false))
                        .padding([4, 10]),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                Space::new().width(Fill),
                // Status message
                text(&self.status_text)
                    .size(11)
                    .color(theme::COLOR_TEXT_MUTED),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([8, 14]),
        )
        .style(theme::controls_panel_style)
        .width(Fill);

        let content = column![title_bar, video_viewport, toolbar,]
            .spacing(10)
            .padding(10);

        container(content)
            .style(theme::root_window_style)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// Picture-in-Picture Mode: Compact, borderless, AlwaysOnTop, auto-hide overlay controls
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

        let overlay_header: Element<'_, Message> = if self.pip_hovered {
            mouse_area(
                container(
                    row![
                        text("AutoCam").size(11),
                        Space::new().width(Fill),
                        button(text(if self.is_frozen { "▶" } else { "❚❚" }).size(11))
                            .on_press(Message::ToggleFreeze)
                            .style(theme::clean_button_style(self.is_frozen, false))
                            .padding([2, 6]),
                        button(text("Exit PiP").size(11))
                            .on_press(Message::TogglePip)
                            .style(theme::clean_button_style(true, false))
                            .padding([2, 8]),
                        button(text("✕").size(11))
                            .on_press(Message::CloseWindow)
                            .style(theme::clean_button_style(false, true))
                            .padding([2, 6]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .padding([4, 8]),
                )
                .style(theme::pip_overlay_title_bar_style)
                .width(Fill),
            )
            .on_press(Message::DragWindow)
            .into()
        } else {
            Space::new().height(Length::Shrink).into()
        };

        let pip_stack = column![overlay_header, video_content,].width(Fill).height(Fill);

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

fn main() -> iced::Result {
    iced::application(AutoCamApp::new, AutoCamApp::update, AutoCamApp::view)
        .subscription(AutoCamApp::subscription)
        .window(window::Settings {
            decorations: false,
            transparent: false,
            size: iced::Size::new(960.0, 600.0),
            resizable: true,
            ..Default::default()
        })
        .run()
}
