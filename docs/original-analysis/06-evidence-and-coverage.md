# ทะเบียนหลักฐาน ความครอบคลุม และสิ่งที่ยังไม่ยืนยัน

ตรวจเมื่อ 17 ตุลาคม 2026 เป้าหมายเดียวคือ `D:\Simple IP v2\source` รหัส `SRC-*` ในเอกสารนี้ตั้งขึ้นเพื่อค้นกลับภายในรายงาน ไม่ใช่ Evidence IDs จาก REA MCP หลักฐานคือข้อความในไฟล์และ baseline SHA-256 ใน [manifest](source-manifest.csv)

## Checkpoint: ตรวจสอบก่อนเขียนบันทึก

- ตรวจโครงสร้าง `D:\Simple IP v2` และต้นไม้ `source/` ก่อนเขียนเอกสาร พบเฉพาะซอร์สและ resources ไม่พบ executable ภายในต้นไม้ดังกล่าว
- ไม่พบ `AGENTS.md` ที่ root ไดรฟ์/โปรเจกต์/ภายในต้นไม้ source ที่ตรวจ
- Git ของโปรเจกต์ยังไม่มี commit; `source/` เป็น untracked และไม่มี remote แสดงในเวลาตรวจ จึงไม่มี commit ID ยืนยัน provenance ของ upstream
- เก็บ hash ก่อนเขียนเอกสารของไฟล์ต้นฉบับครบ 44 ไฟล์ แล้วตรวจซ้ำหลังวิเคราะห์: ยังมี 44 ไฟล์ ไม่มีไฟล์ถูกแก้หรือลบ และ hash ตรงกันทั้งหมด
- เอกสารใหม่อยู่ใน `docs/original-analysis` ของโปรเจกต์ และสำเนาส่งมอบใน outputs ไม่เพิ่มไฟล์เข้า `source/` ไม่แก้ `.gitignore` และไม่ commit

## Finding ledger

