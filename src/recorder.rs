use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum RecorderCmd {
    AppendFrame(Arc<Vec<u8>>),
    Stop,
}

pub struct VideoRecorder {
    sender: Sender<RecorderCmd>,
    handle: Option<JoinHandle<std::io::Result<PathBuf>>>,
    output_path: PathBuf,
}

impl VideoRecorder {
    pub fn start(
        width: u32,
        height: u32,
        fps: u32,
        output_path: PathBuf,
    ) -> std::io::Result<Self> {
        let (tx, rx) = channel::<RecorderCmd>();
        let path_clone = output_path.clone();

        let handle = thread::Builder::new()
            .name("autocam-video-recorder".to_string())
            .spawn(move || run_recorder_worker(width, height, fps, path_clone, rx))?;

        Ok(Self {
            sender: tx,
            handle: Some(handle),
            output_path,
        })
    }

    pub fn append_frame(&self, rgba: Arc<Vec<u8>>) {
        let _ = self.sender.send(RecorderCmd::AppendFrame(rgba));
    }

    pub fn stop(mut self) -> Option<PathBuf> {
        let _ = self.sender.send(RecorderCmd::Stop);
        if let Some(handle) = self.handle.take() {
            handle.join().ok().and_then(|res| res.ok())
        } else {
            Some(self.output_path)
        }
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.output_path
    }
}

struct IndexEntry {
    offset: u32,
    length: u32,
}

fn run_recorder_worker(
    width: u32,
    height: u32,
    fps: u32,
    output_path: PathBuf,
    rx: Receiver<RecorderCmd>,
) -> std::io::Result<PathBuf> {
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = File::create(&output_path)?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, file);

    // 1. Write Initial Header (224 bytes placeholder)
    let micro_sec_per_frame = 1_000_000 / fps.max(1);
    write_initial_avi_header(&mut writer, width, height, fps, micro_sec_per_frame)?;

    let mut index_entries: Vec<IndexEntry> = Vec::new();
    let mut total_frames: u32 = 0;
    let mut rgb_buffer = vec![0u8; (width * height * 3) as usize];
    let mut jpeg_buffer = Vec::with_capacity(128 * 1024);

    let frame_interval = Duration::from_nanos(1_000_000_000 / fps.max(1) as u64);
    let mut last_frame_time = Instant::now() - frame_interval;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            RecorderCmd::AppendFrame(rgba) => {
                let now = Instant::now();
                // Avoid flooding if frames arrive faster than target FPS
                if total_frames > 0 && now.duration_since(last_frame_time) < (frame_interval * 3 / 4) {
                    continue;
                }
                last_frame_time = now;

                if rgba.len() < (width * height * 4) as usize {
                    continue;
                }

                // Convert RGBA -> RGB
                let pixel_count = (width * height) as usize;
                for i in 0..pixel_count {
                    let src_idx = i * 4;
                    let dst_idx = i * 3;
                    rgb_buffer[dst_idx] = rgba[src_idx];
                    rgb_buffer[dst_idx + 1] = rgba[src_idx + 1];
                    rgb_buffer[dst_idx + 2] = rgba[src_idx + 2];
                }

                jpeg_buffer.clear();
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buffer, 82);
                if encoder
                    .encode(
                        &rgb_buffer,
                        width,
                        height,
                        image::ExtendedColorType::Rgb8,
                    )
                    .is_err()
                {
                    continue;
                }

                let jpeg_len = jpeg_buffer.len() as u32;
                // Current offset relative to start of 'movi' fourcc (offset 220)
                let current_file_pos = writer.stream_position()?;
                let offset_rel_movi = (current_file_pos - 220) as u32;

                // Write chunk: '00dc' + length + jpeg_data + optional pad
                writer.write_all(b"00dc")?;
                writer.write_all(&jpeg_len.to_le_bytes())?;
                writer.write_all(&jpeg_buffer)?;

                if jpeg_len % 2 != 0 {
                    writer.write_all(&[0u8])?;
                }

                index_entries.push(IndexEntry {
                    offset: offset_rel_movi,
                    length: jpeg_len,
                });
                total_frames += 1;
            }
            RecorderCmd::Stop => {
                break;
            }
        }
    }

    // Drain any remaining Stop or buffered items
    while let Ok(cmd) = rx.try_recv() {
        if matches!(cmd, RecorderCmd::Stop) {
            break;
        }
    }

    // 2. Finalize AVI
    let movi_end_pos = writer.stream_position()?;
    let movi_size = (movi_end_pos - 224) as u32 + 4; // 'movi' content + 4 for "movi" fourcc

    // Write idx1 chunk
    let idx_size = (index_entries.len() * 16) as u32;
    writer.write_all(b"idx1")?;
    writer.write_all(&idx_size.to_le_bytes())?;
    for entry in &index_entries {
        writer.write_all(b"00dc")?;
        writer.write_all(&0x10u32.to_le_bytes())?; // AVIIF_KEYFRAME
        writer.write_all(&entry.offset.to_le_bytes())?;
        writer.write_all(&entry.length.to_le_bytes())?;
    }

    let final_file_len = writer.stream_position()?;
    let riff_size = (final_file_len - 8) as u32;

    // Patch header fields
    // 1. RIFF size at offset 4
    writer.seek(SeekFrom::Start(4))?;
    writer.write_all(&riff_size.to_le_bytes())?;

    // 2. avih total_frames at offset 48
    writer.seek(SeekFrom::Start(48))?;
    writer.write_all(&total_frames.to_le_bytes())?;

    // 3. strh length at offset 140
    writer.seek(SeekFrom::Start(140))?;
    writer.write_all(&total_frames.to_le_bytes())?;

    // 4. movi LIST size at offset 216
    writer.seek(SeekFrom::Start(216))?;
    writer.write_all(&movi_size.to_le_bytes())?;

    writer.flush()?;

    Ok(output_path)
}

