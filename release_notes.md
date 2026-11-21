### 🚀 AutoCam v0.0.18: Modern Desktop Settings & Full UI/UX Customization

AutoCam v0.0.18 delivers a major settings overhaul featuring a desktop-class sidebar navigation layout, distinct Basic vs Advance camera controls, customizable toolbar button visibility, Dark/Light/System themes, 7 custom accent color palettes, and interface typography scaling.

#### 🔧 What's New & Upgraded in v0.0.18:
- **Desktop Sidebar Navigation**: Redesigned settings window with smooth pill sidebar tabs (`General`, `Application`, `Appearance`, `Shortcuts`, `Notifications`, `Storage & Data`, `About`), perfectly matching modern macOS and professional application standards.
- **Basic vs Advance (Pro) Mode**:
  - **Basic Mode**: Quick controls for Camera Device, Resolution (720p/1080p), Mirror, Rotation, Zoom, Composition Grid, and Auto Exposure/WB.
  - **Advance (Pro) Mode**: High-Speed Shutter Sync, Anti-Flicker (50Hz/60Hz), Manual Kelvin White Balance with Presets, EV Bias, ISO Gain, Curves (Brightness/Contrast/Saturation), Creative Picture Profiles, CAS Adaptive Sharpening, Focus Peaking, Face HUD, 60 FPS Interpolation, TNR Denoise, and OBS Virtual Camera routing.
- **View & Window Customization**:
  - **Toolbar Button Visibility**: Enable or disable any action button (Snapshot, Record, Flip, Freeze, Rotate, Zoom, Focus, EV, Grid, Face HUD, Fullscreen, Gallery Strip, Settings) for a completely clean, minimalist workspace.
  - **Window Controls**: Always-on-top toggle, 16:9 aspect ratio locking, and real-time Telemetry HUD overlay.
- **Appearance & Theming**:
  - **Theme Modes**: Dark (Studio Zinc), Light (Clean Crisp White), and System Match.
  - **Accent Highlight Palette**: 7 curated colors (Neutral Zinc, Studio Blue, Emerald Green, Sunset Amber, Crimson Rose, Electric Violet, Cyber Cyan).
  - **UI & Typography Scale**: Compact (90%), Standard (100%), and Large (115%).
- **Notifications & Audio Feedback**:
  - Configurable toast alerts & duration (1s, 2s, 3s, 5s), camera shutter sound feedback, and microphone selector.
- **Persistent SQLite Storage**: All layout, theme, button visibility, and camera preferences persist across app restarts.

---

### 📦 Available Downloads & Target Packages

| Platform / Architecture | Package Name | Description |
| :--- | :--- | :--- |
| **Windows x86_64 (64-bit)** | `autocam.exe` or `autocam-windows-x86_64.zip` | Standalone portable executable for modern 64-bit Windows (10/11) |
| **Windows x86 (32-bit)** | `autocam-x86.exe` or `autocam-windows-i686.zip` | Standalone executable for legacy 32-bit Windows systems |
| **Windows ARM64** | `autocam-arm64.exe` or `autocam-windows-arm64.zip` | Native binary for Windows on ARM (Snapdragon X Elite, Surface Pro) |
| **macOS Universal (All Macs)** | `autocam-macos-universal.tar.gz` | **Universal Binary**: Runs natively on both Apple Silicon (M1/M2/M3/M4) and Intel Macs |
| **macOS Apple Silicon** | `autocam-macos-arm64.tar.gz` | Optimized for Apple Silicon M1/M2/M3/M4 |
| **macOS Intel** | `autocam-macos-x86_64.tar.gz` | Optimized for older Intel-based Macs |
| **Linux x86_64** | `autocam-linux-x86_64.tar.gz` | Native binary for Linux (Ubuntu, Debian, Fedora, Arch) with Wayland & X11 |

---

### 💻 How to Run on Each Platform

#### 🍏 macOS (Intel & Apple Silicon M1/M2/M3/M4)
1. Download `autocam-macos-universal.tar.gz` (or architecture-specific `.tar.gz`) and extract it:
   ```bash
   tar -xvf autocam-macos-universal.tar.gz
   chmod +x autocam
   ```
2. **First Run & Gatekeeper**: Remove quarantine attribute if needed:
   ```bash
   xattr -cr ./autocam
   ./autocam
   ```

#### 🐧 Linux (Ubuntu, Debian, Fedora, Arch)
1. Download `autocam-linux-x86_64.tar.gz` and extract it:
   ```bash
   tar -xvf autocam-linux-x86_64.tar.gz
   chmod +x autocam
   ```
2. Ensure user has video permissions:
   ```bash
   sudo usermod -a -G video $USER
   ```
3. AutoCam natively supports both modern **Wayland** and **X11**.

#### 🪟 Windows (10, 11, Windows N, Boot Camp)
1. Download `autocam.exe` (or `autocam-x86.exe` for 32-bit systems) and double-click to launch directly. No runtime dependencies required.
