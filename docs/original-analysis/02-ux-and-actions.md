# บันทึกการทำงานเดิม: หน้าจอและเส้นทางการใช้งาน

วันที่ตรวจ: 17 ตุลาคม 2026 • เป้าหมาย: `D:\Simple IP v2\source` • วิธี: อ่านซอร์ส AutoIt แบบ static เท่านั้น

เอกสารนี้อธิบายพฤติกรรมตามซอร์ส ไม่ได้เปิดโปรแกรม ไม่ได้กด Apply ไม่ได้เปลี่ยนเครือข่าย และไม่ใช่ผลทดสอบ executable ที่เผยแพร่จริง ใช้เส้นทาง source-first ตาม skill REA เพราะมีซอร์สพร้อมอ่าน จึงไม่จำเป็นต้องถอด binary หรือเรียกเครื่องมือ REA

การอ้างอิงทั้งหมดเป็น `ไฟล์สัมพัทธ์ภายใต้ source:เลขบรรทัด` สถานะ **พบในโค้ด** หมายถึงอ่านพบโดยตรง, **ข้ออนุมาน** หมายถึงผลที่น่าจะเกิดจาก control flow และ **ยังไม่ยืนยัน** หมายถึงต้องตรวจ runtime หรือเส้นทางอื่นเพิ่ม

## 1. โครงสร้างหน้าจอจริงตามซอร์ส

**พบในโค้ด:** เป็นหน้าหลักเดียว สองคอลัมน์: ซ้ายคือรายชื่อโปรไฟล์; ขวาเรียงจากบนลงล่างเป็นอะแดปเตอร์และค่าปัจจุบัน, ค่าที่ต้องการตั้งและปุ่ม Apply, แล้ว Memo ขนาดเริ่มต้นกำหนด 600 × 675 และพื้นที่ขวา 395 หน่วยก่อน scale (`main.au3:115`, `main.au3:121`, `forms/_form_main.au3:267`, `forms/_form_main.au3:388`, `forms/_form_main.au3:485`, `forms/_form_main.au3:678`)

```text
แถบเมนู File | View | Tools | Help
┌─────────────────┬───────────────────────────────────┐
│ New Save Delete │ เลือกอะแดปเตอร์          Refresh │
│ ค้นหาโปรไฟล์   │ คำอธิบายและ MAC                  │
│                 │ ค่าปัจจุบัน (อ่านอย่างเดียว)       │
│ รายชื่อโปรไฟล์  │ IP / Mask / Gateway / DNS / DHCP  │
│                 │ Adapter state                     │
│                 ├───────────────────────────────────┤
│                 │ IP auto/manual + ช่องที่แก้ได้     │
│                 │ DNS auto/manual + DNS register     │
│                 │               Apply               │
│                 ├───────────────────────────────────┤
│                 │ Memo                              │
└─────────────────┴───────────────────────────────────┘
Computer / Domain             Status และรายละเอียด error
```

แผนภาพเป็นการสรุปตำแหน่งตามคำสั่งสร้าง control ไม่ใช่ภาพที่ capture จากโปรแกรม

