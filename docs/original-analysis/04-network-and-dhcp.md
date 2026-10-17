# บันทึกการทำงานเดิม: เครือข่ายและ DHCP

วันที่ตรวจ: 17 ตุลาคม 2026 · เป้าหมาย: `D:\Simple IP v2\source`

## ขอบเขตและวิธีตรวจ

อ่าน AutoIt source แบบ static เท่านั้น ไม่เรียกโปรแกรม ไม่เรียก `netsh`/`ipconfig` จริง และไม่เปลี่ยนไฟล์ต้นฉบับหรือค่าระบบ หลักฐานด้านล่างเป็นไฟล์พร้อมเลขบรรทัด ไม่ใช่ REA Evidence ID เพราะงานนี้ตอบได้จากซอร์สที่มีอยู่ การกล่าวว่า “พบ” หมายถึงพบในซอร์ส ไม่ได้หมายถึงยืนยันพฤติกรรมตอนรัน

อ่านเต็ม `network.au3`, `libraries/asyncRun.au3`; อ่าน entry point และส่วนเกี่ยวข้องของ `functions.au3`, `events.au3`, `cli.au3`, `main.au3`, `libraries/_NetworkStatistics.au3` และตรวจข้อความ timeout ใน `languages.au3` ไม่ได้ทดสอบกับ Windows หลายภาษา/VPN/อะแดปเตอร์จริง

## เส้นทางจากผู้ใช้ไป Windows

| การกระทำ | เส้นทางที่พบ | ผลที่ source ตั้งใจให้เกิด |
|---|---|---|
| Apply | `functions.au3:572` → `_apply` ที่ `functions.au3:596` → `asyncRun` | ตั้ง IPv4 ก่อน แล้วตั้ง DNS ต่อในคิวเดียวกัน |
| Release DHCP | `events.au3:476` → `network.au3:46` | สั่ง `ipconfig /release` กับชื่ออะแดปเตอร์ที่เลือก |
| Renew DHCP | `events.au3:485` → `network.au3:54` | สั่ง `ipconfig /renew` กับชื่ออะแดปเตอร์ที่เลือก |
| Release/renew cycle | `events.au3:494` → `network.au3:62` | enqueue release ก่อน renew จึงทำตามลำดับ ไม่รันพร้อมกัน |
| อ่าน current properties | `functions.au3:1364` → `network.au3:72` | อ่าน API และ registry แล้วเขียนค่าลง label |
| Pull from adapter | `events.au3:458` → `functions.au3:800` | คัดค่าจาก label ของ current properties ลงช่องแก้ไขและเลือก manual IP/DNS |
| เปิด/ปิดอะแดปเตอร์ | `functions.au3:837` → `network.au3:263` | ใช้ Shell verbs ของ Network Connections |
| ใช้โปรไฟล์จาก CLI | `cli.au3:29` → `cli.au3:50` | ใช้ `_apply` เดียวกับ GUI แต่ callback/รายงานผลต่างกัน |

โปรแกรมประกาศ `#RequireAdmin` ที่ `main.au3:43` จึงร้องขอสิทธิ์ทั้งแอป ไม่ได้แยกเฉพาะการเขียนค่า

## Apply: ค่าที่อ่าน การตรวจ และลำดับคำสั่ง

### ค่าจากหน้าเดียว

`_apply_GUI` อ่านโหมด IP, IPv4, subnet, gateway, โหมด DNS, preferred/alternate DNS, register DNS และชื่ออะแดปเตอร์ แล้วส่งเข้า `_apply` (`functions.au3:572–583`) ค่าจาก IP control ที่เป็น `0.0.0.0` ถูกเปลี่ยนเป็นสตริงว่าง (`functions.au3:1139–1145`)

การสลับ radio บังคับว่า static IP ต้องใช้ manual DNS; ถ้า IP auto ช่อง IP/subnet/gateway ถูกปิด ส่วน DNS ยังเลือก auto/manual ได้ (`functions.au3:537–568`) นี่เป็นกติกา UI ของเดิม ไม่ใช่ข้อสรุปว่าทุก API ของ Windows มีข้อจำกัดเช่นนี้

### การตรวจล่วงหน้าที่พบ

