#![windows_subsystem = "windows"]

mod audio;
mod camera;
mod db;
mod face_detect;
mod icons;
mod recorder;
mod theme;
mod virtual_cam;

use camera::{
    AntiFlickerMode, CameraControl, CameraDeviceInfo, CameraManager, ColorProfile, DenoiseMode,
    FramePayload, VideoResolution,
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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const APP_ICON_BYTES: &[u8] = include_bytes!("../public/icon.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    Off,
    RuleOfThirds,
    CenterCrosshair,
    GoldenRatio,
    SafeMargin,
}

impl GridMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Off => Self::RuleOfThirds,
            Self::RuleOfThirds => Self::CenterCrosshair,
            Self::CenterCrosshair => Self::GoldenRatio,
            Self::GoldenRatio => Self::SafeMargin,
            Self::SafeMargin => Self::Off,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Grid: Off [G]",
            Self::RuleOfThirds => "Grid: 3x3 Thirds [G]",
            Self::CenterCrosshair => "Grid: Crosshair [G]",
            Self::GoldenRatio => "Grid: Golden Ratio [G]",
            Self::SafeMargin => "Grid: Safe Margin 90% [G]",
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
    Record,
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
    pub record: Key,
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
            record: Key::Character("v".into()),
            pip: Key::Character("p".into()),
            fullscreen: Key::Named(key::Named::F11),
            settings: Key::Character("s".into()),
            zoom_in: Key::Character("=".into()),
            zoom_out: Key::Character("-".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Photo,
    Video,
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub path: PathBuf,
    pub filename: String,
    pub kind: MediaKind,
    pub modified: SystemTime,
    pub size_bytes: u64,
    pub thumbnail_handle: Option<image::Handle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumFilter {
    All,
    Photo,
    Video,
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

fn parse_key_str(s: &str) -> Option<Key> {
    match s.to_lowercase().as_str() {
        "space" => Some(Key::Named(key::Named::Space)),
        "f11" => Some(Key::Named(key::Named::F11)),
        "esc" | "escape" => Some(Key::Named(key::Named::Escape)),
        c if c.len() == 1 => Some(Key::Character(c.into())),
        _ => None,
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn scan_captures_folder(captures_dir: &std::path::Path) -> Vec<MediaItem> {
    let mut items = Vec::new();
    let _ = std::fs::create_dir_all(captures_dir);
    if let Ok(entries) = std::fs::read_dir(captures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let is_photo = ext == "png" || ext == "jpg" || ext == "jpeg";
            let is_video = ext == "avi" || ext == "mp4" || ext == "mkv" || ext == "mov";
            if !is_photo && !is_video {
                continue;
            }
            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("").to_string();
            let metadata = entry.metadata().ok();
            let modified = metadata.as_ref().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
            let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            
            let kind = if is_photo {
                MediaKind::Photo
            } else {
                MediaKind::Video
            };

            let thumbnail_handle = if is_photo {
                std::fs::read(&path).ok().map(image::Handle::from_bytes)
            } else if is_video {
                recorder::extract_first_frame_jpeg(&path).map(image::Handle::from_bytes)
            } else {
                None
            };

            items.push(MediaItem {
                path,
                filename,
                kind,
                modified,
                size_bytes,
                thumbnail_handle,
            });
        }
    }
    // Sort descending by modified date
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    items
}

fn get_storage_stats(captures_dir: &std::path::Path) -> (usize, usize, u64) {
    let mut photos = 0;
    let mut videos = 0;
    let mut total_bytes = 0u64;

    if let Ok(entries) = std::fs::read_dir(captures_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let is_photo = ext == "png" || ext == "jpg" || ext == "jpeg";
                let is_video = ext == "avi" || ext == "mp4" || ext == "mkv" || ext == "mov";
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if is_photo {
                    photos += 1;
                    total_bytes += len;
                } else if is_video {
                    videos += 1;
                    total_bytes += len;
                }
            }
        }
    }
    (photos, videos, total_bytes)
}

#[derive(Debug, Clone)]
pub struct ViewerVideoState {
    pub path: PathBuf,
    pub fps: u32,
    pub frames: Vec<(u64, u32)>,
    pub current_frame: usize,
    pub is_playing: bool,
}

pub struct AutoCamApp {
    main_window_id: Option<window::Id>,
    settings_window_id: Option<window::Id>,
    album_window_id: Option<window::Id>,
    viewer_window_id: Option<window::Id>,

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

    // Video Recording
    video_recorder: Option<recorder::VideoRecorder>,
    is_recording: bool,
    recording_start_time: Option<Instant>,
    recording_elapsed_secs: u64,

    // Audio / Microphone Controls
    available_audio_devices: Vec<audio::AudioDeviceInfo>,
    selected_audio_device: Option<audio::AudioDeviceInfo>,
    is_mic_muted: bool,

    // Album & Media Gallery
    media_items: Vec<MediaItem>,
    latest_capture: Option<MediaItem>,
    album_hovered: bool,
    album_filter: AlbumFilter,
    hovered_media_idx: Option<usize>,
    active_media_menu_idx: Option<usize>,

    // Photo Viewer Window
    viewer_selected_path: Option<PathBuf>,
    viewer_image_handle: Option<image::Handle>,
    viewer_info_text: String,
    viewer_video_state: Option<ViewerVideoState>,

    // Customizable Shortcuts
    keybindings: KeyBindings,
    rebinding_action: Option<ShortcutAction>,

    // Video Denoise & Quality Enhancements
    denoise_mode: DenoiseMode,
    low_light_boost: bool,
    auto_white_balance: bool,
    mic_denoise: bool,

    // Pro Photography & Color Tuning
    manual_kelvin: u32,
    iso_gain: f32,
    color_profile: ColorProfile,

    // Developer Telemetry & Performance Stats
    show_telemetry: bool,
    last_frame_instant: Option<Instant>,
    live_fps: f32,
    live_frame_time_ms: f32,

    // Storage Configuration
    captures_path: PathBuf,

    // Virtual Camera Output (OBS / Media Foundation)
    virtual_cam_bridge: Option<virtual_cam::VirtualCamBridge>,
    is_virtual_cam_active: bool,
    virtual_cam_status_text: String,

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
    ToggleMaximizeWindow(window::Id),
    CloseApp,
    RefreshDevices,

    // Quality & Denoise Engine Messages
    SetDenoiseMode(DenoiseMode),
    ToggleLowLightBoost,
    ToggleAutoWhiteBalance,
    ToggleMicDenoise,
    ToggleVirtualCamera,
    SetManualKelvin(u32),
    SetIsoGain(f32),
    SetColorProfile(ColorProfile),
    ToggleTelemetry,
    SetCapturesPath(PathBuf),
    PurgeAllCaptures,
    ResetFactorySettings,
    OpenDatabaseFolder,

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

    // Video Recording
    ToggleRecording,
    RecordingTick,

    // Audio / Microphone Messages
    AudioDeviceSelected(audio::AudioDeviceInfo),
    ToggleMicMute,

    // Album Window & Gallery Messages
    OpenAlbumWindow,
    CloseAlbumWindow,
    AlbumWindowOpened(window::Id),
    AlbumHovered(bool),
    SetAlbumFilter(AlbumFilter),
    RefreshAlbum,
    PlayVideo(PathBuf),
    MediaHovered(Option<usize>),
    ToggleMediaMenu(Option<usize>),
    OpenInExplorer(PathBuf),
    CopyMedia(PathBuf),
    CopyMediaPath(PathBuf),
    DeleteMedia(PathBuf),

    // Photo & Video Viewer Window
    OpenViewerWindow(PathBuf),
    CloseViewerWindow,
    ViewerWindowOpened(window::Id),
    VideoViewerTick,
    ToggleVideoPlayback,
    SeekVideo(usize),
    ReplayVideo,

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
        db::init();
        let db_cfg = db::load_all();

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
        let supported_resolutions = vec![VideoResolution::Hd720p, VideoResolution::Fhd1080p];

        // 1. Restore Persistent Database Config
        let resolution = if let Some(res_str) = db_cfg.get("resolution") {
            if res_str.contains("1080p") && supported_resolutions.contains(&VideoResolution::Fhd1080p) {
                VideoResolution::Fhd1080p
            } else {
                VideoResolution::Hd720p
            }
        } else {
            VideoResolution::Hd720p
        };

        let is_flip = db_cfg.get("is_flip").map(|v| v == "true").unwrap_or(true);
        let rotation_deg = db_cfg.get("rotation_deg").and_then(|v| v.parse().ok()).unwrap_or(0);
        let zoom_level = db_cfg.get("zoom_level").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let action_shutter = db_cfg.get("action_shutter").map(|v| v == "true").unwrap_or(true);
        let auto_focus = db_cfg.get("auto_focus").map(|v| v == "true").unwrap_or(true);
        let manual_focus = db_cfg.get("manual_focus").and_then(|v| v.parse().ok()).unwrap_or(50);
        let auto_ev = db_cfg.get("auto_ev").map(|v| v == "true").unwrap_or(false);
        let ev_bias = db_cfg.get("ev_bias").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let brightness = db_cfg.get("brightness").and_then(|v| v.parse().ok()).unwrap_or(0);
        let contrast = db_cfg.get("contrast").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let saturation = db_cfg.get("saturation").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let focus_peaking = db_cfg.get("focus_peaking").map(|v| v == "true").unwrap_or(false);
        let peaking_threshold = db_cfg.get("peaking_threshold").and_then(|v| v.parse().ok()).unwrap_or(55);
        let cas_sharpen = db_cfg.get("cas_sharpen").map(|v| v == "true").unwrap_or(true);
        let cas_intensity = db_cfg.get("cas_intensity").and_then(|v| v.parse().ok()).unwrap_or(0.45);
        let frame_interpolation = db_cfg.get("frame_interpolation").map(|v| v == "true").unwrap_or(false);
        let face_detection = db_cfg.get("face_detection").map(|v| v == "true").unwrap_or(false);
        let anti_flicker = match db_cfg.get("anti_flicker").map(|s| s.as_str()) {
            Some("60hz") => AntiFlickerMode::Hz60,
            Some("off") => AntiFlickerMode::Off,
            _ => AntiFlickerMode::Hz50,
        };
        let grid_mode = match db_cfg.get("grid_mode").map(|s| s.as_str()) {
            Some("RuleOfThirds") => GridMode::RuleOfThirds,
            Some("CenterCrosshair") => GridMode::CenterCrosshair,
            Some("GoldenRatio") => GridMode::GoldenRatio,
            Some("SafeMargin") => GridMode::SafeMargin,
            _ => GridMode::Off,
        };

        // Restore custom shortcuts
        let mut keybindings = KeyBindings::default();
        if let Some(v) = db_cfg.get("shortcut_freeze").and_then(|s| parse_key_str(s)) { keybindings.freeze = v; }
        if let Some(v) = db_cfg.get("shortcut_flip").and_then(|s| parse_key_str(s)) { keybindings.flip = v; }
        if let Some(v) = db_cfg.get("shortcut_rotate").and_then(|s| parse_key_str(s)) { keybindings.rotate = v; }
        if let Some(v) = db_cfg.get("shortcut_grid").and_then(|s| parse_key_str(s)) { keybindings.grid = v; }
        if let Some(v) = db_cfg.get("shortcut_auto_focus").and_then(|s| parse_key_str(s)) { keybindings.auto_focus = v; }
        if let Some(v) = db_cfg.get("shortcut_auto_ev").and_then(|s| parse_key_str(s)) { keybindings.auto_ev = v; }
        if let Some(v) = db_cfg.get("shortcut_face_hud").and_then(|s| parse_key_str(s)) { keybindings.face_hud = v; }
        if let Some(v) = db_cfg.get("shortcut_interpolation").and_then(|s| parse_key_str(s)) { keybindings.interpolation = v; }
        if let Some(v) = db_cfg.get("shortcut_snapshot").and_then(|s| parse_key_str(s)) { keybindings.snapshot = v; }
        if let Some(v) = db_cfg.get("shortcut_record").and_then(|s| parse_key_str(s)) { keybindings.record = v; }
        if let Some(v) = db_cfg.get("shortcut_pip").and_then(|s| parse_key_str(s)) { keybindings.pip = v; }
        if let Some(v) = db_cfg.get("shortcut_fullscreen").and_then(|s| parse_key_str(s)) { keybindings.fullscreen = v; }
        if let Some(v) = db_cfg.get("shortcut_settings").and_then(|s| parse_key_str(s)) { keybindings.settings = v; }
        if let Some(v) = db_cfg.get("shortcut_zoom_in").and_then(|s| parse_key_str(s)) { keybindings.zoom_in = v; }
        if let Some(v) = db_cfg.get("shortcut_zoom_out").and_then(|s| parse_key_str(s)) { keybindings.zoom_out = v; }

        let denoise_mode = match db_cfg.get("denoise_mode").map(|s| s.as_str()) {
            Some("low") => DenoiseMode::Low,
            Some("medium") => DenoiseMode::Medium,
            Some("high") => DenoiseMode::High,
            _ => DenoiseMode::Off, // Default OFF as requested
        };
        let low_light_boost = db_cfg.get("low_light_boost").map(|v| v == "true").unwrap_or(false);
        let auto_white_balance = db_cfg.get("auto_white_balance").map(|v| v == "true").unwrap_or(false);
        let manual_kelvin = db_cfg.get("manual_kelvin").and_then(|v| v.parse().ok()).unwrap_or(5500);
        let iso_gain = db_cfg.get("iso_gain").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let color_profile = match db_cfg.get("color_profile").map(|s| s.as_str()) {
            Some("vivid") => ColorProfile::Vivid,
            Some("cinemalog") => ColorProfile::CinemaLog,
            Some("bw") => ColorProfile::BwMonochrome,
            _ => ColorProfile::Neutral,
        };
        let show_telemetry = db_cfg.get("show_telemetry").map(|v| v == "true").unwrap_or(false);
        let captures_path = db_cfg.get("captures_path").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("captures"));
        let mic_denoise = db_cfg.get("mic_denoise").map(|v| v == "true").unwrap_or(true);
        let is_virtual_cam_active = db_cfg.get("is_virtual_cam_active").map(|v| v == "true").unwrap_or(false);

        let camera_manager = if let Some(ref dev) = selected_device {
            let mgr = CameraManager::start(dev.index, resolution);
            mgr.send(CameraControl::SetFlip(is_flip));
            mgr.send(CameraControl::SetRotate(rotation_deg));
            mgr.send(CameraControl::SetZoom(zoom_level));
            mgr.send(CameraControl::SetActionShutter(action_shutter));
            mgr.send(CameraControl::SetAutoFocus(auto_focus));
            mgr.send(CameraControl::SetManualFocus(manual_focus));
            mgr.send(CameraControl::SetAutoEv(auto_ev));
            mgr.send(CameraControl::SetEvBias(ev_bias));
            mgr.send(CameraControl::SetBrightness(brightness));
            mgr.send(CameraControl::SetContrast(contrast));
            mgr.send(CameraControl::SetSaturation(saturation));
            mgr.send(CameraControl::SetFocusPeaking(focus_peaking));
            mgr.send(CameraControl::SetPeakingThreshold(peaking_threshold));
            mgr.send(CameraControl::SetCasSharpen(cas_sharpen));
            mgr.send(CameraControl::SetCasIntensity(cas_intensity));
            mgr.send(CameraControl::SetFrameInterpolation(frame_interpolation));
            mgr.send(CameraControl::SetFaceDetection(face_detection));
            mgr.send(CameraControl::SetAntiFlicker(anti_flicker));
            mgr.send(CameraControl::SetDenoiseMode(denoise_mode));
            mgr.send(CameraControl::SetLowLightBoost(low_light_boost));
            mgr.send(CameraControl::SetAutoWhiteBalance(auto_white_balance));
            mgr.send(CameraControl::SetManualKelvin(manual_kelvin));
            mgr.send(CameraControl::SetIsoGain(iso_gain));
            mgr.send(CameraControl::SetColorProfile(color_profile));
            Some(mgr)
        } else {
            None
        };

        let status_text = if !crate::camera::is_media_foundation_available() {
            "Windows Media Feature Pack required".to_string()
        } else if let Some(ref dev) = selected_device {
            format!("Connected: {}", dev.name)
        } else {
            "No camera detected".to_string()
        };

        let media_items = scan_captures_folder(&captures_path);
        let latest_capture = media_items.first().cloned();

        let audio_devices = audio::query_audio_input_devices();
        let saved_audio_name = db_cfg.get("audio_device");
        let selected_audio_device = if let Some(saved_name) = saved_audio_name {
            audio_devices.iter().find(|d| &d.name == saved_name).cloned().or_else(|| audio_devices.first().cloned())
        } else {
            audio_devices.first().cloned()
        };
        let is_mic_muted = db_cfg.get("is_mic_muted").map(|v| v == "true").unwrap_or(false);

        let app = Self {
            main_window_id: Some(main_id),
            settings_window_id: None,
            album_window_id: None,
            viewer_window_id: None,

            camera_manager,
            available_devices: devices,
            selected_device,
            supported_resolutions,
            resolution,
            is_flip,
            is_frozen: false,
            is_fullscreen: false,
            rotation_deg,
            zoom_level,
            image_handle: None,
            app_icon_handle: image::Handle::from_bytes(APP_ICON_BYTES),
            status_text,

            grid_mode,
            is_loading_camera: false,
            action_shutter,
            auto_focus,
            manual_focus,
            auto_ev,
            ev_bias,
            brightness,
            contrast,
            saturation,
            focus_peaking,
            peaking_threshold,
            cas_sharpen,
            cas_intensity,
            frame_interpolation,
            face_detection,
            anti_flicker,
            latest_frame: None,
            snapshot_toast: None,

            // Video Recording
            video_recorder: None,
            is_recording: false,
            recording_start_time: None,
            recording_elapsed_secs: 0,

            // Audio / Microphone
            available_audio_devices: audio_devices,
            selected_audio_device,
            is_mic_muted,

            // Album
            media_items,
            latest_capture,
            album_hovered: false,
            album_filter: AlbumFilter::All,
            hovered_media_idx: None,
            active_media_menu_idx: None,

            // Photo & Video Viewer
            viewer_selected_path: None,
            viewer_image_handle: None,
            viewer_info_text: String::new(),
            viewer_video_state: None,

            keybindings,
            rebinding_action: None,

            denoise_mode,
            low_light_boost,
            auto_white_balance,
            mic_denoise,

            manual_kelvin,
            iso_gain,
            color_profile,
            show_telemetry,
            last_frame_instant: None,
            live_fps: 60.0,
            live_frame_time_ms: 16.6,
            captures_path,

            virtual_cam_bridge: if is_virtual_cam_active {
                Some(virtual_cam::VirtualCamBridge::start())
            } else {
                None
            },
            is_virtual_cam_active,
            virtual_cam_status_text: if is_virtual_cam_active {
                "Virtual Cam: Active (OBS / Meet / Zoom)".to_string()
            } else {
                "Virtual Cam: Inactive".to_string()
            },

            is_pip: false,
            pip_hovered: false,
        };

        (app, open_task.map(Message::MainWindowOpened))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let hotplug_sub = iced::time::every(Duration::from_millis(1500)).map(|_| Message::PollDevices);
        let frame_sub = Subscription::run(camera_frame_stream);
        let keyboard_sub = keyboard::listen().map(Message::KeyboardEvent);
        let timer_sub = iced::time::every(Duration::from_secs(1)).map(|_| Message::RecordingTick);

        let mut subs = vec![hotplug_sub, frame_sub, keyboard_sub, timer_sub];

        if self.viewer_window_id.is_some() {
            if let Some(ref v) = self.viewer_video_state {
                if v.is_playing {
                    let fps = v.fps.clamp(1, 60);
                    let interval_ms = (1000 / fps as u64).max(16);
                    subs.push(iced::time::every(Duration::from_millis(interval_ms)).map(|_| Message::VideoViewerTick));
                }
            }
        }

        Subscription::batch(subs)
    }

    pub fn theme(&self, _window_id: window::Id) -> iced::Theme {
        iced::Theme::Dark
    }

    pub fn refresh_media_items(&mut self) {
        self.media_items = scan_captures_folder(&self.captures_path);
        self.latest_capture = self.media_items.first().cloned();
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

            Message::AlbumWindowOpened(id) => {
                self.album_window_id = Some(id);
                Task::none()
            }

            Message::ViewerWindowOpened(id) => {
                self.viewer_window_id = Some(id);
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

            Message::OpenAlbumWindow => {
                self.refresh_media_items();
                if let Some(id) = self.album_window_id {
                    window::gain_focus(id)
                } else {
                    let (id, task) = window::open(window::Settings {
                        icon: window::icon::from_file_data(APP_ICON_BYTES, None).ok(),
                        decorations: false,
                        transparent: false,
                        size: iced::Size::new(880.0, 620.0),
                        min_size: Some(iced::Size::new(540.0, 420.0)),
                        resizable: true,
                        ..Default::default()
                    });
                    self.album_window_id = Some(id);
                    task.map(Message::AlbumWindowOpened)
                }
            }

            Message::CloseAlbumWindow => {
                self.active_media_menu_idx = None;
                self.hovered_media_idx = None;
                if let Some(id) = self.album_window_id.take() {
                    window::close(id)
                } else {
                    Task::none()
                }
            }

            Message::AlbumHovered(h) => {
                self.album_hovered = h;
                Task::none()
            }

            Message::SetAlbumFilter(f) => {
                self.album_filter = f;
                self.active_media_menu_idx = None;
                Task::none()
            }

            Message::RefreshAlbum => {
                self.refresh_media_items();
                Task::none()
            }

            Message::PlayVideo(path) => {
                self.active_media_menu_idx = None;
                if let Some(info) = recorder::parse_avi_video(&path) {
                    if let Some(loc) = info.frame_locations.first() {
                        if let Some(jpeg) = recorder::read_frame_jpeg(&path, loc.0, loc.1) {
                            self.viewer_image_handle = Some(image::Handle::from_bytes(jpeg));
                        }
                    }
                    let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let total_secs = info.total_frames / info.fps.max(1) as usize;
                    self.viewer_info_text = format!("{} • {} FPS • {} frames ({:02}:{:02})", fname, info.fps, info.total_frames, total_secs / 60, total_secs % 60);
                    self.viewer_selected_path = Some(path.clone());
                    self.viewer_video_state = Some(ViewerVideoState {
                        path: path.clone(),
                        fps: info.fps,
                        frames: info.frame_locations,
                        current_frame: 0,
                        is_playing: true,
                    });

                    if let Some(id) = self.viewer_window_id {
                        return window::gain_focus(id);
                    } else {
                        let (id, task) = window::open(window::Settings {
                            icon: window::icon::from_file_data(APP_ICON_BYTES, None).ok(),
                            decorations: false,
                            transparent: false,
                            size: iced::Size::new(960.0, 640.0),
                            min_size: Some(iced::Size::new(480.0, 360.0)),
                            resizable: true,
                            ..Default::default()
                        });
                        self.viewer_window_id = Some(id);
                        return task.map(Message::ViewerWindowOpened);
                    }
                } else {
                    let path_str = path.to_string_lossy().to_string();
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &path_str])
                        .spawn();
                    Task::none()
                }
            }

            Message::VideoViewerTick => {
                if let Some(ref mut state) = self.viewer_video_state {
                    if state.is_playing && !state.frames.is_empty() {
                        state.current_frame = (state.current_frame + 1) % state.frames.len();
                        let (offset, len) = state.frames[state.current_frame];
                        if let Some(jpeg) = recorder::read_frame_jpeg(&state.path, offset, len) {
                            self.viewer_image_handle = Some(image::Handle::from_bytes(jpeg));
                        }
                    }
                }
                Task::none()
            }

            Message::ToggleVideoPlayback => {
                if let Some(ref mut state) = self.viewer_video_state {
                    state.is_playing = !state.is_playing;
                }
                Task::none()
            }

            Message::SeekVideo(frame_idx) => {
                if let Some(ref mut state) = self.viewer_video_state {
                    let max_idx = state.frames.len().saturating_sub(1);
                    state.current_frame = frame_idx.min(max_idx);
                    let (offset, len) = state.frames[state.current_frame];
                    if let Some(jpeg) = recorder::read_frame_jpeg(&state.path, offset, len) {
                        self.viewer_image_handle = Some(image::Handle::from_bytes(jpeg));
                    }
                }
                Task::none()
            }

            Message::ReplayVideo => {
                if let Some(ref mut state) = self.viewer_video_state {
                    state.current_frame = 0;
                    state.is_playing = true;
                    if let Some(loc) = state.frames.first() {
                        if let Some(jpeg) = recorder::read_frame_jpeg(&state.path, loc.0, loc.1) {
                            self.viewer_image_handle = Some(image::Handle::from_bytes(jpeg));
                        }
                    }
                }
                Task::none()
            }

            Message::MediaHovered(idx) => {
                self.hovered_media_idx = idx;
                Task::none()
            }

            Message::ToggleMediaMenu(idx) => {
                if self.active_media_menu_idx == idx {
                    self.active_media_menu_idx = None;
                } else {
                    self.active_media_menu_idx = idx;
                }
                Task::none()
            }

            Message::OpenInExplorer(path) => {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{}", path.display()))
                    .spawn();
                self.active_media_menu_idx = None;
                Task::none()
            }

            Message::CopyMediaPath(path) => {
                let path_str = path.to_string_lossy().to_string();
                self.active_media_menu_idx = None;
                self.snapshot_toast = Some(("Path copied to clipboard".to_string(), Instant::now()));
                iced::clipboard::write(path_str)
            }

            Message::CopyMedia(path) => {
                let path_str = path.to_string_lossy().to_string();
                self.active_media_menu_idx = None;
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &format!("Set-Clipboard -Path '{}'", path_str.replace('\'', "''"))])
                    .spawn();
                self.snapshot_toast = Some(("Media copied to clipboard".to_string(), Instant::now()));
                Task::perform(
                    tokio::time::sleep(Duration::from_secs(2)),
                    |_| Message::ClearSnapshotToast,
                )
            }

            Message::DeleteMedia(path) => {
                let _ = std::fs::remove_file(&path);
                self.active_media_menu_idx = None;
                self.refresh_media_items();
                self.snapshot_toast = Some(("Deleted media file".to_string(), Instant::now()));
                Task::perform(
                    tokio::time::sleep(Duration::from_secs(2)),
                    |_| Message::ClearSnapshotToast,
                )
            }

            Message::OpenViewerWindow(path) => {
                self.active_media_menu_idx = None;
                self.viewer_video_state = None;
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(img) = ::image::load_from_memory(&bytes) {
                        let w = img.width();
                        let h = img.height();
                        let size_bytes = bytes.len() as u64;
                        let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        self.viewer_info_text = format!("{} • {}x{} • {}", fname, w, h, format_size(size_bytes));
                        self.viewer_image_handle = Some(image::Handle::from_bytes(bytes));
                        self.viewer_selected_path = Some(path);

                        if let Some(id) = self.viewer_window_id {
                            return window::gain_focus(id);
                        } else {
                            let (id, task) = window::open(window::Settings {
                                icon: window::icon::from_file_data(APP_ICON_BYTES, None).ok(),
                                decorations: false,
                                transparent: false,
                                size: iced::Size::new(960.0, 640.0),
                                min_size: Some(iced::Size::new(480.0, 360.0)),
                                resizable: true,
                                ..Default::default()
                            });
                            self.viewer_window_id = Some(id);
                            return task.map(Message::ViewerWindowOpened);
                        }
                    }
                }
                Task::none()
            }

            Message::CloseViewerWindow => {
                self.viewer_image_handle = None;
                self.viewer_selected_path = None;
                self.viewer_video_state = None;
                if let Some(id) = self.viewer_window_id.take() {
                    window::close(id)
                } else {
                    Task::none()
                }
            }

            Message::CloseApp => {
                if let Some(rec) = self.video_recorder.take() {
                    let _ = rec.stop();
                }
                exit()
            }

            Message::DragWindow(id) => window::drag(id),

            Message::ResizeWindow(id, direction) => window::drag_resize(id, direction),

            Message::MinimizeWindow(id) => window::minimize(id, true),

            Message::ToggleMaximizeWindow(id) => window::toggle_maximize(id),

            Message::FrameReceived(frame) => {
                let now = Instant::now();
                if let Some(prev) = self.last_frame_instant {
                    let delta_s = prev.elapsed().as_secs_f32();
                    if delta_s > 0.001 {
                        let fps = 1.0 / delta_s;
                        self.live_fps = self.live_fps * 0.90 + fps * 0.10;
                        self.live_frame_time_ms = delta_s * 1000.0;
                    }
                }
                self.last_frame_instant = Some(now);

                self.is_loading_camera = false;
                self.image_handle = Some(image::Handle::from_rgba(
                    frame.width,
                    frame.height,
                    frame.rgba.clone(),
                ));

                // If recording, feed frame to background video recorder
                if self.is_recording {
                    if let Some(ref rec) = self.video_recorder {
                        rec.append_frame(Arc::new(frame.rgba.clone()));
                    }
                }

                // If virtual camera output is active, feed frame to virtual camera bridge
                if self.is_virtual_cam_active {
                    if let Some(ref bridge) = self.virtual_cam_bridge {
                        bridge.send_frame(frame.width, frame.height, Arc::new(frame.rgba.clone()));
                    }
                }

                self.latest_frame = Some(frame);
                Task::none()
            }

            Message::RecordingTick => {
                if self.is_recording {
                    if let Some(start) = self.recording_start_time {
                        self.recording_elapsed_secs = start.elapsed().as_secs();
                    }
                }
                Task::none()
            }

            Message::AudioDeviceSelected(dev) => {
                self.selected_audio_device = Some(dev.clone());
                db::set("audio_device", &dev.name);
                Task::none()
            }

            Message::ToggleMicMute => {
                self.is_mic_muted = !self.is_mic_muted;
                db::set("is_mic_muted", &self.is_mic_muted.to_string());
                self.snapshot_toast = Some((
                    if self.is_mic_muted { "Microphone Muted".to_string() } else { "Microphone Unmuted".to_string() },
                    Instant::now(),
                ));
                Task::perform(
                    tokio::time::sleep(Duration::from_millis(1200)),
                    |_| Message::ClearSnapshotToast,
                )
            }

            Message::ToggleRecording => {
                if !self.is_recording {
                    let _ = std::fs::create_dir_all(&self.captures_path);
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let path = self.captures_path.join(format!("autocam_rec_{}.avi", timestamp));
                    let (w, h) = if let Some(ref frame) = self.latest_frame {
                        (frame.width, frame.height)
                    } else {
                        (1280, 720)
                    };

                    match recorder::VideoRecorder::start(w, h, 30, path.clone()) {
                        Ok(rec) => {
                            self.video_recorder = Some(rec);
                            self.is_recording = true;
                            self.recording_start_time = Some(Instant::now());
                            self.recording_elapsed_secs = 0;
                            self.snapshot_toast = Some(("Recording started...".to_string(), Instant::now()));
                            return Task::perform(
                                tokio::time::sleep(Duration::from_millis(600)),
                                |_| Message::ClearSnapshotToast,
                            );
                        }
                        Err(e) => {
                            self.snapshot_toast = Some((format!("Record error: {}", e), Instant::now()));
                            return Task::perform(
                                tokio::time::sleep(Duration::from_millis(2000)),
                                |_| Message::ClearSnapshotToast,
                            );
                        }
                    }
                } else {
                    self.is_recording = false;
                    self.recording_start_time = None;
                    if let Some(rec) = self.video_recorder.take() {
                        let saved = rec.stop();
                        if let Some(p) = saved {
                            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                            self.snapshot_toast = Some((format!("Video saved to {}", name), Instant::now()));
                        }
                    }
                    self.refresh_media_items();
                    return Task::perform(
                        tokio::time::sleep(Duration::from_millis(1500)),
                        |_| Message::ClearSnapshotToast,
                    );
                }
            }

            Message::ClearSnapshotToast => {
                self.snapshot_toast = None;
                Task::none()
            }

            Message::PollDevices => {
                Task::perform(
                    async { CameraManager::query_available_devices() },
                    Message::DevicesPolled,
                )
            }

            Message::DevicesPolled(devices) => {
                let diff = devices.len() != self.available_devices.len()
                    || devices
                        .iter()
                        .zip(&self.available_devices)
                        .any(|(a, b)| a.index != b.index);

                if diff {
                    self.available_devices = devices;
                    if self.selected_device.is_none() && !self.available_devices.is_empty() {
                        let dev = self.available_devices[0].clone();
                        return self.update(Message::DeviceSelected(dev));
                    }
                }
                Task::none()
            }

            Message::KeyboardEvent(keyboard::Event::KeyPressed { key, .. }) => {
                if let Some(action) = self.rebinding_action {
                    if matches!(key, Key::Named(key::Named::Escape)) {
                        self.rebinding_action = None;
                        return Task::none();
                    }
                    let key_str = format_key(&key);
                    match action {
                        ShortcutAction::Freeze => { self.keybindings.freeze = key; db::set("shortcut_freeze", &key_str); }
                        ShortcutAction::Flip => { self.keybindings.flip = key; db::set("shortcut_flip", &key_str); }
                        ShortcutAction::Rotate => { self.keybindings.rotate = key; db::set("shortcut_rotate", &key_str); }
                        ShortcutAction::Grid => { self.keybindings.grid = key; db::set("shortcut_grid", &key_str); }
                        ShortcutAction::AutoFocus => { self.keybindings.auto_focus = key; db::set("shortcut_auto_focus", &key_str); }
                        ShortcutAction::AutoEv => { self.keybindings.auto_ev = key; db::set("shortcut_auto_ev", &key_str); }
                        ShortcutAction::FaceHud => { self.keybindings.face_hud = key; db::set("shortcut_face_hud", &key_str); }
                        ShortcutAction::Interpolation => { self.keybindings.interpolation = key; db::set("shortcut_interpolation", &key_str); }
                        ShortcutAction::Snapshot => { self.keybindings.snapshot = key; db::set("shortcut_snapshot", &key_str); }
                        ShortcutAction::Record => { self.keybindings.record = key; db::set("shortcut_record", &key_str); }
                        ShortcutAction::Pip => { self.keybindings.pip = key; db::set("shortcut_pip", &key_str); }
                        ShortcutAction::Fullscreen => { self.keybindings.fullscreen = key; db::set("shortcut_fullscreen", &key_str); }
                        ShortcutAction::Settings => { self.keybindings.settings = key; db::set("shortcut_settings", &key_str); }
                        ShortcutAction::ZoomIn => { self.keybindings.zoom_in = key; db::set("shortcut_zoom_in", &key_str); }
                        ShortcutAction::ZoomOut => { self.keybindings.zoom_out = key; db::set("shortcut_zoom_out", &key_str); }
                    }
                    self.rebinding_action = None;
                    return Task::none();
                }

                if matches!(key, Key::Named(key::Named::Escape)) {
                    if self.viewer_window_id.is_some() {
                        return self.update(Message::CloseViewerWindow);
                    }
                    if self.album_window_id.is_some() {
                        return self.update(Message::CloseAlbumWindow);
                    }
                    if self.settings_window_id.is_some() {
                        return self.update(Message::CloseSettingsWindow);
                    }
                    if self.is_fullscreen {
                        return self.update(Message::ToggleFullscreen);
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
                } else if key_matches(&key, &self.keybindings.record) {
                    return self.update(Message::ToggleRecording);
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
                Task::none()
            }

            Message::KeyboardEvent(_) => Task::none(),

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
                db::set("shortcut_freeze", "Space");
                db::set("shortcut_flip", "F");
                db::set("shortcut_rotate", "R");
                db::set("shortcut_grid", "G");
                db::set("shortcut_auto_focus", "A");
                db::set("shortcut_auto_ev", "E");
                db::set("shortcut_face_hud", "D");
                db::set("shortcut_interpolation", "I");
                db::set("shortcut_snapshot", "C");
                db::set("shortcut_record", "V");
                db::set("shortcut_pip", "P");
                db::set("shortcut_fullscreen", "F11");
                db::set("shortcut_settings", "S");
                db::set("shortcut_zoom_in", "=");
                db::set("shortcut_zoom_out", "-");
                Task::none()
            }

            Message::DeviceSelected(dev) => {
                self.selected_device = Some(dev.clone());
                self.is_loading_camera = true;
                self.status_text = format!("Connecting: {}", dev.name);

                self.supported_resolutions = CameraManager::query_supported_resolutions(dev.index);
                if !self.supported_resolutions.contains(&self.resolution) {
                    self.resolution = self
                        .supported_resolutions
                        .first()
                        .copied()
                        .unwrap_or(VideoResolution::Hd720p);
                }

                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SwitchDevice(dev.index));
                } else {
                    let mgr = CameraManager::start(dev.index, self.resolution);
                    mgr.send(CameraControl::SetFlip(self.is_flip));
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                    mgr.send(CameraControl::SetActionShutter(self.action_shutter));
                    mgr.send(CameraControl::SetAutoFocus(self.auto_focus));
                    mgr.send(CameraControl::SetManualFocus(self.manual_focus));
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
                    mgr.send(CameraControl::SetAntiFlicker(self.anti_flicker));
                    self.camera_manager = Some(mgr);
                }

                db::set("device_index", &dev.index.to_string());
                Task::none()
            }

            Message::ToggleResolution => {
                let next_res = match self.resolution {
                    VideoResolution::Hd720p => {
                        if self.supported_resolutions.contains(&VideoResolution::Fhd1080p) {
                            VideoResolution::Fhd1080p
                        } else {
                            VideoResolution::Hd720p
                        }
                    }
                    VideoResolution::Fhd1080p => VideoResolution::Hd720p,
                };
                self.update(Message::SetResolution(next_res))
            }

            Message::SetResolution(res) => {
                if self.resolution != res {
                    self.resolution = res;
                    self.is_loading_camera = true;
                    db::set("resolution", res.label());
                    if let Some(ref mgr) = self.camera_manager {
                        mgr.send(CameraControl::SwitchResolution(res));
                    }
                }
                Task::none()
            }

            Message::ToggleFlip => {
                self.is_flip = !self.is_flip;
                db::set("is_flip", &self.is_flip.to_string());
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
                self.rotation_deg = (self.rotation_deg + 90) % 360;
                db::set("rotation_deg", &self.rotation_deg.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                }
                Task::none()
            }

            Message::SetRotation(deg) => {
                self.rotation_deg = deg % 360;
                db::set("rotation_deg", &self.rotation_deg.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetRotate(self.rotation_deg));
                }
                Task::none()
            }

            Message::ZoomIn => {
                let next = (self.zoom_level + 0.25).min(4.0);
                self.update(Message::SetZoom(next))
            }

            Message::ZoomOut => {
                let next = (self.zoom_level - 0.25).max(1.0);
                self.update(Message::SetZoom(next))
            }

            Message::SetZoom(level) => {
                self.zoom_level = level.clamp(1.0, 4.0);
                db::set("zoom_level", &format!("{:.2}", self.zoom_level));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetZoom(self.zoom_level));
                }
                Task::none()
            }

            Message::CycleGridMode => {
                self.grid_mode = self.grid_mode.next();
                db::set("grid_mode", match self.grid_mode {
                    GridMode::RuleOfThirds => "RuleOfThirds",
                    GridMode::CenterCrosshair => "CenterCrosshair",
                    GridMode::GoldenRatio => "GoldenRatio",
                    GridMode::SafeMargin => "SafeMargin",
                    GridMode::Off => "Off",
                });
                Task::none()
            }

            Message::SetGridMode(mode) => {
                self.grid_mode = mode;
                db::set("grid_mode", match self.grid_mode {
                    GridMode::RuleOfThirds => "RuleOfThirds",
                    GridMode::CenterCrosshair => "CenterCrosshair",
                    GridMode::GoldenRatio => "GoldenRatio",
                    GridMode::SafeMargin => "SafeMargin",
                    GridMode::Off => "Off",
                });
                Task::none()
            }

            Message::ToggleActionShutter => {
                self.action_shutter = !self.action_shutter;
                db::set("action_shutter", &self.action_shutter.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetActionShutter(self.action_shutter));
                }
                Task::none()
            }

            Message::ToggleAutoFocus => {
                self.auto_focus = !self.auto_focus;
                db::set("auto_focus", &self.auto_focus.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAutoFocus(self.auto_focus));
                }
                Task::none()
            }

            Message::SetManualFocus(val) => {
                self.manual_focus = val;
                db::set("manual_focus", &val.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetManualFocus(val));
                }
                Task::none()
            }

            Message::ToggleAutoEv => {
                self.auto_ev = !self.auto_ev;
                db::set("auto_ev", &self.auto_ev.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAutoEv(self.auto_ev));
                }
                Task::none()
            }

            Message::SetEvBias(val) => {
                self.ev_bias = val;
                db::set("ev_bias", &format!("{:.2}", val));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetEvBias(val));
                }
                Task::none()
            }

            Message::SetBrightness(val) => {
                self.brightness = val;
                db::set("brightness", &val.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetBrightness(val));
                }
                Task::none()
            }

            Message::SetContrast(val) => {
                self.contrast = val;
                db::set("contrast", &format!("{:.2}", val));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetContrast(val));
                }
                Task::none()
            }

            Message::SetSaturation(val) => {
                self.saturation = val;
                db::set("saturation", &format!("{:.2}", val));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetSaturation(val));
                }
                Task::none()
            }

            Message::ToggleFocusPeaking => {
                self.focus_peaking = !self.focus_peaking;
                db::set("focus_peaking", &self.focus_peaking.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFocusPeaking(self.focus_peaking));
                }
                Task::none()
            }

            Message::SetPeakingThreshold(val) => {
                self.peaking_threshold = val;
                db::set("peaking_threshold", &val.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetPeakingThreshold(val));
                }
                Task::none()
            }

            Message::ToggleCasSharpen => {
                self.cas_sharpen = !self.cas_sharpen;
                db::set("cas_sharpen", &self.cas_sharpen.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetCasSharpen(self.cas_sharpen));
                }
                Task::none()
            }

            Message::SetCasIntensity(val) => {
                self.cas_intensity = val;
                db::set("cas_intensity", &format!("{:.2}", val));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetCasIntensity(val));
                }
                Task::none()
            }

            Message::ToggleFrameInterpolation => {
                self.frame_interpolation = !self.frame_interpolation;
                db::set("frame_interpolation", &self.frame_interpolation.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFrameInterpolation(self.frame_interpolation));
                }
                Task::none()
            }

            Message::ToggleFaceDetection => {
                self.face_detection = !self.face_detection;
                db::set("face_detection", &self.face_detection.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetFaceDetection(self.face_detection));
                }
                Task::none()
            }

            Message::SetAntiFlicker(mode) => {
                self.anti_flicker = mode;
                db::set("anti_flicker", match mode {
                    AntiFlickerMode::Hz50 => "50hz",
                    AntiFlickerMode::Hz60 => "60hz",
                    AntiFlickerMode::Off => "off",
                });
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAntiFlicker(mode));
                }
                Task::none()
            }

            Message::SetDenoiseMode(mode) => {
                self.denoise_mode = mode;
                db::set("denoise_mode", match mode {
                    DenoiseMode::Off => "off",
                    DenoiseMode::Low => "low",
                    DenoiseMode::Medium => "medium",
                    DenoiseMode::High => "high",
                });
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetDenoiseMode(mode));
                }
                self.status_text = format!("Denoise: {}", mode.label());
                Task::none()
            }

            Message::ToggleLowLightBoost => {
                self.low_light_boost = !self.low_light_boost;
                db::set("low_light_boost", &self.low_light_boost.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetLowLightBoost(self.low_light_boost));
                }
                self.status_text = if self.low_light_boost {
                    "Low-Light Boost Active".to_string()
                } else {
                    "Low-Light Boost Off".to_string()
                };
                Task::none()
            }

            Message::ToggleAutoWhiteBalance => {
                self.auto_white_balance = !self.auto_white_balance;
                db::set("auto_white_balance", &self.auto_white_balance.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetAutoWhiteBalance(self.auto_white_balance));
                }
                self.status_text = if self.auto_white_balance {
                    "Auto White Balance Active".to_string()
                } else {
                    "Auto White Balance Off".to_string()
                };
                Task::none()
            }

            Message::ToggleMicDenoise => {
                self.mic_denoise = !self.mic_denoise;
                db::set("mic_denoise", &self.mic_denoise.to_string());
                self.status_text = if self.mic_denoise {
                    "Mic AI Noise Suppression Active".to_string()
                } else {
                    "Mic AI Noise Suppression Off".to_string()
                };
                Task::none()
            }

            Message::ToggleVirtualCamera => {
                self.is_virtual_cam_active = !self.is_virtual_cam_active;
                db::set("is_virtual_cam_active", &self.is_virtual_cam_active.to_string());

                if self.is_virtual_cam_active {
                    let status = virtual_cam::probe_virtual_cam_availability();
                    match status {
                        virtual_cam::VirtualCamStatus::ActiveObs => {
                            self.virtual_cam_bridge = Some(virtual_cam::VirtualCamBridge::start());
                            self.virtual_cam_status_text = "Virtual Cam: Active (OBS Virtual Camera)".to_string();
                            self.status_text = "Virtual Camera: Streaming to OBS".to_string();
                        }
                        virtual_cam::VirtualCamStatus::ActiveMediaFoundation => {
                            self.virtual_cam_bridge = Some(virtual_cam::VirtualCamBridge::start());
                            self.virtual_cam_status_text = "Virtual Cam: Active (Media Foundation)".to_string();
                            self.status_text = "Virtual Camera: Streaming to Windows VCam".to_string();
                        }
                        _ => {
                            self.virtual_cam_bridge = Some(virtual_cam::VirtualCamBridge::start());
                            self.virtual_cam_status_text = "Virtual Cam: Active (OBS Driver / Fallback)".to_string();
                            self.status_text = "Virtual Camera Active (Select OBS Virtual Camera in Zoom/Meet)".to_string();
                        }
                    }
                } else {
                    self.virtual_cam_bridge = None;
                    self.virtual_cam_status_text = "Virtual Cam: Off".to_string();
                    self.status_text = "Virtual Camera Output Stopped".to_string();
                }
                Task::none()
            }

            Message::CaptureSnapshot => {
                if let Some(ref frame) = self.latest_frame {
                    let _ = std::fs::create_dir_all(&self.captures_path);
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let target_path = self.captures_path.join(format!("autocam_{}.png", timestamp));
                    let path_str = target_path.to_string_lossy().to_string();

                    if ::image::save_buffer(
                        &target_path,
                        &frame.rgba,
                        frame.width,
                        frame.height,
                        ::image::ExtendedColorType::Rgba8,
                    )
                    .is_ok()
                    {
                        self.snapshot_toast = Some((path_str, Instant::now()));
                        self.refresh_media_items();
                        return Task::perform(
                            tokio::time::sleep(Duration::from_secs(3)),
                            |_| Message::ClearSnapshotToast,
                        );
                    }
                }
                Task::none()
            }

            Message::OpenCapturesFolder => {
                let _ = std::fs::create_dir_all(&self.captures_path);
                let _ = std::process::Command::new("explorer").arg(&self.captures_path).spawn();
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
                self.denoise_mode = DenoiseMode::Off;
                self.low_light_boost = false;
                self.auto_white_balance = false;
                self.manual_kelvin = 5500;
                self.iso_gain = 1.0;
                self.color_profile = ColorProfile::Neutral;

                db::set("ev_bias", "0.0");
                db::set("brightness", "0");
                db::set("contrast", "1.0");
                db::set("saturation", "1.0");
                db::set("zoom_level", "1.0");
                db::set("is_flip", "true");
                db::set("rotation_deg", "0");
                db::set("focus_peaking", "false");
                db::set("peaking_threshold", "55");
                db::set("cas_sharpen", "true");
                db::set("cas_intensity", "0.45");
                db::set("frame_interpolation", "false");
                db::set("face_detection", "false");
                db::set("action_shutter", "true");
                db::set("anti_flicker", "50hz");
                db::set("denoise_mode", "off");
                db::set("low_light_boost", "false");
                db::set("auto_white_balance", "false");
                db::set("manual_kelvin", "5500");
                db::set("iso_gain", "1.0");
                db::set("color_profile", "neutral");

                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::ResetDefaults);
                }
                Task::none()
            }

            Message::SetManualKelvin(k) => {
                self.manual_kelvin = k;
                db::set("manual_kelvin", &k.to_string());
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetManualKelvin(k));
                }
                self.status_text = format!("Color Temperature: {}K", k);
                Task::none()
            }

            Message::SetIsoGain(iso) => {
                self.iso_gain = iso;
                db::set("iso_gain", &format!("{:.2}", iso));
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetIsoGain(iso));
                }
                let simulated_iso = (iso * 100.0) as u32;
                self.status_text = format!("ISO Sensitivity: ~{}", simulated_iso);
                Task::none()
            }

            Message::SetColorProfile(cp) => {
                self.color_profile = cp;
                db::set("color_profile", match cp {
                    ColorProfile::Neutral => "neutral",
                    ColorProfile::Vivid => "vivid",
                    ColorProfile::CinemaLog => "cinemalog",
                    ColorProfile::BwMonochrome => "bw",
                });
                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::SetColorProfile(cp));
                }
                self.status_text = format!("Picture Profile: {}", cp.label());
                Task::none()
            }

            Message::ToggleTelemetry => {
                self.show_telemetry = !self.show_telemetry;
                db::set("show_telemetry", &self.show_telemetry.to_string());
                self.status_text = if self.show_telemetry {
                    "Telemetry HUD Active".to_string()
                } else {
                    "Telemetry HUD Off".to_string()
                };
                Task::none()
            }

            Message::SetCapturesPath(path) => {
                let _ = std::fs::create_dir_all(&path);
                self.captures_path = path.clone();
                db::set("captures_path", &path.to_string_lossy());
                self.refresh_media_items();
                self.status_text = format!("Storage path: {}", path.to_string_lossy());
                Task::none()
            }

            Message::PurgeAllCaptures => {
                if let Ok(entries) = std::fs::read_dir(&self.captures_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let _ = std::fs::remove_file(p);
                        }
                    }
                }
                self.refresh_media_items();
                self.status_text = "All captures cleared from storage".to_string();
                Task::none()
            }

            Message::OpenDatabaseFolder => {
                let (path_str, _, _) = db::get_db_info();
                let path = PathBuf::from(path_str);
                if let Some(parent) = path.parent() {
                    let _ = std::process::Command::new("explorer").arg(parent).spawn();
                }
                Task::none()
            }

            Message::ResetFactorySettings => {
                db::reset_factory_defaults();
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
                self.denoise_mode = DenoiseMode::Off;
                self.low_light_boost = false;
                self.auto_white_balance = false;
                self.manual_kelvin = 5500;
                self.iso_gain = 1.0;
                self.color_profile = ColorProfile::Neutral;
                self.captures_path = PathBuf::from("captures");
                self.show_telemetry = false;

                if let Some(ref mgr) = self.camera_manager {
                    mgr.send(CameraControl::ResetDefaults);
                }
                self.refresh_media_items();
                self.status_text = "Factory reset: Database & settings restored".to_string();
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
        } else if Some(window_id) == self.album_window_id {
            self.view_album_window(window_id)
        } else if Some(window_id) == self.viewer_window_id {
            self.view_viewer_window(window_id)
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
                    // Group 1: Brand / Logo
                    app_logo,
                    text("AutoCam")
                        .size(13)
                        .color(theme::COLOR_TEXT),
                    toolbar_divider(),
                    // Group 2: Camera Stream Devices & Resolution
                    pick_list(
                        self.available_devices.clone(),
                        self.selected_device.clone(),
                        Message::DeviceSelected,
                    )
                    .style(theme::pick_list_style)
                    .menu_style(theme::menu_style)
                    .text_size(11)
                    .padding([3, 8]),
                    // Resolution Toggle Button
                    button(text(self.resolution.label()).size(11))
                        .on_press(Message::ToggleResolution)
                        .style(theme::clean_button_style(false, false))
                        .padding([3, 8]),
                    // Hardware Refresh Button
                    render_icon_btn(
                        icons::ICON_REFRESH,
                        "Scan and Refresh Video Devices",
                        Message::RefreshDevices,
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    toolbar_divider(),
                    // Group 3: Microphone Device & Mute Toggle
                    pick_list(
                        self.available_audio_devices.clone(),
                        self.selected_audio_device.clone(),
                        Message::AudioDeviceSelected,
                    )
                    .style(theme::pick_list_style)
                    .menu_style(theme::menu_style)
                    .text_size(11)
                    .padding([3, 8]),
                    render_icon_btn(
                        if self.is_mic_muted {
                            icons::ICON_MIC_OFF
                        } else {
                            icons::ICON_MIC
                        },
                        if self.is_mic_muted {
                            "Unmute Microphone"
                        } else {
                            "Mute Microphone"
                        },
                        Message::ToggleMicMute,
                        self.is_mic_muted,
                        self.is_mic_muted,
                        tooltip::Position::Bottom,
                    ),
                    Space::new().width(Fill),
                    // Group 4: Window Controls
                    render_icon_btn(
                        icons::ICON_PIP,
                        "Picture-in-Picture [P]",
                        Message::TogglePip,
                        self.is_pip,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    toolbar_divider(),
                    // Minimize (_)
                    render_icon_btn(
                        icons::ICON_MINUS,
                        "Minimize",
                        Message::MinimizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    // Maximize ([])
                    render_icon_btn(
                        icons::ICON_SQUARE,
                        "Maximize / Restore",
                        Message::ToggleMaximizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    // Close (X)
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
            GridMode::GoldenRatio => {
                let golden_lines: Element<'_, Message> = container(
                    stack![
                        column![
                            Space::new().height(Length::FillPortion(382)),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 0.85, 0.4, 0.25))),
                                    ..Default::default()
                                }),
                            Space::new().height(Length::FillPortion(236)),
                            container(Space::new().width(Fill).height(Length::Fixed(1.0)))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 0.85, 0.4, 0.25))),
                                    ..Default::default()
                                }),
                            Space::new().height(Length::FillPortion(382)),
                        ]
                        .width(Fill)
                        .height(Fill),
                        row![
                            Space::new().width(Length::FillPortion(382)),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 0.85, 0.4, 0.25))),
                                    ..Default::default()
                                }),
                            Space::new().width(Length::FillPortion(236)),
                            container(Space::new().width(Length::Fixed(1.0)).height(Fill))
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(1.0, 0.85, 0.4, 0.25))),
                                    ..Default::default()
                                }),
                            Space::new().width(Length::FillPortion(382)),
                        ]
                        .width(Fill)
                        .height(Fill),
                    ]
                    .width(Fill)
                    .height(Fill),
                )
                .center(Fill)
                .into();

                viewport_stack = viewport_stack.push(golden_lines);
            }
            GridMode::SafeMargin => {
                let safe_box: Element<'_, Message> = container(
                    container(Space::new().width(Fill).height(Fill))
                        .style(|_: &Theme| container::Style {
                            border: iced::Border {
                                color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..Default::default()
                        })
                        .width(Length::FillPortion(9))
                        .height(Length::FillPortion(9))
                )
                .center(Fill)
                .into();

                viewport_stack = viewport_stack.push(safe_box);
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

        // Developer / Pro Telemetry Overlay (FPS, Frame Time, Resolution, Color Profile, VCam)
        if self.show_telemetry {
            let tele_overlay: Element<'_, Message> = container(
                row![
                    container(
                        row![
                            text(format!("{:.1} FPS", self.live_fps))
                                .size(10)
                                .color(if self.live_fps >= 50.0 { Color::from_rgb(0.4, 0.9, 0.5) } else { Color::from_rgb(0.9, 0.8, 0.3) }),
                            text("•").size(9).color(theme::COLOR_TEXT_MUTED),
                            text(format!("{:.1} ms", self.live_frame_time_ms))
                                .size(10)
                                .color(theme::COLOR_TEXT),
                            text("•").size(9).color(theme::COLOR_TEXT_MUTED),
                            text(self.resolution.label())
                                .size(10)
                                .color(theme::COLOR_TEXT),
                            text("•").size(9).color(theme::COLOR_TEXT_MUTED),
                            text(self.color_profile.label())
                                .size(10)
                                .color(theme::COLOR_TEXT_MUTED),
                            if self.is_virtual_cam_active {
                                row![
                                    text("•").size(9).color(theme::COLOR_TEXT_MUTED),
                                    text("VCam ON")
                                        .size(10)
                                        .color(Color::from_rgb(0.4, 0.9, 0.5)),
                                ].spacing(4).align_y(Alignment::Center)
                            } else {
                                row![].into()
                            },
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center)
                        .padding([4, 8]),
                    )
                    .style(theme::tooltip_style),
                ]
                .padding([10, 14])
            )
            .align_x(Alignment::Start)
            .align_y(Alignment::Start)
            .width(Fill)
            .height(Fill)
            .into();

            viewport_stack = viewport_stack.push(tele_overlay);
        }

        // Snapshot Toast Notification
        if let Some((ref filename, _)) = self.snapshot_toast {
            let toast: Element<'_, Message> = container(
                container(
                    row![
                        render_svg(icons::ICON_CAMERA, 14.0, true, false),
                        text(filename.as_str())
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
        // 3.1 Bottom-Left Record Video Button
        let record_btn: Element<'_, Message> = if self.is_recording {
            let mins = self.recording_elapsed_secs / 60;
            let secs = self.recording_elapsed_secs % 60;
            let time_str = format!("REC {:02}:{:02}", mins, secs);

            tooltip(
                button(
                    row![
                        render_svg(icons::ICON_RECORDING_STOP, 14.0, true, true),
                        text(time_str).size(11).color(Color::from_rgb(0.94, 0.27, 0.27)),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::ToggleRecording)
                .style(theme::recording_button_style)
                .padding([4, 8]),
                container(text("Stop Video Recording [V]").size(11))
                    .padding([3, 7])
                    .style(theme::tooltip_style),
                tooltip::Position::Top,
            )
            .into()
        } else {
            render_icon_btn(
                icons::ICON_RECORD,
                "Start Video Recording [V]",
                Message::ToggleRecording,
                false,
                true,
                tooltip::Position::Top,
            )
        };

        // 3.2 Bottom-Right Album Button with Floating Thumbnail Hover Preview (No Text, No Layout Shift)
        let tooltip_preview: Element<'_, Message> = if let Some(ref item) = self.latest_capture {
            if let Some(ref handle) = item.thumbnail_handle {
                let img = image(handle)
                    .content_fit(ContentFit::Cover)
                    .width(Length::Fixed(128.0))
                    .height(Length::Fixed(72.0));

                if item.kind == MediaKind::Video {
                    container(
                        stack![
                            img,
                            container(render_svg(icons::ICON_PLAY, 16.0, true, false)).center(Fill),
                        ]
                        .width(Length::Fixed(128.0))
                        .height(Length::Fixed(72.0)),
                    )
                    .style(theme::tooltip_image_preview_style)
                    .into()
                } else {
                    container(img)
                        .style(theme::tooltip_image_preview_style)
                        .into()
                }
            } else if item.kind == MediaKind::Video {
                container(
                    column![
                        render_svg(icons::ICON_VIDEO, 26.0, true, false),
                        Space::new().height(Length::Fixed(4.0)),
                        text(format_size(item.size_bytes)).size(10).color(Color::WHITE),
                    ]
                    .align_x(Alignment::Center),
                )
                .padding([12, 18])
                .style(theme::tooltip_image_preview_style)
                .into()
            } else {
                container(text("No Captures").size(11).color(Color::WHITE))
                    .padding([4, 8])
                    .style(theme::tooltip_style)
                    .into()
            }
        } else {
            container(text("No Captures").size(11).color(Color::WHITE))
                .padding([4, 8])
                .style(theme::tooltip_style)
                .into()
        };

        let album_btn: Element<'_, Message> = tooltip(
            button(render_svg(icons::ICON_ALBUM, 15.0, false, false))
                .on_press(Message::OpenAlbumWindow)
                .style(theme::clean_button_style(false, false))
                .padding([5, 7]),
            tooltip_preview,
            tooltip::Position::Top,
        )
        .into();

        let toolbar = container(
            row![
                // Group 1: Media Capture (Record Video & Still Snapshot)
                record_btn,
                render_icon_btn(
                    icons::ICON_CAMERA,
                    "Take Snapshot [C]",
                    Message::CaptureSnapshot,
                    false,
                    false,
                    tooltip::Position::Top,
                ),
                toolbar_divider(),

                // Group 2: Camera Stream & Orientation
                render_icon_btn(
                    icons::ICON_FLIP,
                    "Mirror / Flip [F]",
                    Message::ToggleFlip,
                    self.is_flip,
                    false,
                    tooltip::Position::Top,
                ),
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
                toolbar_divider(),

                // Group 3: Optics, Zoom & Exposure
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
                toolbar_divider(),

                // Group 4: Overlays & AI Tracking
                render_icon_btn(
                    icons::ICON_GRID,
                    self.grid_mode.label(),
                    Message::CycleGridMode,
                    self.grid_mode != GridMode::Off,
                    false,
                    tooltip::Position::Top,
                ),
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
                toolbar_divider(),

                // Group 5: View Mode
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

                // Group 6: Gallery & Settings
                album_btn,
                toolbar_divider(),
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

    /// ALBUM / GALLERY WINDOW VIEW:
    /// Responsive tabs: All, Photo, Video
    /// Edge-to-edge seamless grid with zero margin, zero padding
    /// Hover revealed 3-dots button with Open in explorer, Copy, Copy path, Delete
    fn view_album_window(&self, window_id: window::Id) -> Element<'_, Message> {
        let photo_count = self.media_items.iter().filter(|i| i.kind == MediaKind::Photo).count();
        let video_count = self.media_items.iter().filter(|i| i.kind == MediaKind::Video).count();
        let all_count = self.media_items.len();

        let header = mouse_area(
            container(
                row![
                    render_svg(icons::ICON_ALBUM, 16.0, true, false),
                    text("AutoCam Media Album")
                        .size(13)
                        .color(theme::COLOR_TEXT),
                    Space::new().width(Fill),
                    render_icon_btn(
                        icons::ICON_FOLDER,
                        "Open Captures Folder in Explorer",
                        Message::OpenCapturesFolder,
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_REFRESH,
                        "Refresh Album",
                        Message::RefreshAlbum,
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_MINUS,
                        "Minimize",
                        Message::MinimizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_SQUARE,
                        "Maximize / Restore",
                        Message::ToggleMaximizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_CLOSE,
                        "Close Album [Esc]",
                        Message::CloseAlbumWindow,
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

        // Tab Filter Bar
        let tabs = container(
            row![
                button(text(format!("All ({})", all_count)).size(11))
                    .on_press(Message::SetAlbumFilter(AlbumFilter::All))
                    .style(theme::clean_button_style(self.album_filter == AlbumFilter::All, false))
                    .padding([4, 12]),
                button(text(format!("Photo ({})", photo_count)).size(11))
                    .on_press(Message::SetAlbumFilter(AlbumFilter::Photo))
                    .style(theme::clean_button_style(self.album_filter == AlbumFilter::Photo, false))
                    .padding([4, 12]),
                button(text(format!("Video ({})", video_count)).size(11))
                    .on_press(Message::SetAlbumFilter(AlbumFilter::Video))
                    .style(theme::clean_button_style(self.album_filter == AlbumFilter::Video, false))
                    .padding([4, 12]),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .padding([6, 12]),
        )
        .style(theme::controls_panel_style)
        .width(Fill);

        // Filter items
        let filtered_items: Vec<(usize, &MediaItem)> = self
            .media_items
            .iter()
            .enumerate()
            .filter(|(_, item)| match self.album_filter {
                AlbumFilter::All => true,
                AlbumFilter::Photo => item.kind == MediaKind::Photo,
                AlbumFilter::Video => item.kind == MediaKind::Video,
            })
            .collect();

        let grid_content: Element<'_, Message> = if filtered_items.is_empty() {
            container(
                column![
                    render_svg(icons::ICON_ALBUM, 40.0, false, false),
                    Space::new().height(Length::Fixed(8.0)),
                    text("No media found").size(13).color(theme::COLOR_TEXT),
                    text("Take snapshots with [C] or record videos with [V]")
                        .size(11)
                        .color(theme::COLOR_TEXT_MUTED),
                ]
                .align_x(Alignment::Center)
                .spacing(4),
            )
            .center(Fill)
            .into()
        } else {
            // Strict NO MARGIN, NO PADDING Grid (4 items per row)
            let items_per_row = 4;
            let mut grid_col = column![].spacing(0).width(Fill);

            for chunk in filtered_items.chunks(items_per_row) {
                let mut row_tiles = row![].spacing(0).width(Fill);

                for &(idx, item) in chunk {
                    let is_hovered = self.hovered_media_idx == Some(idx);
                    let is_menu_open = self.active_media_menu_idx == Some(idx);

                    // Media visual content
                    let preview_el: Element<'_, Message> = match item.kind {
                        MediaKind::Photo => {
                            if let Some(ref handle) = item.thumbnail_handle {
                                image(handle)
                                    .content_fit(ContentFit::Cover)
                                    .width(Fill)
                                    .height(Fill)
                                    .into()
                            } else {
                                container(render_svg(icons::ICON_ALBUM, 28.0, false, false))
                                    .center(Fill)
                                    .into()
                            }
                        }
                        MediaKind::Video => {
                            let video_thumb: Element<'_, Message> = if let Some(ref handle) = item.thumbnail_handle {
                                image(handle)
                                    .content_fit(ContentFit::Cover)
                                    .width(Fill)
                                    .height(Fill)
                                    .into()
                            } else {
                                container(
                                    column![
                                        render_svg(icons::ICON_VIDEO, 28.0, true, false),
                                        Space::new().height(Length::Fixed(4.0)),
                                        text(format_size(item.size_bytes))
                                            .size(10)
                                            .color(theme::COLOR_TEXT_MUTED),
                                    ]
                                    .align_x(Alignment::Center),
                                )
                                .center(Fill)
                                .into()
                            };

                            // Centered semi-transparent play badge
                            let play_badge: Element<'_, Message> = container(
                                container(render_svg(icons::ICON_PLAY, 20.0, true, false))
                                    .padding([8, 10])
                                    .style(|_: &Theme| container::Style {
                                        background: Some(Background::Color(Color::from_rgba(0.06, 0.06, 0.09, 0.70))),
                                        border: iced::Border {
                                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.85),
                                            width: 1.0,
                                            radius: theme::RADIUS_FULL,
                                        },
                                        ..Default::default()
                                    }),
                            )
                            .center(Fill)
                            .into();

                            // Top-left VIDEO indicator pill
                            let video_pill: Element<'_, Message> = container(
                                container(
                                    row![
                                        render_svg(icons::ICON_VIDEO, 11.0, true, false),
                                        text("VIDEO").size(9).color(Color::WHITE),
                                    ]
                                    .spacing(4)
                                    .align_y(Alignment::Center),
                                )
                                .padding([2, 6])
                                .style(|_: &Theme| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(0.06, 0.06, 0.09, 0.85))),
                                    border: iced::Border {
                                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
                                        width: 1.0,
                                        radius: theme::RADIUS_SM,
                                    },
                                    ..Default::default()
                                }),
                            )
                            .padding([5, 6])
                            .into();

                            stack![video_thumb, play_badge, video_pill]
                                .width(Fill)
                                .height(Fill)
                                .into()
                        }
                    };

                    let mut tile_stack = stack![preview_el].width(Fill).height(Fill);

                    // Filename bottom bar overlay
                    let bottom_bar: Element<'_, Message> = container(
                        text(&item.filename)
                            .size(10)
                            .color(theme::COLOR_TEXT),
                    )
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.06, 0.06, 0.08, 0.75))),
                        ..Default::default()
                    })
                    .padding([3, 6])
                    .width(Fill)
                    .into();

                    tile_stack = tile_stack.push(
                        column![Space::new().height(Fill), bottom_bar]
                            .width(Fill)
                            .height(Fill),
                    );

                    // 3-Dots Button in Top-Right with White Border
                    if is_hovered || is_menu_open {
                        let dots_btn: Element<'_, Message> = container(
                            button(render_svg(icons::ICON_MORE_HORIZONTAL, 14.0, true, false))
                                .on_press(Message::ToggleMediaMenu(Some(idx)))
                                .style(theme::dots_button_style)
                                .padding([4, 6]),
                        )
                        .padding([4, 4])
                        .into();

                        tile_stack = tile_stack.push(
                            row![Space::new().width(Fill), dots_btn]
                                .width(Fill)
                                .height(Fill),
                        );
                    }

                    // Context Menu Overlay when 3-dots is clicked
                    if is_menu_open {
                        let primary_menu_btn: Element<'_, Message> = match item.kind {
                            MediaKind::Video => button(
                                row![
                                    render_svg(icons::ICON_PLAY, 12.0, true, false),
                                    text("Play Video").size(11).color(Color::WHITE),
                                ]
                                .spacing(6)
                                .align_y(Alignment::Center),
                            )
                            .on_press(Message::PlayVideo(item.path.clone()))
                            .style(theme::media_menu_btn_style(false))
                            .padding([4, 8])
                            .width(Fill)
                            .into(),

                            MediaKind::Photo => button(
                                row![
                                    render_svg(icons::ICON_CAMERA, 12.0, true, false),
                                    text("View Photo").size(11).color(Color::WHITE),
                                ]
                                .spacing(6)
                                .align_y(Alignment::Center),
                            )
                            .on_press(Message::OpenViewerWindow(item.path.clone()))
                            .style(theme::media_menu_btn_style(false))
                            .padding([4, 8])
                            .width(Fill)
                            .into(),
                        };

                        let menu_overlay: Element<'_, Message> = container(
                            column![
                                primary_menu_btn,
                                button(
                                    row![
                                        render_svg(icons::ICON_FOLDER, 12.0, true, false),
                                        text("Show in Explorer").size(11).color(Color::WHITE),
                                    ]
                                    .spacing(6)
                                    .align_y(Alignment::Center),
                                )
                                .on_press(Message::OpenInExplorer(item.path.clone()))
                                .style(theme::media_menu_btn_style(false))
                                .padding([4, 8])
                                .width(Fill),

                                button(
                                    row![
                                        render_svg(icons::ICON_COPY, 12.0, true, false),
                                        text("Copy Media").size(11).color(Color::WHITE),
                                    ]
                                    .spacing(6)
                                    .align_y(Alignment::Center),
                                )
                                .on_press(Message::CopyMedia(item.path.clone()))
                                .style(theme::media_menu_btn_style(false))
                                .padding([4, 8])
                                .width(Fill),

                                button(
                                    row![
                                        render_svg(icons::ICON_COPY, 12.0, true, false),
                                        text("Copy Path").size(11).color(Color::WHITE),
                                    ]
                                    .spacing(6)
                                    .align_y(Alignment::Center),
                                )
                                .on_press(Message::CopyMediaPath(item.path.clone()))
                                .style(theme::media_menu_btn_style(false))
                                .padding([4, 8])
                                .width(Fill),

                                button(
                                    row![
                                        render_svg(icons::ICON_TRASH, 12.0, true, true),
                                        text("Delete").size(11).color(Color::WHITE),
                                    ]
                                    .spacing(6)
                                    .align_y(Alignment::Center),
                                )
                                .on_press(Message::DeleteMedia(item.path.clone()))
                                .style(theme::media_menu_btn_style(true))
                                .padding([4, 8])
                                .width(Fill),
                            ]
                            .spacing(2)
                            .padding([4, 4]),
                        )
                        .style(theme::media_menu_style)
                        .width(Length::Fixed(150.0))
                        .into();

                        tile_stack = tile_stack.push(
                            column![
                                Space::new().height(Length::Fixed(26.0)),
                                row![Space::new().width(Fill), menu_overlay].width(Fill),
                            ]
                            .width(Fill)
                            .height(Fill),
                        );
                    }

                    // Click image or video to open or play
                    let click_target_path = item.path.clone();
                    let on_press_msg = match item.kind {
                        MediaKind::Photo => Message::OpenViewerWindow(click_target_path),
                        MediaKind::Video => Message::PlayVideo(click_target_path),
                    };

                    let tile_box = mouse_area(
                        container(tile_stack)
                            .style(theme::media_tile_style)
                            .width(Fill)
                            .height(Length::Fixed(165.0)),
                    )
                    .on_enter(Message::MediaHovered(Some(idx)))
                    .on_exit(Message::MediaHovered(None))
                    .on_press(on_press_msg);

                    row_tiles = row_tiles.push(container(tile_box).width(Length::FillPortion(1)));
                }

                // Balance empty cells in row
                let mut cell_count = chunk.len();
                while cell_count < items_per_row {
                    row_tiles = row_tiles.push(
                        container(Space::new().width(Fill).height(Length::Fixed(165.0)))
                            .style(theme::media_tile_style)
                            .width(Length::FillPortion(1)),
                    );
                    cell_count += 1;
                }

                grid_col = grid_col.push(row_tiles);
            }

            scrollable(grid_col)
                .width(Fill)
                .height(Fill)
                .into()
        };

        let content = column![header, tabs, grid_content]
            .spacing(0)
            .width(Fill)
            .height(Fill);

        let root = container(content)
            .style(theme::root_window_style)
            .width(Fill)
            .height(Fill)
            .into();

        with_resize_handles(root, window_id)
    }

    /// MEDIA VIEWER WINDOW VIEW:
    /// Dedicated window displaying full resolution captured photo or native video playback with dark backdrop
    fn view_viewer_window(&self, window_id: window::Id) -> Element<'_, Message> {
        let is_video = self.viewer_video_state.is_some();
        let header = mouse_area(
            container(
                row![
                    render_svg(if is_video { icons::ICON_VIDEO } else { icons::ICON_CAMERA }, 16.0, true, false),
                    text(&self.viewer_info_text)
                        .size(12)
                        .color(theme::COLOR_TEXT),
                    Space::new().width(Fill),
                    if let Some(ref path) = self.viewer_selected_path {
                        render_icon_btn(
                            icons::ICON_FOLDER,
                            "Show in Explorer",
                            Message::OpenInExplorer(path.clone()),
                            false,
                            false,
                            tooltip::Position::Bottom,
                        )
                    } else {
                        Space::new().width(Length::Fixed(0.0)).into()
                    },
                    if let Some(ref path) = self.viewer_selected_path {
                        render_icon_btn(
                            icons::ICON_COPY,
                            if is_video { "Copy Video to Clipboard" } else { "Copy Image to Clipboard" },
                            Message::CopyMedia(path.clone()),
                            false,
                            false,
                            tooltip::Position::Bottom,
                        )
                    } else {
                        Space::new().width(Length::Fixed(0.0)).into()
                    },
                    render_icon_btn(
                        icons::ICON_MINUS,
                        "Minimize",
                        Message::MinimizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_SQUARE,
                        "Maximize / Restore",
                        Message::ToggleMaximizeWindow(window_id),
                        false,
                        false,
                        tooltip::Position::Bottom,
                    ),
                    render_icon_btn(
                        icons::ICON_CLOSE,
                        "Close Viewer [Esc]",
                        Message::CloseViewerWindow,
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

        let image_viewport: Element<'_, Message> = if let Some(ref handle) = self.viewer_image_handle {
            container(
                image(handle)
                    .content_fit(ContentFit::Contain)
                    .width(Fill)
                    .height(Fill),
            )
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.03, 0.03, 0.035))),
                ..Default::default()
            })
            .center(Fill)
            .into()
        } else {
            container(text(if is_video { "Loading video..." } else { "No image loaded" }).size(12).color(theme::COLOR_TEXT_MUTED))
                .center(Fill)
                .into()
        };

        let main_viewport: Element<'_, Message> = if let Some(ref v) = self.viewer_video_state {
            let cur_secs = v.current_frame / v.fps.max(1) as usize;
            let total_secs = v.frames.len() / v.fps.max(1) as usize;
            let time_str = format!("{:02}:{:02} / {:02}:{:02}", cur_secs / 60, cur_secs % 60, total_secs / 60, total_secs % 60);

            let playback_bar = container(
                row![
                    render_icon_btn(
                        if v.is_playing { icons::ICON_PAUSE } else { icons::ICON_PLAY },
                        if v.is_playing { "Pause" } else { "Play" },
                        Message::ToggleVideoPlayback,
                        v.is_playing,
                        false,
                        tooltip::Position::Top,
                    ),
                    render_icon_btn(
                        icons::ICON_REFRESH,
                        "Replay from Beginning",
                        Message::ReplayVideo,
                        false,
                        false,
                        tooltip::Position::Top,
                    ),
                    text(time_str).size(11).color(Color::WHITE),
                    slider(
                        0.0..=(v.frames.len().saturating_sub(1) as f32),
                        v.current_frame as f32,
                        |val| Message::SeekVideo(val as usize),
                    )
                    .step(1.0)
                    .style(theme::clean_slider_style)
                    .width(Length::Fixed(280.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([6, 14]),
            )
            .style(theme::controls_panel_style);

            stack![
                image_viewport,
                column![
                    Space::new().height(Fill),
                    container(playback_bar)
                        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 16.0, left: 0.0 })
                        .center_x(Fill),
                ]
                .width(Fill)
                .height(Fill),
            ]
            .width(Fill)
            .height(Fill)
            .into()
        } else {
            image_viewport
        };

        let content = column![header, main_viewport]
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
                        .menu_style(theme::menu_style)
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
                ]
                .spacing(10)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 2: Low-Level Shutter & Anti-Flicker
        let flicker_section = column![
            text("2. Shutter Timing & Powerline Frequency").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("High-Speed Shutter Sync:").size(11).color(theme::COLOR_TEXT),
                            text("Minimizes motion blur on fast finger & hardware movement").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.action_shutter { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleActionShutter)
                            .style(theme::clean_button_style(self.action_shutter, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Anti-Flicker Filter:").size(11).color(theme::COLOR_TEXT),
                            text("Neutralizes fluorescent/LED light pulsing").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("50 Hz (TH/EU)").size(11))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Hz50))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Hz50, false))
                            .padding([3, 8]),
                        button(text("60 Hz (US)").size(11))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Hz60))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Hz60, false))
                            .padding([3, 8]),
                        button(text("Off").size(11))
                            .on_press(Message::SetAntiFlicker(AntiFlickerMode::Off))
                            .style(theme::clean_button_style(self.anti_flicker == AntiFlickerMode::Off, false))
                            .padding([3, 8]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ]
                .spacing(10)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 3: Pro Exposure, ISO Sensitivity & Color Temperature (Kelvin)
        let image_tune_section = column![
            text("3. Pro Optical Exposure, ISO & White Balance").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Auto Exposure (AE):").size(11).color(theme::COLOR_TEXT),
                            text("Dynamic ambient light metering & auto shutter/gain").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.auto_ev { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleAutoEv)
                            .style(theme::clean_button_style(self.auto_ev, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(format!("Exposure Bias (EV): {:+.1} EV", self.ev_bias)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(-3.0..=3.0, self.ev_bias, Message::SetEvBias)
                            .step(0.1)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(format!("ISO Sensitivity: ~{} ({:.1}x)", (self.iso_gain * 100.0) as u32, self.iso_gain)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(1.0..=6.0, self.iso_gain, Message::SetIsoGain)
                            .step(0.1)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Auto White Balance (AWB):").size(11).color(theme::COLOR_TEXT),
                            text("Dynamic Gray-World chromatic adaptation").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.auto_white_balance { "Auto [AWB]" } else { "Manual Kelvin" }).size(11))
                            .on_press(Message::ToggleAutoWhiteBalance)
                            .style(theme::clean_button_style(self.auto_white_balance, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    if !self.auto_white_balance {
                        column![
                            row![
                                text(format!("Color Temperature: {}K", self.manual_kelvin)).size(11).color(theme::COLOR_TEXT),
                                Space::new().width(Fill),
                                slider(2800..=7500, self.manual_kelvin, Message::SetManualKelvin)
                                    .step(50u32)
                                    .style(theme::clean_slider_style)
                                    .width(Length::Fixed(200.0)),
                            ]
                            .align_y(Alignment::Center),
                            row![
                                text("Kelvin Presets:").size(10).color(theme::COLOR_TEXT_MUTED),
                                Space::new().width(Fill),
                                button(text("3200K Tungsten").size(10))
                                    .on_press(Message::SetManualKelvin(3200))
                                    .style(theme::clean_button_style(self.manual_kelvin == 3200, false))
                                    .padding([2, 6]),
                                button(text("4000K Fluorescent").size(10))
                                    .on_press(Message::SetManualKelvin(4000))
                                    .style(theme::clean_button_style(self.manual_kelvin == 4000, false))
                                    .padding([2, 6]),
                                button(text("5500K Daylight").size(10))
                                    .on_press(Message::SetManualKelvin(5500))
                                    .style(theme::clean_button_style(self.manual_kelvin == 5500, false))
                                    .padding([2, 6]),
                                button(text("6500K Cloudy").size(10))
                                    .on_press(Message::SetManualKelvin(6500))
                                    .style(theme::clean_button_style(self.manual_kelvin == 6500, false))
                                    .padding([2, 6]),
                            ]
                            .spacing(6)
                            .align_y(Alignment::Center),
                        ]
                        .spacing(6)
                    } else {
                        column![].into()
                    },
                    row![
                        text(format!("Brightness Offset: {}", self.brightness)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(-100..=100, self.brightness, Message::SetBrightness)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(format!("Contrast Curve: {:.2}x", self.contrast)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(0.5..=2.0, self.contrast, Message::SetContrast)
                            .step(0.05)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(format!("Color Saturation: {:.2}x", self.saturation)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(0.0..=2.0, self.saturation, Message::SetSaturation)
                            .step(0.05)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 4: Creative Picture Profile & Film Look
        let profile_section = column![
            text("4. Creative Picture Profile & Tone Curve").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Profile Emulation:").size(11).color(theme::COLOR_TEXT),
                            text("Color science curves for live presentation & post-production").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Neutral").size(11))
                            .on_press(Message::SetColorProfile(ColorProfile::Neutral))
                            .style(theme::clean_button_style(self.color_profile == ColorProfile::Neutral, false))
                            .padding([3, 8]),
                        button(text("Vivid").size(11))
                            .on_press(Message::SetColorProfile(ColorProfile::Vivid))
                            .style(theme::clean_button_style(self.color_profile == ColorProfile::Vivid, false))
                            .padding([3, 8]),
                        button(text("Cinema Log").size(11))
                            .on_press(Message::SetColorProfile(ColorProfile::CinemaLog))
                            .style(theme::clean_button_style(self.color_profile == ColorProfile::CinemaLog, false))
                            .padding([3, 8]),
                        button(text("B&W").size(11))
                            .on_press(Message::SetColorProfile(ColorProfile::BwMonochrome))
                            .style(theme::clean_button_style(self.color_profile == ColorProfile::BwMonochrome, false))
                            .padding([3, 8]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 5: Geometric Orientation, Zoom & Framing
        let geom_section = column![
            text("5. Geometry & Composition Framing").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        text(format!("Digital Zoom: {:.2}x", self.zoom_level)).size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        slider(1.0..=4.0, self.zoom_level, Message::SetZoom)
                            .step(0.05)
                            .style(theme::clean_slider_style)
                            .width(Length::Fixed(200.0)),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Mirror Feed:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text(if self.is_flip { "Mirrored [F]" } else { "Normal [F]" }).size(11))
                            .on_press(Message::ToggleFlip)
                            .style(theme::clean_button_style(self.is_flip, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Rotation:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("0°").size(11))
                            .on_press(Message::SetRotation(0))
                            .style(theme::clean_button_style(self.rotation_deg == 0, false))
                            .padding([3, 8]),
                        button(text("90°").size(11))
                            .on_press(Message::SetRotation(90))
                            .style(theme::clean_button_style(self.rotation_deg == 90, false))
                            .padding([3, 8]),
                        button(text("180°").size(11))
                            .on_press(Message::SetRotation(180))
                            .style(theme::clean_button_style(self.rotation_deg == 180, false))
                            .padding([3, 8]),
                        button(text("270°").size(11))
                            .on_press(Message::SetRotation(270))
                            .style(theme::clean_button_style(self.rotation_deg == 270, false))
                            .padding([3, 8]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                    row![
                        text("Composition Grid:").size(11).color(theme::COLOR_TEXT),
                        Space::new().width(Fill),
                        button(text("Off").size(11))
                            .on_press(Message::SetGridMode(GridMode::Off))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::Off, false))
                            .padding([3, 8]),
                        button(text("3x3 Thirds").size(11))
                            .on_press(Message::SetGridMode(GridMode::RuleOfThirds))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::RuleOfThirds, false))
                            .padding([3, 8]),
                        button(text("Crosshair").size(11))
                            .on_press(Message::SetGridMode(GridMode::CenterCrosshair))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::CenterCrosshair, false))
                            .padding([3, 8]),
                        button(text("Golden Ratio φ").size(11))
                            .on_press(Message::SetGridMode(GridMode::GoldenRatio))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::GoldenRatio, false))
                            .padding([3, 8]),
                        button(text("Safe 90%").size(11))
                            .on_press(Message::SetGridMode(GridMode::SafeMargin))
                            .style(theme::clean_button_style(self.grid_mode == GridMode::SafeMargin, false))
                            .padding([3, 8]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 6: AI, DSP Filters & Focus Assist
        let ai_section = column![
            text("6. AI, DSP Filters & Focus Assist").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("CAS Adaptive Sharpening:").size(11).color(theme::COLOR_TEXT),
                            text("Clean contrast-adaptive high-pass edge crisping").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.cas_sharpen { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleCasSharpen)
                            .style(theme::clean_button_style(self.cas_sharpen, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    if self.cas_sharpen {
                        row![
                            text(format!("CAS Intensity: {:.2}", self.cas_intensity)).size(11).color(theme::COLOR_TEXT),
                            Space::new().width(Fill),
                            slider(0.0..=1.0, self.cas_intensity, Message::SetCasIntensity)
                                .step(0.05)
                                .style(theme::clean_slider_style)
                                .width(Length::Fixed(200.0)),
                        ]
                        .align_y(Alignment::Center)
                    } else {
                        row![].into()
                    },
                    row![
                        column![
                            text("Hardware Focus Peaking:").size(11).color(theme::COLOR_TEXT),
                            text("Highlights in-focus edges with green contours").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.focus_peaking { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleFocusPeaking)
                            .style(theme::clean_button_style(self.focus_peaking, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Face Tracking HUD Corner Brackets:").size(11).color(theme::COLOR_TEXT),
                            text("Real-time presenter face targeting overlay").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.face_detection { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleFaceDetection)
                            .style(theme::clean_button_style(self.face_detection, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("60 FPS Inter-Frame Interpolation:").size(11).color(theme::COLOR_TEXT),
                            text("Synthesizes smooth in-between video frames").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.frame_interpolation { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleFrameInterpolation)
                            .style(theme::clean_button_style(self.frame_interpolation, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 7: Denoise & Studio Quality Engine
        let denoise_section = column![
            text("7. Denoise & Studio Quality Engine").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Temporal Noise Reduction (TNR):").size(11).color(theme::COLOR_TEXT),
                            text("Multi-frame motion-adaptive grain filter with studio skin polish").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Off").size(11))
                            .on_press(Message::SetDenoiseMode(DenoiseMode::Off))
                            .style(theme::clean_button_style(self.denoise_mode == DenoiseMode::Off, false))
                            .padding([3, 8]),
                        button(text("Low").size(11))
                            .on_press(Message::SetDenoiseMode(DenoiseMode::Low))
                            .style(theme::clean_button_style(self.denoise_mode == DenoiseMode::Low, false))
                            .padding([3, 8]),
                        button(text("Med").size(11))
                            .on_press(Message::SetDenoiseMode(DenoiseMode::Medium))
                            .style(theme::clean_button_style(self.denoise_mode == DenoiseMode::Medium, false))
                            .padding([3, 8]),
                        button(text("High").size(11))
                            .on_press(Message::SetDenoiseMode(DenoiseMode::High))
                            .style(theme::clean_button_style(self.denoise_mode == DenoiseMode::High, false))
                            .padding([3, 8]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Low-Light Shadow Boost:").size(11).color(theme::COLOR_TEXT),
                            text("Recovers dark room shadows smoothly using S-curve lookup").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.low_light_boost { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleLowLightBoost)
                            .style(theme::clean_button_style(self.low_light_boost, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Microphone AI Noise Suppression (RNNoise):").size(11).color(theme::COLOR_TEXT),
                            text("Neural real-time background fan and keyboard hum reduction").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.mic_denoise { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleMicDenoise)
                            .style(theme::clean_button_style(self.mic_denoise, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 8: Virtual Camera Output (External Route)
        let vcam_section = column![
            text("8. Virtual Camera Output (External Route)").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Virtual Camera Bridge:").size(11).color(theme::COLOR_TEXT),
                            text("Output live enhanced feed to OBS, Zoom, Discord, or Google Meet").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.is_virtual_cam_active { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleVirtualCamera)
                            .style(theme::clean_button_style(self.is_virtual_cam_active, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(format!("Status: {}", self.virtual_cam_status_text))
                            .size(10)
                            .color(if self.is_virtual_cam_active { Color::from_rgb(0.4, 0.9, 0.5) } else { theme::COLOR_TEXT_MUTED }),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 9: Storage & Capture Data Management
        let (storage_photos, storage_videos, storage_bytes) = get_storage_stats(&self.captures_path);
        let storage_stats_text = format!("{} Photos • {} Videos • Total Space: {}", storage_photos, storage_videos, format_size(storage_bytes));
        let default_captures_path = PathBuf::from("captures");
        let windows_pictures_path = std::env::var("USERPROFILE")
            .ok()
            .map(|u| PathBuf::from(u).join("Pictures").join("AutoCam"));

        let storage_section = column![
            text("9. Storage & Capture Data Management").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Captures Storage Directory:").size(11).color(theme::COLOR_TEXT),
                            text(format!("Path: {}", self.captures_path.to_string_lossy())).size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Open in Explorer").size(11))
                            .on_press(Message::OpenCapturesFolder)
                            .style(theme::clean_button_style(false, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text("Quick Location Presets:").size(10).color(theme::COLOR_TEXT_MUTED),
                        Space::new().width(Fill),
                        button(text("Default (./captures)").size(10))
                            .on_press(Message::SetCapturesPath(default_captures_path.clone()))
                            .style(theme::clean_button_style(self.captures_path == default_captures_path, false))
                            .padding([2, 8]),
                        if let Some(ref pic_path) = windows_pictures_path {
                            let is_pic_active = &self.captures_path == pic_path;
                            button(text("Pictures Folder").size(10))
                                .on_press(Message::SetCapturesPath(pic_path.clone()))
                                .style(theme::clean_button_style(is_pic_active, false))
                                .padding([2, 8])
                        } else {
                            button(text("").size(0)).padding([0, 0])
                        },
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Disk Space Occupied:").size(11).color(theme::COLOR_TEXT),
                            text(storage_stats_text).size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Purge All Media").size(11))
                            .on_press(Message::PurgeAllCaptures)
                            .style(theme::danger_menu_item_style)
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(10)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 10: Database & Developer Diagnostics
        let (db_path_str, db_size_bytes, db_key_count) = db::get_db_info();
        let db_stats_text = format!("Size: {} • Stored Keys: {} (SQLite WAL mode)", format_size(db_size_bytes), db_key_count);

        let dev_tools_section = column![
            text("10. Database & Developer Diagnostics").size(11).color(theme::COLOR_TEXT_MUTED),
            container(
                column![
                    row![
                        column![
                            text("Real-Time Telemetry HUD Overlay:").size(11).color(theme::COLOR_TEXT),
                            text("Shows live FPS, frame latency (ms), resolution & status on viewport").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text(if self.show_telemetry { "Active" } else { "Off" }).size(11))
                            .on_press(Message::ToggleTelemetry)
                            .style(theme::clean_button_style(self.show_telemetry, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("SQLite Database File:").size(11).color(theme::COLOR_TEXT),
                            text(format!("Location: {}", db_path_str)).size(10).color(theme::COLOR_TEXT_MUTED),
                            text(db_stats_text).size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Open DB Folder").size(11))
                            .on_press(Message::OpenDatabaseFolder)
                            .style(theme::clean_button_style(false, false))
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                    row![
                        column![
                            text("Factory Reset All App Data:").size(11).color(theme::COLOR_TEXT),
                            text("Clears SQLite table and restores all camera settings to factory defaults").size(10).color(theme::COLOR_TEXT_MUTED),
                        ],
                        Space::new().width(Fill),
                        button(text("Factory Reset Database").size(11))
                            .on_press(Message::ResetFactorySettings)
                            .style(theme::danger_menu_item_style)
                            .padding([3, 10]),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(10)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        // SECTION 11: Customizable Keyboard Shortcuts Manager
        let shortcuts_section = column![
            row![
                text("11. Custom Keyboard Shortcuts Manager").size(11).color(theme::COLOR_TEXT_MUTED),
                Space::new().width(Fill),
                button(text("Reset to Defaults").size(10))
                    .on_press(Message::ResetShortcuts)
                    .style(theme::clean_button_style(false, false))
                    .padding([2, 6]),
            ]
            .align_y(Alignment::Center),
            container(
                column![
                    shortcut_config_row("Freeze Live Frame:", format_key(&self.keybindings.freeze), self.rebinding_action == Some(ShortcutAction::Freeze), Message::StartRebinding(ShortcutAction::Freeze)),
                    shortcut_config_row("Mirror / Flip:", format_key(&self.keybindings.flip), self.rebinding_action == Some(ShortcutAction::Flip), Message::StartRebinding(ShortcutAction::Flip)),
                    shortcut_config_row("Rotate 90°:", format_key(&self.keybindings.rotate), self.rebinding_action == Some(ShortcutAction::Rotate), Message::StartRebinding(ShortcutAction::Rotate)),
                    shortcut_config_row("Cycle Grid:", format_key(&self.keybindings.grid), self.rebinding_action == Some(ShortcutAction::Grid), Message::StartRebinding(ShortcutAction::Grid)),
                    shortcut_config_row("Toggle Auto Focus:", format_key(&self.keybindings.auto_focus), self.rebinding_action == Some(ShortcutAction::AutoFocus), Message::StartRebinding(ShortcutAction::AutoFocus)),
                    shortcut_config_row("Toggle Auto EV:", format_key(&self.keybindings.auto_ev), self.rebinding_action == Some(ShortcutAction::AutoEv), Message::StartRebinding(ShortcutAction::AutoEv)),
                    shortcut_config_row("Face Tracking HUD:", format_key(&self.keybindings.face_hud), self.rebinding_action == Some(ShortcutAction::FaceHud), Message::StartRebinding(ShortcutAction::FaceHud)),
                    shortcut_config_row("Frame Interpolation:", format_key(&self.keybindings.interpolation), self.rebinding_action == Some(ShortcutAction::Interpolation), Message::StartRebinding(ShortcutAction::Interpolation)),
                    shortcut_config_row("Take Snapshot:", format_key(&self.keybindings.snapshot), self.rebinding_action == Some(ShortcutAction::Snapshot), Message::StartRebinding(ShortcutAction::Snapshot)),
                    shortcut_config_row("Record Video:", format_key(&self.keybindings.record), self.rebinding_action == Some(ShortcutAction::Record), Message::StartRebinding(ShortcutAction::Record)),
                    shortcut_config_row("Picture-in-Picture:", format_key(&self.keybindings.pip), self.rebinding_action == Some(ShortcutAction::Pip), Message::StartRebinding(ShortcutAction::Pip)),
                    shortcut_config_row("Toggle Fullscreen:", format_key(&self.keybindings.fullscreen), self.rebinding_action == Some(ShortcutAction::Fullscreen), Message::StartRebinding(ShortcutAction::Fullscreen)),
                    shortcut_config_row("Toggle Settings:", format_key(&self.keybindings.settings), self.rebinding_action == Some(ShortcutAction::Settings), Message::StartRebinding(ShortcutAction::Settings)),
                    shortcut_config_row("Zoom In:", format_key(&self.keybindings.zoom_in), self.rebinding_action == Some(ShortcutAction::ZoomIn), Message::StartRebinding(ShortcutAction::ZoomIn)),
                    shortcut_config_row("Zoom Out:", format_key(&self.keybindings.zoom_out), self.rebinding_action == Some(ShortcutAction::ZoomOut), Message::StartRebinding(ShortcutAction::ZoomOut)),
                ]
                .spacing(6)
                .padding([10, 14]),
            )
            .style(theme::settings_inner_box_style)
            .width(Fill),
        ]
        .spacing(6);

        let bottom_actions = row![
            button(text("Open Captures Folder").size(11))
                .on_press(Message::OpenCapturesFolder)
                .style(theme::clean_button_style(false, false))
                .padding([5, 12]),
            button(text("Reset Camera Defaults").size(11))
                .on_press(Message::ResetDefaults)
                .style(theme::clean_button_style(false, false))
                .padding([5, 12]),
            Space::new().width(Fill),
            button(text("Done").size(11))
                .on_press(Message::CloseSettingsWindow)
                .style(theme::clean_button_style(true, false))
                .padding([5, 16]),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let scroll_body = scrollable(
            column![
                dev_section,
                flicker_section,
                image_tune_section,
                profile_section,
                geom_section,
                ai_section,
                denoise_section,
                vcam_section,
                storage_section,
                dev_tools_section,
                shortcuts_section,
                bottom_actions,
            ]
            .spacing(14)
            .padding([14, 16]),
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

/// Helper function to create a sleek vertical divider '|'
fn toolbar_divider<'a>() -> Element<'a, Message> {
    container(
        container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(12.0)))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.18))),
                ..Default::default()
            })
    )
    .padding([0, 1])
    .align_y(Alignment::Center)
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

#[cfg(target_os = "windows")]
fn show_native_message_box(title_str: &str, msg_str: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let title: Vec<u16> = OsStr::new(title_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let text: Vec<u16> = OsStr::new(msg_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: usize, lp_text: *const u16, lp_caption: *const u16, u_type: u32) -> i32;
    }
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0x10);
    }
}

#[cfg(target_os = "windows")]
fn setup_crash_protection() {
    // 1. Rust Panic Hook
    std::panic::set_hook(Box::new(|info| {
        let err_text = format!(
            "AutoCam encountered an unexpected error:\n\n{}\n\nPlease check permissions or hardware access.",
            info
        );
        let _ = std::fs::write("autocam_crash.log", &err_text);
        show_native_message_box("AutoCam Startup Error", &err_text);
    }));

    // 2. Windows Vectored Exception Handler (catches native faults like missing delay-load DLLs or CPU illegal instructions)
    #[repr(C)]
    struct EXCEPTION_RECORD {
        exception_code: u32,
        _exception_flags: u32,
        _exception_record: *mut EXCEPTION_RECORD,
        exception_address: *mut std::ffi::c_void,
        _number_parameters: u32,
        _exception_information: [usize; 15],
    }

    #[repr(C)]
    struct EXCEPTION_POINTERS {
        exception_record: *mut EXCEPTION_RECORD,
        _context_record: *mut std::ffi::c_void,
    }

    unsafe extern "system" fn veh_handler(pointers: *mut EXCEPTION_POINTERS) -> i32 {
        unsafe {
            if !pointers.is_null() && !(*pointers).exception_record.is_null() {
                let code = (*(*pointers).exception_record).exception_code;
                if code == 0xC0000005 || code == 0xC000001D || code == 0xC06D007E {
                    let reason = match code {
                        0xC0000005 => "Access Violation (0xC0000005) - Memory fault",
                        0xC000001D => "Illegal Instruction (0xC000001D) - CPU instruction set mismatch",
                        0xC06D007E => "Module Not Found (0xC06D007E) - Missing Windows Media Feature Pack or system DLL",
                        _ => "Native System Exception",
                    };
                    let msg = format!(
                        "AutoCam Native Error:\n\n{}\n\nAddress: {:?}\n\nIf you are on Windows N or Boot Camp, please install Windows Media Feature Pack.",
                        reason, (*(*pointers).exception_record).exception_address
                    );
                    let _ = std::fs::write("autocam_crash.log", &msg);
                    show_native_message_box("AutoCam Compatibility Warning", &msg);
                }
            }
        }
        0 // EXCEPTION_CONTINUE_SEARCH
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AddVectoredExceptionHandler(
            first: u32,
            handler: unsafe extern "system" fn(*mut EXCEPTION_POINTERS) -> i32,
        ) -> *mut std::ffi::c_void;
    }

    unsafe {
        AddVectoredExceptionHandler(1, veh_handler);
    }
}

fn main() -> iced::Result {
    #[cfg(target_os = "windows")]
    setup_crash_protection();

    iced::daemon(AutoCamApp::boot, AutoCamApp::update, AutoCamApp::view)
        .subscription(AutoCamApp::subscription)
        .theme(AutoCamApp::theme)
        .run()
}