| พื้นที่ | ข้อมูลและการกระทำ | หลักฐาน |
|---|---|---|
| ซ้ายบน | ปุ่มไอคอน New, Save, Delete | `forms/_form_main.au3:282`, `forms/_form_main.au3:287`, `forms/_form_main.au3:295` |
| ซ้าย | ช่องค้นหาเริ่มด้วย `*`, listview ซ่อนหัวคอลัมน์และแก้ชื่อในแถวได้ | `forms/_form_main.au3:329`, `forms/_form_main.au3:340` |
| ขวาบน | dropdown อะแดปเตอร์, description, MAC, Refresh | `forms/_form_main.au3:399`, `forms/_form_main.au3:402`, `forms/_form_main.au3:405`, `forms/_form_main.au3:412` |
| ค่าปัจจุบัน | ช่อง read-only: IP, subnet, gateway, DNS สองตัว, DHCP และ adapter state | `forms/_form_main.au3:425`, `forms/_form_main.au3:429`, `forms/_form_main.au3:477`, `functions.au3:1389` |
| ค่าที่จะตั้ง | radio IP/DNS, IPv4 controls, copy/paste รายช่อง, Register DNS, Apply | `forms/_form_main.au3:497`, `forms/_form_main.au3:515`, `forms/_form_main.au3:595`, `forms/_form_main.au3:654`, `forms/_form_main.au3:668` |
| Memo | ช่อง multiline ผูกกับโปรไฟล์ | `forms/_form_main.au3:684`, `functions.au3:1114`, `functions.au3:1045` |
| สถานะ | แยกข้อความปกติกับ error; error เปิดรายละเอียดได้และโค้ดเปิด popup ทันที | `forms/_form_main.au3:217`, `forms/_form_main.au3:224`, `forms/_form_main.au3:228`, `functions.au3:1461`, `functions.au3:1473` |

## 2. เส้นทางหลักและความแตกต่างที่สำคัญ

### เลือกโปรไฟล์: โหลดเข้าแบบฟอร์ม

1. คลิกแถว → `_onSelect` → `_setProperties` (`events.au3:217`, `functions.au3:1086`)
2. อ่านค่าจาก object โปรไฟล์ แล้วใส่ IP, mask, gateway, DNS, DNS register และ memo ลงช่องแก้ไข (`functions.au3:1091`, `functions.au3:1114`)
3. ถ้าเปิด SaveAdapterToProfile จะพยายามเลือกอะแดปเตอร์จากชื่อที่เก็บไว้; ถ้าไม่พบจะล้าง selection และอัปเดตค่าปัจจุบัน (`functions.au3:1117`)
4. เรียก `_radios` เพื่อเปิด/ปิดช่องตามโหมด (`functions.au3:1130`)

**พบในโค้ด:** การเลือกโปรไฟล์ไม่ได้เรียก `_apply` จาก handler นี้ จึงเป็นการเตรียมแบบฟอร์มก่อนใช้จริง ข้อมูลโปรไฟล์, ช่องแก้ไข และค่าปัจจุบันเป็นสามสถานะที่ต้องแยกความเข้าใจ แม้หน้าจอจะมีสองคอลัมน์

### Apply: ใช้ค่าปัจจุบันในแบบฟอร์ม

ปุ่ม Apply และ File → Apply ไป `_apply_GUI`; ฟังก์ชันอ่าน radio, ช่อง IP/DNS และอะแดปเตอร์ที่เลือกขณะนั้น แล้วส่งไป `_apply` ไม่ได้อ่านโปรไฟล์จากไฟล์ซ้ำ (`events.au3:226`, `forms/_form_main.au3:670`, `functions.au3:572`, `functions.au3:583`)

**ข้ออนุมาน:** ผู้ใช้สามารถแก้ค่าชั่วคราวแล้ว Apply โดยยังไม่ Save ลงโปรไฟล์ และการกด Apply ไม่เท่ากับบันทึกโปรไฟล์ จุดนี้ควรรักษาความชัดเจนใน Rust v2

จุดเรียก Apply เพิ่มเติม:

- ดับเบิลคลิกแถว: `_clickDn` เทียบเวลาตามระบบกับแถวก่อนหน้า, `_clickUp` เรียก Apply เมื่อ flag double-click เป็นจริง (`functions.au3:359`, `functions.au3:365`, `functions.au3:400`, `functions.au3:403`, `main.au3:309`)
- Enter: accelerator ระดับหน้าต่างส่งไป `_onLvEnter`; เมื่อไม่แก้ชื่อจะ Apply ยกเว้นกรณี memo ที่ส่ง Enter กลับให้ช่อง (`forms/_form_main.au3:727`, `events.au3:399`)

### Save: เขียนแบบฟอร์มลงโปรไฟล์

