#![windows_subsystem = "windows"]

mod camera;
mod face_detect;
mod icons;
mod theme;

use camera::{
    AntiFlickerMode, CameraControl, CameraDeviceInfo, CameraManager, FramePayload, VideoResolution,
};
use iced::futures::sink::SinkExt;
use iced::keyboard::{self, key, Key};
use iced::mouse;
use iced::stream;
use iced::widget::{
    button, column, container, image, mouse_area, pick_list, row, scrollable, slider, stack, svg, text, tooltip,
    Space,
};
use iced::window;
use iced::{
    exit, Alignment, Background, Border, Color, ContentFit, Element, Fill, Length, Subscription, Task,
    Theme,
};
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
    main_window_id: Option<window::Id>,
    settings_window_id: Option<window::Id>,

    camera_manager: Option<CameraManager>,
    available_devices: Vec<CameraDeviceInfo>,
    selected_device: Option<CameraDeviceInfo>,
    supported_resolutions: Vec<VideoResolution>,
    resolution: VideoResolution,
    is_flip: bool,
    is_frozen: bool,
    is_fullscreen: bool,
    rotation_deg: u16,
    zoom_level: f32,
    image_handle: Option<image::Handle>,
    app_icon_handle: image::Handle,
    frame_count: u64,
    status_text: String,

    // Detailed Camera Engine Controls
    grid_mode: GridMode,
    is_loading_camera: bool,
    action_shutter: bool,
    auto_focus: bool,
    manual_focus: u32,
    auto_ev: bool,
    ev_bias: f32,
    brightness: i32,
    contrast: f32,
    saturation: f32,
    focus_peaking: bool,
    peaking_threshold: i32,
    cas_sharpen: bool,
    cas_intensity: f32,
    frame_interpolation: bool,
    face_detection: bool,
    anti_flicker: AntiFlickerMode,
    latest_frame: Option<FramePayload>,
    snapshot_toast: Option<(String, Instant)>,

    // PiP state
    is_pip: bool,
    pip_hovered: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    FrameReceived(FramePayload),
    ClearSnapshotToast,
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
    OpenSettingsWindow,
    CloseSettingsWindow,
    MainWindowOpened(window::Id),
    SettingsWindowOpened(window::Id),
    RotateNext,
    SetRotation(u16),
    ZoomIn,
    ZoomOut,
    SetZoom(f32),
    TogglePip,
    PipMouseEnter,
    PipMouseExit,
    DragWindow(window::Id),
    ResizeWindow(window::Id, window::Direction),
    MinimizeWindow(window::Id),
    CloseApp,
    RefreshDevices,

    // Detailed Camera Engine Messages
    CycleGridMode,
    SetGridMode(GridMode),
    ToggleActionShutter,
    ToggleAutoFocus,
    SetManualFocus(u32),
    ToggleAutoEv,
    SetEvBias(f32),
    SetBrightness(i32),
    SetContrast(f32),
    SetSaturation(f32),
    ToggleFocusPeaking,
    SetPeakingThreshold(i32),
    ToggleCasSharpen,
    SetCasIntensity(f32),
    ToggleFrameInterpolation,
    ToggleFaceDetection,
    SetAntiFlicker(AntiFlickerMode),
    CaptureSnapshot,
    OpenCapturesFolder,
    ResetDefaults,
}

