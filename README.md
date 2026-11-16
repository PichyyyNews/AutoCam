# AutoCam ⚡

โปรแกรมเปิดกล้องเว็บแคมสำหรับช่วยสอน (เช่น การต่อวงจรบน Breadboard, ส่องชิ้นส่วนอุปกรณ์อิเล็กทรอนิกส์, หรืองานสาธิตการปฏิบัติจริง) พัฒนาด้วยภาษา **Rust** โดยใช้ **Iced GUI (v0.14)** และ **Windows Media Foundation** ผ่านสถาปัตยกรรมที่ออกแบบมาเพื่อ **ประหยัด RAM สูงสุด (<30-50MB)** และหน้าจอที่สะอาดเรียบตามแนวทาง **Clean UI Design**.

---

## ✨ ฟีเจอร์หลัก (Features)

### 1. 🪟 Custom Draggable Title Bar (Borderless Window)
* หน้าต่างแบบไร้ขอบ (`decorations: false`)
* สามารถคลิกค้างที่แถบด้านบนเพื่อลากย้ายหน้าต่างได้ทันที (`window::drag`)
* มีปุ่มย่อหน้าต่าง (`window::minimize`) และปุ่มปิด (`window::close`)

### 2. 📌 Picture-in-Picture (PiP Mode)
* ย่อหน้าต่างเป็นกล้องมุมจอลอยขนาดกะทัดรัด อัตราส่วน 16:9 (`384x216`)
* ตรึงหน้าต่างอยู่บนสุดตลอดเวลา (`Level::AlwaysOnTop`) เพื่อให้ผู้สอนสามารถเปิดสไลด์ หรือแชร์จออื่นไปพร้อมกับแสดงภาพโต๊ะวงจรได้
* **Auto-hide Controls**: เมื่อเมาส์อยู่นอกหน้าต่าง จะแสดงภาพกล้องเต็ม 100% โดยไม่มีแถบหรือปุ่มมารบกวน และเมื่อเลื่อนเมาส์ชี้ แถบควบคุมจะปรากฏขึ้นอัตโนมัติ

### 3. 🔬 Circuit Teaching Utilities (เครื่องมือช่วยสอนต่อวงจร)
* **Device Selector**: สลับเลือกกล้องเว็บแคมที่เชื่อมต่อบนเครื่องได้อิสระ
* **Resolution Switcher**: ปรับสลับความละเอียด 720p (ค่าเริ่มต้น ประหยัด RAM) และ 1080p (คมชัดสูงสุด)
* **Horizontal Flip / Mirror `[F]`**: กลับภาพซ้าย-ขวา เพื่อให้อ่านตัวอักษรบนบอร์ดทดลอง/แผ่นวงจรได้ถูกต้อง
* **Freeze Frame `[Space]`**: หยุดภาพนิ่งชั่วคราว เพื่อค้างภาพไว้ชี้อธิบายจุดต่อสายไฟหรือขาชิป IC
* **Rotate `[R]`**: หมุนภาพ 0°, 90°, 180°, 270° รองรับกล้องติดขาตั้งมุมก้มหรือกล้องติดกลับหัว
* **Digital Zoom `[+ / -]`**: ซูมดิจิทัล 1.0x ถึง 2.5x เพื่อส่องดูรหัสบนตัวต้านทานและอุปกรณ์ขนาดเล็ก

### 4. ⌨️ Shortcuts (คีย์ลัดสำหรับผู้สอน)
| ปุ่ม | หน้าที่ |
|---|---|
| `Space` | Freeze / Unfreeze (หยุดภาพนิ่ง / เล่นต่อ) |
| `F` | Flip (กลับภาพแนวนอน ซ้าย-ขวา) |
| `R` | Rotate (หมุนมุมภาพทีละ 90 องศา) |
| `P` | Toggle PiP (สลับโหมดหน้าต่างลอย Always-on-Top) |
| `+` / `=` | Zoom In (ซูมภาพเข้า) |
| `-` | Zoom Out (ซูมภาพออก) |

---

## ⚡ Low RAM Architecture (<30-50MB)

1. **Recycled Buffer Pipeline**: ภาพจากกล้องจะถูกถอดรหัสลงใน Pre-allocated Scratch Buffer เดิมซ้ำๆ ไม่มีการสร้าง Memory Allocation ใหม่ในทุก Frame ลดปัญหา Memory Fragmentation และ Garbage build-up
2. **Non-blocking Camera Worker**: กล้องทำงานใน Thread แยกอิสระ สตรีมภาพผ่าน Atomic Slot ส่งต่อไปยัง Iced UI ทำให้ GUI ตอบสนองทันที ไม่มีอาการกระตุกหรือ Frame drop
3. **Optimized Release Binary**:
   * Binary ขนาดเพียง ~9.7 MB (Standalone รวมไดรเวอร์กราฟิกและกล้องทั้งหมด)
   * Link-Time Optimization (`lto = "fat"`)
   * Optimization Level 3 (`opt-level = 3`)
   * Strip Debug Symbols (`strip = true`)

---

## 🎨 Clean UI Design Standards
* **Palette**: Dark neutral palette (Slate/Zinc: `#121214`, `#1a1a1d`, `#2e2e33`)
* **Typography**: ลำดับชัดเจน ข้อความสั้น กระชับ ตรงไปตรงมา
* **No AI Slop**: ไม่มี gradient ฉูดฉาด, ไม่มีขอบเรืองแสง (glow), ไม่มี card ซ้อน card, ใช้กรอบบางเรียบ 1px

---

## 🚀 วิธีการคอมไพล์และรัน

```powershell
# โหมดพัฒนา (Development)
cargo run

# โหมดประสิทธิภาพสูงสุด (Release Build)
cargo run --release
```
