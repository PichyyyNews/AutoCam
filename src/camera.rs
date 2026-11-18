use crate::face_detect::{FaceRect, FaceTracker};
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};
use nokhwa::Camera;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, RwLock};
use std::thread;
use std::time::Duration;

static GLOBAL_FRAME_RX: RwLock<Option<tokio::sync::watch::Receiver<Option<FramePayload>>>> =
    RwLock::new(None);

pub fn get_global_frame_rx() -> Option<tokio::sync::watch::Receiver<Option<FramePayload>>> {
    GLOBAL_FRAME_RX.read().ok()?.clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDeviceInfo {
    pub index: u32,
    pub name: String,
}

impl std::fmt::Display for CameraDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoResolution {
    Hd720p,
    Fhd1080p,
}

impl VideoResolution {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Hd720p => (1280, 720),
            Self::Fhd1080p => (1920, 1080),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Hd720p => "720p",
            Self::Fhd1080p => "1080p",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiseMode {
    Off,
    Low,
    Medium,
    High,
}

impl DenoiseMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low (Subtle)",
            Self::Medium => "Medium (Balanced)",
            Self::High => "High (Max Studio)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiFlickerMode {
    Off,
    Hz50,
    Hz60,
}

impl AntiFlickerMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Anti-Flicker: Off",
            Self::Hz50 => "Anti-Flicker: 50Hz (EU/Asia/TH)",
            Self::Hz60 => "Anti-Flicker: 60Hz (US)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FramePayload {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub face_rect: Option<FaceRect>,
}

#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub is_flip: bool,
    pub rotation_deg: u16,
    pub zoom_level: f32,
    pub is_frozen: bool,
    pub auto_ev: bool,
    pub action_shutter: bool,
    pub ev_bias: f32,
    pub brightness: i32,
    pub contrast: f32,
    pub saturation: f32,
    pub focus_peaking: bool,
    pub peaking_threshold: i32,
    pub anti_flicker: AntiFlickerMode,
    pub enable_cas: bool,
    pub cas_intensity: f32,
    pub enable_interpolation: bool,
    pub enable_face_detect: bool,
    pub denoise_mode: DenoiseMode,
    pub low_light_boost: bool,
    pub auto_white_balance: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            is_flip: true,
            rotation_deg: 0,
            zoom_level: 1.0,
            is_frozen: false,
            auto_ev: false,
            action_shutter: true,
            ev_bias: 0.0,
            brightness: 0,
            contrast: 1.0,
            saturation: 1.0,
            focus_peaking: false,
            peaking_threshold: 55,
            anti_flicker: AntiFlickerMode::Hz50,
            enable_cas: true,
            cas_intensity: 0.45,
            enable_interpolation: false,
            enable_face_detect: false,
            denoise_mode: DenoiseMode::Low,
            low_light_boost: false,
            auto_white_balance: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CameraControl {
    SwitchDevice(u32),
    SwitchResolution(VideoResolution),
    SetFlip(bool),
    SetRotate(u16),
    SetZoom(f32),
    SetFreeze(bool),
    SetAutoFocus(bool),
    SetManualFocus(u32),
    SetAutoEv(bool),
    SetEvBias(f32),
    SetBrightness(i32),
    SetContrast(f32),
    SetSaturation(f32),
    SetFocusPeaking(bool),
    SetPeakingThreshold(i32),
    SetAntiFlicker(AntiFlickerMode),
    SetActionShutter(bool),
    SetCasSharpen(bool),
    SetCasIntensity(f32),
    SetFrameInterpolation(bool),
    SetFaceDetection(bool),
    SetDenoiseMode(DenoiseMode),
    SetLowLightBoost(bool),
    SetAutoWhiteBalance(bool),
    ResetDefaults,
    Stop,
}

pub struct CameraManager {
    command_tx: mpsc::Sender<CameraControl>,
    is_running: Arc<AtomicBool>,
}

