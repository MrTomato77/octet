# บันทึกการทำงานเดิม: โปรไฟล์ การเก็บข้อมูล และ CLI

วันที่ตรวจ: 17 ตุลาคม 2026 · เป้าหมาย: `D:\Simple IP v2\source` · วิธี: อ่านซอร์สอย่างเดียว

เอกสารนี้อธิบายพฤติกรรมที่ซอร์สระบุ ไม่ได้ยืนยันว่าเคยเห็นโปรแกรมทำงานจริง ไม่มีการเปิดแอป รัน AutoIt เปลี่ยนค่าระบบ หรือแก้ไฟล์ต้นฉบับ ใช้แนวทาง source-analysis ของ REA skill ซึ่งระบุให้ใช้เครื่องมืออ่าน repository ตามปกติเมื่อมีซอร์สอยู่แล้ว ไม่ต้องใช้ REA engine/doctor

รูปแบบหลักฐาน `source/functions.au3:572` หมายถึงไฟล์ที่อยู่ใต้ `D:\Simple IP v2` และเลขบรรทัดจริง ณ วันที่ตรวจ

## 1. สิ่งที่ตอบได้และขอบเขต

| คำถาม | สถานะ | ผลจากซอร์ส |
|---|---|---|
| โปรไฟล์เก็บอะไร | ตอบได้ | ค่า IPv4, โหมด IP/DNS, DNS registration, ชื่ออะแดปเตอร์, memo |
| Save ต่างจาก Apply อย่างไร | ตอบได้ | Save เขียนโปรไฟล์; Apply อ่านค่าที่แสดงใน editor แล้วส่งคำสั่งเปลี่ยน Windows |
| Portable/install เลือกไฟล์อย่างไร | ตอบได้ | ให้ไฟล์ข้างโปรแกรมมีลำดับสูงสุด แล้วตรวจ install location |
| Open/Import/Export ต่างกันอย่างไร | ตอบได้ | Open เปลี่ยนไฟล์หลัก; Import รวมและทับตามชื่อ; Export คัดลอกทั้งไฟล์หลัก |
| เรียกโปรไฟล์ผ่าน shortcut/CLI ได้อย่างไร | ตอบได้ | `/set-config "ชื่อโปรไฟล์"` ใช้ AdapterName ที่บันทึกไว้ |
| พฤติกรรมจริงเมื่อเขียนไฟล์ไม่สำเร็จ/CLI ผิดพลาด | ตอบได้บางส่วน | ระบุเส้นทางผิดพลาดในซอร์สได้ แต่ยังไม่ทดสอบ runtime |
| มีโปรไฟล์จริงของผู้ใช้ รูปแบบ encoding จริง หรือ installer defaults อะไร | ยังไม่ยืนยัน | ไม่มี `profiles.ini` ใน inventory ของ `source`; ไม่ค้น AppData/registry ของผู้ใช้เพื่ออ่านโปรไฟล์จริง |

## 2. โมเดลข้อมูล

### 2.1 ตัวจัดการโปรไฟล์

**ข้อสังเกตจากซอร์ส:** `_Profiles()` สร้าง object ที่มี `count`, `names` array และ `Profiles` linked list เป็นข้อมูลสามชุดที่ต้องสอดคล้องกัน มีเมธอด create/add/move/remove/get/set/sort และ serialization ชื่อโปรไฟล์ทำหน้าที่เป็น lookup key; ไม่เห็น ID ถาวรหรือ GUID ของโปรไฟล์ในโมเดลนี้ (`source/data/profiles.au3:18`, `:23`, `:28`, `:125`, `:173`)

### 2.2 Schema ของหนึ่งโปรไฟล์

หนึ่ง INI section ใช้ชื่อโปรไฟล์เป็นชื่อ section; `ProfileName` อยู่ใน object และไม่ได้เป็น key ภายใน section (`source/data/profiles.au3:43`, `:224`, `:240`)

