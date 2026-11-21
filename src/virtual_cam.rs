use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use virtualcam::{BackendKind, Camera, PixelFormat};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualCamStatus {
    Inactive,
    ActiveObs,
    ActiveMediaFoundation,
    NotAvailable,
}

#[allow(dead_code)]
impl VirtualCamStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Inactive => "Off",
            Self::ActiveObs => "Active (OBS Virtual Camera)",
            Self::ActiveMediaFoundation => "Active (Media Foundation)",
            Self::NotAvailable => "Unavailable (Install OBS)",
        }
    }
}

pub struct VirtualCamBridge {
    sender: SyncSender<(u32, u32, Arc<Vec<u8>>)>,
    is_running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl VirtualCamBridge {
    pub fn start() -> Self {
        // Sync channel with depth 2 to guarantee zero latency and zero UI stutter
        let (tx, rx) = sync_channel::<(u32, u32, Arc<Vec<u8>>)>(2);
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);

        let handle = thread::Builder::new()
            .name("autocam-vcam-output".to_string())
            .spawn(move || {
                run_vcam_loop(rx, is_running_clone);
            })
            .ok();

        Self {
            sender: tx,
            is_running,
            handle,
        }
    }

    /// Non-blocking frame dispatch. If consumer is busy, drops frame to prevent lag.
    pub fn send_frame(&self, width: u32, height: u32, rgba: Arc<Vec<u8>>) {
        let _ = self.sender.try_send((width, height, rgba));
    }
}

impl Drop for VirtualCamBridge {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run_vcam_loop(
    rx: Receiver<(u32, u32, Arc<Vec<u8>>)>,
    is_running: Arc<AtomicBool>,
) {
    let mut camera: Option<Camera> = None;
    let mut current_dim = (0u32, 0u32);

    while is_running.load(Ordering::Relaxed) {
        let (w, h, rgba) = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(item) => item,
            Err(_) => continue,
        };

        if camera.is_none() || current_dim != (w, h) {
            #[cfg(target_os = "windows")]
            {
                // Attempt OBS backend first (DirectShow Shared Memory)
                let obs_cam = Camera::builder(w, h, 30.0)
                    .format(PixelFormat::RGBA)
                    .backend(BackendKind::Obs)
                    .build();

                if let Ok(c) = obs_cam {
                    camera = Some(c);
                    current_dim = (w, h);
                } else if crate::camera::is_media_foundation_available() {
                    // Fallback to Media Foundation on Windows 11
                    let mf_cam = Camera::builder(w, h, 30.0)
                        .format(PixelFormat::RGBA)
                        .backend(BackendKind::MediaFoundation)
                        .build();
                    if let Ok(c) = mf_cam {
                        camera = Some(c);
                        current_dim = (w, h);
                    } else {
                        camera = None;
                    }
                } else {
                    camera = None;
                }
            }

            #[cfg(target_os = "linux")]
            {
                // Attempt v4l2loopback device on Linux
                let v4l2_cam = Camera::builder(w, h, 30.0)
                    .format(PixelFormat::RGBA)
                    .backend(BackendKind::V4l2)
                    .build();
                if let Ok(c) = v4l2_cam {
                    camera = Some(c);
                    current_dim = (w, h);
                } else {
                    camera = None;
                }
            }

            #[cfg(target_os = "macos")]
            {
                camera = None;
            }
        }

        if let Some(ref mut cam) = camera {
            let _ = cam.send(&rgba);
        }
    }
}

/// Check if virtual camera driver is available on the host machine
pub fn probe_virtual_cam_availability() -> VirtualCamStatus {
    #[cfg(target_os = "windows")]
    {
        if Camera::builder(640, 480, 30.0)
            .format(PixelFormat::RGBA)
            .backend(BackendKind::Obs)
            .build()
            .is_ok()
        {
            return VirtualCamStatus::ActiveObs;
        }

        if Camera::builder(640, 480, 30.0)
            .format(PixelFormat::RGBA)
            .backend(BackendKind::MediaFoundation)
            .build()
            .is_ok()
        {
            return VirtualCamStatus::ActiveMediaFoundation;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if Camera::builder(640, 480, 30.0)
            .format(PixelFormat::RGBA)
            .backend(BackendKind::V4l2)
            .build()
            .is_ok()
        {
            return VirtualCamStatus::ActiveObs;
        }
    }

    VirtualCamStatus::NotAvailable
}
