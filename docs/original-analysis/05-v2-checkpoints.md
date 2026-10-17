# ข้อกำหนดที่ถอดจากพฤติกรรมเดิม และ checkpoints สำหรับ v2

เอกสารนี้เป็นข้อเสนอสำหรับงานถัดไป ไม่ใช่คำอ้างว่าความสามารถเหล่านี้มีในของเดิมหรือได้รับการ implement แล้ว อ่านหลักฐานเดิมจากเอกสาร 01–04 ก่อน

## สิ่งที่ควรรักษา

1. งานหลักอยู่หน้าเดียว เลือกอะแดปเตอร์ เห็นค่าจริง เลือกโปรไฟล์ และแก้ค่าที่ต้องการได้พร้อมกัน
2. เก็บ draft ของฟอร์มแยกจาก saved profile และค่าจริงของระบบ Save ไม่เท่ากับ Apply; Refresh และ Pull ต้องอธิบายว่ากระทบส่วนไหน
3. IP อัตโนมัติและ DNS อัตโนมัติเลือกแยกกันได้ รวม static IP ที่ไม่มี gateway และ custom DNS
4. โปรไฟล์มีชื่อ ลำดับ ค้นหา และโน้ต มี import/export พร้อม preview ผลการนำเข้า
5. ผู้ใช้เห็น adapter name, description, MAC และสถานะ ชื่อช่วยอ่านแต่ไม่ควรเป็น identity ถาวรเพียงอย่างเดียว
6. Release และ Renew เป็นคนละคำสั่ง พร้อมสถานะระหว่างทำงาน ปุ่มไม่ควรทำให้ผู้ใช้สั่งงานซ้อนโดยไม่รู้ตัว

## สิ่งที่ต้องตัดสินใหม่อย่างตั้งใจ

| ประเด็นเดิม | สิ่งที่เสนอให้ v2 ทำ | เหตุผล/หลักฐานเดิม |
|---|---|---|
| Double-click/Enter ไป Apply | กำหนด focus และแสดงผลของ shortcut ให้ชัด; ตัดสินเรื่อง double-click ก่อนเขียน UI | เส้นทาง event ในเอกสาร 02 |
| ปิดหน้าต่างอาจลง tray | ใช้คำและตัวเลือกที่แยก Close/Minimize ชัดเจน | `events.au3:33`, เอกสาร 02 |
| ใช้ชื่อ adapter ในโปรไฟล์และ command | ใช้ Windows adapter GUID ภายใน และให้ผู้ใช้แก้ binding ได้เมื่อย้ายเครื่อง | เอกสาร 03–04 |
| Export คัดลอกทั้ง INI รวม Options | แยก export profiles จาก application settings และ preview ข้อมูล | `main.au3:373`–`381` |
| จับ error จากข้อความภาษาอังกฤษ | เก็บ structured result, error code, stage และผลอ่านกลับ | `functions.au3:47`, `libraries/asyncRun.au3:58` |
| หลัง Apply แค่อัปเดต current labels | เทียบค่าที่ต้องการกับระบบจริงต่อ stage และรายงาน partial success | `functions.au3:89`, `functions.au3:596` |
| รันเป็น admin ทั้งโปรแกรม | ทดลองแยกส่วนอ่านกับงานเขียนที่ขอ elevation | `main.au3:43` |

## Checkpoint 0 — หลักฐานเดิม

**สถานะ:** วิเคราะห์ซอร์สและจัดเอกสารแล้ว; การยืนยัน runtime ยังเปิดอยู่

เกณฑ์ผ่านก่อนเริ่ม implementation: มี file manifest, flow UI/profile/network, data fields, ข้อจำกัด DHCP และรายการ unknowns ซอร์สเดิมต้องไม่เปลี่ยน รอบนี้ทำเฉพาะ checkpoint นี้

## Checkpoint 1 — ยืนยัน contract และหน้าจอด้วยข้อมูลจำลอง

ทำ spec ของ draft/saved/applied state; ความหมาย Apply/Save/Pull/Refresh; ปุ่ม DHCP/renew; กำหนด UX เมื่อเปลี่ยน adapter ขณะมี draft; กำหนด shortcut focus และ dirty-state behavior ใช้ assets และข้อความใหม่

เกณฑ์ผ่าน: หน้าต้นแบบหนึ่งหน้าเห็นค่าเดิมกับค่าที่ต้องการชัดเจน งานหลักใช้คีย์บอร์ดได้ และไม่มีการเขียนค่าเครือข่าย ทดลองชุด UI Rust ก่อนเลือกโดยวัด DPI, responsiveness และการจัดแพ็กเกจบนเครื่องนี้