- ถ้า adapter ว่าง: แสดง select adapter แล้วคืน `1` (`functions.au3:597–600`)
- ถ้า static IP แต่ IP หรือ subnet ว่าง: แสดง error แล้วคืน `1` (`functions.au3:610–615`)
- ไม่พบใน `_apply` การตรวจ subnet mask ต่อเนื่อง, gateway อยู่ subnet เดียวกัน, IP ซ้ำ, GUID ยังตรงอะแดปเตอร์เดิม หรือการสำรองค่าเพื่อ rollback ข้อสรุปนี้จำกัดที่ฟังก์ชัน `functions.au3:596–686`; ตัว IP input เป็น IPv4 control จึงไม่ควรตีความว่ารับทุกสตริงแบบอิสระ
- ไม่มี explicit success return ในท้าย `_apply`; comment ที่ระบุ `0 = success` จึงต้องอ่านร่วมกับข้อเท็จจริงว่าคำสั่งยังทำงานภายหลังผ่าน async queue (`functions.au3:592–595`, `functions.au3:627`, `functions.au3:686`)

### ลำดับที่ enqueue

1. IPv4: `netsh interface ip set address "<adapter>" dhcp` หรือ static IP/subnet พร้อม gateway metric `1`; ถ้า gateway ว่างใช้ `none` (`functions.au3:602–627`)
2. DNS auto: `netsh interface ip set dns "<adapter>" dhcp` (`functions.au3:638–644`)
3. DNS manual มี preferred: สั่ง `set dns ... static <preferred>`; register DNS เป็น `both` หรือ `none`; OS version ≥ 6 เพิ่ม `no` ท้ายคำสั่ง (`functions.au3:646–658`)
4. ถ้ามี alternate ด้วย: สั่ง `add dns ... <alternate> 2 no` บน OS version ≥ 6 (`functions.au3:659–666`)
5. ถ้าช่อง DNS ว่างทั้งคู่: `delete dns "<adapter>" all` (`functions.au3:676–682`)

**ข้อสังเกตจากซอร์ส:** branch “preferred ว่าง แต่ alternate มีค่า” ใช้ `$dnsp` ซึ่งยังว่างในการสร้างคำสั่ง แทน `$dnsa` (`functions.au3:668–675`) ยืนยันได้ว่าตัวแปรผิดจาก flow แต่ผลตอบกลับของ Windows ยังไม่ได้รันพิสูจน์ นอกจากนี้ branch OS รุ่นเก่าใช้ literal `"$cmdReg"` (`functions.au3:655`, `functions.au3:672`); มีนัยต่อการสร้างคำสั่งรุ่นเก่า ยังไม่ได้ทดสอบ

IP กับ DNS ถูก enqueue โดยไม่รอผล IP จึงไม่ใช่ transaction: การตั้ง IP ล้มเหลวไม่ได้ทำให้ DNS ถูกยกเลิกโดยอัตโนมัติจากการตรวจคำว่า `failed` (`functions.au3:59–64`, `functions.au3:627–682`)

## async queue และการตัดสินผล

### สิ่งที่พบโดยตรง

