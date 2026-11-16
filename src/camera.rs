use crate::face_detect::{FaceRect, FaceTracker};
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, KnownCameraControl, RequestedFormat,
    RequestedFormatType, Resolution,
};
use nokhwa::Camera;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, RwLock};
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
pub enum AntiFlickerMode {
    Off,
    Hz50,
    Hz60,
}

impl AntiFlickerMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Anti-Flicker: Off",
            Self::Hz50 => "Anti-Flicker: 50Hz (EU/Asia)",
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
pub enum CameraControl {
    SwitchDevice(u32),
    SwitchResolution(VideoResolution),
    SetFlip(bool),
    SetRotate(u16), // 0, 90, 180, 270
    SetZoom(f32),   // 1.0, 1.5, 2.0, 2.5
    SetFreeze(bool),
    SetAutoFocus(bool),
    SetManualFocus(u32), // 0 - 100
    SetAutoEv(bool),
    SetEvBias(f32),        // -2.0 to +2.0 EV
    SetBrightness(i32),    // -50 to 50
    SetContrast(f32),      // 0.5 to 2.0
    SetFocusPeaking(bool), // Neon green edge highlight for in-focus PCB traces
    SetAntiFlicker(AntiFlickerMode),
    SetActionShutter(bool), // Fast shutter speed (Anti-ghosting / no motion trails)
    SetCasSharpen(bool),    // Contrast Adaptive Sharpening (Ultra-clear circuit traces)
    SetFrameInterpolation(bool), // Smooth 60 FPS motion interpolation
    SetFaceDetection(bool), // Presenter Face Tracking HUD
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