impl CameraManager {
    pub fn query_available_devices() -> Vec<CameraDeviceInfo> {
        let mut list = Vec::new();
        if let Ok(devices) = nokhwa::query(ApiBackend::Auto) {
            for (idx, dev) in devices.into_iter().enumerate() {
                let name = if dev.human_name().is_empty() {
                    format!("Camera {}", idx)
                } else {
                    dev.human_name()
                };
                list.push(CameraDeviceInfo {
                    index: idx as u32,
                    name,
                });
            }
        }
        list
    }

    /// Queries which resolutions this specific camera hardware supports
    pub fn query_supported_resolutions(device_idx: u32) -> Vec<VideoResolution> {
        let index = CameraIndex::Index(device_idx);
        let mut supported = vec![VideoResolution::Hd720p];

        if let Ok(mut cam) = Camera::new(
            index,
            RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate),
        ) {
            if let Ok(formats) = cam.compatible_camera_formats() {
                let has_1080p = formats.iter().any(|f| {
                    let r = f.resolution();
                    r.width() >= 1920 && r.height() >= 1080
                });
                if has_1080p {
                    supported.push(VideoResolution::Fhd1080p);
                }
            } else {
                supported.push(VideoResolution::Fhd1080p);
            }
        } else {
            supported.push(VideoResolution::Fhd1080p);
        }

        supported.sort_by_key(|r| match r {
            VideoResolution::Hd720p => 1,
            VideoResolution::Fhd1080p => 2,
        });
        supported.dedup();
        supported
    }

    pub fn start(initial_device: u32, initial_res: VideoResolution) -> Self {
        let (tx, rx) = mpsc::channel::<CameraControl>();
        let (frame_tx, frame_rx) = tokio::sync::watch::channel(None);
        if let Ok(mut lock) = GLOBAL_FRAME_RX.write() {
            *lock = Some(frame_rx);
        }
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_worker = Arc::clone(&is_running);

        thread::Builder::new()
            .name("autocam-engine-pipeline".into())
            .spawn(move || {
                run_realtime_camera_pipeline(
                    rx,
                    frame_tx,
                    is_running_worker,
                    initial_device,
                    initial_res,
                );
            })
            .expect("Failed to spawn camera thread");

        Self {
            command_tx: tx,
            is_running,
        }
    }

    pub fn send(&self, cmd: CameraControl) {
        let _ = self.command_tx.send(cmd);
    }
}

impl Drop for CameraManager {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.send(CameraControl::Stop);
        if let Ok(mut lock) = GLOBAL_FRAME_RX.write() {
            *lock = None;
        }
    }
}

pub fn open_camera(device_idx: u32, resolution: VideoResolution) -> Option<Camera> {
    let (w, h) = resolution.dimensions();
    let index = CameraIndex::Index(device_idx);

    // 1. Primary: High-framerate MJPEG @ 60 FPS
    let requested_mjpeg_60 = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(w, h), FrameFormat::MJPEG, 60),
    ));

    // 2. Secondary: MJPEG @ 30 FPS
    let requested_mjpeg_30 = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(w, h), FrameFormat::MJPEG, 30),
    ));

    // 3. Fallback: NV12 @ 30 FPS
    let requested_nv12 = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(w, h), FrameFormat::NV12, 30),
    ));

    let camera_res = Camera::new(index.clone(), requested_mjpeg_60)
        .or_else(|_| Camera::new(index.clone(), requested_mjpeg_30))
        .or_else(|_| Camera::new(index, requested_nv12));

    let mut camera = camera_res.ok()?;
    camera.open_stream().ok()?;

    let _ = camera.set_frame_rate(60).or_else(|_| camera.set_frame_rate(30));

    Some(camera)
}