| ID | ข้อค้นพบ | หลักฐานต้นทาง | ประเภท/ความแน่นอน | ขอบเขตและสิ่งที่ยังขาด |
|---|---|---|---|---|
| SRC-01 | เป็น AutoIt source ระบุรุ่น 2.9.8-b1 | [main.au3:112](<D:/Simple IP v2/source/main.au3:112>), manifest | พบในไฟล์ / สูง | ไม่ยืนยันว่า executable ที่เคยใช้ตรงกับรุ่นนี้ |
| SRC-02 | ขอ admin ตั้งแต่เปิด | [main.au3:43](<D:/Simple IP v2/source/main.au3:43>) | พบ directive / สูง | ไม่ได้ทดสอบ UAC prompt |
| SRC-03 | CLI ตรวจ argument ก่อน singleton GUI | [main.au3:227](<D:/Simple IP v2/source/main.au3:227>) | พบลำดับ / สูง | ไม่ได้ทดลอง concurrent invocation |
| SRC-04 | หน้าหลักสองคอลัมน์และขนาดเริ่ม 600×675 | [main.au3:115](<D:/Simple IP v2/source/main.au3:115>), [form:267](<D:/Simple IP v2/source/forms/_form_main.au3:267>) | พบ geometry / สูง | ไม่มี rendered screenshot ของ runtime |
| SRC-05 | เลือกโปรไฟล์โหลดฟอร์ม ส่วน Apply อ่านฟอร์ม | [events.au3:217](<D:/Simple IP v2/source/events.au3:217>), [functions.au3:572](<D:/Simple IP v2/source/functions.au3:572>) | พบเส้นทาง / สูง | notification และ focus ของ Windows ยังไม่ทดสอบ |
| SRC-06 | Save เขียน INI แยกจาก Apply | [functions.au3:1019](<D:/Simple IP v2/source/functions.au3:1019>) | พบเส้นทาง / สูง | ไม่มี runtime write-failure evidence |
| SRC-07 | Pull อ่าน labels แล้วบังคับ manual | [functions.au3:800](<D:/Simple IP v2/source/functions.au3:800>) | พบเส้นทาง / สูง | ความสดของ labels ขึ้นกับการ refresh ก่อนหน้า |
| SRC-08 | Enter และ double-click เรียก Apply ได้ | [events.au3:399](<D:/Simple IP v2/source/events.au3:399>), [functions.au3:400](<D:/Simple IP v2/source/functions.au3:400>) | พบ handler / สูง | ผลเมื่อ focus control ต่างชนิดยังไม่ยืนยัน |
| SRC-09 | static IP บังคับ manual DNS ใน UI | [functions.au3:537](<D:/Simple IP v2/source/functions.au3:537>) | พบเงื่อนไข / สูง | ไม่ใช่ข้อจำกัดทั่วไปของ Windows API |
| SRC-10 | Profile ใช้ชื่อ lookup และเก็บ IP/DNS/adapter/memo | [data/profiles.au3:43](<D:/Simple IP v2/source/data/profiles.au3:43>) | พบ schema / สูง | ไม่ได้อ่าน INI จริงของผู้ใช้ |
| SRC-11 | Options อยู่ใน INI เดียวกัน มี 14 keys | [data/options.au3:18](<D:/Simple IP v2/source/data/options.au3:18>) | พบ schema / สูง | ไม่ทราบ installer defaults |
| SRC-12 | portable INI ข้างโปรแกรมมีลำดับสูงสุด | [main.au3:412](<D:/Simple IP v2/source/main.au3:412>) | พบเงื่อนไข / สูง | ไม่ได้อ่าน installation registry จริง |
| SRC-13 | Open เปลี่ยนไฟล์, Import รวม, Export copy ทั้ง INI | [main.au3:351](<D:/Simple IP v2/source/main.au3:351>), [functions.au3:1266](<D:/Simple IP v2/source/functions.au3:1266>) | พบเส้นทาง / สูง | duplicate replace ใน memory ต้องตรวจ semantics เพิ่ม |
| SRC-14 | Apply enqueue IPv4 ก่อน DNS | [functions.au3:596](<D:/Simple IP v2/source/functions.au3:596>) | พบลำดับ / สูง | ไม่ได้ตรวจผล OS; ไม่มี transaction ใน flow นี้ |
| SRC-15 | Release/Renew/Cycle ใช้ ipconfig ตามชื่อ adapter | [network.au3:46](<D:/Simple IP v2/source/network.au3:46>) | พบคำสั่ง / สูง | ไม่ได้รันคำสั่งจริง |
| SRC-16 | queue default timeout 10s, poll 100ms, marker แทน exit code | [libraries/asyncRun.au3:33](<D:/Simple IP v2/source/libraries/asyncRun.au3:33>), [libraries/asyncRun.au3:58](<D:/Simple IP v2/source/libraries/asyncRun.au3:58>) | พบ implementation / สูง | child process cleanup/countdown ยังไม่ทดสอบ |
| SRC-17 | GUI ตรวจ failed/exists และ refresh; ไม่เทียบ target กับ observed state | [functions.au3:47](<D:/Simple IP v2/source/functions.au3:47>) | พบเงื่อนไข / สูง | ผล output หลายภาษายังเป็นความเสี่ยง |
| SRC-18 | alternate-DNS-only branch ใช้ preferred ที่ว่าง | [functions.au3:668](<D:/Simple IP v2/source/functions.au3:668>) | พบ branch / สูง; ผลกระทบอนุมาน | ยังไม่ reproduce กับ netsh |
| SRC-19 | current IP อ่าน API/registry; Unplugged อิง IP ว่าง | [network.au3:72](<D:/Simple IP v2/source/network.au3:72>), [network.au3:141](<D:/Simple IP v2/source/network.au3:141>) | พบเงื่อนไข / สูง | ไม่ยืนยัน physical cable state |
| SRC-20 | library มี lease fields แต่ flow UI ไม่แสดงอายุ lease | [library:928](<D:/Simple IP v2/source/libraries/_NetworkStatistics.au3:928>), [functions.au3:1364](<D:/Simple IP v2/source/functions.au3:1364>) | พบ data/consumer / สูง | ไม่ได้อ่าน lease ของระบบจริง |
| SRC-21 | CLI ตรวจ failed แต่ไม่ตรวจ return ของ _apply | [cli.au3:50](<D:/Simple IP v2/source/cli.au3:50>), [cli.au3:76](<D:/Simple IP v2/source/cli.au3:76>) | พบ control flow / สูง | false success เป็นความเสี่ยงที่ต้องทดสอบ |
| SRC-22 | Close อาจส่ง tray; menu Exit ออกจริง | [events.au3:33](<D:/Simple IP v2/source/events.au3:33>) | พบ handler / สูง | minimize behavior ยังต้องทดสอบ |
| SRC-23 | Screenshot flag เปิดเป็น 1 ในซอร์ส | [main.au3:103](<D:/Simple IP v2/source/main.au3:103>) | พบค่าคงที่ / สูง | placeholder ใน binary จริงยังไม่ยืนยัน |
| SRC-24 | update UI เปิด release page; ไม่พบ caller ของ _DoUpdate | [events.au3:590](<D:/Simple IP v2/source/events.au3:590>), [functions.au3:246](<D:/Simple IP v2/source/functions.au3:246>) | พบ call path/ผลค้นจำกัด / สูง | ไม่ได้เรียก API หรือตรวจ behavior ของ release |