| Key / property | ความหมายตามการใช้ | รูปแบบที่บันทึก |
|---|---|---|
| `ProfileName` | ชื่อและ lookup key | ชื่อ section ที่ encode แล้ว |
| `AdapterName` | ชื่อ connection ของ Windows | string ที่ผ่าน `iniNameEncode` |
| `IpAuto` | IP จาก DHCP | string `true` / `false` เมื่อบันทึกจาก UI |
| `IpAddress` | IPv4 แบบ manual | string หรือว่าง |
| `IpSubnet` | subnet mask | string หรือว่าง |
| `IpGateway` | default gateway | string หรือว่าง |
| `DnsAuto` | DNS จาก DHCP | string `true` / `false` เมื่อบันทึกจาก UI |
| `IpDnsPref` | DNS หลัก | string หรือว่าง |
| `IpDnsAlt` | DNS รอง | string หรือว่าง |
| `RegisterDns` | DNS registration | string `true` / `false` |
| `Memo` | บันทึกของโปรไฟล์ | string ที่ encode CRLF/วงเล็บ |

หลักฐาน: ประกาศ properties ที่ `source/data/profiles.au3:48`; serialization ที่ `:224` และ `:240`; จับค่าจาก UI ที่ `source/functions.au3:1033` และ `:1167`

**ข้อสังเกตจากซอร์ส:** property ที่ไม่ได้ให้ค่าเริ่มต้นจะเป็น empty string เพราะ `_AutoItObject_AddProperty(..., $vData = "")` (`source/libraries/AutoItObject.au3:1195`) โมเดลโปรไฟล์ไม่มี schema version ของตัวเอง, รายการ IP หลายค่า, DNS มากกว่าสองค่า, interface GUID, routes, metric หรือ IPv6 ใน properties ที่ตรวจ (`source/data/profiles.au3:48`)

### 2.3 Options อยู่ในไฟล์เดียวกับโปรไฟล์

section `[Options]` เก็บ 14 keys: `Version`, `MinToTray`, `StartupMode`, `Language`, `StartupAdapter`, `Theme`, `SaveAdapterToProfile`, `AdapterBlacklist`, `PositionX`, `PositionY`, `PositionW`, `PositionH`, `AutoUpdate`, `LastUpdateCheck` (`source/data/options.au3:22`, `:45`, `:64`)

- `StartupMode=true/1` หมายถึงเริ่มซ่อนใน tray; ไม่ได้เป็นคำสั่งให้ใช้โปรไฟล์อัตโนมัติ (`source/forms/_form_main.au3:762`)
- `MinToTray=true/1` ทำให้ปุ่มปิดหน้าต่างส่งไป tray; menu Exit ออกจริง (`source/events.au3:33`, `:47`)
- `StartupAdapter` จดอะแดปเตอร์ที่เลือกครั้งล่าสุดทันทีเมื่อ combo เปลี่ยน (`source/events.au3:563`)
- `SaveAdapterToProfile` ควบคุมการเปลี่ยน combo เมื่อเลือกโปรไฟล์ แต่ Save/New จด `AdapterName` เสมอแม้ option นี้ปิด (`source/functions.au3:987`, `:1033`, `:1117`)
- `AdapterBlacklist` เป็นชื่ออะแดปเตอร์คั่นด้วย `|`; รายการดังกล่าวถูกข้ามในการสร้าง combo (`source/functions.au3:283`; การบันทึกใน `source/forms/_form_blacklist.au3:82`)
- ตำแหน่ง/ขนาดหน้าต่างบันทึกเมื่อออก; `PositionW=PositionH=-1` เป็นเครื่องหมาย maximized (`source/events.au3:56`, `source/forms/_form_main.au3:764`)

## 3. เส้นทางการใช้งานหลัก

### 3.1 เลือกโปรไฟล์ → แก้ไข → Save หรือ Apply

1. เลือกรายการเรียก `_setProperties()` (`source/events.au3:217`)
2. โหลดค่าจาก object ลง radio, ช่อง IP/DNS และ memo (`source/functions.au3:1086`)
3. ถ้า `SaveAdapterToProfile` เปิด จะหาชื่ออะแดปเตอร์ในรายการและเลือกให้; หาไม่พบจะล้าง combo (`source/functions.au3:1117`)
4. **Save** อ่านค่าจาก editor ลง object เดิม แล้ว `IniWriteSection` ไปไฟล์หลัก (`source/functions.au3:1019`)
5. **Apply** อ่านค่าจาก editor และ combo โดยตรง แล้วเรียก `_apply`; เส้นทางนี้ไม่มีการบันทึกโปรไฟล์ (`source/functions.au3:572`, `source/events.au3:226`)