fn camera_frame_stream() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(1, |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
        loop {
            if let Some(mut rx) = camera::get_global_frame_rx() {
                while rx.changed().await.is_ok() {
                    let frame = rx.borrow_and_update().clone();
                    if let Some(f) = frame {
                        if output.send(Message::FrameReceived(f)).await.is_err() {
                            return;
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}

impl AutoCamApp {
    pub fn boot() -> (Self, Task<Message>) {
        let (main_id, open_task) = window::open(window::Settings {
            icon: window::icon::from_file_data(APP_ICON_BYTES, None).ok(),
            decorations: false,
            transparent: false,
            size: iced::Size::new(960.0, 600.0),
            min_size: Some(iced::Size::new(480.0, 320.0)),
            resizable: true,
            ..Default::default()
        });

        let devices = CameraManager::query_available_devices();
        let selected_device = devices.first().cloned();
        let supported_resolutions = if let Some(ref dev) = selected_device {
            CameraManager::query_supported_resolutions(dev.index)
        } else {
            vec![VideoResolution::Hd720p, VideoResolution::Fhd1080p]
        };
        let resolution = VideoResolution::Hd720p;

        let camera_manager = if let Some(ref dev) = selected_device {
            let mgr = CameraManager::start(dev.index, resolution);
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

        let app = Self {
            main_window_id: Some(main_id),
            settings_window_id: None,
            camera_manager,
            available_devices: devices,
            selected_device,
            supported_resolutions,
            resolution,
            is_flip: true,
            is_frozen: false,
            is_fullscreen: false,
            rotation_deg: 0,
            zoom_level: 1.0,
            image_handle: None,
            app_icon_handle: image::Handle::from_bytes(APP_ICON_BYTES),
            frame_count: 0,
            status_text,

            grid_mode: GridMode::Off,
            is_loading_camera: false,
            action_shutter: true,
            auto_focus: true,
            manual_focus: 50,
            auto_ev: false,
            ev_bias: 0.0,
            brightness: 0,
            contrast: 1.0,
            saturation: 1.0,
            focus_peaking: false,
            peaking_threshold: 55,
            cas_sharpen: true,
            cas_intensity: 0.45,
            frame_interpolation: false,
            face_detection: false,
            anti_flicker: AntiFlickerMode::Hz50,
            latest_frame: None,
            snapshot_toast: None,

            is_pip: false,
            pip_hovered: false,
        };

        (app, open_task.map(Message::MainWindowOpened))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let hotplug_sub = iced::time::every(Duration::from_millis(1500)).map(|_| Message::PollDevices);
        let frame_sub = Subscription::run(camera_frame_stream);
        let keyboard_sub = keyboard::listen().map(Message::KeyboardEvent);

        Subscription::batch(vec![hotplug_sub, frame_sub, keyboard_sub])
    }

    pub fn theme(&self, _window_id: window::Id) -> iced::Theme {
        iced::Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MainWindowOpened(id) => {
                self.main_window_id = Some(id);
                Task::none()
            }

            Message::SettingsWindowOpened(id) => {
                self.settings_window_id = Some(id);
                Task::none()
            }

            Message::OpenSettingsWindow | Message::ToggleSettings => {
                if let Some(id) = self.settings_window_id {
                    window::gain_focus(id)
                } else {
                    let (id, task) = window::open(window::Settings {
                        icon: window::icon::from_file_data(APP_ICON_BYTES, None).ok(),
                        decorations: false,
                        transparent: false,
                        size: iced::Size::new(540.0, 780.0),
                        min_size: Some(iced::Size::new(480.0, 500.0)),
                        resizable: true,
                        ..Default::default()
                    });
                    self.settings_window_id = Some(id);
                    task.map(Message::SettingsWindowOpened)
                }
            }

            Message::CloseSettingsWindow => {
                if let Some(id) = self.settings_window_id.take() {
                    window::close(id)
                } else {
                    Task::none()
                }
            }

            Message::CloseApp => {
                exit()
            }

            Message::DragWindow(id) => window::drag(id),

            Message::ResizeWindow(id, direction) => window::drag_resize(id, direction),

            Message::MinimizeWindow(id) => window::minimize(id, true),

            Message::FrameReceived(frame) => {
                self.frame_count += 1;
                self.is_loading_camera = false;
                self.image_handle = Some(image::Handle::from_rgba(
                    frame.width,
                    frame.height,
                    frame.rgba.clone(),
                ));
                self.latest_frame = Some(frame);
                Task::none()
            }

            Message::ClearSnapshotToast => {
                self.snapshot_toast = None;
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
                        Key::Named(key::Named::Escape) => {
                            if self.settings_window_id.is_some() {
                                return self.update(Message::CloseSettingsWindow);
                            }
                        }
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
                        Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                            return self.update(Message::ToggleFaceDetection)
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("i") => {
                            return self.update(Message::ToggleFrameInterpolation)
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
                if let Some(ref current) = self.selected_device {
                    if current.index == dev.index && self.camera_manager.is_some() {
                        return Task::none();
                    }
                }
                self.selected_device = Some(dev.clone());
                self.status_text = format!("Connected: {}", dev.name);
                self.is_loading_camera = true;

                self.supported_resolutions = CameraManager::query_supported_resolutions(dev.index);
                if !self.supported_resolutions.contains(&self.resolution) {
                    self.resolution = VideoResolution::Hd720p;
                }

                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchDevice(dev.index));
                    mgr.send(CameraControl::SwitchResolution(self.resolution));
                } else {
                    let mgr = CameraManager::start(dev.index, self.resolution);
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                    mgr.send(CameraControl::SetAutoEv(self.auto_ev));
                    mgr.send(CameraControl::SetEvBias(self.ev_bias));
                    mgr.send(CameraControl::SetBrightness(self.brightness));
                    mgr.send(CameraControl::SetContrast(self.contrast));
                    mgr.send(CameraControl::SetSaturation(self.saturation));
                    mgr.send(CameraControl::SetFocusPeaking(self.focus_peaking));
                    mgr.send(CameraControl::SetPeakingThreshold(self.peaking_threshold));
                    mgr.send(CameraControl::SetCasSharpen(self.cas_sharpen));
                    mgr.send(CameraControl::SetCasIntensity(self.cas_intensity));
                    mgr.send(CameraControl::SetFrameInterpolation(self.frame_interpolation));
                    mgr.send(CameraControl::SetFaceDetection(self.face_detection));
                    self.camera_manager = Some(mgr);
                }
                Task::none()
            }

            Message::ToggleResolution => {
                let next_res = match self.resolution {
                    VideoResolution::Hd720p => VideoResolution::Fhd1080p,
                    VideoResolution::Fhd1080p => VideoResolution::Hd720p,
                };
                if self.supported_resolutions.contains(&next_res) {
                    self.update(Message::SetResolution(next_res))
                } else {
                    Task::none()
                }
            }

            Message::SetResolution(res) => {
                if self.supported_resolutions.contains(&res) && res != self.resolution {
                    self.resolution = res;
                    self.is_loading_camera = true;
                    if let Some(ref mgr) = self.camera_manager {
                        mgr.send(CameraControl::SwitchResolution(res));
                    }
                }
                Task::none()
            }

            Message::ToggleCasSharpen => {
                self.cas_sharpen = !self.cas_sharpen;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetCasSharpen(self.cas_sharpen));
                }
                Task::none()
            }

            Message::SetCasIntensity(val) => {
                self.cas_intensity = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetCasIntensity(val));
                }
                Task::none()
            }

            Message::ToggleFrameInterpolation => {
                self.frame_interpolation = !self.frame_interpolation;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFrameInterpolation(self.frame_interpolation));
                }
                Task::none()
            }

            Message::ToggleFaceDetection => {
                self.face_detection = !self.face_detection;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFaceDetection(self.face_detection));
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
                let next_deg = match self.rotation_deg {
                    0 => 90,
                    90 => 180,
                    180 => 270,
                    _ => 0,
                };
                self.update(Message::SetRotation(next_deg))
            }

            Message::SetRotation(deg) => {
                self.rotation_deg = deg;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                }
                Task::none()
            }

            Message::ZoomIn => {
                let new_zoom = (self.zoom_level + 0.25).min(3.0);
                self.update(Message::SetZoom(new_zoom))
            }

            Message::ZoomOut => {
                let new_zoom = (self.zoom_level - 0.25).max(1.0);
                self.update(Message::SetZoom(new_zoom))
            }

            Message::SetZoom(val) => {
                self.zoom_level = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetZoom(val));
                }
                Task::none()
            }

            Message::CycleGridMode => {
                self.grid_mode = self.grid_mode.next();
                Task::none()
            }

            Message::SetGridMode(mode) => {
                self.grid_mode = mode;
                Task::none()
            }

            Message::ToggleActionShutter => {
                self.action_shutter = !self.action_shutter;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetActionShutter(self.action_shutter));
                }
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

            Message::SetSaturation(val) => {
                self.saturation = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetSaturation(val));
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

            Message::SetPeakingThreshold(val) => {
                self.peaking_threshold = val;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetPeakingThreshold(val));
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
                        return Task::perform(
                            tokio::time::sleep(Duration::from_secs(3)),
                            |_| Message::ClearSnapshotToast,
                        );
                    }
                }
                Task::none()
            }

            Message::OpenCapturesFolder => {
                let _ = std::fs::create_dir_all("captures");
                let _ = std::process::Command::new("explorer").arg("captures").spawn();
                Task::none()
            }

            Message::ResetDefaults => {
                self.ev_bias = 0.0;
                self.brightness = 0;
                self.contrast = 1.0;
                self.saturation = 1.0;
                self.zoom_level = 1.0;
                self.is_flip = true;
                self.rotation_deg = 0;
                self.focus_peaking = false;
                self.peaking_threshold = 55;
                self.cas_sharpen = true;
                self.cas_intensity = 0.45;
                self.frame_interpolation = false;
                self.face_detection = false;
                self.action_shutter = true;
                self.anti_flicker = AntiFlickerMode::Hz50;
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::ResetDefaults);
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
                if let Some(id) = self.main_window_id {
                    window::set_mode(id, mode)
                } else {
                    Task::none()
                }
            }

            Message::TogglePip => {
                self.is_pip = !self.is_pip;
                if let Some(id) = self.main_window_id {
                    if self.is_pip {
                        Task::batch(vec![
                            window::resize(id, iced::Size::new(384.0, 216.0)),
                            window::set_level(id, window::Level::AlwaysOnTop),
                        ])
                    } else {
                        Task::batch(vec![
                            window::resize(id, iced::Size::new(960.0, 600.0)),
                            window::set_level(id, window::Level::Normal),
                        ])
                    }
                } else {
                    Task::none()
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

    /// Master View Router: Routes rendering to the corresponding window
    pub fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        if Some(window_id) == self.settings_window_id {
            self.view_settings_window(window_id)
        } else if self.is_pip {
            self.view_pip(window_id)
        } else {
            self.view_normal(window_id)
        }
    }

    /// Normal Camera Mode View
    fn view_normal(&self, window_id: window::Id) -> Element<'_, Message> {
        let app_logo = image(self.app_icon_handle.clone())
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0));

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
                    if self.supported_resolutions.contains(&VideoResolution::Fhd1080p) {
                        tooltip(
                            button(text(self.resolution.label()).size(11))
                                .on_press(Message::ToggleResolution)
                                .style(theme::clean_button_style(false, false))
                                .padding([4, 8]),
                            container(text("Toggle 720p / 1080p").size(11))
                                .padding([3, 7])
                                .style(theme::tooltip_style),
                            tooltip::Position::Bottom,
                        )
                    } else {
                        tooltip(
                            button(text("720p (Max)").size(11))
                                .style(theme::disabled_button_style)
                                .padding([4, 8]),
                            container(text("1080p unsupported by current camera").size(11))
                                .padding([3, 7])
                                .style(theme::tooltip_style),
                            tooltip::Position::Bottom,
                        )
                    },
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
                        Message::MinimizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_CLOSE,
                        "Close",
                        Message::CloseApp,
                        false,
                        true,
                        tooltip::Position::Bottom,
                    ),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .padding([5, 10]),
            )
            .style(theme::title_bar_style)
            .width(Fill),
        )
        .on_press(Message::DragWindow(window_id));

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
                    render_svg(icons::ICON_CAMERA, 32.0, false, false),
                    Space::new().height(Length::Fixed(8.0)),
                    text("No camera feed active")
                        .size(13)
                        .color(theme::COLOR_TEXT),
                    text("Connect a USB camera or click refresh above")
                        .size(11)
                        .color(theme::COLOR_TEXT_MUTED),
                ]
                .align_x(Alignment::Center)
                .spacing(4),
            )
            .center(Fill)
            .into()
        };

        let mut viewport_stack = stack![video_content].width(Fill).height(Fill);

        // Grid Alignment Overlay
        match self.grid_mode {
            GridMode::Off => {}
            GridMode::RuleOfThirds => {
                let grid_lines: Element<'_, Message> = container(
                    stack![
                        column![
                            Space::new().height(Length::FillPortion(1)),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().height(Length::FillPortion(1)),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().height(Length::FillPortion(1)),
                        ]
                        .width(Fill)
                        .height(Fill),
                        row![
                            Space::new().width(Length::FillPortion(1)),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().width(Length::FillPortion(1)),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
                                    ..Default::default()
                                }),
                            Space::new().width(Length::FillPortion(1)),
                        ]
                        .width(Fill)
                        .height(Fill),
                    ]
                    .width(Fill)
                    .height(Fill),
                )
                .center(Fill)
                .into();

                viewport_stack = viewport_stack.push(grid_lines);
            }
            GridMode::CenterCrosshair => {
                let crosshair: Element<'_, Message> = container(
                    stack![
                        column![
                            Space::new().height(Fill),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.28))),
                                    ..Default::default()
                                }),
                            Space::new().height(Fill),
                        ]
                        .width(Fill)
                        .height(Fill),
                        row![
                            Space::new().width(Fill),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_: &Theme| container::Style {
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

                viewport_stack = viewport_stack.push(crosshair);
            }
        }

        // Presenter Face Detection HUD Overlay
        if self.face_detection {
            if let Some(ref frame) = self.latest_frame {
                if let Some(rect) = frame.face_rect {
                    let top_ratio = (rect.y * 1000.0) as u16;
                    let bottom_ratio = ((1.0 - rect.y - rect.height).max(0.0) * 1000.0) as u16;
                    let left_ratio = (rect.x * 1000.0) as u16;
                    let right_ratio = ((1.0 - rect.x - rect.width).max(0.0) * 1000.0) as u16;

                    let face_hud: Element<'_, Message> = container(
                        column![
                            Space::new().height(Length::FillPortion(top_ratio.max(1))),
                            row![
                                Space::new().width(Length::FillPortion(left_ratio.max(1))),
                                container(Space::new().width(Fill).height(Fill))
                                    .style(|_: &Theme| container::Style {
                                        border: Border {
                                            color: Color::from_rgba(0.95, 0.95, 0.98, 0.70),
                                            width: 1.0,
                                            radius: theme::RADIUS_SM,
                                        },
                                        ..Default::default()
                                    })
                                    .width(Length::FillPortion(((rect.width * 1000.0) as u16).max(10)))
                                    .height(Length::FillPortion(((rect.height * 1000.0) as u16).max(10))),
                                Space::new().width(Length::FillPortion(right_ratio.max(1))),
                            ]
                            .width(Fill)
                            .height(Length::FillPortion(((rect.height * 1000.0) as u16).max(10))),
                            Space::new().height(Length::FillPortion(bottom_ratio.max(1))),
                        ]
                        .width(Fill)
                        .height(Fill),
                    )
                    .width(Fill)
                    .height(Fill)
                    .into();

                    viewport_stack = viewport_stack.push(face_hud);
                }
            }
        }

        // Micro Loading Overlay
        if self.is_loading_camera {
            let loader: Element<'_, Message> = container(
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

            viewport_stack = viewport_stack.push(loader);
        }

        // Snapshot Toast Notification
        if let Some((ref filename, _)) = self.snapshot_toast {
            let toast: Element<'_, Message> = container(
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

            viewport_stack = viewport_stack.push(toast);
        }

        let video_viewport = container(viewport_stack)
            .style(theme::video_viewport_style)
            .width(Fill)
            .height(Fill);

        // 3. Bottom Controls Toolbar
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
                // Freeze / Play
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
                // Presenter Face Tracking HUD
                render_icon_btn(
                    icons::ICON_FACE,
                    if self.face_detection {
                        "Face HUD: Active [D]"
                    } else {
                        "Face HUD: Off [D]"
                    },
                    Message::ToggleFaceDetection,
                    self.face_detection,
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
                // Bottom-right Settings Window Opener Button
                render_icon_btn(
                    icons::ICON_SETTINGS,
                    "Camera Engine & Driver Settings [S]",
                    Message::ToggleSettings,
                    self.settings_window_id.is_some(),
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

        let root = container(content)
            .style(theme::root_window_style)
            .width(Fill)
            .height(Fill)
            .into();

        with_resize_handles(root, window_id)
    }

    /// SEPARATE SETTINGS WINDOW VIEW:
    /// Does NOT cover the camera screen! Offers full, comprehensive and fine-grained camera driver tuning.
    fn view_settings_window(&self, window_id: window::Id) -> Element<'_, Message> {
        // Title Bar for Settings Window
        let header = mouse_area(
            container(
                row![
                    render_svg(icons::ICON_SETTINGS, 16.0, true, false),
                    text("AutoCam Engine & Driver Settings")
                        .size(13)
                        .color(theme::COLOR_TEXT),
                    Space::new().width(Fill),
                    render_icon_btn(
                        icons::ICON_CLOSE,
                        "Close Settings Window [Esc]",
                        Message::CloseSettingsWindow,
                        false,
                        true,
                        tooltip::Position::Bottom,
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([6, 12]),
            )
            .style(theme::title_bar_style)
            .width(Fill),
        )
        .on_press(Message::DragWindow(window_id));

        // SECTION 1: Video Device & Resolution
        let btn_1080p = if self.supported_resolutions.contains(&VideoResolution::Fhd1080p) {
            button(text("1080p FHD").size(11))
                .on_press(Message::SetResolution(VideoResolution::Fhd1080p))
                .style(theme::clean_button_style(
                    self.resolution == VideoResolution::Fhd1080p,
                    false,
                ))
                .padding([4, 12])
        } else {
            button(text("1080p (Unsupported)").size(11))
                .style(theme::disabled_button_style)
                .padding([4, 12])
        };

        let dev_section = column![
            text("1. Video Device & Hardware Stream").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        text("Camera Device:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        pick_list(
                            self.available_devices.clone(),
                            self.selected_device.clone(),
                            Message::DeviceSelected,
                        )
                        .style(theme::pick_list_style)
                        .text_size(11)
                        .padding([3, 8]),
                        render_icon_btn(
                            icons::ICON_REFRESH,
                            "Refresh Devices",
                            Message::RefreshDevices,
                            false,
                            false,
                            tooltip::Position::Bottom,
                        ),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Resolution Target:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("720p HD").size(11))
                            .on_press(Message::SetResolution(VideoResolution::Hd720p))
                            .style(theme::clean_button_style(
                                self.resolution == VideoResolution::Hd720p,
                                false,
                            ))
                            .padding([4, 12]),
                        btn_1080p,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    row![
                        text(format!("Telemetry: Real-Time Pipeline • Sensor 60/30 FPS • Frames: {}", self.frame_count))
                            .size(10)
                            .color(theme::COLOR_TEXT_MUTED),
                    ],
                ]
                .spacing(8),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        // SECTION 2: Shutter, Exposure & Anti-Flicker
        let shutter_section = column![
            text("2. Exposure, Shutter & Lighting Dynamics").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        render_svg(icons::ICON_CAMERA, 14.0, self.action_shutter, false),
                        column![
                            text("Anti-Ghosting (High-Speed Action Shutter)").size(11).color(theme::COLOR_TEXT),
                            text("Prevents motion blur & ghost trails on rapid hand/circuit movement").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.action_shutter { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleActionShutter)
                            .style(theme::clean_button_style(self.action_shutter, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        render_svg(icons::ICON_EV, 14.0, self.auto_ev, false),
                        column![
                            text("Software Auto-EV (Adaptive Lighting)").size(11).color(theme::COLOR_TEXT),
                            text("Smooth DSP exposure normalization with zero flicker").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.auto_ev { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleAutoEv)
                            .style(theme::clean_button_style(self.auto_ev, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("EV Exposure Bias:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{:.1} EV", self.ev_bias))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(-20..=20, (self.ev_bias * 10.0) as i32, |val| {
                            Message::SetEvBias(val as f32 / 10.0)
                        })
                        .width(Length::Fixed(160.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Anti-Flicker Frequency:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("50 Hz (TH/EU)").size(10))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Hz50))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Hz50, false))
                            .padding([3, 8]),
                        button(text("60 Hz (US)").size(10))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Hz60))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Hz60, false))
                            .padding([3, 8]),
                        button(text("Off").size(10))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Off))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Off, false))
                            .padding([3, 8]),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                ]
                .spacing(10),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        // SECTION 3: Color, Brightness & Contrast
        let color_section = column![
            text("3. Color Grading & Contrast (PCB Inspect)").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        text("Brightness:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{}", self.brightness))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(-50..=50, self.brightness, Message::SetBrightness)
                            .width(Length::Fixed(160.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Contrast:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{:.1}x", self.contrast))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(5..=20, (self.contrast * 10.0) as i32, |val| {
                            Message::SetContrast(val as f32 / 10.0)
                        })
                        .width(Length::Fixed(160.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Saturation / Color:").size(11).color(theme::COLOR_TEXT),
                        text(if self.saturation <= 0.05 {
                            "B&W Circuit Mode".to_string()
                        } else {
                            format!("{:.1}x", self.saturation)
                        })
                        .size(11)
                        .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(0..=20, (self.saturation * 10.0) as i32, |val| {
                            Message::SetSaturation(val as f32 / 10.0)
                        })
                        .width(Length::Fixed(160.0)),
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

        // SECTION 4: Optics, Zoom & Orientation
        let optics_section = column![
            text("4. Optics, Digital Zoom & Orientation").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        render_svg(icons::ICON_FOCUS, 14.0, self.auto_focus, false),
                        text("Hardware Auto-Focus [A]").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text(if self.auto_focus { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleAutoFocus)
                            .style(theme::clean_button_style(self.auto_focus, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Digital Zoom:").size(11).color(theme::COLOR_TEXT),
                        text(format!("{:.2}x", self.zoom_level))
                            .size(11)
                            .color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        slider(100..=300, (self.zoom_level * 100.0) as i32, |val| {
                            Message::SetZoom(val as f32 / 100.0)
                        })
                        .width(Length::Fixed(160.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Mirror / Horizontal Flip:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text(if self.is_flip { "Mirrored [F]" } else { "Normal [F]" }).size(10))
                            .on_press(Message::ToggleFlip)
                            .style(theme::clean_button_style(self.is_flip, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Rotation Angle:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("0°").size(10))
                            .on_press(Message::SetRotation(0))
                            .style(theme::clean_button_style(self.rotation_deg == 0, false))
                            .padding([3, 8]),
                        button(text("90°").size(10))
                            .on_press(Message::SetRotation(90))
                            .style(theme::clean_button_style(self.rotation_deg == 90, false))
                            .padding([3, 8]),
                        button(text("180°").size(10))
                            .on_press(Message::SetRotation(180))
                            .style(theme::clean_button_style(self.rotation_deg == 180, false))
                            .padding([3, 8]),
                        button(text("270°").size(10))
                            .on_press(Message::SetRotation(270))
                            .style(theme::clean_button_style(self.rotation_deg == 270, false))
                            .padding([3, 8]),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                ]
                .spacing(8),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        // SECTION 5: Circuit Teaching & Inspection Tools
        let teaching_section = column![
            text("5. Circuit Teaching & Micro-Inspection Tools").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        render_svg(icons::ICON_WAND, 14.0, self.cas_sharpen, false),
                        column![
                            text("CAS Circuit Sharpening (AMD FidelityFX)").size(11).color(theme::COLOR_TEXT),
                            text("Contrast Adaptive Sharpening for IC pins and circuit traces").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.cas_sharpen { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleCasSharpen)
                            .style(theme::clean_button_style(self.cas_sharpen, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    if self.cas_sharpen {
                        row![
                            text("CAS Sharpness Intensity:").size(11).color(theme::COLOR_TEXT),
                            text(format!("{:.0}%", self.cas_intensity * 100.0)).size(11).color(theme::COLOR_TEXT_MUTED),
                            Space::new().width(Fill),
                            slider(10..=100, (self.cas_intensity * 100.0) as i32, |val| {
                                Message::SetCasIntensity(val as f32 / 100.0)
                            })
                            .width(Length::Fixed(160.0)),
                        ]
                        .align_y(Alignment::Center)
                    } else {
                        row![]
                    },
                    row![
                        render_svg(icons::ICON_PEAKING, 14.0, self.focus_peaking, false),
                        column![
                            text("Focus Peaking (Highlight In-Focus Traces)").size(11).color(theme::COLOR_TEXT),
                            text("Highlights sharp solder points and IC text in neon green").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.focus_peaking { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleFocusPeaking)
                            .style(theme::clean_button_style(self.focus_peaking, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    if self.focus_peaking {
                        row![
                            text("Peaking Sensitivity:").size(11).color(theme::COLOR_TEXT),
                            text(format!("{}", self.peaking_threshold)).size(11).color(theme::COLOR_TEXT_MUTED),
                            Space::new().width(Fill),
                            slider(20..=90, self.peaking_threshold, Message::SetPeakingThreshold)
                                .width(Length::Fixed(160.0)),
                        ]
                        .align_y(Alignment::Center)
                    } else {
                        row![]
                    },
                    row![
                        render_svg(icons::ICON_FACE, 14.0, self.face_detection, false),
                        column![
                            text("Presenter Face Tracking HUD [D]").size(11).color(theme::COLOR_TEXT),
                            text("Subtle 1px indicator framing presenter in lesson stream").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.face_detection { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleFaceDetection)
                            .style(theme::clean_button_style(self.face_detection, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        render_svg(icons::ICON_MOTION, 14.0, self.frame_interpolation, false),
                        column![
                            text("Motion Smoothing (60 FPS Interpolation)").size(11).color(theme::COLOR_TEXT),
                            text("Sub-frame motion blending for silky smooth camera video").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.frame_interpolation { "Active" } else { "Off" }).size(10))
                            .on_press(Message::ToggleFrameInterpolation)
                            .style(theme::clean_button_style(self.frame_interpolation, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Grid Alignment Overlay:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("Off").size(10))
                            .on_press(Message::SetGridMode(GridMode::Off))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::Off, false))
                            .padding([3, 8]),
                        button(text("3x3 Thirds").size(10))
                            .on_press(Message::SetGridMode(GridMode::RuleOfThirds))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::RuleOfThirds, false))
                            .padding([3, 8]),
                        button(text("Crosshair").size(10))
                            .on_press(Message::SetGridMode(GridMode::CenterCrosshair))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::CenterCrosshair, false))
                            .padding([3, 8]),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                ]
                .spacing(10),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        // SECTION 6: Storage, Captures & Defaults
        let storage_section = column![
            text("6. Captures & Maintenance").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Snapshot Directory: captures/").size(11).color(theme::COLOR_TEXT),
                            text("High-resolution PNG snapshots with zero compression loss").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Open Captures Folder").size(11))
                            .on_press(Message::OpenCapturesFolder)
                            .style(theme::clean_button_style(false, false))
                            .padding([4, 12]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Reset Configuration:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("Reset All to Defaults").size(11))
                            .on_press(Message::ResetDefaults)
                            .style(theme::clean_button_style(false, true))
                            .padding([4, 12]),
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

        // Shortcuts Cheatsheet
        let shortcuts_guide = column![
            text("Quick Keyboard Shortcuts").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    shortcut_row("Space", "Freeze / Resume live feed"),
                    shortcut_row("F", "Mirror / Flip camera"),
                    shortcut_row("R", "Rotate 90 degrees"),
                    shortcut_row("G", "Cycle Grid (Off -> 3x3 -> Crosshair)"),
                    shortcut_row("A", "Toggle Auto Focus"),
                    shortcut_row("E", "Toggle Auto EV"),
                    shortcut_row("C", "Capture high-res snapshot"),
                    shortcut_row("P", "Toggle Picture-in-Picture"),
                    shortcut_row("F11", "Toggle Fullscreen"),
                    shortcut_row("D", "Toggle Presenter Face Tracking HUD"),
                    shortcut_row("I", "Toggle 60 FPS Motion Interpolation"),
                    shortcut_row("+ / -", "Zoom in / Zoom out"),
                    shortcut_row("S / Esc", "Toggle / Close Settings Window"),
                ]
                .spacing(5),
            )
            .padding(10)
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(4);

        let scroll_body = scrollable(
            column![
                dev_section,
                shutter_section,
                color_section,
                optics_section,
                teaching_section,
                storage_section,
                shortcuts_guide,
            ]
            .spacing(12)
            .padding([12, 16]),
        )
        .width(Fill)
        .height(Fill);

        let content = column![header, scroll_body]
            .width(Fill)
            .height(Fill);

        let root = container(content)
            .style(theme::settings_window_style)
            .width(Fill)
            .height(Fill)
            .into();

        with_resize_handles(root, window_id)
    }

    /// Picture-in-Picture Mode: Compact, borderless, AlwaysOnTop.
    fn view_pip(&self, window_id: window::Id) -> Element<'_, Message> {
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

        // Presenter Face Detection HUD in PiP
        if self.face_detection {
            if let Some(ref frame) = self.latest_frame {
                if let Some(rect) = frame.face_rect {
                    let top_ratio = (rect.y * 1000.0) as u16;
                    let bottom_ratio = ((1.0 - rect.y - rect.height).max(0.0) * 1000.0) as u16;
                    let left_ratio = (rect.x * 1000.0) as u16;
                    let right_ratio = ((1.0 - rect.x - rect.width).max(0.0) * 1000.0) as u16;

                    let face_hud: Element<'_, Message> = container(
                        column![
                            Space::new().height(Length::FillPortion(top_ratio.max(1))),
                            row![
                                Space::new().width(Length::FillPortion(left_ratio.max(1))),
                                container(Space::new().width(Fill).height(Fill))
                                    .style(|_: &Theme| container::Style {
                                        border: Border {
                                            color: Color::from_rgba(0.95, 0.95, 0.98, 0.70),
                                            width: 1.0,
                                            radius: theme::RADIUS_SM,
                                        },
                                        ..Default::default()
                                    })
                                    .width(Length::FillPortion(((rect.width * 1000.0) as u16).max(10)))
                                    .height(Length::FillPortion(((rect.height * 1000.0) as u16).max(10))),
                                Space::new().width(Length::FillPortion(right_ratio.max(1))),
                            ]
                            .width(Fill)
                            .height(Length::FillPortion(((rect.height * 1000.0) as u16).max(10))),
                            Space::new().height(Length::FillPortion(bottom_ratio.max(1))),
                        ]
                        .width(Fill)
                        .height(Fill),
                    )
                    .width(Fill)
                    .height(Fill)
                    .into();

                    layers.push(face_hud);
                }
            }
        }

        // Overlay toolbar when hovering
        if self.pip_hovered {
            let overlay_header: Element<'_, Message> = mouse_area(
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
                            Message::CloseApp,
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
            .on_press(Message::DragWindow(window_id))
            .into();

            layers.push(overlay_header);
        }

        let pip_stack = stack(layers).width(Fill).height(Fill);

        let content = mouse_area(
            container(pip_stack)
                .style(theme::root_window_style)
                .width(Fill)
                .height(Fill),
        )
        .on_enter(Message::PipMouseEnter)
        .on_exit(Message::PipMouseExit)
        .on_press(Message::DragWindow(window_id))
        .into();

        with_resize_handles(content, window_id)
    }
}

/// Helper row for shortcut documentation
fn shortcut_row<'a>(key_name: &'static str, description: &'static str) -> Element<'a, Message> {
    row![
        container(text(key_name).size(11).color(theme::COLOR_TEXT))
            .padding([2, 6])
            .style(theme::tooltip_style),
        text(description).size(11).color(theme::COLOR_TEXT_MUTED),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// Helper function to render SVG icon element with crisp white tint
fn render_svg<'a>(
    svg_str: &'static str,
    size: f32,
    _is_active: bool,
    _is_danger: bool,
) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(svg_str.as_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(|_theme, _status| svg::Style {
            color: Some(Color::WHITE),
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

/// Borderless Window Responsive Resize Handles for multi-window
fn with_resize_handles<'a>(content: Element<'a, Message>, window_id: window::Id) -> Element<'a, Message> {
    let border_thickness = 4.0;
    let corner_size = 20.0;

    // Top border
    let top = mouse_area(
        container(Space::new().width(Fill).height(Length::Fixed(border_thickness)))
            .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::North))
    .interaction(mouse::Interaction::ResizingVertically);

    // Bottom border
    let bottom = mouse_area(
        container(Space::new().width(Fill).height(Length::Fixed(border_thickness)))
            .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::South))
    .interaction(mouse::Interaction::ResizingVertically);

    // Left border
    let left = mouse_area(
        container(Space::new().width(Length::Fixed(border_thickness)).height(Fill))
            .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::West))
    .interaction(mouse::Interaction::ResizingHorizontally);

    // Right border
    let right = mouse_area(
        container(Space::new().width(Length::Fixed(border_thickness)).height(Fill))
            .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::East))
    .interaction(mouse::Interaction::ResizingHorizontally);

    // Top-left corner
    let top_left = mouse_area(
        container(
            Space::new()
                .width(Length::Fixed(corner_size))
                .height(Length::Fixed(corner_size)),
        )
        .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::NorthWest))
    .interaction(mouse::Interaction::ResizingDiagonallyDown);

    // Top-right corner
    let top_right = mouse_area(
        container(
            Space::new()
                .width(Length::Fixed(corner_size))
                .height(Length::Fixed(corner_size)),
        )
        .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::NorthEast))
    .interaction(mouse::Interaction::ResizingDiagonallyUp);

    // Bottom-left corner
    let bottom_left = mouse_area(
        container(
            Space::new()
                .width(Length::Fixed(corner_size))
                .height(Length::Fixed(corner_size)),
        )
        .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::SouthWest))
    .interaction(mouse::Interaction::ResizingDiagonallyUp);

    // Bottom-right corner
    let bottom_right = mouse_area(
        container(
            Space::new()
                .width(Length::Fixed(corner_size))
                .height(Length::Fixed(corner_size)),
        )
        .style(|_: &Theme| container::Style::default()),
    )
    .on_press(Message::ResizeWindow(window_id, window::Direction::SouthEast))
    .interaction(mouse::Interaction::ResizingDiagonallyDown);

    stack![
        content,
        // Edges
        column![top, Space::new().height(Fill), bottom].width(Fill).height(Fill),
        row![left, Space::new().width(Fill), right].width(Fill).height(Fill),
        // Corners
        column![
            row![top_left, Space::new().width(Fill), top_right].width(Fill),
            Space::new().height(Fill),
            row![bottom_left, Space::new().width(Fill), bottom_right].width(Fill),
        ]
        .width(Fill)
        .height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn main() -> iced::Result {
    iced::daemon(AutoCamApp::boot, AutoCamApp::update, AutoCamApp::view)
        .subscription(AutoCamApp::subscription)
        .theme(AutoCamApp::theme)
        .run()
}
