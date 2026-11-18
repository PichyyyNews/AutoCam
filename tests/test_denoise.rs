use nnnoiseless::DenoiseState;
use std::time::Instant;

#[test]
fn test_nnnoiseless_basic() {
    let mut denoise = DenoiseState::new();
    let mut input = [0.0f32; DenoiseState::FRAME_SIZE];
    let mut output = [0.0f32; DenoiseState::FRAME_SIZE];

    for (i, sample) in input.iter_mut().enumerate() {
        *sample = (i as f32 * 0.1).sin() * 1000.0;
    }

    denoise.process_frame(&mut output, &input);
    assert_eq!(output.len(), DenoiseState::FRAME_SIZE);
}

/// Benchmark Temporal Noise Reduction (TNR) on full 1080p frame
#[test]
fn test_video_tnr_speed_1080p() {
    let width = 1920usize;
    let height = 1080usize;
    let frame_len = width * height * 4;

    let mut current_frame = vec![128u8; frame_len];
    let mut history_frame = vec![130u8; frame_len];

    // Introduce some random sensor grain
    for i in (0..frame_len).step_by(4) {
        current_frame[i] = (128 + ((i % 17) as u8)) % 255;
    }

    let start = Instant::now();
    let threshold: i32 = 25; // Noise threshold
    
    // Process pixel chunks
    for i in (0..frame_len).step_by(4) {
        let cr = current_frame[i] as i32;
        let cg = current_frame[i + 1] as i32;
        let cb = current_frame[i + 2] as i32;

        let hr = history_frame[i] as i32;
        let hg = history_frame[i + 1] as i32;
        let hb = history_frame[i + 2] as i32;

        let dr = (cr - hr).abs();
        let dg = (cg - hg).abs();
        let db = (cb - hb).abs();

        if dr < threshold && dg < threshold && db < threshold {
            // Static region: average out noise (75% history, 25% new)
            let nr = ((hr * 3 + cr) >> 2) as u8;
            let ng = ((hg * 3 + cg) >> 2) as u8;
            let nb = ((hb * 3 + cb) >> 2) as u8;

            history_frame[i] = nr;
            history_frame[i + 1] = ng;
            history_frame[i + 2] = nb;

            current_frame[i] = nr;
            current_frame[i + 1] = ng;
            current_frame[i + 2] = nb;
        } else {
            // Motion detected: update history directly, zero ghosting
            history_frame[i] = cr as u8;
            history_frame[i + 1] = cg as u8;
            history_frame[i + 2] = cb as u8;
        }
    }

    let elapsed = start.elapsed();
    println!("1080p TNR elapsed: {:?}", elapsed);
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_millis() < 15, "TNR must complete in under 15ms in release mode!");
    }
}