## Checkpoint 2 — อ่านระบบจริงอย่างเดียว

ทำส่วนอ่าน adapters, address/prefix, routes, DNS, DHCP mode, lease/server/timestamps เท่าที่ Windows ให้ได้ แยกข้อมูลที่ไม่มี/อ่านไม่ได้ออกจากค่าศูนย์ แสดงแหล่งข้อมูลและเวลาที่อ่าน ใช้ adapter GUID ภายใน

เกณฑ์ผ่าน: เทียบผลกับ Windows บนอะแดปเตอร์หลายแบบและกรณีออฟไลน์ ไม่มีคำสั่งเขียน ไม่มีการสรุปว่า DHCP เสียจาก ping ไม่ตอบ

## Checkpoint 3 — โปรไฟล์และ import โดยยังไม่ Apply

ออกแบบ schema ใหม่ที่มี version และ atomic save; ถ้าต้องรองรับ legacy INI ให้เขียน importer จาก data contract ที่จดไว้ ทดสอบ duplicate names, Options case, bracket/escape, Unicode, memo และ missing adapter โดยไม่คัดลอก serializer เดิม

เกณฑ์ผ่าน: export/import round trip ไม่สูญข้อมูล มี preview วิธีจัดการชื่อซ้ำ ไม่แก้ต้นฉบับนำเข้า และไม่ import application settings โดยเงียบ

## Checkpoint 4 — เขียนค่าแบบตรวจผลและคืนค่าได้

ก่อนเปลี่ยน ตรวจ adapter identity และ validation ใหม่ เก็บ snapshot พร้อมแสดง diff แล้วขอ elevation ตามการออกแบบ ใช้ operation ID ที่ผูกกับ adapter GUID ไม่อิง adapter ที่ผู้ใช้เพิ่งเลือกใหม่ใน UI

ดำเนิน IP และ DNS ตามลำดับที่ออกแบบ เก็บผลแต่ละ stage, timeout, cancellation และอ่านกลับเทียบเป้าหมาย รายงาน partial success ได้ หากต้องคืนค่าให้บอกข้อจำกัดอย่างตรงไปตรงมา ไม่รับประกันว่าจะกู้ connectivity ได้ทุกกรณี

เกณฑ์ผ่าน: ทดสอบบนอะแดปเตอร์ทดลองหรือ VM ก่อน การตรวจสิทธิ์, validation, timeout และ error mapping ผ่าน จึงเริ่มทดสอบการเปลี่ยนค่าจริงที่ระบุเป้าหมายชัดเจน

## Checkpoint 5 — DHCP diagnostics

แยกอย่างน้อย: adapter ปิด/ไม่มี link, DHCP ปิด, กำลังรอ address, มี lease, APIPA, ไม่มี route, DNS ใช้งานไม่ได้ และการตรวจปลายทางอินเทอร์เน็ต แต่ละผลมีเวลาและหลักฐานประกอบ

Renew สำเร็จในระดับคำสั่งไม่เท่ากับได้รับ lease; APIPA ไม่พิสูจน์ว่า DHCP server ล่ม; ping ไม่ตอบไม่พิสูจน์ว่าไม่มี gateway; ต้องแสดง “ยังระบุสาเหตุไม่ได้” ได้เมื่อข้อมูลไม่พอ ไม่เพิ่ม packet capture โดยปริยาย

เกณฑ์ผ่าน: กรณี DHCP ใช้งานได้/ไม่ได้, service มีปัญหา, adapter หายระหว่างรอ, DNS ผิด, route ผิด และเครื่องมี VPN ให้ผลจำแนกได้โดยไม่อ้างเกินข้อมูล

## Checkpoint 6 — ตรวจครบก่อนส่งมอบ

ตรวจ DPI, keyboard/focus, Unicode, profile path เขียนไม่ได้, หลายอะแดปเตอร์, restore settings, ปิดแอประหว่างทำงาน และการติดตั้ง/portable ตาม scope ที่ตกลง เก็บ test evidence และรายการข้อจำกัดที่ยังเหลือ

ห้ามถือว่าการอ่านซอร์สรอบนี้คือผลทดสอบ Windows runtime หรืออนุมัติให้เปลี่ยนอะแดปเตอร์จริงในทันที