**ข้อสรุปจากเส้นทางโค้ด:** ผู้ใช้สามารถแก้ค่าแล้ว Apply โดยไม่ Save และ Apply ไม่จำเป็นต้องมีโปรไฟล์ที่บันทึกไว้ก่อน ในทางกลับกัน Save ไม่เปลี่ยนค่าระบบ การเลือกโปรไฟล์ใหม่เรียกเขียน editor ทับค่าปัจจุบัน; ไม่พบ dirty-state prompt ในเส้นทางเลือก/บันทึกที่ตรวจ

ปุ่ม Enter ที่ไม่ได้แก้ชื่อและไม่ได้อยู่ใน memo เรียก Apply; เมื่ออยู่ใน memo จะเป็น newline ตามปกติ (`source/events.au3:399`) คำสั่ง Pull คัดลอกค่าที่แสดงใน Current ลง editor และเลือกโหมด manual ทั้ง IP และ DNS ไม่ได้คงโหมด DHCP จากต้นทาง (`source/functions.au3:800`)

### 3.2 New / Rename / Delete / จัดลำดับ / ค้นหา

- New เพิ่มรายการชั่วคราวด้วยชื่อไม่ซ้ำแล้วเปิด inline rename; ยืนยันชื่อด้วย Enter จึงเข้าสู่ `_new()` เพื่อจับค่าปัจจุบันและเขียน INI (`source/events.au3:286`, `:797`, `source/functions.au3:858`, `:975`)
- `_new()` จับ IP/DNS/AdapterName แต่ไม่ได้จับข้อความ memo ปัจจุบัน; `_save()` จับ memo (`source/functions.au3:990`, `:1045`)
- Rename ใช้ `_iniRename()` เพื่อรักษาตำแหน่ง section แทนย้ายไปท้ายไฟล์; ชื่อชนกันแจ้งเตือน (`source/functions.au3:914`, `:1550`)
- Delete ลบ section และ object ที่เลือก แล้วเลือกแถวถัดไป/ก่อนหน้า (`source/functions.au3:939`)
- Sort A–Z/Z–A โหลดไฟล์ใหม่ เรียง names/linked list แล้วเขียนทั้งไฟล์ใหม่เป็น Options และทุกโปรไฟล์ (`source/functions.au3:304`, `source/data/profiles.au3:193`)
- Drag reorder เคลื่อน section ใน INI และปรับ linked list (`source/functions.au3:400`, `:439`, `source/data/profiles.au3:81`)
- Filter ใช้ regex แบบไม่สนตัวพิมพ์ เปลี่ยน `*` เป็น `.*`; อักขระ regex อื่นไม่ได้ escape จึงมีความหมายเป็น regex ด้วย (`source/functions.au3:1405`)

### 3.3 Refresh

Refresh ยกเลิก inline rename ที่กำลังทำ, โหลดอะแดปเตอร์/Options/โปรไฟล์ใหม่จาก disk, สร้าง list ใหม่ตามความจำเป็น และอัปเดต Current (`source/functions.au3:1061`) จึงอาจแทนที่ object ที่แก้ใน memory ด้วยค่าจากไฟล์ ส่วนผลต่อ editor หลัง refresh ขึ้นกับ event ของ selection และยังไม่ได้ทดสอบ runtime

## 4. ไฟล์อยู่ที่ใด

ก่อนสร้าง GUI และก่อนอ่าน CLI จะเรียก `_setProfilesIniLocation()` (`source/main.au3:106`, `:218`)

| เงื่อนไข | ไฟล์หลักที่เลือก |
|---|---|
| มี `profiles.ini` ข้าง executable/script | `@ScriptDir\profiles.ini` เสมอ |
| ไม่มีไฟล์ข้างโปรแกรม และไม่พบ installation | `@ScriptDir\profiles.ini` |
| พบ installation แต่กำลังรันจากโฟลเดอร์อื่น | `@ScriptDir\profiles.ini` |
| พบ installation และกำลังรันจาก install directory | `@LocalAppDataDir\Simple IP Config\profiles.ini` |

หลักฐาน `source/main.au3:412` การตรวจ installation อ่าน registry Uninstall locations และ fallback ไป DisplayIcon; ไม่ใช่ตรวจจากไฟล์ marker (`source/libraries/GetInstalledPath.au3:27`, `:32`, `:78`)

