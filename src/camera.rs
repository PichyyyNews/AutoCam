use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};
use nokhwa::Camera;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

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

#[derive(Clone)]
pub struct FramePayload {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub enum CameraControl {
    SwitchDevice(u32),
    SwitchResolution(VideoResolution),
    SetFlip(bool),
    SetRotate(u16), // 0, 90, 180, 270
    SetZoom(f32),   // 1.0, 1.5, 2.0, 2.5
    SetFreeze(bool),
    Stop,
}

pub struct CameraManager {
    command_tx: mpsc::Sender<CameraControl>,
    frame_slot: Arc<Mutex<Option<FramePayload>>>,
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

    pub fn start(
        initial_device: u32,
        initial_res: VideoResolution,
    ) -> (Self, Arc<Mutex<Option<FramePayload>>>) {
        let (tx, rx) = mpsc::channel::<CameraControl>();
        let frame_slot = Arc::new(Mutex::new(None));
        let frame_slot_worker = Arc::clone(&frame_slot);
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_worker = Arc::clone(&is_running);

        thread::Builder::new()
            .name("autocam-capture-worker".into())
            .spawn(move || {
                run_capture_loop(
                    rx,
                    frame_slot_worker,
                    is_running_worker,
                    initial_device,
                    initial_res,
                );
            })
            .expect("Failed to spawn camera thread");

        (
            Self {
                command_tx: tx,
                frame_slot: Arc::clone(&frame_slot),
                is_running,
            },
            frame_slot,
        )
    }

    pub fn send(&self, cmd: CameraControl) {
        let _ = self.command_tx.send(cmd);
    }

    pub fn take_latest_frame(&self) -> Option<FramePayload> {
        if let Ok(mut lock) = self.frame_slot.lock() {
            lock.take()
        } else {
            None
        }
    }
}

impl Drop for CameraManager {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        let _ = self.command_tx.send(CameraControl::Stop);
    }
}

fn open_camera(device_idx: u32, resolution: VideoResolution) -> Option<Camera> {
    let (w, h) = resolution.dimensions();
    let index = CameraIndex::Index(device_idx);

    // Prefer NV12 (direct raw video, hardware accelerated, no dropped JPEG packets)
    // with automatic fallback to MJPEG
    let requested_nv12 = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(w, h), FrameFormat::NV12, 30),
    ));

    let camera_res = Camera::new(index.clone(), requested_nv12).or_else(|_| {
        let requested_mjpeg = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Closest(
            CameraFormat::new(Resolution::new(w, h), FrameFormat::MJPEG, 30),
        ));
        Camera::new(index, requested_mjpeg)
    });

    let mut camera = camera_res.ok()?;
    camera.open_stream().ok()?;
    let _ = camera.set_frame_rate(30);
    Some(camera)
}

fn run_capture_loop(
    cmd_rx: mpsc::Receiver<CameraControl>,
    frame_slot: Arc<Mutex<Option<FramePayload>>>,
    is_running: Arc<AtomicBool>,
    mut current_device: u32,
    mut current_res: VideoResolution,
) {
    let mut camera = open_camera(current_device, current_res);
    let mut is_flip = false;
    let mut rotation_deg = 0u16;
    let mut zoom_level = 1.0f32;
    let mut is_frozen = false;

    // Pre-allocated scratch buffer for image transforms to achieve minimal RAM footprint
    let mut decode_buf: Vec<u8> = Vec::new();
    let mut scratch_buf: Vec<u8> = Vec::new();

    while is_running.load(Ordering::Relaxed) {
        // Drain pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                CameraControl::SwitchDevice(new_dev) => {
                    if new_dev != current_device {
                        current_device = new_dev;
                        camera = open_camera(current_device, current_res);
                    }
                }
                CameraControl::SwitchResolution(new_res) => {
                    if new_res != current_res {
                        current_res = new_res;
                        camera = open_camera(current_device, current_res);
                    }
                }
                CameraControl::SetFlip(flip) => is_flip = flip,
                CameraControl::SetRotate(deg) => rotation_deg = deg,
                CameraControl::SetZoom(zoom) => zoom_level = zoom,
                CameraControl::SetFreeze(freeze) => is_frozen = freeze,
                CameraControl::Stop => return,
            }
        }

        // If frozen, sleep lightly to conserve CPU/RAM without querying camera
        if is_frozen {
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if let Some(ref mut cam) = camera {
            match cam.frame() {
                Ok(raw_buffer) => {
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
                        // Apply transforms: Zoom -> Flip -> Rotate
                        let (final_w, final_h, final_pixels) = process_frame(
                            &decode_buf,
                            width,
                            height,
                            zoom_level,
                            is_flip,
                            rotation_deg,
                            &mut scratch_buf,
                        );

                        // Publish frame to slot
                        if let Ok(mut slot) = frame_slot.lock() {
                            *slot = Some(FramePayload {
                                width: final_w,
                                height: final_h,
                                rgba: final_pixels,
                            });
                        }
                    }
                }
                Err(_) => {
                    // Slight delay only on frame grab error
                    thread::sleep(Duration::from_millis(8));
                }
            }
        } else {
            // No camera open, wait and retry occasionally
            thread::sleep(Duration::from_millis(500));
            camera = open_camera(current_device, current_res);
        }

        // Do NOT sleep here! cam.frame() blocks at camera hardware sync
    }
}

/// Applies zoom, flip, and rotation with minimal allocations
fn process_frame(
    src: &[u8],
    width: u32,
    height: u32,
    zoom: f32,
    flip: bool,
    rotate: u16,
    scratch: &mut Vec<u8>,
) -> (u32, u32, Vec<u8>) {
    let (cur_w, cur_h) = (width, height);
    let total_pixels = (cur_w * cur_h) as usize;
    let total_bytes = total_pixels * 4;

    scratch.resize(total_bytes, 0);

    // 1. Zoom (Digital center crop and bilinear/nearest stretch)
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

    // 3. Rotation (90, 180, 270 degrees)
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