ต้องมีโปรไฟล์ที่เลือก; ถ้ากำลังแก้ชื่อจะส่ง Enter จบการแก้ก่อน จากนั้นอ่านช่องทั้งหมดรวม memo และชื่ออะแดปเตอร์ เขียน INI section ของโปรไฟล์ (`functions.au3:1019`, `functions.au3:1031`, `functions.au3:1036`, `functions.au3:1048`)

**พบในโค้ด:** ชื่ออะแดปเตอร์ถูกบันทึกทุกครั้งใน `_save`; ตัวเลือก SaveAdapterToProfile ใช้กำกับการคืนอะแดปเตอร์ตอนโหลดในเส้นทางที่อ่านนี้ (`functions.au3:1044`, `functions.au3:1117`)

### Pull: คัดลอกค่าที่เห็นเป็นค่าคงที่

Tools → Pull / Ctrl+P → `_pull`: นำค่าจาก control ของ Current IP/Mask/Gateway/DNS ไปยังช่องแก้ไข แล้วตั้ง IP manual และ DNS manual ก่อนปรับสถานะช่อง (`events.au3:458`, `functions.au3:800`)

**พบในโค้ด:** Pull ไม่ได้ query Windows ใหม่ในฟังก์ชันนี้, ไม่ได้รักษาโหมด DHCP ของอะแดปเตอร์ และไม่ได้ Apply หรือ Save อัตโนมัติ (`functions.au3:800` ถึง `functions.au3:809`)

**ข้ออนุมาน:** ถ้าหน้า Current ล้าสมัย Pull จะนำค่าล้าสมัยไปเตรียมเป็น static ด้วย ใน v2 ควรระบุว่าเป็น “คัดลอกเป็นค่าคงที่” และ refresh ก่อนอ่านหรือแสดง timestamp

### Refresh: โหลดรายการและค่าปัจจุบันใหม่

`_onRefresh` ล้าง warning, จำ index โปรไฟล์ → `_refresh` → เรียก `_updateCombo`, `_loadProfiles`, `_updateProfileList`, `_updateCurrent` → คืน selection ตาม index (`events.au3:338`, `functions.au3:1061`)

**พบในโค้ด:** Refresh ยกเลิกการแก้ชื่อที่กำลังทำ, โหลดโปรไฟล์จากไฟล์ใหม่ และไม่เรียก `_setProperties` ตรง ๆ ในตัวฟังก์ชันนี้ (`functions.au3:1062`, `functions.au3:1067`)

**ข้ออนุมาน:** อย่าอธิบาย Refresh ว่าโหลดเฉพาะ IP; มันเปลี่ยน adapter list และรายการโปรไฟล์ด้วย การคืนด้วย index เสี่ยงเลื่อนไปคนละโปรไฟล์หากไฟล์ถูกเปลี่ยนลำดับระหว่างนั้น

### เลือกอะแดปเตอร์

การเปลี่ยน dropdown เรียก `_updateCurrent` และเขียน StartupAdapter ลง INI; WM_COMMAND ส่ง dummy event ซ้ำเมื่อ dropdown ปิด (`events.au3:563`, `events.au3:755`)

Current panel อ่าน `_getIPs` แล้วแสดง 7 ค่า และเปลี่ยนชื่อเมนู Enable/Disable ตาม adapter state (`functions.au3:1364`, `functions.au3:1389`, `functions.au3:1397`)

## 3. เงื่อนไขของ IP และ DNS

**พบในโค้ด:**