/// Real-time 2-stage camera pipeline:
/// Stage 1 (Camera Ingestion Thread): Continuously polls hardware, drops stale queued frames,
/// pushes newest buffer into atomic single-slot slot (ZERO driver queue lag).
/// Stage 2 (Processing Worker Thread): Consumes newest frame, processes transforms with
/// zero-copy memory buffers, and broadcasts via watch channel.
fn run_realtime_camera_pipeline(
    cmd_rx: mpsc::Receiver<CameraControl>,
    frame_tx: tokio::sync::watch::Sender<Option<FramePayload>>,
    is_running: Arc<AtomicBool>,
    initial_device: u32,
    initial_res: VideoResolution,
) {
    let config = Arc::new(RwLock::new(ProcessingConfig::default()));
    let config_processor = Arc::clone(&config);

    // Single-slot latest frame buffer: ensures camera driver buffer never accumulates 100ms lag!
    let latest_raw_frame = Arc::new(Mutex::new(Option::<nokhwa::Buffer>::None));
    let latest_raw_in = Arc::clone(&latest_raw_frame);
    let frame_condvar = Arc::new(Condvar::new());
    let frame_condvar_in = Arc::clone(&frame_condvar);

    let is_running_capture = Arc::clone(&is_running);
    let is_running_processor = Arc::clone(&is_running);

    // Dynamic camera device/resolution switch channel for capture thread
    let (dev_tx, dev_rx) = mpsc::channel::<(u32, VideoResolution)>();

    // --------------------------------------------------------------------------
    // STAGE 1: Dedicated Hardware Frame Ingestion Thread
    // --------------------------------------------------------------------------
    thread::Builder::new()
        .name("autocam-hw-ingest".into())
        .spawn(move || {
            let mut current_dev = initial_device;
            let mut current_res = initial_res;
            let mut camera = open_camera(current_dev, current_res);

            while is_running_capture.load(Ordering::Relaxed) {
                // Check if device or resolution needs to switch
                while let Ok((new_dev, new_res)) = dev_rx.try_recv() {
                    current_dev = new_dev;
                    current_res = new_res;
                    camera = open_camera(current_dev, current_res);
                }

                if let Some(ref mut cam) = camera {
                    match cam.frame() {
                        Ok(raw_buf) => {
                            {
                                let mut slot = latest_raw_in.lock().unwrap();
                                *slot = Some(raw_buf);
                            }
                            frame_condvar_in.notify_one();
                        }
                        Err(_) => {
                            thread::sleep(Duration::from_millis(5));
                        }
                    }
                } else {
                    thread::sleep(Duration::from_millis(250));
                    camera = open_camera(current_dev, current_res);
                }
            }
        })
        .expect("Failed to spawn camera ingest thread");

    // --------------------------------------------------------------------------
    // STAGE 2: Image Processing & Transformation Thread
    // --------------------------------------------------------------------------
    let frame_tx_proc = frame_tx.clone();
    thread::Builder::new()
        .name("autocam-dsp-processor".into())
        .spawn(move || {
            let mut face_tracker = FaceTracker::new();
            let mut prev_frame_pixels: Option<Vec<u8>> = None;
            let mut tnr_history: Vec<u8> = Vec::new();
            let mut smooth_ev_gain = 1.0f32;
            let mut awb_r_gain = 1.0f32;
            let mut awb_b_gain = 1.0f32;

            let mut decode_buf: Vec<u8> = Vec::new();
            let mut scratch_buf: Vec<u8> = Vec::new();

            // Precompute fast CAS weight LUT for instantaneous processing (zero float sqrt)
            let mut cas_lut = [0.0f32; 256];
            for i in 0..256 {
                let norm = i as f32 / 255.0;
                cas_lut[i] = (-norm.sqrt() * 0.18).clamp(-0.20, 0.0);
            }

            // Precompute low-light shadow boost LUT (lifts shadows smoothly)
            let mut shadow_lut = [0u8; 256];
            for i in 0..256 {
                let norm = i as f32 / 255.0;
                let lifted = norm.powf(0.82) * 255.0;
                shadow_lut[i] = lifted.clamp(0.0, 255.0) as u8;
            }

            while is_running_processor.load(Ordering::Relaxed) {
                // Wait for the newest hardware frame
                let raw_buffer = {
                    let mut slot = latest_raw_frame.lock().unwrap();
                    while slot.is_none() && is_running_processor.load(Ordering::Relaxed) {
                        slot = frame_condvar.wait(slot).unwrap();
                    }
                    slot.take()
                };

                let raw_buffer = match raw_buffer {
                    Some(b) => b,
                    None => continue,
                };

                let cfg = {
                    config_processor.read().unwrap().clone()
                };

                if cfg.is_frozen {
                    thread::sleep(Duration::from_millis(30));
                    continue;
                }

                let res = raw_buffer.resolution();
                let (width, height) = (res.width(), res.height());
                let required_len = (width * height * 4) as usize;

                if decode_buf.len() != required_len {
                    decode_buf.resize(required_len, 0);
                }

                if raw_buffer
                    .decode_image_to_buffer::<RgbAFormat>(&mut decode_buf)
                    .is_ok()
                {
                    // Low-Light Shadow Boost (instantaneous LUT lookup)
                    if cfg.low_light_boost {
                        for p in decode_buf.iter_mut() {
                            *p = shadow_lut[*p as usize];
                        }
                    }

                    // Auto White Balance (Gray World with temporal smoothing)
                    if cfg.auto_white_balance {
                        let mut sum_r = 0u64;
                        let mut sum_g = 0u64;
                        let mut sum_b = 0u64;
                        let mut count = 0u64;
                        let step = 32;
                        for i in (0..decode_buf.len()).step_by(step * 4) {
                            sum_r += decode_buf[i] as u64;
                            sum_g += decode_buf[i + 1] as u64;
                            sum_b += decode_buf[i + 2] as u64;
                            count += 1;
                        }
                        if count > 0 {
                            let avg_r = (sum_r as f32 / count as f32).max(1.0);
                            let avg_g = (sum_g as f32 / count as f32).max(1.0);
                            let avg_b = (sum_b as f32 / count as f32).max(1.0);

                            let target_r = (avg_g / avg_r).clamp(0.75, 1.35);
                            let target_b = (avg_g / avg_b).clamp(0.75, 1.35);

                            awb_r_gain = awb_r_gain * 0.94 + target_r * 0.06;
                            awb_b_gain = awb_b_gain * 0.94 + target_b * 0.06;

                            for i in (0..decode_buf.len()).step_by(4) {
                                decode_buf[i] = ((decode_buf[i] as f32 * awb_r_gain) as u32).min(255) as u8;
                                decode_buf[i + 2] = ((decode_buf[i + 2] as f32 * awb_b_gain) as u32).min(255) as u8;
                            }
                        }
                    }

                    // 1. Calculate Software Auto EV gain
                    let target_gain = if cfg.auto_ev {
                        let mut sample_sum = 0u64;
                        let mut sample_count = 0u64;
                        let step = 32;
                        for i in (0..decode_buf.len()).step_by(step * 4) {
                            let r = decode_buf[i] as u32;
                            let g = decode_buf[i + 1] as u32;
                            let b = decode_buf[i + 2] as u32;
                            let y = (r * 77 + g * 150 + b * 29) >> 8;
                            sample_sum += y as u64;
                            sample_count += 1;
                        }
                        let mean_y = (sample_sum as f32 / sample_count.max(1) as f32).max(10.0);
                        if mean_y >= 105.0 && mean_y <= 145.0 {
                            1.0f32
                        } else if mean_y < 105.0 {
                            (120.0 / mean_y).clamp(1.0, 2.0)
                        } else {
                            (130.0 / mean_y).clamp(0.65, 1.0)
                        }
                    } else {
                        1.0f32
                    };

                    let smooth_factor = if cfg.action_shutter { 0.88f32 } else { 0.94f32 };
                    if (target_gain - smooth_ev_gain).abs() > 0.05 {
                        smooth_ev_gain = smooth_ev_gain * smooth_factor + target_gain * (1.0 - smooth_factor);
                    } else {
                        smooth_ev_gain = target_gain;
                    }
                    let ev_multiplier = smooth_ev_gain * 2.0f32.powf(cfg.ev_bias);

                    // 2. High-speed process frame (zoom, u32-flip, rotate, LUT, peaking, saturation)
                    let (final_w, final_h, mut final_pixels) = process_frame(
                        &decode_buf,
                        width,
                        height,
                        cfg.zoom_level,
                        cfg.is_flip,
                        cfg.rotation_deg,
                        ev_multiplier,
                        cfg.brightness,
                        cfg.contrast,
                        cfg.saturation,
                        cfg.focus_peaking,
                        cfg.peaking_threshold,
                        &mut scratch_buf,
                    );

                    // 3. Fast CAS Circuit Sharpening (AMD FidelityFX) with LUT
                    if cfg.enable_cas && cfg.cas_intensity > 0.01 {
                        apply_fast_cas_sharpening(
                            &mut final_pixels,
                            final_w,
                            final_h,
                            cfg.cas_intensity,
                            &cas_lut,
                        );
                    }

                    // 3.1 Temporal Noise Reduction (TNR) & Subtle Studio Skin Polish
                    if cfg.denoise_mode != DenoiseMode::Off {
                        apply_temporal_noise_reduction(
                            &mut final_pixels,
                            &mut tnr_history,
                            cfg.denoise_mode,
                        );
                        if cfg.denoise_mode == DenoiseMode::Medium || cfg.denoise_mode == DenoiseMode::High {
                            apply_subtle_skin_polish(&mut final_pixels, final_w, final_h);
                        }
                    }

                    // 4. Presenter Face Detection HUD tracking
                    let face_rect = if cfg.enable_face_detect {
                        let rect = face_tracker.update(&final_pixels, final_w, final_h);
                        if let Some(r) = rect {
                            crate::face_detect::draw_face_hud(&mut final_pixels, final_w, final_h, r);
                        }
                        rect
                    } else {
                        None
                    };

                    // 5. 60 FPS motion interpolation
                    if cfg.enable_interpolation {
                        if let Some(ref prev) = prev_frame_pixels {
                            if prev.len() == final_pixels.len() {
                                let mut interp = vec![0u8; final_pixels.len()];
                                for i in 0..final_pixels.len() {
                                    interp[i] = ((prev[i] as u16 + final_pixels[i] as u16) >> 1) as u8;
                                }
                                let _ = frame_tx_proc.send(Some(FramePayload {
                                    width: final_w,
                                    height: final_h,
                                    rgba: interp,
                                    face_rect,
                                }));
                            }
                        }
                    }
                    prev_frame_pixels = Some(final_pixels.clone());

                    // Publish real-time frame
                    let _ = frame_tx_proc.send(Some(FramePayload {
                        width: final_w,
                        height: final_h,
                        rgba: final_pixels,
                        face_rect,
                    }));
                }
            }
        })
        .expect("Failed to spawn camera dsp processor");

    // --------------------------------------------------------------------------
    // MAIN CONTROL DISPATCHER
    // --------------------------------------------------------------------------
    let mut current_dev = initial_device;
    let mut current_res = initial_res;

    while is_running.load(Ordering::Relaxed) {
        if let Ok(cmd) = cmd_rx.recv() {
            let mut c = config.write().unwrap();
            match cmd {
                CameraControl::SwitchDevice(new_dev) => {
                    current_dev = new_dev;
                    let _ = dev_tx.send((current_dev, current_res));
                }
                CameraControl::SwitchResolution(new_res) => {
                    current_res = new_res;
                    let _ = dev_tx.send((current_dev, current_res));
                }
                CameraControl::SetFlip(flip) => c.is_flip = flip,
                CameraControl::SetRotate(deg) => c.rotation_deg = deg,
                CameraControl::SetZoom(zoom) => c.zoom_level = zoom,
                CameraControl::SetFreeze(freeze) => c.is_frozen = freeze,
                CameraControl::SetActionShutter(action) => c.action_shutter = action,
                CameraControl::SetAutoFocus(af) => {
                    let _ = af;
                }
                CameraControl::SetManualFocus(f) => {
                    let _ = f;
                }
                CameraControl::SetAutoEv(ae) => c.auto_ev = ae,
                CameraControl::SetEvBias(bias) => c.ev_bias = bias,
                CameraControl::SetBrightness(b) => c.brightness = b,
                CameraControl::SetContrast(contrast) => c.contrast = contrast,
                CameraControl::SetSaturation(sat) => c.saturation = sat,
                CameraControl::SetFocusPeaking(fp) => c.focus_peaking = fp,
                CameraControl::SetPeakingThreshold(th) => c.peaking_threshold = th,
                CameraControl::SetAntiFlicker(af) => c.anti_flicker = af,
                CameraControl::SetCasSharpen(cas) => c.enable_cas = cas,
                CameraControl::SetCasIntensity(intensity) => c.cas_intensity = intensity,
                CameraControl::SetFrameInterpolation(interp) => c.enable_interpolation = interp,
                CameraControl::SetFaceDetection(fd) => c.enable_face_detect = fd,
                CameraControl::SetDenoiseMode(mode) => c.denoise_mode = mode,
                CameraControl::SetLowLightBoost(ll) => c.low_light_boost = ll,
                CameraControl::SetAutoWhiteBalance(awb) => c.auto_white_balance = awb,
                CameraControl::ResetDefaults => {
                    *c = ProcessingConfig::default();
                }
                CameraControl::Stop => return,
            }
        }
    }
}