- `asyncRun` เพิ่ม command/callback/description/timeout ลง array; timeout default `10000` ms; ใช้ `AdlibRegister` ทุก `100` ms (`libraries/asyncRun.au3:33–45`)
- เมื่อเริ่มคิวเรียก callback ด้วย description เดิมทั้ง current/next เพื่ออัปเดตสถานะก่อนรัน (`libraries/asyncRun.au3:40–44`)
- รันผ่าน command interpreter แบบซ่อนหน้าต่าง โดยต่อ `& echo __asyncRun cmd done` และใช้ `/k` เพื่อให้ process ยังอยู่หลังคำสั่งจบ (`libraries/asyncRun.au3:98–102`)
- อ่าน output สะสม แล้วใช้ข้อความ marker เป็นสัญญาณจบ ไม่ได้อ่าน exit code ของ `netsh`/`ipconfig` (`libraries/asyncRun.au3:65–73`)
- เมื่อพบ marker หรือครบ timeout: ปิด process ที่ PID ถูกเก็บไว้, callback ด้วย output, ลบคำสั่งแถวแรก แล้วไปคำสั่งถัดไป (`libraries/asyncRun.au3:81–100`) ซอร์สนี้ไม่ยืนยันว่าการปิด command interpreter ยุติ child ทุกตัวหรือย้อนค่าที่แก้แล้ว
- timeout สร้างข้อความ literal `Command timeout` (`libraries/asyncRun.au3:72`) แต่ GUI เปรียบเทียบกับข้อความจาก language object (`functions.au3:48`); ค่าอังกฤษ default ตรงกัน (`languages.au3:639`), ภาษาอื่นต้องตรวจไฟล์แปล/ทดสอบเพิ่ม
- callback GUI จัดเป็น error เมื่อ output มี `failed` หรือ `exists` เป็นภาษาอังกฤษ; branch `exists` ล้างคิว แต่ `failed`/timeout ไม่ล้างคิวโดยตรง (`functions.au3:48–69`)
- เมื่อคิวว่างและไม่เข้าข้อผิดพลาดที่รู้จัก callback แสดง Ready; ไม่ได้เปรียบเทียบค่าที่ตั้งใจตั้งกับค่าจริง (`functions.au3:71–89`)
- ทุก callback GUI เรียก `_updateCurrent` ซึ่งอ่านอะแดปเตอร์ที่ UI เลือกขณะนั้น (`functions.au3:89`, `functions.au3:1366–1368`) ไม่ได้ผูก immutable target ID ของ operation ที่เพิ่งจบ
- CLI callback ไม่ทำอะไร; CLI loop ตรวจเพียง substring `failed` ตอนคิวว่าง มิฉะนั้นแสดง applied successfully (`cli.au3:70–95`) จึงไม่มี branch แยก timeout ชัดเจนใน flow นี้

### ความเสี่ยงที่อนุมานได้ ต้องทดสอบก่อนเรียกว่า bug ที่เกิดจริง

- output ของ Windows ที่ไม่ใช้คำอังกฤษ `failed`/`exists` อาจถูกนับเป็นเสร็จปกติ เพราะไม่อ่าน exit code และไม่ตรวจ postcondition
- `$sStdOut` ถูกล้างเฉพาะเมื่อเริ่มคิว (`libraries/asyncRun.au3:41`) ไม่ได้ล้างทุก command; output จึงอาจสะสมข้าม step (`libraries/asyncRun.au3:65`, `libraries/asyncRun.au3:69`, `libraries/asyncRun.au3:95–102`) และ error เก่ามีผลต่อ callback ขั้นต่อไป
- ถ้า process หายไปโดยไม่มี marker โค้ด `ElseIf ... > 1` เริ่ม command แถวเดิมอีกครั้ง แทน dequeue/report failure (`libraries/asyncRun.au3:61–62`, `libraries/asyncRun.au3:98–102`) ต้องจำลองแยกก่อนยืนยันการ retry ที่ผู้ใช้เห็น
- countdown ใช้ `$pRuntime` แทน runtime timer ใน array และ hard-code `10` (`libraries/asyncRun.au3:75` เทียบ `libraries/asyncRun.au3:101`); timeout จริงใช้ timer ใน array ถูกจุด (`libraries/asyncRun.au3:71`) จึงต้องแยกความน่าเชื่อถือของ countdown จาก timeout
- กรณี callback ล้างคิว `exists` แล้ว runner ยัง `_ArrayDelete(...,1)` ต่อ (`functions.au3:65–69`, `libraries/asyncRun.au3:94–95`) มีความเสี่ยงด้าน state; ยังไม่ได้ทดสอบ runtime ของ AutoIt

## รายชื่ออะแดปเตอร์และการอ่านค่าจริง

### แหล่งข้อมูลและการเชื่อมข้อมูล