- IP automatic ปิดช่อง IP/mask/gateway และเปิดสิทธิ์เลือก DNS automatic (`functions.au3:537`)
- IP manual บังคับ DNS manual และปิด radio DNS automatic; จึงรองรับ DHCP IP + manual DNS แต่ static IP + automatic DNS ถูกป้องกันใน UI (`functions.au3:546`, `functions.au3:547`)
- DNS automatic ปิด DNS สองช่องและ checkbox Register DNS; DNS manual เปิดกลับ (`functions.au3:557`)
- บาง click handler ตั้ง radio ชั่วคราวแล้ว `_radios` ปรับอีกครั้ง จึงควรอ่านผลสุดท้ายจาก `_radios` (`events.au3:167`, `functions.au3:537`)
- เมื่อย้าย focus ไป subnet และค่าว่าง/ศูนย์ทั้งหมด จะใส่ `255.255.255.0` ให้อัตโนมัติ (`events.au3:820`, `functions.au3:1511`)
- ตัวอ่านช่องแปลง `0.0.0.0` เป็นค่าว่าง (`functions.au3:1139`)
- Clear ล้างเฉพาะ 5 ช่อง address ไม่เปลี่ยน memo หรือ radio ในฟังก์ชันนี้ (`functions.au3:881`)

## 4. รายชื่อโปรไฟล์และ keyboard

| การกระทำ | พฤติกรรมตามโค้ด | หลักฐาน |
|---|---|---|
| New / Ctrl+N | สร้างชื่อชั่วคราวไม่ซ้ำ, focus list, เริ่มแก้ชื่อ | `events.au3:286` |
| F2 / Rename | ทำงานเฉพาะเมื่อ list มี focus | `events.au3:254` |
| จบแก้ชื่อ | เส้นทางยอมรับตรวจ Enter; ถ้า cancel หรือชื่อใหม่ว่างลบแถวใหม่, flags ให้ main loop บันทึก/rename ต่อ | `events.au3:797`, `functions.au3:858`, `main.au3:339` |
| Ctrl+S | Save แบบฟอร์มลงโปรไฟล์ที่เลือก | `forms/_form_main.au3:735`, `functions.au3:1019` |
| Delete | ลบโปรไฟล์เมื่อ list focus; ที่อื่นส่ง key กลับให้ control | `events.au3:350` |
| Up / Down | เมื่อ list focus เลื่อนแถวและโหลดค่าโปรไฟล์; ที่อื่นส่ง key กลับ | `events.au3:365`, `events.au3:382` |
| Enter | Apply เมื่อไม่แก้ชื่อ; memo ได้ newline; handler ไม่มีเงื่อนไขบังคับ list focus | `events.au3:399` |
| Tab | ถ้าแก้ชื่อใช้ Enter จบก่อนและ main loop ส่ง Tab ต่อ; นอกนั้นส่ง Tab ให้ระบบ | `events.au3:269`, `main.au3:385` |
| Ctrl+R / Ctrl+P / Ctrl+T / F1 | Refresh / Pull / ส่ง tray / Help | `forms/_form_main.au3:729`, `forms/_form_main.au3:739` |
| คลิกขวา list | Rename, Delete, เรียง A–Z / Z–A, สร้าง shortcut | `forms/_form_main.au3:348` |
| ลากแถว | พยายามย้ายลำดับใน INI และใน memory | `functions.au3:407` |

Filter ทำงานเมื่อข้อความเปลี่ยน ใช้ regular expression แบบ case-insensitive โดยแทน `*` เป็น `.*`, ลบรายการเดิมแล้วสร้างแถวที่ match ใหม่; ข้อความว่างแสดงทั้งหมด (`events.au3:750`, `functions.au3:1405`)

**ข้ออนุมาน:** ตัวกรองไม่ใช่ literal substring ธรรมดา อักขระ regex อื่นไม่ได้ escape และในฟังก์ชันนี้ไม่มี error UI สำหรับ pattern ที่ไม่ถูกต้อง การกรองยังไม่คืน selected item เดิมโดยตรง (`functions.au3:1413`, `functions.au3:1425`)

## 5. Tray, theme, settings และการซ่อนอะแดปเตอร์