**ข้อสังเกตจากซอร์ส:** installed branch สร้างโฟลเดอร์ AppData หากไม่มี แต่ฟังก์ชันนี้ไม่ได้สร้าง `profiles.ini` เริ่มต้น; `_loadProfiles()` จะคืน error หากไฟล์หาย (`source/main.au3:444`, `source/functions.au3:1176`) ค่าของ Options ไม่มี defaults เฉพาะแอปใน constructor; จึงเริ่มเป็น empty strings หากไม่มีการอ่านค่า (`source/data/options.au3:18`, `source/libraries/AutoItObject.au3:1195`) ไม่ได้ตรวจ installer เพื่อยืนยันว่าภายนอกซอร์สชุดนี้สร้างไฟล์เริ่มต้นให้อย่างไร

## 5. Open / Import / Export เป็นคนละงาน

| คำสั่ง | ผลกับไฟล์หลักและ memory | หลักฐาน |
|---|---|---|
| Open profiles | ตั้ง `$sProfileName` เป็นไฟล์ที่ผู้ใช้เลือก สร้าง Options/Profiles ใหม่ แล้ว refresh; การ Save ต่อจากนี้เขียนไฟล์นั้น | `source/main.au3:351` |
| Import profiles | อ่านไฟล์อื่น ข้าม Options และรวมโปรไฟล์เข้าชุดปัจจุบัน; ชื่อซ้ำเข้าสู่ replace path; เขียนแต่ละ section ทันทีลงไฟล์หลัก | `source/main.au3:363`, `source/functions.au3:1266` |
| Export profiles | เติม `.ini` หากจำเป็น แล้ว `FileCopy` ไฟล์หลักทั้งหมดไปปลายทาง | `source/main.au3:373` |

**ข้อสรุปจากโค้ด:** Export รวม Options และค่าที่บันทึกอยู่บน disk ด้วย ไม่ใช่ export เฉพาะแถวที่เลือก และไม่รวม editor changes ที่ยังไม่ Save ไม่มี preview/conflict chooser ใน import path ที่ตรวจ ส่วน Open ไม่ได้บันทึก path ใหม่ไว้เป็น preference สำหรับการเริ่มโปรแกรมครั้งต่อไป; รอบหน้าเลือก path ตาม portable/install อีกครั้ง (`source/main.au3:106`, `:351`, `:412`)

## 6. การ encode ข้อความใน INI

`iniNameEncode` แทน `[` → `{lb}`, `]` → `{rb}`, CRLF → `{crlf}`; decode ทำกลับกัน (`source/functions.au3:1522`) ใช้กับชื่อ section, memo และ adapter name หลายเส้นทาง

**อนุมานจากการแทนข้อความ:** scheme นี้ไม่ escape token literal ก่อน เช่น ชื่อที่ผู้ใช้ตั้งมี `{lb}` จริงจะถูก decode เป็น `[` ทำให้ round-trip ไม่สมบูรณ์ ส่วน adapter name โหลดเข้า object ด้วย encode อีกครั้ง แทนที่จะ decode ค่าใน INI (`source/functions.au3:1251`, `:1306`) จึงมีโอกาสไม่ตรงชื่อ Windows เมื่อมีวงเล็บ; ยังไม่ได้ทดสอบ runtime

## 7. CLI และ shortcut

เริ่มต้น: สร้าง Options/Profiles/Adapters → init language → `CheckCmdLine()` → ถ้าไม่มี CLI จึงทำ singleton check และ GUI (`source/main.au3:218`) ดังนั้น CLI อยู่ก่อน GUI singleton gate

- รองรับ `/set-config "ชื่อโปรไฟล์"` เมื่อมี arguments สองรายการเท่านั้น (`source/cli.au3:25`)
- โหลด INI จาก path ที่ startup เลือก แล้วหาชื่อโปรไฟล์ (`source/cli.au3:32`)
- ดึงค่าจาก profile object รวม `AdapterName` ส่ง `_apply` โดยตรง ไม่อ่าน UI และไม่สน `SaveAdapterToProfile` (`source/cli.au3:40`)
- มี toast กำลังใช้โปรไฟล์ และวนรอ `asyncRun_isIdle()` ทุก 100 ms; ถ้า stdout มี `failed` แจ้ง error มิฉะนั้นแจ้งสำเร็จ (`source/cli.au3:76`)
- callback ของ CLI ไม่มี logic ตรวจผล (`source/cli.au3:70`)
- Create shortcut ตั้ง executable ปัจจุบันเป็น target และ `/set-config "..."` เป็น argument; ไม่มี argument ชี้ custom INI ที่เปิดอยู่ (`source/functions.au3:110`)
- `#RequireAdmin` อยู่ระดับโปรแกรม (`source/main.au3:43`); ยังไม่ได้ทดสอบ prompt/UAC จริง

