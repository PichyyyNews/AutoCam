#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub id: usize,
    pub name: String,
}

impl std::fmt::Display for AudioDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub fn query_audio_input_devices() -> Vec<AudioDeviceInfo> {
    let result = std::panic::catch_unwind(|| {
        #[allow(unused_mut)]
        let mut devices = Vec::new();

        #[cfg(target_os = "windows")]
        unsafe {
            #[repr(C)]
            struct WAVEINCAPSW {
                _w_mid: u16,
                _w_pid: u16,
                _v_driver_version: u32,
                sz_pname: [u16; 32],
                _dw_formats: u32,
                _w_channels: u16,
                _w_reserved1: u16,
            }

            #[link(name = "winmm")]
            unsafe extern "system" {
                fn waveInGetNumDevs() -> u32;
                fn waveInGetDevCapsW(u_device_id: usize, pwic: *mut WAVEINCAPSW, cbwic: u32) -> u32;
            }

            let num_devs = waveInGetNumDevs();
            for i in 0..(num_devs as usize).min(32) {
                let mut caps = std::mem::zeroed::<WAVEINCAPSW>();
                if waveInGetDevCapsW(i, &mut caps, std::mem::size_of::<WAVEINCAPSW>() as u32) == 0 {
                    let len = caps.sz_pname.iter().position(|&c| c == 0).unwrap_or(caps.sz_pname.len());
                    let name = String::from_utf16_lossy(&caps.sz_pname[..len]);
                    devices.push(AudioDeviceInfo { id: i, name });
                }
            }
        }

        devices
    });

    let mut devices = result.unwrap_or_default();
    if devices.is_empty() {
        devices.push(AudioDeviceInfo {
            id: 0,
            name: "Default Microphone".to_string(),
        });
    }

    devices
}

#[allow(dead_code)]
pub struct AudioDenoiser {
    state: Box<nnnoiseless::DenoiseState<'static>>,
}

impl Default for AudioDenoiser {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl AudioDenoiser {
    pub fn new() -> Self {
        Self {
            state: nnnoiseless::DenoiseState::new(),
        }
    }

    /// Process 480 samples (10ms at 48kHz) in-place or into output buffer
    pub fn process_frame(&mut self, output: &mut [f32; 480], input: &[f32; 480]) {
        self.state.process_frame(output, input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_audio_devices() {
        let devs = query_audio_input_devices();
        println!("Found audio devices: {:?}", devs);
        assert!(!devs.is_empty());
    }
}