fn write_initial_avi_header<W: Write>(
    w: &mut W,
    width: u32,
    height: u32,
    fps: u32,
    micro_sec_per_frame: u32,
) -> std::io::Result<()> {
    // Offset 0: RIFF header
    w.write_all(b"RIFF")?;
    w.write_all(&0u32.to_le_bytes())?; // placeholder for total_size - 8 (offset 4)
    w.write_all(b"AVI ")?;

    // Offset 12: LIST hdrl
    w.write_all(b"LIST")?;
    w.write_all(&192u32.to_le_bytes())?; // hdrl size: 4 + 64 + (12 + 64 + 48) = 192
    w.write_all(b"hdrl")?;

    // Offset 24: avih (MainAVIHeader - 56 bytes)
    w.write_all(b"avih")?;
    w.write_all(&56u32.to_le_bytes())?;
    w.write_all(&micro_sec_per_frame.to_le_bytes())?; // 32: micro_sec_per_frame
    w.write_all(&0u32.to_le_bytes())?; // 36: max_bytes_per_sec
    w.write_all(&0u32.to_le_bytes())?; // 40: padding_granularity
    w.write_all(&0x10u32.to_le_bytes())?; // 44: flags (AVIF_HASINDEX = 0x10)
    w.write_all(&0u32.to_le_bytes())?; // 48: total_frames (placeholder)
    w.write_all(&0u32.to_le_bytes())?; // 52: initial_frames
    w.write_all(&1u32.to_le_bytes())?; // 56: streams
    w.write_all(&0u32.to_le_bytes())?; // 60: suggested_buffer_size
    w.write_all(&width.to_le_bytes())?; // 64: width
    w.write_all(&height.to_le_bytes())?; // 68: height
    w.write_all(&[0u8; 16])?; // 72..88: reserved [u32; 4]

    // Offset 88: LIST strl
    w.write_all(b"LIST")?;
    w.write_all(&116u32.to_le_bytes())?; // 4 + 64 + 48 = 116
    w.write_all(b"strl")?;

    // Offset 100: strh (AVIStreamHeader - 56 bytes)
    w.write_all(b"strh")?;
    w.write_all(&56u32.to_le_bytes())?;
    w.write_all(b"vids")?; // 108: fcc_type
    w.write_all(b"MJPG")?; // 112: fcc_handler
    w.write_all(&0u32.to_le_bytes())?; // 116: flags
    w.write_all(&0u16.to_le_bytes())?; // 120: priority
    w.write_all(&0u16.to_le_bytes())?; // 122: language
    w.write_all(&0u32.to_le_bytes())?; // 124: initial_frames
    w.write_all(&1u32.to_le_bytes())?; // 128: scale
    w.write_all(&fps.to_le_bytes())?; // 132: rate
    w.write_all(&0u32.to_le_bytes())?; // 136: start
    w.write_all(&0u32.to_le_bytes())?; // 140: length (placeholder for total_frames)
    w.write_all(&(width * height * 3).to_le_bytes())?; // 144: suggested_buffer_size
    w.write_all(&10000u32.to_le_bytes())?; // 148: quality
    w.write_all(&0u32.to_le_bytes())?; // 152: sample_size
    w.write_all(&0i16.to_le_bytes())?; // 156: rc_frame.left
    w.write_all(&0i16.to_le_bytes())?; // 158: rc_frame.top
    w.write_all(&(width as i16).to_le_bytes())?; // 160: rc_frame.right
    w.write_all(&(height as i16).to_le_bytes())?; // 162: rc_frame.bottom

    // Offset 164: strf (BITMAPINFOHEADER - 40 bytes)
    w.write_all(b"strf")?;
    w.write_all(&40u32.to_le_bytes())?;
    w.write_all(&40u32.to_le_bytes())?; // bi_size
    w.write_all(&(width as i32).to_le_bytes())?; // bi_width
    w.write_all(&(height as i32).to_le_bytes())?; // bi_height
    w.write_all(&1u16.to_le_bytes())?; // bi_planes
    w.write_all(&24u16.to_le_bytes())?; // bi_bit_count
    w.write_all(b"MJPG")?; // bi_compression
    w.write_all(&(width * height * 3).to_le_bytes())?; // bi_size_image
    w.write_all(&0i32.to_le_bytes())?; // bi_x_pels_per_meter
    w.write_all(&0i32.to_le_bytes())?; // bi_y_pels_per_meter
    w.write_all(&0u32.to_le_bytes())?; // bi_clr_used
    w.write_all(&0u32.to_le_bytes())?; // bi_clr_important

    // Offset 212: LIST movi
    w.write_all(b"LIST")?;
    w.write_all(&0u32.to_le_bytes())?; // placeholder for movi size at offset 216
    w.write_all(b"movi")?; // offset 220..224

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_recorder_creates_valid_avi() {
        let test_dir = std::env::temp_dir().join("autocam_test");
        let _ = std::fs::create_dir_all(&test_dir);
        let path = test_dir.join("test_rec.avi");

        let recorder = VideoRecorder::start(64, 64, 30, path.clone()).expect("Start recorder");
        
        // Append 5 dummy frames
        let dummy_rgba = Arc::new(vec![128u8; 64 * 64 * 4]);
        for _ in 0..5 {
            recorder.append_frame(dummy_rgba.clone());
            thread::sleep(Duration::from_millis(15));
        }

        let saved = recorder.stop().expect("Stop recorder");
        assert_eq!(saved, path);
        assert!(path.exists());

        let bytes = std::fs::read(&path).expect("Read avi file");
        assert!(bytes.len() > 224);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"AVI ");
        assert_eq!(&bytes[12..16], b"LIST");
        assert_eq!(&bytes[20..24], b"hdrl");

        let _ = std::fs::remove_file(&path);
    }
}