**อนุมาน/ความเสี่ยงที่ต้องตรวจต่อ:**

1. Shortcut encode `[`/`]` ในชื่อ แต่ `_loadProfiles()` decode ชื่อ และ CLI lookup ไม่ได้ decode argument; shortcut ของชื่อมีวงเล็บจึงมีแนวโน้มหาไม่พบ (`source/functions.au3:112`, `:1193`, `source/cli.au3:34`)
2. Shortcut ที่สร้างขณะ Open custom INI ไม่มี path ของ INI นั้น; รอบใหม่อาจอ่านอีกไฟล์และหาโปรไฟล์ผิด/ไม่พบ (`source/functions.au3:118`, `source/main.au3:412`)
3. CLI ไม่ตรวจค่าคืนจาก `_apply`; หาก adapter ว่างหรือ IP/subnet ว่างจน `_apply` return ก่อน queue คำสั่ง ขณะ queue idle และ stdout ไม่มี `failed` ก็อาจแสดง success ทั้งที่ยังไม่ได้เปลี่ยนค่า (`source/cli.au3:50`, `:78`, `source/functions.au3:596`)
4. Boolean ที่เป็น `1` ถูก UI helper อ่านว่า true แต่ `_apply` เช็ก string `true` โดยตรง; CLI ส่งค่าจากไฟล์โดยไม่ normalize จึงมีความต่างระหว่างใช้ผ่าน UI กับ CLI สำหรับ legacy values (`source/functions.au3:1158`, `:606`, `:638`, `source/cli.au3:41`)
5. CLI ตัดสินจาก text `failed` ใน output ไม่มี structured result/exit code ที่ตรวจใน callback นี้; ไม่ใช่หลักฐานว่า OS state ตรงค่าที่ต้องการ (`source/cli.au3:70`, `:80`, `source/libraries/asyncRun.au3:65`, `:100`)

## 8. จุดเปราะบางของ persistence ที่เห็นจากซอร์ส

ทั้งหมดต่อไปนี้เป็นการอนุมานจากเส้นทางโค้ด ยังไม่ใช่ผล reproduce:

- **Rename กับ names cache:** Rename แก้ `oProfile.ProfileName` แต่ไม่ได้แก้ `profiles.names`; list/filter/shortcut lookup บางส่วนอาจไม่สอดคล้องจน reload (`source/functions.au3:926`, `source/data/profiles.au3:119`)
- **Import ชื่อซ้ำ:** `set` เขียน `$oProfile = $oNewProfile` ใน `For ... In` แต่ไม่ได้แทนโหนด linked list อย่างชัดเจน; ต้องตรวจ semantics ของ AutoItObject/iteration ก่อนยืนยันว่าทับ memory จริง อย่างไรก็ดีมีเขียน INI แล้ว caller refresh จึง reload จาก disk หลัง import (`source/data/profiles.au3:183`, `source/functions.au3:1315`, `source/main.au3:368`)
- **Import ชื่อพิเศษ:** ตอนเขียน section ใช้ชื่อที่ decode แล้วโดยไม่ผ่าน `iniNameEncode` ต่างจาก Save/New; ชื่อมีวงเล็บ/CRLF ควรเป็น fixture ตรวจความเสียหาย (`source/functions.au3:1280`, `:1320`, `:1047`)
- **เขียนไม่สำเร็จแต่ memory เปลี่ยนแล้ว:** New ยัง add object แม้ IniWriteSection fail; Save แก้ object ก่อนเขียน; Delete ยัง remove object แม้ IniDelete fail (`source/functions.au3:1002`, `:1036`, `:950`)
- **เขียนทับทั้งไฟล์โดยตรง:** Sort/Move/Rename เปิดปลายทางแบบ overwrite และไม่มี temp-file + atomic replace ในฟังก์ชันที่ตรวจ; crash/เขียนผิดพลาดกลางทางอาจทำไฟล์ไม่ครบ และ Sort serializer จะทิ้ง keys/sections ที่ parser ไม่รู้จัก (`source/functions.au3:304`, `:514`, `:1568`)
- **รายงาน import/export เสร็จโดยไม่เช็กผลครบ:** caller ตั้งข้อความสำเร็จโดยไม่ตรวจค่าคืนของ `_ImportProfiles` หรือ `FileCopy`; import ไม่ตรวจผลการเขียนแต่ละ section (`source/main.au3:367`, `:380`, `source/functions.au3:1320`)
- **Options encoding ไม่สม่ำเสมอ:** StartupAdapter ถูก decode ตอน load แต่ serializers เขียน string ที่อยู่ใน object ตรง ๆ; ส่วน profile AdapterName เก็บ encoded form อยู่ใน object (`source/functions.au3:1211`, `:1251`, `source/data/options.au3:51`, `source/data/profiles.au3:234`)
- **คำสั่งเปลี่ยนเครือข่ายกับ persistence ไม่เป็น transaction ร่วมกัน:** Save/Apply แยกกันโดยตั้งใจ แต่ไม่มีความหมายเรื่อง revision ของโปรไฟล์ที่ apply หรือ snapshot/rollback ในโมเดลนี้ (`source/functions.au3:572`, `:1019`, `source/data/profiles.au3:48`)