1. `_GetAdapters` ใช้ `Shell.Application.Namespace(49)` และอ่าน `.Items[].name` ของ Network Connections (`network.au3:399–426`)
2. `_loadAdapters` อ่าน `_Network_IPAllAddressTable(0,0,1)` เพื่อจับคู่ชื่อกับ MAC/description แล้วสร้างรายการ adapter ของแอป (`network.au3:24–42`)
3. `_getIPs` เรียก `_Network_IPAllAddressTable(1,0,1)` แบบ IPv4 และ `_Network_IPv4AdaptersInfoEx()`; หา friendly name ให้ตรงชื่อที่เลือก แล้วจับคู่สองตารางด้วย MAC address (`network.au3:77–83`, `network.au3:128–136`)
4. Library แรกเรียก `GetAdaptersAddresses` ผ่าน `iphlpapi` และเก็บ friendly name/GUID/operational status/IPv4/IPv6/DNS (`libraries/_NetworkStatistics.au3:584–605`, `libraries/_NetworkStatistics.au3:643–697`)
5. Library ที่สองเรียก `GetAdaptersInfo`, เก็บ DHCP enabled flag, IP, mask, gateway, DHCP server และ lease obtained/expires (`libraries/_NetworkStatistics.au3:879–912`, `libraries/_NetworkStatistics.au3:928–968`)

**ขอบเขต:** library มีข้อมูลมากกว่าที่ UI นำมาใช้; อย่าสรุปว่ามี DHCP diagnostics เพียงเพราะ struct มี lease timestamps

### กรณี disabled และไม่มี IP

- ถ้า `_AdapterMod(...,2)` บอก disabled จะ fallback registry (`network.au3:85–126`)
- ถ้าพบ adapter ในตาราง แต่ IP ว่าง จะกำหนดสถานะ Unplugged แล้ว fallback registry (`network.au3:141–162`) ดังนั้นคำว่า Unplugged เป็นการอนุมานจาก IP ว่างใน flow นี้ ไม่ใช่ผลตรวจ physical cable โดยตรง
- แปลงชื่ออะแดปเตอร์เป็น GUID โดย enumerate `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\<GUID>\Connection\Name` (`network.au3:215–230`)
- อ่านค่าจาก `...\Services\Tcpip\Parameters\Interfaces\<GUID>` (`network.au3:193–210`): ถ้า `EnableDHCP=0` หรือ `DisableDhcpOnConnect=1` ใช้ static values; มิฉะนั้นใช้ DhcpIPAddress/DhcpSubnetMask/DhcpNameServer (`network.au3:88–106`, `network.au3:143–161`)
- ใน DHCP registry fallback ยังอ่าน gateway จาก `DefaultGateway` ไม่ใช่ชื่อ `DhcpDefaultGateway` (`network.au3:100`, `network.au3:155`); ไม่ได้ยืนยันค่าที่แต่ละ Windows version เก็บจริง
- DNS ถูกแยกแล้วส่งออกมากสุดสองรายการ (`network.au3:109–118`, `network.au3:167–176`)
- `props[5]` ที่ส่งเข้า Current DHCP label เป็น DHCP server address (`network.au3:136`, `network.au3:184`, `functions.au3:1395`); disabled branch ไม่มีการกำหนด `$dhcpServer` ก่อนใส่ `props[5]` (`network.au3:75`, `network.au3:126`)
- API library ข้าม software loopback และรับเฉพาะ interface ที่มี FirstUnicastAddress (`libraries/_NetworkStatistics.au3:633–641`) แม้ caller ขอ down interfaces ด้วย จึงยังมีช่องว่างเมื่อ adapter ใหม่ไม่มี unicast เลย
- `_getIPs` ไม่ตรวจ `@error` ก่อนใช้อาร์เรย์จาก API และ loop ด้วย `@extended` (`network.au3:77–81`, `network.au3:128–136`); ต้องทดสอบกรณี API คืน error/ไม่มี adapter

### การอ่าน/เปลี่ยน enable state

`_AdapterMod` หา Shell verbs enable/disable ซึ่งกำหนดชื่อเฉพาะ English/French (`network.au3:267–277`), รอการเปลี่ยนสถานะไม่เกินประมาณ 10 วินาที (`network.au3:346–366`) helper `_GetNicState` คืนค่าใน iteration แรกทันที: verb แรกตรง Enable → disabled; มิฉะนั้น → enabled (`network.au3:371–379`) จึงอนุมานได้ว่าลำดับ verbs/ภาษาของ Windows มีผลต่อความแม่นยำ ยังไม่รันยืนยันกับเครื่องจริง

`_disable` เปลี่ยน menu label หลังเรียกโดยไม่ตรวจ return code (`functions.au3:837–848`) จึงไม่มีหลักฐานว่าหน้าจอรับรองการเปลี่ยนสำเร็จจริง

