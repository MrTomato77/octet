# โครงสร้างและวงจรชีวิตโปรแกรม

หลักฐานทั้งหมดเป็น static source analysis ของ `D:\Simple IP v2\source` ณ 17 ตุลาคม 2026

## 1. ตัวตนและขอบเขต

`main.au3:26`–`43` ระบุชื่อ Simple IP Config และผู้เขียน Kurtis Liggett; `main.au3:112`–`113` ระบุ `2.9.8-b1` / `5/15/2023` ส่วน wrapper ระบุสร้างโปรแกรม x86 และไฟล์ `Simple IP Config 2.9.8-b1.exe` ที่ `main.au3:84`–`90` การมี directive เหล่านี้ไม่ได้ยืนยันว่า build ผ่านหรือมี executable นั้นอยู่จริง

`main.au3:43` ใช้ `#RequireAdmin` ดังนั้นการเปิดตามซอร์สนี้ขอสิทธิ์ผู้ดูแลตั้งแต่เริ่ม ไม่ได้แยกโหมดอ่านกับเขียน

README ระบุ GNU GPL v3.0 และหัว `main.au3:4`–`18` ระบุ GPL v3 หรือใหม่กว่า เอกสารนี้บันทึกสิ่งที่พบในไฟล์ ไม่ได้ตัดสินเรื่องสิทธิ์ของแต่ละ third-party dependency การทำ v2 จะเขียนโค้ดและใช้ assets ใหม่ ไม่ยกไอคอนใน `icon.ico` หรือ `hexIcons.au3` ไปใช้

## 2. แผนที่ไฟล์

| ส่วน | หน้าที่ที่พบ | จุดเริ่มอ่าน |
|---|---|---|
| `main.au3` | ตั้งค่ารันไทม์ ประกอบโมดูล เริ่มโปรแกรม event loop และตำแหน่ง INI | `main.au3:68`, `main.au3:193`, `main.au3:218`, `main.au3:248` |
| `forms/_form_main.au3` | สร้าง controls, toolbar, tray, shortcuts, DPI และหน้าหลัก | `_form_main` |
| `events.au3` | เชื่อม event UI กับการทำงาน รวม close/tray/keyboard/list events | `events.au3:33`, `events.au3:740` |
| `functions.au3` | กลุ่มงานระดับแอป: Apply, Save, refresh, profile I/O, status, theme, update | `functions.au3:47`, `functions.au3:572`, `functions.au3:1176` |
| `network.au3` | ค้นหาอะแดปเตอร์ อ่าน IP/registry สั่ง DHCP และ enable/disable | `network.au3:24`, `network.au3:46`, `network.au3:72` |
| `data/profiles.au3` | profile object และ collection/ordering/serialization | `_Profiles`, `_Profiles_createProfile` |
| `data/options.au3` | options object และ serialize เป็น INI section | `data/options.au3:18`, `data/options.au3:45` |
| `data/adapters.au3` | array ของ name, MAC, description และ helper functions | `data/adapters.au3:19`, `data/adapters.au3:22` |
| `cli.au3` | parser และใช้โปรไฟล์จาก command line | `cli.au3:25`, `cli.au3:76` |
| `languages.au3`, `lang/*.json` | language objects, fallback English, อัปเดตข้อความ controls | `languages.au3:18`, `languages.au3:266` |
| `libraries/asyncRun.au3` | คิว process และ callback | `libraries/asyncRun.au3:33`, `libraries/asyncRun.au3:58` |
| `libraries/_NetworkStatistics.au3` | wrapper สำหรับข้อมูลเครือข่าย Windows; อ่านเฉพาะส่วนที่เส้นทางหลักใช้และข้อมูล DHCP ที่เกี่ยวข้อง | ดูเอกสารเครือข่าย |
| `forms/_form_*.au3` ที่เหลือ | dialogs เช่น Settings, Blacklist, About, Update, Debug และ error/restart | แต่ละ `_form_*` |
| `hexIcons.au3`, `icon.ico` | assets ของโปรแกรมเดิม | `hexIcons.au3:20`, `hexIcons.au3:29` |

ข้อควรระวัง: แม้ชื่อและ comment บางแห่งเรียกว่า objects แต่ adapters เป็น array; profiles และ options สร้างด้วย AutoItObject ไม่ควรอนุมานชนิดข้อมูลจากชื่ออย่างเดียว

## 3. ลำดับเปิดโปรแกรม

