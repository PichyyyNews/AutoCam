# AutoCam

AutoCam is a lightweight desktop camera viewer written in Rust, designed for live demonstrations, hardware inspections, and teaching environments.

## Overview

The application provides a responsive, borderless camera feed with low latency, allowing users to monitor and present physical workspace activities such as circuit assembly, document review, and component inspection alongside other desktop tasks.

## Capabilities

- Real-Time Video Streaming: Direct video capture from connected USB webcams with automatic device hot-plug detection.
- Orientation and Framing: Horizontal mirroring, 90-degree rotation, and digital zoom for overhead or macro camera setups.
- Freeze and Still Capture: Pause the live feed on demand to explain details, and save high-resolution PNG snapshots to disk.
- Picture-in-Picture Mode: A compact, borderless, always-on-top floating window that stays visible over slides and schematics.
- Inspection Aids: Optional focus peaking for highlighting in-focus edges, adaptive sharpening for fine traces, and alignment grid overlays.
- Dedicated Settings Window: An independent floating panel to configure exposure, color, contrast, and resolution targets without obstructing the camera view.
- Configurable Shortcuts: Fully customizable keyboard shortcuts for all controls and viewer operations.

## Installation and Usage

Pre-compiled standalone binaries for 64-bit Windows are available in the Releases section.

1. Download `autocam.exe` from the latest release.
2. Launch the executable directly (no installation or runtime dependencies required).

## Building from Source

Prerequisites: Rust toolchain (stable).

```bash
cargo build --release
```

The compiled binary will be located at `target/release/autocam.exe`.

## License

This project is licensed under the MIT License.