## DHCP diagnostics ที่ของเดิมทำได้และที่ยังไม่มีหลักฐาน

**ยืนยันจาก source:** สั่ง release, renew และ cycle กับชื่อ adapter; แสดงข้อความคืบหน้า; timeout แต่ละคำสั่ง; อ่าน IP/subnet/gateway/DNS/DHCP server และ enabled/disabled/unplugged แบบ heuristic กลับมาใน current panel

**ไม่พบใน flow ที่ตรวจ:** การวิเคราะห์ APIPA `169.254.x.x`, แสดงอายุ/หมดอายุ lease, ตรวจ DHCP service, แยก DHCP disabled กับไม่มี lease, ตรวจ default route จริงจาก route table, ทดสอบ DNS query, ทดสอบ gateway/อินเทอร์เน็ต, ตรวจ DHCP packet/server response หรือรายงาน structured error code. การค้นจำกัด `main.au3`, `events.au3`, `functions.au3`, `network.au3`, `cli.au3`, `libraries/asyncRun.au3` และอ่านส่วนอ่าน adapter ของ `_NetworkStatistics.au3`; ไม่ใช่คำรับรองเชิง runtime หรือการค้นทุก resource ภายนอก

ไม่ควรสรุปจาก source นี้ว่า “renew สำเร็จเท่ากับได้ lease”, “ไม่มี IP เท่ากับสายหลุด” หรือ “คิวหมดเท่ากับ Apply ตรงค่าที่ขอ”

## Checkpoint สำหรับการเขียนใหม่ (ข้อเสนอ ไม่ใช่พฤติกรรมเดิม)

- [x] ระบุ API/registry/Shell และ command order ที่มีอยู่ พร้อมจุดอ้างอิง
- [x] ระบุการแยกค่ากำลังแก้กับ current panel และ Pull from adapter
- [x] ระบุ DHCP actions, queue, timeout, error recognition และข้อจำกัด
- [ ] runtime validation แยกต่างหาก: ทดสอบ read-only state ก่อน แล้วใช้ adapter ทดลองเมื่ออนุญาตให้เปลี่ยนค่า
- [ ] ตั้ง identity ด้วย GUID/LUID และเก็บ target เดิมของ operation; เปรียบเทียบค่าหลัง apply แยกแต่ละ step
- [ ] ออกแบบ state ที่แยก admin disabled, media disconnected, no unicast, DHCP enabled, lease absent/expired, APIPA, route absent, DNS failure และ unknown
- [ ] ผลลัพธ์แต่ละ operation เก็บ exit/API code, stdout/stderr, timeout, observed state และข้อสรุปที่หลักฐานรองรับ; ถ้าขั้นแรก fail ต้องกำหนดนโยบายขั้นถัดไปชัดเจน
- [ ] ตรวจ input, เก็บ snapshot ก่อนเปลี่ยน, รายงาน partial success, รองรับคืนค่าที่ทำได้ และไม่เปลี่ยน IPv6 จากงาน IPv4
- [ ] regression scenarios: DNS alternate-only, ชื่อภาษาไทย, output ของ Windows หลายภาษา, DHCP ตอบช้า, adapter เปลี่ยนชื่อ/หาย, หลาย NIC/VPN ที่ MAC ซ้ำหรือว่าง, API อ่านไม่สำเร็จ และเปลี่ยน target selection ขณะคิวทำงาน

## ตัวตนไฟล์หลักที่อ่าน (SHA-256)

| ไฟล์ภายใต้ source | SHA-256 |
|---|---|
| `network.au3` | `229DDD9269DAD2B779D2D70494BF8CCD6458BCD93029F025DB86DF796AEFB090` |
| `functions.au3` | `2154779E58150A43BDCF368918D92EC3A679171555F2E6045AB774C56B8D0CD9` |
| `libraries/asyncRun.au3` | `AF655D806D69F5F6920C4A145FE9A32E0FC0A91808D642430939D5422DD367F7` |
| `libraries/_NetworkStatistics.au3` | `9912ECD7FFD3F12DFFF82E76E842C2DA6931E60460E453425A6C726B3FA5DE8F` |