## 9. ข้อกำหนดที่นำไปออกแบบใหม่ได้ โดยไม่คัดลอก implementation

รายการนี้เป็น **ข้อเสนอออกแบบใหม่** ไม่ใช่พฤติกรรมที่ของเดิมมีแล้ว:

1. แยก `SavedProfile`, `EditorDraft`, `ObservedAdapterState` ชัดเจน แต่ยังอยู่หน้าเดียว; มี dirty indication และ Apply/Save แยกความหมายเดิม
2. ใช้ profile ID และ interface GUID เป็นตัวตน; เก็บชื่อเป็น display metadata รองรับการเปลี่ยนชื่ออะแดปเตอร์
3. กำหนด schema version/typed boolean/IP values; migrate INI เก่าแบบอ่านเข้าเท่านั้น พร้อมรายงาน fields/ชื่อที่ต้องแก้
4. Import มีผลสรุปเพิ่ม/ชน/ข้าม/เสียและตัวเลือก conflict; Export ระบุว่าโปรไฟล์หรือรวม app settings
5. เขียนไฟล์ผ่าน temp + atomic replace และ backup เมื่อ migrate; เปลี่ยน in-memory state ต่อเมื่อ commit สำเร็จ
6. CLI รับ profile ID/name และ explicit config path; คืน nonzero exit code เมื่อ validation/apply/verify ล้มเหลว
7. ใช้ผลแบบ structured และตรวจอ่าน OS กลับหลัง Apply ทุกขั้น; timeout ต้องถือเป็น unknown/failed verification ไม่ใช่ success
8. ชุดตรวจภายหลังควรครอบคลุมชื่อภาษาไทย, วงเล็บ/token literals, profile ชื่อซ้ำ, file read-only, disk write fail, rename→filter→save, custom INI→shortcut, ค่า boolean `1/true`, missing adapter และ CLI invalid profile data

## 10. งานที่ยังต้องตรวจ ไม่ควรอ้างว่าเสร็จแล้ว

- Runtime UI/CLI และ encoding ของไฟล์จริงเมื่อ AutoIt ถูก compile ด้วย toolchain ที่ใช้จริง
- ผลของ object reassignment ใน `_Profiles_setProfile` ต่อ linked list ที่ implementation นี้ใช้
- installer defaults และ migration จาก Simple IP Config รุ่นอื่น
- DHCP/application/network sequencing แบบเต็ม ให้ใช้บันทึกเฉพาะระบบเครือข่ายประกอบเอกสารนี้
- การเปรียบเทียบกับ release binary จริง หากผู้ใช้ต้องการยืนยันความตรงกันของซอร์สกับ shipped app ต้องได้ artifact ที่ระบุชัดและวิเคราะห์แยก; การอ่านครั้งนี้ไม่ได้พิสูจน์ว่า executable ที่ผู้ใช้เคยใช้ตรงกับซอร์สชุดนี้