1. Include Windows/GUI helpers และเรียก `_AutoItObject_StartUp()` ที่ `main.au3:46`–`68`
2. เปิด GUI/Tray event mode ปิด tray auto-pause และปิด Escape-to-close ที่ `main.au3:70`–`81`
3. เลือกที่เก็บ INI ก่อนเริ่มอ่านข้อมูลผ่าน `_setProfilesIniLocation()` ที่ `main.au3:103`–`107`
4. Include โมดูลแอปและฟอร์มทั้งหมดที่ `main.au3:193`–`216`
5. สร้าง options, profiles, adapters และเริ่มภาษา ที่ `main.au3:219`–`225`
6. ตรวจ command line ก่อน singleton/GUI ที่ `main.au3:227`–`238` จึงต้องวิเคราะห์ CLI แยก ไม่ถือว่าพฤติกรรมของ instance GUI ครอบทุกกรณี
7. GUI instance ที่ซ้ำส่ง registered window message แล้วออก; ตัวเดิมรับ `_NewInstance` และ `_maximize()` ที่ `main.au3:231`–`238`, `main.au3:399`–`408`
8. `_main()` หา DPI, โหลด profiles/options, เลือกภาษา, สร้างหน้าหลัก, โหลด adapters และ refresh ที่ `main.au3:255`–`307`
9. อ่าน double-click time ของ Windows, แสดง changelog ตามเงื่อนไข, อ่าน domain และตรวจ update ถ้าเปิดตัวเลือก ที่ `main.au3:309`–`324`

## 4. ขณะทำงานและการออก

Events เรียกฟังก์ชันงานเป็นหลัก ส่วน loop ทุก 100 ms จัดการ flags ของ rename, tab navigation, Open/Import/Export และงานเลื่อน focus (`main.au3:328`–`391`) คิว network process มี Adlib callback ทุก 100 ms แยกอีกส่วนหนึ่ง (`libraries/asyncRun.au3:45`) ไม่ใช่ thread worker ที่เขียนใน Rust

Open ตั้ง path ใหม่และสร้าง options/profiles ใหม่ ก่อน refresh; Import ผสานเข้าไฟล์เดิม; Export เป็น `FileCopy` ของ active INI (`main.au3:351`–`382`)

ปุ่มปิดเรียก `_onExit`: ถ้า MinToTray เปิดจะซ่อนลง tray ถ้าไม่เปิดจะ `_Exit` (`events.au3:33`–`40`) เมนู Exit เรียก `_Exit` โดยตรง (`events.au3:47`–`49`) `_Exit` บันทึกขนาด/ตำแหน่งหน้าต่างลง Options และออก (`events.au3:56`–`83`) ยังไม่ได้ทดสอบพฤติกรรมการปิดขณะคิวเปลี่ยนเครือข่ายทำงาน

## 5. ภาษา การอัปเดต และข้อมูล debug

- รายการภาษามาจาก `lang/lang-*.json` และ metadata ใน JSON (`languages.au3:18`–`34`); ใช้ภาษา OS หรือค่าที่บันทึกไว้ (`main.au3:263`–`269`)
- ถ้าเปิดไฟล์ภาษาไม่ได้ `_setLangStrings` ใช้ English ในตัว (`languages.au3:266`–`281`) ยังไม่ได้ยืนยัน fallback เมื่อ JSON เสียหรือ key ขาด
- เมื่อ AutoUpdate เปิด โปรแกรมเรียก GitHub releases API จาก `_checksSICUpdate` (`main.au3:319`–`324`, `functions.au3:131`–`151`) ชื่อ option ไม่ได้พิสูจน์ว่ามีการติดตั้ง update อัตโนมัติ
- เส้นทาง UI ที่ตรวจพบแสดงเวอร์ชันและลิงก์เปิด release page (`forms/_form_update.au3:64`–`74`, `events.au3:590`–`594`)
- มี `_DoUpdate` ซึ่งเขียน batch เพื่อ kill/delete/start (`functions.au3:246`–`257`) แต่การค้น `_DoUpdate` ทั้ง `source/` พบเพียง definition/end marker ไม่พบ call site จึงจัดเป็นฟังก์ชันที่ยังไม่พบผู้เรียก ไม่ใช่ update flow ที่ยืนยันว่าถูกใช้
- Debug dialog แสดง OS/build/language/architecture/resolution/DPI (`forms/_form_debug.au3:37`–`48`) ไม่ใช่ DHCP diagnostic report ส่วน `_print` เขียนเวลาสะสมและช่วงห่างลง console (`functions.au3:1591`–`1604`)

## 6. สิ่งที่ยังไม่ยืนยัน

- ไม่พบ AutoIt3/Aut2exe ใน command lookup ของ session นี้ แต่ไม่ได้ค้นทุกตำแหน่งติดตั้ง และไม่ได้ติดตั้งเพิ่ม
- ไม่มีการ build ดังนั้นยังไม่ยืนยัน dependency compatibility ของ AutoIt standard include files กับ source รุ่นนี้
- ไม่ทราบ commit หรือ release ต้นทางแน่นอน: repository ของ `D:\Simple IP v2` ยังไม่มี commit และไม่มี remote ที่แสดงในเวลาตรวจ README อ้างชื่อ upstream ได้ แต่ไม่ใช่หลักฐานยืนยัน snapshot identity; ใช้ hashes ใน manifest ระบุต้นฉบับที่วิเคราะห์แทน
- ไม่เปิดเน็ตเพื่อค้นรุ่นล่าสุด ไม่มีการเรียก GitHub API ที่พบในโค้ด ไม่มีการเปิด GUI และไม่มี live/runtime evidence