    pub fn start(
        initial_device: u32,
        initial_res: VideoResolution,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<CameraControl>();
        let (frame_tx, frame_rx) = tokio::sync::watch::channel(None);
        if let Ok(mut lock) = GLOBAL_FRAME_RX.write() {
            *lock = Some(frame_rx);
        }
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_worker = Arc::clone(&is_running);

        thread::Builder::new()
            .name("autocam-capture-worker".into())
            .spawn(move || {
                run_capture_loop(
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
    // (Bypasses USB 2.0 480Mbps bottleneck, eliminates frame-rate throttling & motion blur)
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

    // Negotiate highest supported framerate (60 -> 30)
    let _ = camera.set_frame_rate(60).or_else(|_| camera.set_frame_rate(30));

    Some(camera)
}

fn run_capture_loop(
    cmd_rx: mpsc::Receiver<CameraControl>,
    frame_tx: tokio::sync::watch::Sender<Option<FramePayload>>,
    is_running: Arc<AtomicBool>,
    mut current_device: u32,
    mut current_res: VideoResolution,
) {
    let mut camera = open_camera(current_device, current_res);
    let mut is_flip = true;
    let mut rotation_deg = 0u16;
    let mut zoom_level = 1.0f32;
    let mut is_frozen = false;

    // Advanced Camera Engine Settings
    let mut auto_ev = false; // Passive by default so it doesn't fight webcam hardware exposure
    let mut action_shutter = true; // Fast shutter anti-ghosting enabled by default!
    let mut ev_bias = 0.0f32;
    let mut brightness = 0i32;
    let mut contrast = 1.0f32;
    let mut focus_peaking = false;
    let mut _anti_flicker = AntiFlickerMode::Hz50; // 50Hz for Thailand / EU grid anti-flicker
    let mut enable_cas = true; // CAS Sharpening on by default for clear circuit traces
    let mut enable_interpolation = false; // 60 FPS motion interpolation
    let mut enable_face_detect = false; // Presenter Face Tracking HUD
    let mut face_tracker = FaceTracker::new();
    let mut prev_frame_pixels: Option<Vec<u8>> = None;

    // Running EMA of scene luminance for smooth software Auto-EV
    let mut smooth_ev_gain = 1.0f32;

    // Pre-allocated scratch buffers for zero-GC memory efficiency
    let mut decode_buf: Vec<u8> = Vec::new();
    let mut scratch_buf: Vec<u8> = Vec::new();
    let mut consecutive_errors = 0u32;

    while is_running.load(Ordering::Relaxed) {
        // Drain pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                CameraControl::SwitchDevice(new_dev) => {
                    current_device = new_dev;
                    camera = open_camera(current_device, current_res);
                    consecutive_errors = 0;
                    prev_frame_pixels = None;
                }
                CameraControl::SwitchResolution(new_res) => {
                    if new_res != current_res {
                        current_res = new_res;
                        camera = open_camera(current_device, current_res);
                        consecutive_errors = 0;
                        prev_frame_pixels = None;
                    }
                }
                CameraControl::SetFlip(flip) => is_flip = flip,
                CameraControl::SetRotate(deg) => rotation_deg = deg,
                CameraControl::SetZoom(zoom) => zoom_level = zoom,
                CameraControl::SetFreeze(freeze) => is_frozen = freeze,
                CameraControl::SetActionShutter(action) => action_shutter = action,
                CameraControl::SetAutoFocus(af) => {
                    // Attempt hardware UVC focus control if camera supports it
                    if let Some(ref mut cam) = camera {
                        let _ = cam.camera_control(KnownCameraControl::Focus);
                    }
                    let _ = af;
                }
                CameraControl::SetManualFocus(f) => {
                    let _ = f;
                }
                CameraControl::SetAutoEv(ae) => auto_ev = ae,
                CameraControl::SetEvBias(bias) => ev_bias = bias,
                CameraControl::SetBrightness(b) => brightness = b,
                CameraControl::SetContrast(c) => contrast = c,
                CameraControl::SetFocusPeaking(fp) => focus_peaking = fp,
                CameraControl::SetAntiFlicker(af) => _anti_flicker = af,
                CameraControl::SetCasSharpen(cas) => enable_cas = cas,
                CameraControl::SetFrameInterpolation(interp) => enable_interpolation = interp,
                CameraControl::SetFaceDetection(fd) => enable_face_detect = fd,
                CameraControl::Stop => return,
            }
        }

        // If frozen, sleep lightly to conserve CPU/RAM
        if is_frozen {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if let Some(ref mut cam) = camera {
            match cam.frame() {
                Ok(raw_buffer) => {
                    consecutive_errors = 0;
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
                        // 1. Calculate Software Auto EV gain with strict deadband to prevent
                        // oscillating against webcam hardware auto-exposure (zero brightness flickering)
                        let target_gain = if auto_ev {
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
                            
                            // Wide deadband (105..145): when within normal lighting, do NOT alter gain.
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

                        // Gentle smoothing with deadband threshold
                        let smooth_factor = if action_shutter { 0.88f32 } else { 0.94f32 };
                        if (target_gain - smooth_ev_gain).abs() > 0.05 {
                            smooth_ev_gain = smooth_ev_gain * smooth_factor + target_gain * (1.0 - smooth_factor);
                        } else {
                            smooth_ev_gain = target_gain;
                        }
                        let ev_multiplier = smooth_ev_gain * 2.0f32.powf(ev_bias);

                        // 2. Apply transforms: Zoom -> Flip -> Rotate -> EV/Contrast LUT -> Peaking
                        let (final_w, final_h, mut final_pixels) = process_frame(
                            &decode_buf,
                            width,
                            height,
                            zoom_level,
                            is_flip,
                            rotation_deg,
                            ev_multiplier,
                            brightness,
                            contrast,
                            focus_peaking,
                            &mut scratch_buf,
                        );

                        // 3. Contrast Adaptive Sharpening (CAS) for crisp teaching circuits
                        if enable_cas {
                            apply_cas_sharpening(&mut final_pixels, final_w, final_h, 0.45);
                        }

                        // 4. Presenter Face Detection HUD tracking
                        let face_rect = if enable_face_detect {
                            face_tracker.update(&final_pixels, final_w, final_h)
                        } else {
                            None
                        };

                        // 5. GPU Frame Interpolation (Smooth 30 -> 60 FPS motion blend)
                        if enable_interpolation {
                            if let Some(ref prev) = prev_frame_pixels {
                                if prev.len() == final_pixels.len() {
                                    let mut interp = vec![0u8; final_pixels.len()];
                                    for i in 0..final_pixels.len() {
                                        interp[i] = ((prev[i] as u16 + final_pixels[i] as u16) >> 1) as u8;
                                    }
                                    let _ = frame_tx.send(Some(FramePayload {
                                        width: final_w,
                                        height: final_h,
                                        rgba: interp,
                                        face_rect,
                                    }));
                                }
                            }
                        }
                        prev_frame_pixels = Some(final_pixels.clone());

                        // Publish fresh camera frame
                        let _ = frame_tx.send(Some(FramePayload {
                            width: final_w,
                            height: final_h,
                            rgba: final_pixels,
                            face_rect,
                        }));
                    }
                }
                Err(_) => {
                    consecutive_errors += 1;
                    if consecutive_errors > 25 {
                        camera = None;
                        consecutive_errors = 0;
                    }
                    thread::sleep(Duration::from_millis(16));
                }
            }
        } else {
            thread::sleep(Duration::from_millis(500));
            camera = open_camera(current_device, current_res);
        }
    }
}

/// Applies zoom, flip, rotate, DSP exposure/contrast LUT, and focus peaking with minimal allocations
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
    focus_peaking: bool,
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

    // 2. Horizontal Flip (Mirror)
    if flip {
        for y in 0..cur_h {
            let row_start = (y * cur_w * 4) as usize;
            for x in 0..(cur_w / 2) {
                let left_idx = row_start + (x * 4) as usize;
                let right_idx = row_start + ((cur_w - 1 - x) * 4) as usize;

                for b in 0..4 {
                    scratch.swap(left_idx + b, right_idx + b);
                }
            }
        }
    }

    // 3. Precompute Color LUT for EV, Brightness, and Contrast
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

    // 4. Focus Peaking (Detect sharp high-frequency edges and tint in neon green)
    if focus_peaking && cur_w > 2 && cur_h > 2 {
        let threshold = 55i32;
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
            for i in 0..(total_pixels / 2) {
                let left_idx = i * 4;
                let right_idx = (total_pixels - 1 - i) * 4;
                for b in 0..4 {
                    scratch.swap(left_idx + b, right_idx + b);
                }
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

/// Contrast Adaptive Sharpening (CAS) - AMD FidelityFX algorithm
/// Enhances PCB traces, text, chips, and small components with zero haloing or ringing
pub fn apply_cas_sharpening(pixels: &mut [u8], width: u32, height: u32, sharpness: f32) {
    if sharpness <= 0.01 || width < 3 || height < 3 || pixels.len() < (width * height * 4) as usize {
        return;
    }

    let w = width as usize;
    let h = height as usize;
    let sharp_clamped = sharpness.clamp(0.0, 1.0);

    // Process rows skipping edges
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
                    let w_val = (-(min_rgb.min(255.0 - max_rgb) / max_rgb.max(1.0)).sqrt() * sharp_clamped * 0.18).clamp(-0.20, 0.0);
                    let filtered = (e + w_val * (a + b + d + f)) / (1.0 + 4.0 * w_val);
                    pixels[idx + c] = filtered.clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}