- Tray click ซ้ายและเมนู Restore/Hide ใช้ข้อความเมนูเป็นตัวเลือกว่าจะ restore หรือ hide (`events.au3:108`, `events.au3:121`)
- ส่ง tray คือ minimize แล้ว hide; restore คือ restore/show และเปลี่ยนเมนูกลับเป็น Hide (`functions.au3:1432`, `functions.au3:1453`)
- ปุ่มปิดหน้าต่างจะ hide แทน exit เมื่อ MinToTray เปิด; File/Tray Exit ออกจากโปรแกรมเสมอ (`events.au3:33`, `events.au3:47`)
- การผูก minimize event ถูก comment; จึงยังยืนยันไม่ได้ว่าตัวเลือก MinToTray ทำให้ปุ่มย่อหน้าต่างส่ง tray จริง เส้นทาง close ทำเช่นนั้นแน่นอน (`forms/_form_main.au3:83`, `functions.au3:1444`)
- StartupMode เปิดแล้วไม่แสดง main window และตั้ง tray menu เป็น Restore; ถ้าไม่เปิดจะคืนขนาดปกติหรือ maximized ตามค่าเดิม (`forms/_form_main.au3:762`)
- Theme Light/Dark เปลี่ยนสี custom controls และบันทึก INI ทันที; startup อ่านค่ากลับ (`events.au3:430`, `events.au3:444`, `forms/_form_main.au3:709`, `functions.au3:1626`)
- Settings เป็น child window ที่ disable หน้าหลัก มีภาษา, เริ่มใน tray, MinToTray, SaveAdapterToProfile และ AutoUpdate; Save เขียน options section; เปลี่ยนภาษาแล้วลบ/สร้าง GUI ใหม่ (`forms/_form_settings.au3:18`, `forms/_form_settings.au3:61`, `forms/_form_settings.au3:80`, `forms/_form_settings.au3:85`, `forms/_form_settings.au3:106`)
- Hide adapters เป็น child window แบบ checkbox บันทึกชื่อที่เลือกต่อกันด้วย `|`, เขียน AdapterBlacklist แล้ว rebuild dropdown; เป็นการซ่อนจากรายการ ไม่ได้ disable network adapter (`forms/_form_blacklist.au3:48`, `forms/_form_blacklist.au3:82`, `forms/_form_blacklist.au3:99`, `functions.au3:283`)
- Main menu มี Tools: Network Connections, Pull, Enable/Disable, Release, Renew, Cycle, เปิดที่เก็บโปรไฟล์ และ Settings (`forms/_form_main.au3:154`)

## 6. จุดเสี่ยงจากซอร์สที่ควรยืนยันก่อนออกแบบ v2

| ประเด็น | ระดับหลักฐาน | ผลกระทบที่น่าทดสอบ | อ้างอิง |
|---|---|---|---|
| Enter เป็น accelerator ระดับหน้าต่างแต่ handler ไม่ตรวจ list focus ยกเว้น memo/rename | พบในโค้ด; ผลบน control แต่ละชนิดยังไม่ยืนยัน | กด Enter ใน search/IP field อาจ Apply โดยไม่ได้ตั้งใจ | `forms/_form_main.au3:727`, `events.au3:399` |
| คืนอะแดปเตอร์จากโปรไฟล์ด้วยชื่อและค้นเริ่ม row 1 | พบในโค้ด; เป็น defect candidate | อะแดปเตอร์ row 0 อาจไม่พบ; rename adapter ทำให้ binding หลุด | `functions.au3:1120`, `data/adapters.au3:22` |
| เมื่อคืนอะแดปเตอร์สำเร็จไม่มี `_updateCurrent` ตรงใน branch | พบในโค้ด; notification ของ Windows ยังไม่ยืนยัน | Current panel อาจยังแสดงอะแดปเตอร์ก่อนหน้า | `functions.au3:1121` |
| Pull ใช้ค่าบนจอและบังคับ manual | พบในโค้ด | snapshot ของ DHCP อาจถูกเข้าใจว่าเก็บ DHCP profile | `functions.au3:800` |
| ไม่พบ dirty-state guard ในเส้นทางเลือกโปรไฟล์/Pull ที่อ่าน | ข้อสังเกตจำกัดเฉพาะเส้นทางเหล่านี้ | ค่าที่ยังไม่ Save ถูกแทนโดยไม่มี prompt | `events.au3:217`, `functions.au3:1086`, `functions.au3:800` |
| Filter ใช้ regex โดยตรงและ Refresh สร้างรายการจากชื่อทั้งหมด | พบในโค้ด; ผลร่วมยังไม่ยืนยัน | pattern ไม่ถูกต้อง/selection หาย; Refresh อาจแสดงทั้งหมดทั้งที่ช่อง filter ยังมีค่า | `functions.au3:1405`, `functions.au3:1326`, `functions.au3:1061` |
| Screenshot flag เปิดเป็น 1 ในซอร์ส | พบในโค้ด | MAC, computer/domain แสดง placeholder ถ้า build จากไฟล์นี้ตรง ๆ | `main.au3:103`, `functions.au3:1375`, `forms/_form_main.au3:251`, `functions.au3:1488` |
| Memo geometry มี scale ซ้ำในบางตำแหน่ง | พบในสูตร; ผลต้องยืนยัน runtime | ตำแหน่ง/ขนาดบน DPI สูงอาจไม่ถูกต้อง | `forms/_form_main.au3:682`, `forms/_form_main.au3:684` |
| หน้าต่างทั้งแอป require admin | พบในโค้ด | การเปิดอ่าน/แก้โปรไฟล์ก็ถูกยกระดับตั้งแต่เริ่ม | `main.au3:43` |

