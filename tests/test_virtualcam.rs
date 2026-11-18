use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use virtualcam::{BackendKind, Camera, PixelFormat};

pub struct VirtualCamBridge {
    sender: SyncSender<(u32, u32, Arc<Vec<u8>>)>,
    is_running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl VirtualCamBridge {
    pub fn start() -> Self {
        let (tx, rx) = sync_channel::<(u32, u32, Arc<Vec<u8>>)>(2);
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);

        let handle = thread::Builder::new()
            .name("autocam-vcam-bridge".to_string())
            .spawn(move || {
                run_vcam_worker(rx, is_running_clone);
            })
            .ok();

        Self {
            sender: tx,
            is_running,
            handle,
        }
    }

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

fn run_vcam_worker(
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
            // Attempt OBS first, then MediaFoundation
            let obs_cam = Camera::builder(w, h, 30.0)
                .format(PixelFormat::RGBA)
                .backend(BackendKind::Obs)
                .build();

            if let Ok(c) = obs_cam {
                camera = Some(c);
                current_dim = (w, h);
            } else {
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
            }
        }

        if let Some(ref mut cam) = camera {
            let _ = cam.send(&rgba);
        }
    }
}

#[test]
fn test_bridge_start_drop() {
    let bridge = VirtualCamBridge::start();
    let frame = Arc::new(vec![0u8; 1280 * 720 * 4]);
    bridge.send_frame(1280, 720, frame);
    drop(bridge);
}
