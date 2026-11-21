### 🚀 AutoCam v0.0.17: Complete Cross-Platform Release (Windows, macOS, Linux)

AutoCam v0.0.17 introduces full, verified multi-platform releases across **Windows (x64, x86, ARM64)**, **macOS (Universal Binary for Apple Silicon M1–M4 & Intel)**, and **Linux (x86_64 for Ubuntu, Debian, Fedora, Arch)**.

#### 🔧 What's New & Resolved in v0.0.17:
- **Linux Release Asset**: Resolved the Linux `Task` clipboard typing constraint and enabled full X11 and Wayland display backends.
- **macOS Universal Package**: Includes single unified binary supporting 100% of Macs (Apple Silicon + Intel).
- **Hardened SQLite & File Management**: Seamless cross-platform paths for AppData, macOS Application Support, and Linux XDG Data Home.

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
2. **First Run & Gatekeeper**: Because AutoCam is an open-source binary without an Apple Developer ID signature, macOS Gatekeeper may show a security notice. Remove the quarantine attribute with:
   ```bash
   xattr -cr ./autocam
   ./autocam
   ```
3. **Camera & Microphone Permissions**: When prompted by macOS, allow Camera access so AVFoundation can stream frames to AutoCam.

#### 🐧 Linux (Ubuntu, Debian, Fedora, Arch)
1. Download `autocam-linux-x86_64.tar.gz` and extract it:
   ```bash
   tar -xvf autocam-linux-x86_64.tar.gz
   chmod +x autocam
   ```
2. **Camera Permissions**: Ensure your user account belongs to the `video` group:
   ```bash
   sudo usermod -a -G video $USER
   ```
   *(Log out and log back in for group changes to take effect)*
3. **Display Server**: AutoCam natively supports both modern **Wayland** (Ubuntu 22.04/24.04, Fedora) and standard **X11 / Xorg**.
4. **Virtual Camera (Optional)**: If you wish to use virtual camera streaming on Linux:
   ```bash
   sudo apt install v4l2loopback-dkms
   sudo modprobe v4l2loopback devices=1 video_nr=10 card_label="AutoCam Virtual Camera" exclusive_caps=1
   ```

#### 🪟 Windows (10, 11, Windows N, Boot Camp)
1. Download `autocam.exe` (or `autocam-x86.exe` for 32-bit systems) and double-click to launch directly. No installation or runtime dependencies required (statically linked C-Runtime).
2. **MacBook Intel on Windows Boot Camp**: FaceTime HD cameras are automatically handled using uncompressed `YUYV` (`YUY2`) fallbacks without freezing the driver.

---

### 🛡️ Comprehensive Fallback Architecture (Run Everywhere)

AutoCam is designed to never crash regardless of hardware, operating system, or environment limitations:

1. **Multi-Tier Camera Format Negotiation**:
   - `MJPEG 60fps` ➔ `MJPEG 30fps` ➔ `NV12 30fps` ➔ `YUYV (YUY2) 30fps` ➔ `AbsoluteHighestFrameRate` ➔ `Driver Native Default`
   - If a camera is unplugged or busy, the background reconnection thread polls smoothly every 1.5s while the entire GUI remains 100% responsive.
2. **Audio Input Device Resilience**:
   - Safe `catch_unwind` protects against uninitialized audio drivers, and automatically falls back to `"Default Microphone"` without panicking.
3. **Platform-Aware SQLite Persistence**:
   - Windows: `%APPDATA%\AutoCam\autocam.db`
   - macOS: `~/Library/Application Support/AutoCam/autocam.db`
   - Linux: `$XDG_DATA_HOME/autocam/autocam.db` (or `~/.local/share/autocam/`)
   - Fallbacks: System Temp directory ➔ In-Memory SQLite (`Connection::open_in_memory()`) if disk is read-only or locked.
4. **Virtual Camera Bridge**:
   - Windows: OBS Virtual Camera DirectShow Shared Memory + Windows 11 Media Foundation VCam.
   - Linux: `v4l2loopback` support.
   - macOS: Gracefully disabled with zero CPU overhead and zero memory leaks.
5. **Operating System Delay-Load & Native Exception Protection**:
   - Checks `mfplat.dll`, `mf.dll`, `mfreadwrite.dll` before loading on Windows N / stripped Windows.
   - Vectored Exception Handler (VEH) captures hardware exceptions and low-level faults to provide clean user diagnostics instead of silent crashes.
