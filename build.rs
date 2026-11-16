fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("public/icon.ico");
        res.set("InternalName", "autocam.exe");
        res.set("FileDescription", "AutoCam - High Performance Camera Streamer");
        res.set("ProductName", "AutoCam");
        res.set("OriginalFilename", "autocam.exe");
        res.compile().expect("Failed to compile Windows resource icon");
    }
}