## Negative searches และขอบเขต

1. Inventory recursive ใต้ `source/` มีไฟล์ตาม manifest และไม่มี `.exe`/`.dll`/archive package ข้อความชื่อ `.exe` ใน wrapper ไม่ถือเป็นไฟล์ที่มีจริง ไม่ได้ค้น binary ที่อื่นบนเครื่อง
2. ค้น `_DoUpdate` ทั้งต้นไม้ `source/` พบ definition และ end marker ไม่พบ call site ของชื่อนี้ การเรียกทาง reflection/string ภายนอก scope ไม่ได้ยืนยัน
3. ค้น `169\.254`, `APIPA`, DHCP-service patterns, `DnsQuery`, `Resolve-Dns`, `GetIpForwardTable`, `GetBestRoute`, `ping(` ใน main/functions/events/network/cli/asyncRun ไม่พบ match ประกอบกับอ่าน flow `_getIPs`, Apply และ callback จึงสรุปได้เพียงว่าไม่พบ diagnostics เหล่านี้ในเส้นทางแอปที่ตรวจ ไม่ใช่หลักฐานว่าไม่มี Windows API ภายใน library อื่นหรือไม่มีพฤติกรรมภายนอกทุกชนิด
4. ไม่พบ `profiles.ini` ใน inventory ของ source; ไม่ค้น AppData เพื่ออ่านข้อมูลส่วนตัวหรือ profile ที่อาจมีอยู่จริง
5. Tool lookup ไม่พบ AutoIt3/Aut2exe ใน session PATH แต่ไม่ได้ยืนยันว่าเครื่องไม่มี AutoIt ติดตั้งที่อื่น

## Checklist ปิดรอบการวิเคราะห์

| คำถามเดิม | สถานะรอบนี้ | ตำแหน่งคำตอบ/งานที่ยังเหลือ |
|---|---|---|
| source หรือ binary | ตอบแล้ว | manifest และเอกสาร 01 |
| เปิดโปรแกรมและประกอบส่วนต่าง ๆ อย่างไร | ตอบแล้วจากซอร์ส | เอกสาร 01 |
| หน้าเดียวจัด workflow อย่างไร | ตอบแล้วจากซอร์ส | เอกสาร 02 |
| Save/Apply/Pull/Refresh ต่างกันอย่างไร | ตอบแล้วจากซอร์ส | เอกสาร 02–03 |
| โปรไฟล์/schema/path/import/export/CLI | ตอบแล้วจากซอร์ส; object replacement บางจุดยังไม่ยืนยัน | เอกสาร 03 |
| เปลี่ยน IP/DNS/DHCP และแปล error อย่างไร | ตอบแล้วจากซอร์ส | เอกสาร 04 |
| DHCP แยกสาเหตุได้แค่ไหน | ตอบได้ใน flow ที่อ่าน | เอกสาร 04; ไม่มีการทดลอง lease จริง |
| ผลจริงบน Windows ภาษาไทย/VPN/DPI/ข้อผิดพลาด | ยังไม่ยืนยัน | ต้องมี build/runtime evidence แยก |
| ซอร์สตรงกับโปรแกรมเดิมที่ผู้ใช้ใช้อยู่หรือไม่ | ยังไม่ยืนยัน | ต้องระบุ executable ตัวจริงและเปรียบเทียบ |
| ข้อกำหนดและ checkpoints ของ Rust v2 | จดข้อเสนอแล้ว ยังไม่ implement | เอกสาร 05 |
| ต้นฉบับยังเหมือนก่อนตรวจหรือไม่ | ยืนยันด้วย hash | 44/44 files ตรง baseline |

สถานะรวม: **จบ checkpoint ทำความเข้าใจจากซอร์ส** โดยเก็บ unknowns ไว้ชัดเจน ยังไม่ถือว่าจบการยืนยันพฤติกรรม runtime หรือพร้อมเริ่มเปลี่ยนเครือข่ายจริง
