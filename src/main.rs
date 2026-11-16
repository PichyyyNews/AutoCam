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
    exit, Alignment, Background, Color, ContentFit, Element, Fill, Length, Subscription, Task,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    Freeze,
    Flip,
    Rotate,
    Grid,
    AutoFocus,
    AutoEv,
    FaceHud,
    Interpolation,
    Snapshot,
    Pip,
    Fullscreen,
    Settings,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
    pub freeze: Key,
    pub flip: Key,
    pub rotate: Key,
    pub grid: Key,
    pub auto_focus: Key,
    pub auto_ev: Key,
    pub face_hud: Key,
    pub interpolation: Key,
    pub snapshot: Key,
    pub pip: Key,
    pub fullscreen: Key,
    pub settings: Key,
    pub zoom_in: Key,
    pub zoom_out: Key,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            freeze: Key::Named(key::Named::Space),
            flip: Key::Character("f".into()),
            rotate: Key::Character("r".into()),
            grid: Key::Character("g".into()),
            auto_focus: Key::Character("a".into()),
            auto_ev: Key::Character("e".into()),
            face_hud: Key::Character("d".into()),
            interpolation: Key::Character("i".into()),
            snapshot: Key::Character("c".into()),
            pip: Key::Character("p".into()),
            fullscreen: Key::Named(key::Named::F11),
            settings: Key::Character("s".into()),
            zoom_in: Key::Character("=".into()),
            zoom_out: Key::Character("-".into()),
        }
    }
}

fn key_matches(k1: &Key, k2: &Key) -> bool {
    match (k1, k2) {
        (Key::Character(c1), Key::Character(c2)) => c1.eq_ignore_ascii_case(c2),
        (k1, k2) => k1 == k2,
    }
}