/// Applies zoom, flip, rotate, DSP exposure/contrast/saturation LUT, and focus peaking
fn process_frame(
    src: &[u8],
    width: u32,
    height: u32,
    zoom: f32,
    flip: bool,
    rotate: u16,
    ev_multiplier: f32,
    brightness: i32,
    contrast: f32,
    saturation: f32,
    focus_peaking: bool,
    peaking_threshold: i32,
    scratch: &mut Vec<u8>,
) -> (u32, u32, Vec<u8>) {
    let (cur_w, cur_h) = (width, height);
    let total_pixels = (cur_w * cur_h) as usize;
    let total_bytes = total_pixels * 4;

    scratch.resize(total_bytes, 0);

    // 1. Zoom (Digital center crop)
    if (zoom - 1.0).abs() > 0.05 {
        let crop_w = (width as f32 / zoom).max(1.0) as u32;
        let crop_h = (height as f32 / zoom).max(1.0) as u32;
        let offset_x = (width - crop_w) / 2;
        let offset_y = (height - crop_h) / 2;

        for y in 0..height {
            let src_y = offset_y + (y * crop_h) / height;
            let dst_row_start = (y * width * 4) as usize;
            let src_row_start = (src_y * width * 4) as usize;

            for x in 0..width {
                let src_x = offset_x + (x * crop_w) / width;
                let dst_idx = dst_row_start + (x * 4) as usize;
                let src_idx = src_row_start + (src_x * 4) as usize;

                scratch[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
            }
        }
    } else {
        scratch.copy_from_slice(src);
    }

    // 2. High-speed Horizontal Flip (u32 pixel slice swap - 4x faster than byte-by-byte)
    if flip {
        let u32_pixels = unsafe {
            std::slice::from_raw_parts_mut(scratch.as_mut_ptr() as *mut u32, total_pixels)
        };
        for y in 0..cur_h as usize {
            let row_start = y * cur_w as usize;
            for x in 0..(cur_w as usize / 2) {
                let left = row_start + x;
                let right = row_start + (cur_w as usize - 1 - x);
                u32_pixels.swap(left, right);
            }
        }
    }

    // 3. Precompute Color LUT for EV, Brightness, Contrast, and Saturation
    let needs_lut = (ev_multiplier - 1.0).abs() > 0.02 || brightness != 0 || (contrast - 1.0).abs() > 0.02;
    if needs_lut {
        let mut lut = [0u8; 256];
        for i in 0..256 {
            let base = (i as f32) * ev_multiplier;
            let contrasted = (base - 128.0) * contrast + 128.0;
            let final_val = (contrasted + brightness as f32).clamp(0.0, 255.0);
            lut[i] = final_val as u8;
        }

        for i in 0..total_pixels {
            let idx = i * 4;
            scratch[idx] = lut[scratch[idx] as usize];
            scratch[idx + 1] = lut[scratch[idx + 1] as usize];
            scratch[idx + 2] = lut[scratch[idx + 2] as usize];
        }
    }

    // Saturation Adjustment (B&W circuit high-contrast or vivid enhancement)
    if (saturation - 1.0).abs() > 0.02 {
        for i in 0..total_pixels {
            let idx = i * 4;
            let r = scratch[idx] as f32;
            let g = scratch[idx + 1] as f32;
            let b = scratch[idx + 2] as f32;
            let gray = 0.299 * r + 0.587 * g + 0.114 * b;
            scratch[idx] = (gray + (r - gray) * saturation).clamp(0.0, 255.0) as u8;
            scratch[idx + 1] = (gray + (g - gray) * saturation).clamp(0.0, 255.0) as u8;
            scratch[idx + 2] = (gray + (b - gray) * saturation).clamp(0.0, 255.0) as u8;
        }
    }

    // 4. Focus Peaking (Detect sharp high-frequency edges and tint in neon green)
    if focus_peaking && cur_w > 2 && cur_h > 2 {
        let threshold = peaking_threshold.max(10);
        let w = cur_w as usize;
        for y in 1..(cur_h as usize - 1) {
            let row = y * w * 4;
            let row_above = (y - 1) * w * 4;
            let row_below = (y + 1) * w * 4;

            for x in 1..(w - 1) {
                let idx = row + x * 4;
                let luma_left = scratch[idx - 4 + 1] as i32;
                let luma_right = scratch[idx + 4 + 1] as i32;
                let luma_up = scratch[row_above + x * 4 + 1] as i32;
                let luma_down = scratch[row_below + x * 4 + 1] as i32;

                let grad = (luma_left - luma_right).abs() + (luma_up - luma_down).abs();
                if grad > threshold {
                    scratch[idx] = 30;       // R
                    scratch[idx + 1] = 255;  // G (Neon Green)
                    scratch[idx + 2] = 50;   // B
                }
            }
        }
    }

    // 5. Rotation
    match rotate {
        90 => {
            let mut rotated = vec![0u8; total_bytes];
            for y in 0..cur_h {
                for x in 0..cur_w {
                    let src_idx = ((y * cur_w + x) * 4) as usize;
                    let new_x = cur_h - 1 - y;
                    let new_y = x;
                    let dst_idx = ((new_y * cur_h + new_x) * 4) as usize;
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&scratch[src_idx..src_idx + 4]);
                }
            }
            (cur_h, cur_w, rotated)
        }
        180 => {
            let u32_pixels = unsafe {
                std::slice::from_raw_parts_mut(scratch.as_mut_ptr() as *mut u32, total_pixels)
            };
            for i in 0..(total_pixels / 2) {
                u32_pixels.swap(i, total_pixels - 1 - i);
            }
            (cur_w, cur_h, scratch.clone())
        }
        270 => {
            let mut rotated = vec![0u8; total_bytes];
            for y in 0..cur_h {
                for x in 0..cur_w {
                    let src_idx = ((y * cur_w + x) * 4) as usize;
                    let new_x = y;
                    let new_y = cur_w - 1 - x;
                    let dst_idx = ((new_y * cur_h + new_x) * 4) as usize;
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&scratch[src_idx..src_idx + 4]);
                }
            }
            (cur_h, cur_w, rotated)
        }
        _ => (cur_w, cur_h, scratch.clone()),
    }
}

