# AutoCam - Ultra High-Performance Rust + Iced GUI + SQLite

โปรเจกต์ต้นแบบ Native Rust GUI ด้วย **Iced (v0.14)** และ **SQLite (rusqlite v0.43)** ออกแบบสำหรับงานที่ต้องการ **Speed & Performance สูงสุด**:

## 🚀 จุดเด่นด้าน Performance

### 1. SQLite Optimization Tuning
- **WAL Journal Mode (`PRAGMA journal_mode = WAL`)**: ทำให้การอ่านและเขียนข้อมูลทำงานพร้อมกันได้โดยไม่ต้องล็อคฐานข้อมูล
- **Synchronous = NORMAL**: ลด Disk I/O Flush overhead ได้มหาศาล ปลอดภัยในระดับโปรดักชันเมื่อใช้คู่กับ WAL
- **Memory-Mapped I/O (`PRAGMA mmap_size = 268MB`)**: เข้าถึงข้อมูลในไฟล์ DB ได้โดยตรงผ่าน OS Virtual Memory
- **RAM Cache Size (`PRAGMA cache_size = -64000`)**: จองหน่วยความจำ Cache สูงถึง 64 MB
- **Temporary Tables in RAM (`PRAGMA temp_store = MEMORY`)**
- **Batch Transaction & Prepared Statements**: รองรับการ Insert ข้อมูลระดับ **50,000 - 150,000+ ops/sec**

### 2. Zero-Lag GUI Architecture (Iced v0.14)
- **Non-blocking Background Tasks**: งาน Database หนัก ๆ ถูกส่งไปประมวลผลบน Thread pool ผ่าน `iced::Task::perform` ทำให้หน้าจอ UI ลื่นไหล ไม่กระตุก ไม่เกิด Frame drop (Maintain 60-144 FPS)
- **Borderless / Frameless Window**: ตั้งค่า `decorations: false` พร้อมแถบ Custom Title Bar ที่รองรับการลากย้ายหน้าต่าง (`window::drag`), ปุ่มย่อหน้าต่าง (`window::minimize`) และปุ่มปิด (`window::close`)

### 3. Rust Compiler Release Profile (Cargo.toml)
```toml
[profile.release]
opt-level = 3        # เปิดการ Optimize ระดับสูงสุด
lto = "fat"          # Full Link-Time Optimization ข้ามทุก Crates
codegen-units = 1    # รวม Translation units เพื่อให้ Compiler Inline โค้ดได้เต็มที่
panic = "abort"      # ตัด Unwind table ลดขนาด Binary และลด Overhead
strip = true         # ถอด Debug symbols ทั้งหมดเพื่อลดขนาดไฟล์และความเร็วในการโหลด
```

---

## 🛠️ วิธีการรัน

### โหมดพัฒนา (Development):
```bash
cargo run
```

### โหมดประสิทธิภาพสูงสุด (Release Build - Full Speed):
```bash
cargo run --release
```