fn format_key(key: &Key) -> String {
    match key {
        Key::Named(key::Named::Space) => "Space".to_string(),
        Key::Named(key::Named::F11) => "F11".to_string(),
        Key::Named(key::Named::Escape) => "Esc".to_string(),
        Key::Named(named) => format!("{:?}", named),
        Key::Character(c) => c.to_uppercase(),
        _ => "Key".to_string(),
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

    // Customizable Shortcuts
    keybindings: KeyBindings,
    rebinding_action: Option<ShortcutAction>,

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

    // Shortcut Rebinding
    StartRebinding(ShortcutAction),
    CancelRebinding,
    ResetShortcuts,
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

            keybindings: KeyBindings::default(),
            rebinding_action: None,

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
                self.rebinding_action = None;
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
                    // 1. If currently recording a custom shortcut:
                    if let Some(action) = self.rebinding_action.take() {
                        if key == Key::Named(key::Named::Escape) {
                            return Task::none();
                        }
                        match action {
                            ShortcutAction::Freeze => self.keybindings.freeze = key,
                            ShortcutAction::Flip => self.keybindings.flip = key,
                            ShortcutAction::Rotate => self.keybindings.rotate = key,
                            ShortcutAction::Grid => self.keybindings.grid = key,
                            ShortcutAction::AutoFocus => self.keybindings.auto_focus = key,
                            ShortcutAction::AutoEv => self.keybindings.auto_ev = key,
                            ShortcutAction::FaceHud => self.keybindings.face_hud = key,
                            ShortcutAction::Interpolation => self.keybindings.interpolation = key,
                            ShortcutAction::Snapshot => self.keybindings.snapshot = key,
                            ShortcutAction::Pip => self.keybindings.pip = key,
                            ShortcutAction::Fullscreen => self.keybindings.fullscreen = key,
                            ShortcutAction::Settings => self.keybindings.settings = key,
                            ShortcutAction::ZoomIn => self.keybindings.zoom_in = key,
                            ShortcutAction::ZoomOut => self.keybindings.zoom_out = key,
                        }
                        return Task::none();
                    }

                    // 2. Normal key action dispatching:
                    if key == Key::Named(key::Named::Escape) {
                        if self.settings_window_id.is_some() {
                            return self.update(Message::CloseSettingsWindow);
                        }
                    } else if key_matches(&key, &self.keybindings.freeze) {
                        return self.update(Message::ToggleFreeze);
                    } else if key_matches(&key, &self.keybindings.flip) {
                        return self.update(Message::ToggleFlip);
                    } else if key_matches(&key, &self.keybindings.rotate) {
                        return self.update(Message::RotateNext);
                    } else if key_matches(&key, &self.keybindings.grid) {
                        return self.update(Message::CycleGridMode);
                    } else if key_matches(&key, &self.keybindings.auto_focus) {
                        return self.update(Message::ToggleAutoFocus);
                    } else if key_matches(&key, &self.keybindings.auto_ev) {
                        return self.update(Message::ToggleAutoEv);
                    } else if key_matches(&key, &self.keybindings.face_hud) {
                        return self.update(Message::ToggleFaceDetection);
                    } else if key_matches(&key, &self.keybindings.interpolation) {
                        return self.update(Message::ToggleFrameInterpolation);
                    } else if key_matches(&key, &self.keybindings.snapshot) {
                        return self.update(Message::CaptureSnapshot);
                    } else if key_matches(&key, &self.keybindings.pip) {
                        return self.update(Message::TogglePip);
                    } else if key_matches(&key, &self.keybindings.fullscreen) {
                        return self.update(Message::ToggleFullscreen);
                    } else if key_matches(&key, &self.keybindings.settings) {
                        return self.update(Message::ToggleSettings);
                    } else if key_matches(&key, &self.keybindings.zoom_in) {
                        return self.update(Message::ZoomIn);
                    } else if key_matches(&key, &self.keybindings.zoom_out) {
                        return self.update(Message::ZoomOut);
                    }
                }
                Task::none()
            }

            Message::StartRebinding(action) => {
                self.rebinding_action = Some(action);
                Task::none()
            }

            Message::CancelRebinding => {
                self.rebinding_action = None;
                Task::none()
            }

            Message::ResetShortcuts => {
                self.keybindings = KeyBindings::default();
                self.rebinding_action = None;
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
    /// High-performance, zero stutter, white sliders, custom shortcut manager.
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
                        text("Telemetry: Real-Time Sub-20ms Engine • Direct Sensor Pipe")
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
                            text("Prevents motion blur & ghost trails on rapid movement").size(10).color(theme::COLOR_TEXT_MUTED),
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
                        .style(theme::clean_slider_style)
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

        // SECTION 3: Color, Brightness & Contrast (Clean White Sliders)
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
                            .style(theme::clean_slider_style)
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
                        .style(theme::clean_slider_style)
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
                        .style(theme::clean_slider_style)
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
                        text("Hardware Auto-Focus").size(11).color(theme::COLOR_TEXT),
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
                        .style(theme::clean_slider_style)
                        .width(Length::Fixed(160.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Mirror / Horizontal Flip:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text(if self.is_flip { "Mirrored" } else { "Normal" }).size(10))
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
                            text("CAS Sharpness Strength:").size(11).color(theme::COLOR_TEXT),
                            text(format!("{:.0}%", self.cas_intensity * 100.0)).size(11).color(theme::COLOR_TEXT_MUTED),
                            Space::new().width(Fill),
                            slider(10..=100, (self.cas_intensity * 100.0) as i32, |val| {
                                Message::SetCasIntensity(val as f32 / 100.0)
                            })
                            .style(theme::clean_slider_style)
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
                                .style(theme::clean_slider_style)
                                .width(Length::Fixed(160.0)),
                        ]
                        .align_y(Alignment::Center)
                    } else {
                        row![]
                    },
                    row![
                        render_svg(icons::ICON_FACE, 14.0, self.face_detection, false),
                        column![
                            text("Presenter Face Tracking HUD").size(11).color(theme::COLOR_TEXT),
                            text("High-precision head clustering with zero-offset direct overlay").size(10).color(theme::COLOR_TEXT_MUTED),
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

        // SECTION 6: Customizable Keyboard Shortcuts
        let shortcut_rows = column![
            shortcut_config_row(
                "Freeze / Resume Live",
                format_key(&self.keybindings.freeze),
                self.rebinding_action == Some(ShortcutAction::Freeze),
                Message::StartRebinding(ShortcutAction::Freeze),
            ),
            shortcut_config_row(
                "Mirror / Horizontal Flip",
                format_key(&self.keybindings.flip),
                self.rebinding_action == Some(ShortcutAction::Flip),
                Message::StartRebinding(ShortcutAction::Flip),
            ),
            shortcut_config_row(
                "Rotate 90 Degrees",
                format_key(&self.keybindings.rotate),
                self.rebinding_action == Some(ShortcutAction::Rotate),
                Message::StartRebinding(ShortcutAction::Rotate),
            ),
            shortcut_config_row(
                "Cycle Grid Overlay",
                format_key(&self.keybindings.grid),
                self.rebinding_action == Some(ShortcutAction::Grid),
                Message::StartRebinding(ShortcutAction::Grid),
            ),
            shortcut_config_row(
                "Toggle Auto Focus",
                format_key(&self.keybindings.auto_focus),
                self.rebinding_action == Some(ShortcutAction::AutoFocus),
                Message::StartRebinding(ShortcutAction::AutoFocus),
            ),
            shortcut_config_row(
                "Toggle Auto EV",
                format_key(&self.keybindings.auto_ev),
                self.rebinding_action == Some(ShortcutAction::AutoEv),
                Message::StartRebinding(ShortcutAction::AutoEv),
            ),
            shortcut_config_row(
                "Presenter Face Tracking HUD",
                format_key(&self.keybindings.face_hud),
                self.rebinding_action == Some(ShortcutAction::FaceHud),
                Message::StartRebinding(ShortcutAction::FaceHud),
            ),
            shortcut_config_row(
                "Toggle 60 FPS Motion Smoothing",
                format_key(&self.keybindings.interpolation),
                self.rebinding_action == Some(ShortcutAction::Interpolation),
                Message::StartRebinding(ShortcutAction::Interpolation),
            ),
            shortcut_config_row(
                "Capture Snapshot Still",
                format_key(&self.keybindings.snapshot),
                self.rebinding_action == Some(ShortcutAction::Snapshot),
                Message::StartRebinding(ShortcutAction::Snapshot),
            ),
            shortcut_config_row(
                "Toggle Picture-in-Picture (PiP)",
                format_key(&self.keybindings.pip),
                self.rebinding_action == Some(ShortcutAction::Pip),
                Message::StartRebinding(ShortcutAction::Pip),
            ),
            shortcut_config_row(
                "Toggle Fullscreen",
                format_key(&self.keybindings.fullscreen),
                self.rebinding_action == Some(ShortcutAction::Fullscreen),
                Message::StartRebinding(ShortcutAction::Fullscreen),
            ),
            shortcut_config_row(
                "Toggle Settings Window",
                format_key(&self.keybindings.settings),
                self.rebinding_action == Some(ShortcutAction::Settings),
                Message::StartRebinding(ShortcutAction::Settings),
            ),
            shortcut_config_row(
                "Zoom In",
                format_key(&self.keybindings.zoom_in),
                self.rebinding_action == Some(ShortcutAction::ZoomIn),
                Message::StartRebinding(ShortcutAction::ZoomIn),
            ),
            shortcut_config_row(
                "Zoom Out",
                format_key(&self.keybindings.zoom_out),
                self.rebinding_action == Some(ShortcutAction::ZoomOut),
                Message::StartRebinding(ShortcutAction::ZoomOut),
            ),
        ]
        .spacing(4);

        let shortcuts_section = column![
            row![
                text("6. Custom Keyboard Shortcuts (Click to Rebind)").size(11).color(theme::COLOR_TEXT_MUTED),
                Space::new().width(Fill),
                button(text("Reset Shortcuts").size(10))
                    .on_press(Message::ResetShortcuts)
                    .style(theme::clean_button_style(false, false))
                    .padding([2, 6]),
            ]
            .align_y(Alignment::Center),
            container(shortcut_rows)
                .padding(10)
                .style(theme::settings_inner_box_style)
                .width(Fill),
        ]
        .spacing(4);

        // SECTION 7: Storage & Maintenance
        let storage_section = column![
            text("7. Captures & Maintenance").size(11).color(theme::COLOR_TEXT_MUTED),
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
                        text("Reset All Configuration:").size(11).color(theme::COLOR_TEXT),
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

        let scroll_body = scrollable(
            column![
                dev_section,
                shutter_section,
                color_section,
                optics_section,
                teaching_section,
                shortcuts_section,
                storage_section,
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

/// Helper row for shortcut rebinding configuration
fn shortcut_config_row<'a>(
    description: &'static str,
    key_label: String,
    is_rebinding: bool,
    on_click: Message,
) -> Element<'a, Message> {
    let btn_text = if is_rebinding {
        "[ Press any key... ]".to_string()
    } else {
        key_label
    };

    row![
        text(description).size(11).color(theme::COLOR_TEXT),
        Space::new().width(Fill),
        button(text(btn_text).size(11))
            .on_press(on_click)
            .style(theme::clean_button_style(is_rebinding, false))
            .padding([3, 10]),
    ]
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