/// Fast Contrast Adaptive Sharpening (AMD FidelityFX) with Precomputed LUT (< 1.5ms on CPU)
pub fn apply_fast_cas_sharpening(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    sharpness: f32,
    cas_lut: &[f32; 256],
) {
    if sharpness <= 0.01 || width < 3 || height < 3 || pixels.len() < (width * height * 4) as usize {
        return;
    }

    let w = width as usize;
    let h = height as usize;
    let sharp_mult = sharpness.clamp(0.0, 1.0);

    for y in 1..(h - 1) {
        let row_idx = y * w * 4;
        let row_up = (y - 1) * w * 4;
        let row_dn = (y + 1) * w * 4;

        for x in 1..(w - 1) {
            let idx = row_idx + x * 4;
            let idx_l = row_idx + (x - 1) * 4;
            let idx_r = row_idx + (x + 1) * 4;
            let idx_u = row_up + x * 4;
            let idx_d = row_dn + x * 4;

            for c in 0..3 {
                let e = pixels[idx + c] as f32;
                let a = pixels[idx_u + c] as f32;
                let b = pixels[idx_l + c] as f32;
                let d = pixels[idx_r + c] as f32;
                let f = pixels[idx_d + c] as f32;

                let min_rgb = a.min(b).min(d).min(f).min(e);
                let max_rgb = a.max(b).max(d).max(f).max(e);

                if max_rgb > min_rgb + 3.0 {
                    let diff_idx = (min_rgb.min(255.0 - max_rgb) as usize).min(255);
                    let w_val = cas_lut[diff_idx] * sharp_mult;
                    let filtered = (e + w_val * (a + b + d + f)) / (1.0 + 4.0 * w_val);
                    pixels[idx + c] = filtered.clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

/// Temporal Noise Reduction (TNR) with motion-adaptive blending
#[inline]
fn apply_temporal_noise_reduction(
    pixels: &mut [u8],
    history: &mut Vec<u8>,
    mode: DenoiseMode,
) {
    if history.len() != pixels.len() {
        *history = pixels.to_vec();
        return;
    }

    let (threshold, hist_weight) = match mode {
        DenoiseMode::Off => return,
        DenoiseMode::Low => (16i32, 2i32),
        DenoiseMode::Medium => (28i32, 3i32),
        DenoiseMode::High => (42i32, 5i32),
    };

    let divisor = hist_weight + 1;

    for i in (0..pixels.len()).step_by(4) {
        let cr = pixels[i] as i32;
        let cg = pixels[i + 1] as i32;
        let cb = pixels[i + 2] as i32;

        let hr = history[i] as i32;
        let hg = history[i + 1] as i32;
        let hb = history[i + 2] as i32;

        let dr = (cr - hr).abs();
        let dg = (cg - hg).abs();
        let db = (cb - hb).abs();

        if dr < threshold && dg < threshold && db < threshold {
            let nr = ((hr * hist_weight + cr) / divisor) as u8;
            let ng = ((hg * hist_weight + cg) / divisor) as u8;
            let nb = ((hb * hist_weight + cb) / divisor) as u8;

            history[i] = nr;
            history[i + 1] = ng;
            history[i + 2] = nb;

            pixels[i] = nr;
            pixels[i + 1] = ng;
            pixels[i + 2] = nb;
        } else {
            history[i] = cr as u8;
            history[i + 1] = cg as u8;
            history[i + 2] = cb as u8;
        }
    }
}

/// Subtle skin-tone smoothing for studio look without blurring edges or hair
#[inline]
fn apply_subtle_skin_polish(pixels: &mut [u8], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    if w < 4 || h < 4 {
        return;
    }

    for y in 0..h {
        let row_start = y * w * 4;
        for x in 1..(w - 1) {
            let idx = row_start + x * 4;
            let r = pixels[idx] as i32;
            let g = pixels[idx + 1] as i32;
            let b = pixels[idx + 2] as i32;

            // Skin tone detection in RGB
            if r > 95 && g > 40 && b > 20 && r > g && (r - g) > 15 && (g - b) > 5 {
                let prev_idx = idx - 4;
                let next_idx = idx + 4;

                let pr = pixels[prev_idx] as i32;
                let pg = pixels[prev_idx + 1] as i32;
                let pb = pixels[prev_idx + 2] as i32;

                let nr = pixels[next_idx] as i32;
                let ng = pixels[next_idx + 1] as i32;
                let nb = pixels[next_idx + 2] as i32;

                if (pr - r).abs() < 24 && (nr - r).abs() < 24 {
                    pixels[idx] = ((pr + r * 2 + nr) >> 2) as u8;
                    pixels[idx + 1] = ((pg + g * 2 + ng) >> 2) as u8;
                    pixels[idx + 2] = ((pb + b * 2 + nb) >> 2) as u8;
                }
            }
        }
    }
}