หมายเหตุ: `Adapter()` คืน array สองมิติจริง แม้ comment ใช้คำว่า object; การใช้ `_ArraySearch` จึงไม่ใช่ข้อผิดพลาดเรื่องชนิดข้อมูล (`data/adapters.au3:22`)

## 7. Checklist ความครอบคลุมและ checkpoint ถัดไป

### ตอบแล้วจาก static source

- [x] โครงสร้างหน้าเดียว ตำแหน่ง controls และค่าปัจจุบันเทียบค่าที่กำลังแก้
- [x] เลือกโปรไฟล์ / Apply / Save / Pull / Refresh เป็นคนละการกระทำ
- [x] double-click, Enter, rename, filter, drag และ keyboard shortcuts
- [x] IP/DNS mode restrictions และ subnet default
- [x] tray, theme, settings, blacklist และ status popup
- [x] ระบุ candidate pitfalls พร้อมไฟล์และบรรทัด ไม่เรียกว่า runtime bug ที่พิสูจน์แล้ว

### ยังต้องยืนยันภายหลัง หากต้องการตรวจพฤติกรรมที่เผยแพร่จริง

- [ ] executable ที่ใช้จริงตรงกับซอร์สเวอร์ชัน 2.9.8-b1 นี้หรือไม่
- [ ] กด Enter ในแต่ละ field, double-click และ drag มีผลจริงอย่างไรบน Windows ปัจจุบัน
- [ ] DPI 100/125/150/200%, resize และ memo; screenshot flag ใน binary จริง
- [ ] current panel refresh หลังเลือก profile ที่ผูกอะแดปเตอร์, filtering + refresh, ไม่มีอะแดปเตอร์/ซ่อนทั้งหมด
- [ ] network execution, DHCP diagnostics, timeout และ partial failure: อยู่ในการวิเคราะห์ subsystem เครือข่าย ไม่ได้ยืนยันด้วยรายงาน UX นี้

Checkpoint สำหรับ Rust v2 เป็น **ข้อเสนอออกแบบใหม่**: คง workflow หลักในหน้าจอเดียว แต่กำหนด 3 state ชัดเจน (saved profile / edited draft / live adapter snapshot), คง Save แยก Apply, จำกัด Enter ให้ไม่เปลี่ยนเครือข่ายโดยไม่ตั้งใจ, ใช้ stable adapter identity, ทำ Pull ให้บอกว่า snapshot หรือ DHCP semantics, และใส่ timestamp/refresh state ให้ Current panel ทั้งหมดนี้เป็นข้อเสนอจากข้อค้นพบ ไม่ใช่พฤติกรรมที่อ้างว่าแอปเดิมมีอยู่แล้ว
