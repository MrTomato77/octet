# แผนพัฒนา Simple IP v2 ด้วย Rust

ฉบับ 1.0 • 17 ตุลาคม 2026 • โปรเจกต์ `D:\Simple IP v2`

สถานะล่าสุด: CP1 Rust UI พร้อมข้อมูลจำลองและ local persistence สร้างและทดสอบแล้ว ดู docs/checkpoints/CP1-completion.md ส่วน Windows network backend และ DHCP diagnostics ยังอยู่ในแผน

## 1. เป้าหมายและฐานความรู้

สร้างโปรแกรม desktop สำหรับ Windows ที่จัดการโปรไฟล์ IPv4 ได้รวดเร็วในหน้าจอเดียว ผู้ใช้เลือกอะแดปเตอร์ เห็นค่าปัจจุบัน แก้ค่าที่ต้องการ บันทึกโปรไฟล์ และ Apply ได้โดยแยกความหมายแต่ละอย่างชัดเจน เพิ่มการตรวจผลและวินิจฉัย DHCP ที่อธิบายได้จากหลักฐาน

ฐานการออกแบบคือรายงาน `docs/original-analysis` ซึ่งวิเคราะห์ AutoIt source เดิม 44 ไฟล์ แยกข้อเท็จจริงกับข้ออนุมานไว้แล้ว ใช้เป็น behavioral reference เท่านั้น เขียนโค้ด UI/logic/assets/text ใหม่ ไม่แปลโค้ดเดิมทีละบรรทัด ไม่คัดลอก assets และไม่จำเป็นต้องถอด binary เพื่อเริ่มงานนี้

พบในของเดิม: หน้าเดียวสองคอลัมน์, Save แยก Apply, Pull ใช้ค่าบนจอแล้วเปลี่ยนเป็น manual, profile INI, และคิว netsh/ipconfig การวิเคราะห์ดังกล่าวยังไม่ยืนยันพฤติกรรม executable ที่ผู้ใช้เคยใช้งาน

เครื่องพัฒนาที่ตรวจรอบวางแผน: Windows 11 Home x64, Rust/Cargo 1.98.0, toolchain `stable-x86_64-pc-windows-msvc` การมี toolchain ไม่ได้ยืนยันว่ามี linker/Windows SDK พร้อม ต้องพิสูจน์ด้วย build ใน CP1

**ผลลัพธ์รุ่นแรก:** โปรแกรม Windows x64 แบบ portable ที่อ่านข้อมูลได้โดยไม่ขอ admin; เปลี่ยนค่าตามอะแดปเตอร์ที่เลือกอย่างตรวจสอบได้; เก็บโปรไฟล์และรายงานปัญหา DHCP ได้ โดยมีเส้นทางคืนค่าที่ทำได้และอธิบายข้อจำกัด

## 2. ขอบเขตงานและลำดับส่งมอบ

| ความสามารถ | Core beta | รุ่นใช้งานแรก | ภายหลัง |
|---|---|---|---|
| หน้าหลักหนึ่งหน้าและเลือกอะแดปเตอร์ | มี | มี | ปรับตาม feedback |
| อ่านชื่อ/GUID/MAC/link/IP/DNS/routes | มี | มี | รองรับมุมมองขั้นสูง |
| IPv4 static หนึ่ง address, subnet, gateway ไม่บังคับ | มี เมื่อ backend ผ่าน gate | มี | หลาย address |
| DHCP IP + automatic หรือ custom IPv4 DNS | มี | มี | นโยบาย DNS ขั้นสูง |
| Save/New/Rename/Delete/Search/Memo | มี | มี | การจัดกลุ่ม |
| Apply และ verification แยกแต่ละขั้น | มี | มี | ขยาย provider |
| Snapshot และ Restore previous settings | มี | มี | ประวัติหลายรุ่น |
| Renew DHCP; Release แยกและ Cycle แบบชัดเจน | มี หลังผ่าน CP6 | มี | งาน batch หลาย NIC |
| Diagnostics local state และ lease | มี | มี | รายละเอียดเพิ่มเมื่อมีหลักฐาน |
| DNS/Internet probes ที่ผู้ใช้เริ่มเอง | จำกัดและมี target ให้ดู | มีตามผลทดสอบ routing | ตัวเลือกขั้นสูง |
| Import INI เดิมแบบ preview; export JSON ใหม่ | หลัง core network ผ่าน | มี | export INI เมื่อจำเป็น |
| CLI ใช้ profile และคืน exit code | ยังไม่ใช่ core gate | มี | automation เพิ่มเติม |
| Light/Dark/System และ tray | UI พื้นฐานก่อน | มี | polish |
| Enable/Disable adapter | แยกเป็นช่วงเสริม | มีถ้าผ่าน provider tests | ขยาย driver coverage |
| แก้ IPv6, VLAN, Wi-Fi credentials, firewall, proxy | ไม่มี | ไม่มี | ต้องออกแบบ scope ใหม่ |
| Cloud sync, บัญชีผู้ใช้, telemetry, auto updater | ไม่มี | ไม่มี | พิจารณาเป็นงานต่างหาก |

อ่านและแสดง IPv6 ได้เพื่อให้ผู้ใช้เห็นภาพระบบ แต่ action IPv4 ต้องไม่แก้ IPv6 โดยปริยาย หาก provider ที่ทดลองรักษา IPv6 ไม่ได้ ให้ปิด action นั้นจนแก้ provider สำเร็จ

อะแดปเตอร์ที่มีหลาย IPv4, policy จากองค์กร, VPN/virtual configuration ที่ซับซ้อน หรือข้อมูลก่อนเปลี่ยนอ่านไม่ครบ สามารถแสดงแบบ read-only พร้อมเหตุผล รุ่นแรกต้องไม่เงียบแล้วลบค่าที่เกินขอบเขต

## 3. ข้อกำหนด UX ของหน้าจอเดียว

### 3.1 การจัดพื้นที่

ยึดสองคอลัมน์แบบที่ผู้ใช้คุ้นเคย แล้วทำ typography/spacing/สถานะให้อ่านง่ายขึ้น

```text
┌──────────────────────────────────────────────────────────────────┐
│ Simple IP v2                       Refresh   Theme   More         │
├────────────────────┬─────────────────────────────────────────────┤
│ Profiles           │ Adapter [Ethernet                         ▾] │
│ Search             │ Description · MAC · Link state              │
│                    ├─────────────────────────────────────────────┤
│ Office             │ Current settings          Updated 14:30:12  │
│ Lab                │ IPv4 / Prefix / Gateway / DNS / DHCP        │
│ Home               ├─────────────────────────────────────────────┤
│                    │ Desired settings                            │
│ New  Save  More    │ IP:  Automatic / Manual                     │
│                    │ Address / Subnet / Gateway                   │
│                    │ DNS: Automatic / Manual                     │
│                    │ Preferred / Alternate                        │
│                    │ Copy current              Apply changes     │
│                    ├─────────────────────────────────────────────┤
│                    │ Memo / Diagnostics / Operation details      │
├────────────────────┴─────────────────────────────────────────────┤
│ Status · Target adapter · Progress · View details                  │
└──────────────────────────────────────────────────────────────────┘
```

ไดอะแกรมเป็นข้อเสนอใหม่ ไม่ใช่ภาพหน้าจอเดิมหรือแบบ visual ที่ถือว่าอนุมัติแล้ว ส่วนล่างใช้ expanders/พื้นที่สลับรายละเอียดเฉพาะจุดได้ แต่งานหลักไม่ต้องย้ายไปหลายหน้า

ขนาดตั้งต้นทดลองประมาณ 900×700 logical pixels; ทดสอบที่พื้นที่ใช้งาน 1280×720 และ DPI 100/125/150/200% กำหนด minimum size จากผลจริง ถ้าพื้นที่ไม่พอให้เฉพาะส่วนรายละเอียดเลื่อน โดยปุ่มหลักและเป้าหมายยังอ่านได้

### 3.2 ความหมายของการกระทำ

| Action | อ่านอะไร | เปลี่ยนอะไร | เปลี่ยนเครือข่ายหรือไม่ |
|---|---|---|---|
| เลือกโปรไฟล์ | SavedProfile | EditorDraft | ไม่ |
| Save | EditorDraft | โปรไฟล์บน disk | ไม่ |
| Save as new | EditorDraft | โปรไฟล์ ID ใหม่ | ไม่ |
| Refresh | Windows | ObservedAdapterState | ไม่ และไม่เขียนทับ draft |
| Copy current | snapshot ที่ refresh สำเร็จล่าสุด | EditorDraft แบบ manual | ไม่; แสดงว่าคัดเป็นค่าคงที่ |
| Apply changes | draft ที่ freeze + fresh snapshot | เครือข่ายเฉพาะ diff ที่วางแผนไว้ | ใช่ |
| Use DHCP | ตั้ง draft เป็น DHCP IP + automatic DNS | EditorDraft | ยังไม่ จนกด Apply |
| Renew DHCP | target GUID ที่ freeze | ขอ renew และอ่านผลใหม่ | ใช่ |
| Release DHCP | target GUID ที่ freeze | ขอ release | ใช่; เป็น action แยกจาก Renew |
| Restore previous | snapshot ของ operation ก่อนหน้า | คืน configuration ที่รองรับ | ใช่ และต้อง verify อีกครั้ง |

การตั้ง Use DHCP เป็นการแก้ draft ช่วยให้ปุ่มทุกตัวมีความหมายสม่ำเสมอ ถ้าต้องการ shortcut DHCP-and-Apply ในอนาคตต้องตั้งชื่อให้สื่อว่าทำทันที

### 3.3 รายละเอียด interaction

- แสดงเครื่องหมาย draft ยังไม่ Save และแยกจากสถานะ “ค่าที่ใช้จริงต่างจาก draft” ไม่ใช้สีอย่างเดียว
- เลือกโปรไฟล์ใหม่ขณะ draft dirty: เก็บ draft ต่อโปรไฟล์ใน session หรือให้ Save/Discard/Cancel อย่างชัดเจน ตัดสินทางเดียวใน CP1; default proposal คือถามเฉพาะตอนที่จะทิ้งงานจริง
- เปลี่ยนอะแดปเตอร์ไม่ rebind โปรไฟล์เงียบ ๆ; แสดงว่า profile ผูกอะแดปเตอร์อื่นหรือยังไม่ผูก ผู้ใช้เลือก target เองได้
- ดับเบิลคลิกโปรไฟล์ทำหน้าที่โหลด/เลือกเท่านั้นในแบบใหม่ การ Apply ผ่านปุ่มหรือ Ctrl+Enter ที่เห็นชัด
- Enter ภายในช่องกรอกทำงานตาม control; Enter ใน Memo เป็นขึ้นบรรทัดใหม่ Ctrl+S บันทึก, Ctrl+N สร้าง, Ctrl+F ค้นหา, F5 refresh; Delete profile ทำงานเฉพาะ list focus
- ขณะ operation ทำงาน ดูอะแดปเตอร์อื่นได้ แต่ไม่เปิด mutation ใหม่ใน core beta ผลลัพธ์ยังผูก target เดิม
- ข้อผิดพลาดบอกขั้นตอน ผลที่ทราบ ค่าที่ยังไม่ยืนยัน และ action ถัดไป รายละเอียด API/output อยู่ในส่วนขยาย
- รองรับข้อความและชื่อภาษาไทยตั้งแต่ prototype ไม่ตั้งภาษาแอปตามชื่อโฟลเดอร์หรือ locale โดยเดา
- Save/Refresh/Copy current ต้องเข้าถึงด้วย keyboard และ focus order สม่ำเสมอ

### 3.4 กติกา IP/DNS

Core beta รองรับ DHCP IP + automatic DNS, DHCP IP + manual DNS และ static IP + manual DNS โดย UI อธิบายเมื่อโหมดใดไม่รองรับ ไม่รับ static IP + DNS automatic จน backend พิสูจน์ semantics ที่ต้องการได้

ช่อง DNS manual ว่างไม่ถูกตีความว่า automatic หรือล้าง DNS เงียบ ๆ; ต้องกรอกอย่างน้อยหนึ่งค่า Alternate-only ให้ normalize เป็นรายการแรกและแสดงผลก่อน Apply สำหรับ legacy profile ที่ตั้งใจไม่มี DNS ให้ importer ทำ NeedsReview และยังไม่ Apply จนมี policy รองรับ

## 4. เทคโนโลยีและจุดที่ต้องทดลองก่อนเลือก

### 4.1 ตัวเลือกเริ่มต้น

| ส่วน | ข้อเสนอ | เหตุผลและ gate |
|---|---|---|
| ภาษา/target | Rust stable, Windows x64 MSVC, edition ที่ toolchain รองรับ | ใช้เครื่องมือที่มีอยู่; pin rust-toolchain หลัง build ผ่าน |
| GUI | เริ่ม spike ด้วย egui/eframe | UI utility หน้าเดียวทำ prototype ได้ตรง; ต้องผ่านภาษาไทย/DPI/keyboard/accessibility ตาม scope |
| GUI สำรอง | iced | ทดลองเฉพาะเมื่อ egui ไม่ผ่านข้อสำคัญ; ไม่พัฒนาสอง UI พร้อมกัน |
| Windows interop | `windows` crate จาก windows-rs | รวม Win32/COM calls ภายในโมดูล platform |
| Data | serde + serde_json, typed domain values | schema version และ import/export ที่ตรวจได้ |
| งานเบื้องหลัง | worker thread + typed messages และ bounded channels | UI ไม่รอ network/COM; ยังไม่เพิ่ม async runtime จนมีเหตุผล |
| Error/log | typed error + tracing | operation ID, stage และ diagnostic details |
| Profile ID | UUID จาก OS randomness | ชื่อเปลี่ยนได้แต่ identity คงเดิม |
| เวลา | UTC timestamps ใน storage; local display | lease/log เปรียบเทียบได้; ไม่ใช้ wall clock วัด timeout |

eframe เป็น framework ทางการของ egui สำหรับ native desktop; windows-rs ให้ Rust bindings สำหรับ Windows APIs การเลือกข้างบนเป็น design proposal ไม่ใช่ผล benchmark ของโปรเจกต์นี้ [eframe](https://github.com/emilk/egui/blob/main/crates/eframe/README.md), [windows-rs](https://github.com/microsoft/windows-rs), [iced](https://book.iced.rs/)

### 4.2 Gate เลือก GUI

Prototype ต้องเปิดได้บนเครื่องนี้, พิมพ์/วาง/แก้ชื่อไทยและ Memo ได้ถูกต้อง, ทดสอบ combining marks/IME, ไม่ตัดข้อความที่ DPI สูง, รองรับ tab focus และปุ่มหลักครบ ทดสอบ Contrast และ screen-reader เฉพาะสิ่งที่ประกาศรองรับ ถ้าไม่ผ่านให้ทดลอง iced ด้วย fixture เดียวกันก่อนเลือก

ใช้ฟอนต์ระบบหรือฟอนต์ที่ตรวจสิทธิ์แจกจ่ายแล้ว หากต้องฝังฟอนต์ไทยให้บันทึกที่มาและ license แยกจาก assets ที่สร้างเอง

ไม่รับประกันขนาดไฟล์ ความเร็ว หรือ memory จากคำอธิบาย library ให้บันทึก release build size/startup/idle CPU/memory ที่เครื่องนี้จริง แล้วตั้ง baseline ก่อน optimize

### 4.3 Backend อ่านและเขียน

**อ่าน:** ใช้ IP Helper/NetIO สำหรับ adapters, unicast addresses, link state และ route table; เติม WMI/CIM สำหรับ DHCP lease/server และ config source ที่ API หลักให้ไม่ครบ ทุก field ระบุ known/unknown/error และเวลาที่อ่าน

เริ่มจาก `GetAdaptersAddresses` แล้วเสริม inventory/state ให้ครอบอะแดปเตอร์ที่ไม่มี address ใช้ GUID เป็น identity, LUID/index เป็น runtime routing handles ที่ resolve ใหม่ได้ Route diagnostics อ่าน route table จริง [GetAdaptersAddresses](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses), [GetIfEntry2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getifentry2), [GetIpForwardTable2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getipforwardtable2)

**เขียน core beta:** ทดลอง provider ที่เรียก `netsh.exe` ของ Windows โดยตรงสำหรับ IPv4 address/gateway/DNS และใช้ WMI per-adapter method สำหรับ Renew/Release ให้ map จาก GUID ไป instance ทุกครั้ง ไม่มี shell wrapper หรือการ parse คำว่า failed/exists เป็นตัวตัดสินหลัก

Netsh มี context IPv4 สำหรับ address และ DNS จึงเป็น candidate ที่จำกัดงาน IPv4 ได้ การรับรองพฤติกรรมจริงต้องผ่าน lab tests ของโปรเจกต์ โดยเฉพาะการเปลี่ยนจาก static กลับ DHCP และการรักษาค่าอื่น [Microsoft netsh interface](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/netsh-interface)

กติกาการเรียก process: resolve absolute System32 executable path, ส่ง args แยกตาม typed fields ผ่าน process API, ซ่อนหน้าต่าง, เก็บ exit code/stdout/stderr ต่อคำสั่ง, จำกัด output/time, ไม่ให้ UI ส่ง command text หรือ executable path เข้า helper

ต้องพิสูจน์การระบุ interface ด้วย runtime identifier ที่ผูกกับ GUID ในแต่ละคำสั่ง และตรวจ mapping ทันก่อนเรียก ถ้าคำสั่งใดรับเพียงชื่อและไม่สามารถจัดการ rename/reuse race ให้ปลอดจากการเปลี่ยนผิด NIC ได้ ให้เปลี่ยน action นั้นเป็น API/CIM provider หรือปิด action ไว้ ไม่ถือว่า readback หลังเขียนผิดเป้าหมายเป็นการป้องกันที่เพียงพอ

WMI เป็นตัวเลือกเสริมที่ให้ ReturnValue เป็นตัวเลข แต่ code สำเร็จอาจต้อง reboot และบาง method มี caveat: EnableDHCP ไม่ล้าง static gateway ทั้งหมด ส่วน EnableStatic มีข้อยกเว้นเรื่อง code 81 จึงไม่ใช้ return code อย่างเดียวพิสูจน์ผล ต้องอ่านกลับเสมอ [EnableDHCP](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/enabledhcp-method-in-class-win32-networkadapterconfiguration), [EnableStatic](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/enablestatic-method-in-class-win32-networkadapterconfiguration)

**กติกาการเปลี่ยน provider:** มี `NetworkBackend` interface เดียว ถ้า candidate ไม่ผ่าน identity/IPv6/route verification ให้แก้หรือเปลี่ยนเป็น Windows API/CIM provider ใน CP5 ก่อนเปิด Apply จริง ไม่มี silent fallback ที่อาจเปลี่ยน semantics ในระหว่าง operation

ไม่เขียน Registry เพื่อเปลี่ยน network configuration และไม่เรียก reset network stack ทั้งเครื่องเป็นวิธีแก้ทั่วไป

## 5. โครงสร้างโค้ดและข้อมูล

### 5.1 โครงสร้างโปรเจกต์ที่เสนอ

```text
D:\Simple IP v2\
  source/                         # อ้างอิง AutoIt เดิม ไม่แก้
  docs/
    original-analysis/
    RUST-IMPLEMENTATION-PLAN.md
    decisions/                    # เหตุผลเลือก GUI/provider/protocol
    checkpoints/                  # ผลตรวจแต่ละช่วง
  Cargo.toml                      # workspace ใหม่
  Cargo.lock
  rust-toolchain.toml
  crates/
    core/                         # types, validation, diff, operation state
    windows-backend/              # Win32/COM/process adapters
    app/                          # GUI และ helper entry mode
    cli/                          # เพิ่มเมื่อ core ผ่าน
  assets/                         # assets ใหม่เท่านั้น
  tests/
    fixtures/                     # ข้อมูลสมมติ ไม่ใช่ network ส่วนตัว
    lab/                          # integration scenarios ที่ opt-in
  dist/                           # artifacts ที่สร้างแล้ว
```

เริ่มเพียง core/windows-backend/app ก่อน เพิ่ม cli เมื่อถึง checkpoint ไม่สร้าง abstraction หรือ crates ที่ยังไม่มีผู้ใช้ `unsafe` จำกัดอยู่ interop layer พร้อมอธิบาย lifetime/buffer/ownership/cleanup

### 5.2 โมเดลหลัก

| Model | ข้อมูลที่ต้องมี |
|---|---|
| `AdapterIdentity` | GUID, display name, description, MAC optional; current LUID/index แยกจาก persistent identity |
| `ObservedAdapterState` | timestamp, admin/media/oper status, address list + prefix + DAD state, DHCP mode/source, DNS source/list, default routes, lease/server optional, read errors |
| `SavedProfile` | ID, name, order, memo, optional adapter binding, typed IP/DNS config, schema version ผ่าน container |
| `EditorDraft` | profile ID optional, fields, revision, dirty state, validation issues และ target GUID |
| `OperationRequest` | protocol version, operation ID, target GUID, frozen desired config, precondition fingerprint, timeout policy |
| `ChangePlan` | minimal stages, ก่อน/หลัง, unsupported fields, expected postconditions และ restore capability |
| `OperationResult` | ผลรวม, stage results, native error/exit codes, timestamps, snapshots, restore availability |
| `DiagnosticResult` | check ID, target GUID, observed time, evidence, severity, confidence และ bounded conclusion |

`IpMode` เป็น Dhcp หรือ Static(address,prefix,gateway optional); `DnsMode` เป็น Automatic หรือ Manual(list) ไม่มี sentinel string เช่น 0.0.0.0 แทน empty ส่วน DNS registration ใช้ Keep เป็น default และเพิ่มตัวเลือกเปลี่ยนค่าเมื่อทดสอบเฉพาะได้แล้ว

Snapshot เก็บ config mode/source และ observed runtime แยกกัน เพราะการคืน DHCP ต้องคืนโหมด ไม่ใช่ตั้ง IP ของ lease เก่ากลับเป็น static

### 5.3 Validation

- Parse IP ด้วย typed IPv4 values ไม่ยอมรับ hostname หรือ command fragments ในช่อง address
- รับ subnet แบบ dotted mask หรือ prefix แต่เก็บแบบ prefix และตรวจ mask ต่อเนื่อง
- Gateway ว่างได้; ห้ามเท่ากับ IP ตัวเอง; gateway นอก subnet เป็น unsupported ในโหมดพื้นฐานจนมี advanced route contract
- แยก address ปกติ/unspecified/multicast/loopback/broadcast ตามบริบท; /31 และ /32 ไม่ใช้กฎ network/broadcast แบบ /24 ต้องทดสอบเฉพาะหรือปิด Apply สำหรับกรณีที่ยังไม่รองรับ
- DNS order มีความหมาย; normalize duplicates โดย preview และไม่คัด IP จาก alternate-only ไปผิดช่อง
- DHCP profile ไม่เอาค่า static ที่ซ่อนไว้ไปเขียน และ manual DNS ไม่ถูกล้างเพราะเปลี่ยน IP mode โดยไม่มี diff แสดง
- Adapter ไม่มีอยู่/ถูกลบ/identity ไม่ตรงหรืออ่านค่าก่อนเปลี่ยนไม่ครบ: หยุดก่อน write
- มี multiple IP/route policy/ข้อกำหนดองค์กรที่ provider ไม่รองรับ: read-only หรือ NeedsReview

## 6. การเก็บโปรไฟล์และ migration

### 6.1 ตำแหน่งและ schema

โหมดปกติใช้ `%LOCALAPPDATA%\SimpleIPV2\` มี `profiles.json`, `settings.json`, `operations/` และ `logs/` แยกกัน Portable ใช้ marker `portable.mode` หรือ explicit `--data-dir` แล้วเก็บข้อมูลใต้โฟลเดอร์โปรแกรม ถ้าเขียนไม่ได้แสดงเหตุผล ไม่ fallback ไปอีก path เงียบ ๆ

ไฟล์โปรไฟล์มี `schema_version` และรายการ profiles; แต่ละ profile มี UUID ชื่อไม่เป็น identity เก็บตัวเลข/boolean/IP ตาม type ไม่มี token escape `{lb}` แบบเดิม ไม่เก็บ GUID ของอะแดปเตอร์ในฐานะว่าใช้ข้ามเครื่องได้เสมอ

Save: validate → serialize ลง temp file directory เดียวกัน → flush/replace ตาม Windows semantics → อัปเดต memory เมื่อ commit สำเร็จ สำรองก่อน migrate และคืน error ถ้าเขียนไม่ได้ CP3 ต้องทดสอบทั้งไฟล์เดิมมี/ยังไม่มีและกรณี disk failure ไม่เรียกว่าป้องกันไฟดับได้สมบูรณ์

### 6.2 Legacy INI import

อ่านจากไฟล์ที่ผู้ใช้เลือกเท่านั้น ไม่ค้น AppData ของโปรแกรมเดิมอัตโนมัติ

1. อ่าน encoding แบบมีหลักฐาน; รองรับ UTF-8/BOM และ UTF-16 เมื่อระบุได้ สำหรับ legacy codepage ให้ preview/เลือกก่อน ไม่เดาจนชื่อเสีย
2. แยก Options ออกจาก profiles แบบ case-insensitive
3. map IpAuto, IpAddress, IpSubnet, IpGateway, DnsAuto, IpDnsPref, IpDnsAlt, RegisterDns, AdapterName, Memo
4. รองรับ true/false และ 1/0; ไม่รู้จักให้ NeedsReview
5. decode scheme เดิมพร้อมเตือนความกำกวมของ literal tokens; ไม่ encode adapter name ซ้ำ
6. map adapter name เป็น GUID เมื่อพบตรงเพียงตัวเดียว; ถ้าไม่พบหรือชื่อชนให้ Unbound และผู้ใช้เลือกเอง
7. preview เพิ่ม/ชื่อซ้ำ/เปลี่ยนชื่อ/ข้าม/เสีย และให้เลือก conflict policy
8. สร้าง UUID ใหม่และ commit ชุดใหม่เมื่อทุกขั้นที่จำเป็นผ่าน ไม่แก้ไฟล์นำเข้า

Export รุ่นใหม่ส่ง profiles JSON เป็น default แยกตัวเลือก include adapter binding และ memo ส่วน app settings, logs และ operation snapshots ไม่ออกไปโดยปริยาย

## 7. สิทธิ์ผู้ดูแลและ operation engine

### 7.1 การขอสิทธิ์

GUI เปิดแบบผู้ใช้ปกติ อ่านและจัดโปรไฟล์ได้ เมื่อต้องเขียน launch executable เดียวกันใน helper mode ผ่าน Windows elevation helper ทำ operation เดียวแล้วออก ไม่ติดตั้ง service ถาวรในรุ่นแรก

IPC ใช้ named pipe ที่กำหนด ACL จำกัดผู้ใช้/session และตรวจ peer process identity/PID ทั้งสองฝั่งเมื่อทำได้ มี protocol version, operation ID, size limit, timeout และ request allowlist helper ตรวจ target/config ใหม่เองทั้งหมด ไม่รับ arbitrary shell/command/path ต้องทดสอบ UAC cancel และการ elevate ด้วยบัญชี admin คนละบัญชี; ถ้าไม่รองรับให้แจ้งอย่างชัดเจนก่อน write

รายละเอียด Windows launch/pipe ต้องพิสูจน์ใน CP4 โดยไม่เปลี่ยนเครือข่าย [ShellExecuteExW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw), [Named pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)

### 7.2 เส้นทาง Apply

1. Freeze draft revision และ target GUID สร้าง operation ID
2. Read fresh snapshot; resolve GUID → runtime identifiers; ตรวจ capability/validation
3. สร้าง diff และ stage plan; ถ้าไม่มีการเปลี่ยนคืน NoChange โดยไม่ขอ elevation
4. แสดง diff ในหน้าเดียว การกด Apply คือการเริ่มงาน ไม่เพิ่ม confirmation ทุกคลิก; action ที่ตัดการเชื่อมต่อเช่น Release แสดงผลกระทบชัดเจน
5. เก็บ pre-change configuration snapshot และ precondition fingerprint
6. Launch helper; helper อ่านค่าซ้ำ ถ้า state เปลี่ยนไปอย่างมีนัยสำคัญ return PreconditionChanged ก่อน write แล้วให้ GUI สร้าง diff ใหม่
7. เขียนทีละ stage เฉพาะ diff เช่น IP mode/address → gateway → DNS; ลำดับที่แน่นอนต้องยืนยันตาม provider ใน CP5
8. หลังแต่ละ stage อ่านกลับบน GUID เดิม ตรวจ address/prefix/route/DNS source/value ตาม postcondition
9. ถ้าขั้นจำเป็น fail หรือผล unknown หยุด mutation ถัดไป; ไม่แกล้งทำ DNS ต่อหลัง IP fail
10. Final readback และแสดง applied/partial/unknown/reboot required ตามหลักฐาน พร้อม Restore เมื่อ snapshot ครบ

การรอ DHCP แยก “เปิดโหมดสำเร็จ” กับ “ได้ lease แล้ว” สามารถแสดง ConfiguredAwaitingLease โดยไม่หมุนค้าง และไม่สรุปความล้มเหลวของ DNS เพียงเพราะยังไม่มี DHCP address

### 7.3 States และเวลา

ลำดับหลัก: Idle → Validating → Preparing → AwaitingElevation → Applying(stage) → Verifying → terminal state

Terminal states: NoChange, AppliedVerified, ConfiguredAwaitingLease, RebootRequired, ValidationRejected, ElevationCancelled, FailedBeforeWrite, PartialFailure, TimedOutUnverified, AdapterGone, CancelledBeforeWrite, CancelledAfterWriteUnverified

Timeout ตั้งต้นเพื่อทดลอง: read 5s, elevation wait 120s พร้อมยกเลิก, write stage 15s, DHCP renew/lease observation 45s และ advanced retry 90s ทุกค่าปรับจาก lab evidence และบันทึกใน result ใช้ monotonic clock; ไม่อ้างว่าเป็น timeout ที่ Windows รับรอง

Timeout ของ UI ไม่พิสูจน์ว่า OS call หยุดทำงาน หาก process/COM call ยังไม่จบ ให้ state เป็น unknown และ reconcile ก่อนเปิด mutation ใหม่ ไม่ retry อัตโนมัติที่อาจทำซ้ำ

Cancel ก่อน write ทำได้ทันที ระหว่าง write หยุดขั้นถัดไปและอ่านผลจริง ไม่สัญญาว่ายกเลิกการเปลี่ยนที่เกิดแล้ว ปิด GUI ขณะงานทำอยู่ต้องแสดงสถานะและ policy; helper ที่เริ่มเขียนแล้วหยุดหลัง current stage + verification ไม่เริ่ม stage ใหม่หาก client หาย

รุ่นแรกให้ mutation ทั้งแอปได้ครั้งละหนึ่ง operation พร้อม named synchronization ที่ป้องกัน GUI/CLI ซ้อนกัน read-only refresh ยังทำได้แต่ result มี generation เพื่อทิ้งข้อมูลเก่า

### 7.4 Restore และ crash recovery

- Restore เปรียบเทียบ current state กับค่าหลัง operation ก่อน เพื่อไม่เขียนทับการแก้จาก Windows/แอปอื่นโดยไม่รู้ตัว
- คืน configuration mode/source และเฉพาะ fields ที่เปลี่ยน ไม่ยัด lease เก่ากลับเป็น static
- Restore ก็ผ่าน validation/elevation/stage verification เหมือน Apply และอาจล้มเหลวได้
- ถ้า snapshot ไม่ครอบคลุมหลาย IP/routes ให้ปิด mutation ก่อนแทนการเสนอ restore ที่ทำไม่ได้
- Journal มี prepared/started/stage-complete/final state และ operation ID; เปิดโปรแกรมใหม่แล้วอ่านระบบจริงเพื่อ reconcile ไม่ rerun write จาก journal อัตโนมัติ
- ไม่รับประกันว่า connectivity จะกลับมา เพราะ DHCP server, cable, policy หรือ device อาจเปลี่ยนระหว่างทาง

## 8. DHCP diagnostics ที่ใช้ตัดสินใจได้

ผลวินิจฉัยเก็บเป็นหลาย checks ไม่ยุบเหลือไฟเขียว/แดงหนึ่งค่า แยก adapter configuration, DHCP acquisition, routes, DNS และ Internet probe

| Check | หลักฐานที่ต้องอ่าน | ข้อความที่สรุปได้ | สิ่งที่ยังห้ามฟันธง |
|---|---|---|---|
| Adapter present/admin state | GUID inventory + interface state | มี/ไม่มี/ปิดใช้งาน/อ่านไม่ได้ | ไม่มี address = adapter หาย |
| Media/oper state | Windows interface information | link down/up/unknown ตาม driver | IP ว่าง = สายหลุด |
| DHCP mode | config source/flag | DHCP เปิดหรือปิด | DHCP เปิด = มี lease |
| DHCP service | service status ที่อ่านได้ | service running/stopped/unknown | service running = server ตอบ |
| IPv4 acquisition | address origin, DAD state, lease fields | กำลังรอ, มี address, duplicate/tentative | ทุก address คือ DHCP lease |
| APIPA | link-local IPv4 + mode/source | พบ 169.254/16 และบริบท | DHCP server ล่มแน่นอน |
| Lease | server/obtained/expires เมื่อมี | มีข้อมูล lease และเวลาที่ทราบ | field ว่าง = lease หมดอายุ |
| Default route | route table ของ target, metric/compartment | มี/ไม่มี default route บนเป้าหมาย | มี route = gateway ติดต่อได้ |
| DNS configuration | source และ ordered server list | ตั้ง DNS ไว้อย่างไร | ตั้งค่าแล้ว = resolve ได้ |
| DNS probe | query ที่บอก server/target/interface ได้ | query นี้ได้/ไม่ได้/timeout | การล้มเหลวครั้งเดียวพิสูจน์สาเหตุทั้งหมด |
| Internet probe | explicit endpoint/protocol/interface path | ปลายทางนี้ติดต่อได้ในเงื่อนไขนี้ | ผ่าน NIC อื่น = NIC เป้าหมายออนไลน์ |

DNS/Internet probe เป็น opt-in ไม่เรียกบริการภายนอกทุกครั้งที่เปิดแอป default diagnostics ตรวจ local state ก่อน แสดง endpoint ก่อนเริ่ม probe ถ้าผูก source/interface ไม่ได้ให้บอกว่าเป็น system-wide result และไม่เอาไปยืนยันอะแดปเตอร์เป้าหมาย

Renew: ตรวจ DHCP mode + adapter → call per-adapter renew → เก็บ API result → poll snapshot/lease → รายงาน VerifiedLease / AwaitingLease / TimedOutUnverified / NativeError ไม่ใช้ข้อความ “renew สำเร็จ” เมื่อรู้เพียงว่าคำสั่งถูกเรียก

Release เป็นคำสั่งแยก ทำให้ไม่มี lease เป็นผลที่คาดได้ ไม่สร้าง fail เพราะไม่มี IP หลัง release Cycle คือ release ตามด้วย renew ตาม policy ที่มองเห็น ไม่ทำ release ก่อน renew ปกติโดยอัตโนมัติ

ไม่ทำ packet capture, restart DHCP service, disable/enable adapter หรือ reset network stack โดยอัตโนมัติจากผลวินิจฉัย

## 9. แผน checkpoint พร้อมงานและเกณฑ์ผ่าน

ทุก checkpoint ต้องตรวจสภาพไฟล์ก่อนแก้ ทำเฉพาะ scope ที่ระบุ บันทึกผลทดสอบ/ข้อจำกัด แล้วจึงไปช่วงถัดไป เมื่อเกณฑ์ผ่านให้เดินหน้าต่อได้ตามงานที่อนุญาต ไม่ต้องขออนุมัติซ้ำสำหรับงานอ่าน/โค้ด/ทดสอบจำลองตามปกติ งานเปลี่ยนค่าจริงต้องมี target lab ชัดเจนก่อน

### CP0 — Baseline และ behavior reference

**สถานะ:** เสร็จแล้วจากรอบวิเคราะห์ มีรายงาน 7 Markdown และ manifest 44 ไฟล์

ก่อนเริ่มโค้ด: ตรวจ Git status และ source hashes อีกครั้ง ไม่สร้าง initial commit รวม source อ้างอิงโดยเงียบ; แยก third-party reference ออกจาก license ของโค้ดใหม่

ผ่านเมื่อ: source ตรง baseline, มี decision log สำหรับ scope และสรุปประเด็นที่ยังไม่ยืนยัน

### CP1 — Build feasibility และ UI contract

งาน: ทดลอง MSVC/linker/SDK ด้วย minimal Rust build; prototype egui และทดสอบภาษาไทย/DPI/focus; เลือก GUI; สร้าง core types + fixture ของ adapters/profiles; วาดหน้าเดียวและกำหนด semantics ของ actions

ส่งมอบ: executable prototype ใช้ข้อมูลจำลอง, ภาพหน้าจอใน DPI ที่ทดสอบ, decision record เลือก GUI, keyboard map และ draft rules

ผ่านเมื่อ: build/run ได้, ชื่อไทย/Memo ไม่เสีย, หลักฐานหน้าจอไม่ถูกตัดในพื้นที่เป้าหมาย, Save/Apply/Refresh แยกชัด และ prototype ไม่มี backend writes

### CP2 — อ่าน Windows จริงอย่างเดียว

งาน: implement inventory identity/state, IPv4/IPv6 display, routes, DNS source, DHCP/lease enrichment; worker messages, cancellation/coalescing refresh และ unknown states

ส่งมอบ: app read-only ที่ใช้กับเครื่องนี้ได้, sample redacted snapshots และ comparison notes กับ Windows

ผ่านเมื่อ: adapter ที่มี/ไม่มี IP/disabled ปรากฏหรือมีข้อจำกัดที่แสดงชัด, GUID mapping ไม่ใช้ MAC เป็น key, ไม่มี panic เมื่อ API fail, ข้อมูลจาก refresh เก่าไม่เขียนทับของใหม่ และยังไม่มี write route

### CP3 — Profiles, validation และ persistence

งาน: New/Save/Save as/Rename/Delete/Search/Order/Memo, settings แยกไฟล์, JSON schema, atomic-save behavior, duplicate names/Unicode/dirty-state handling; importer ทำหลัง persistence ชัดเจนได้

ส่งมอบ: app เก็บโปรไฟล์ข้ามเปิดปิดได้, schema specification และ fixtures

ผ่านเมื่อ: round trip ไม่สูญข้อมูล, Save fail ไม่ทำ memory แสร้งว่าสำเร็จ, source/ไฟล์นำเข้าไม่เปลี่ยน, validation แยก IP/DNS modes ถูกต้อง, export ไม่มี settings/logs แอบรวม

### CP4 — Operation planner, helper และ dry-run

งาน: diff/minimal plan, identity/precondition checks, named-pipe protocol/elevation, journal, typed result, cancel/timeout และ mock fault injection

ส่งมอบ: Apply dry-run เห็น stages/diff; helper ติดต่อได้แต่ build/test mode นี้ปิด network mutation

ผ่านเมื่อ: NoChange ไม่ขอ UAC, UAC cancel ไม่เกิด write, malformed/oversized/wrong-target request ถูกปฏิเสธ, GUI responsive, fail กลางแผนไม่ส่ง stage ถัดไป และ operation ไม่เปลี่ยน target ตาม dropdown

### CP5 — Apply/Verify/Restore ใน lab

เริ่มเมื่อมี Windows VM/test adapter และบันทึก baseline/วิธีกลับสภาพพร้อม ไม่ใช้อะแดปเตอร์หลักของเครื่องเป็นค่าเริ่มต้น

งาน: prove provider กับ DHCP→static, static→static, static→DHCP, gateway none/present, DNS automatic/manual, alternate-only, route cleanup และ IPv6 preservation; implement readback/reboot/partial/unknown + Restore

ส่งมอบ: write-enabled lab build, per-scenario operation reports, provider decision และ restore evidence

ผ่านเมื่อ: ทุก action ผูก GUID เดียว, state จริงตรง contract, IPv6/other NIC ไม่เปลี่ยน, native return/timeout ไม่ถูกแปลเป็น success โดยไม่มีหลักฐาน, Restore คืน configuration mode ที่รองรับ และกรณีไม่รองรับหยุดก่อน write

### CP6 — DHCP diagnostics และ Renew/Release

งาน: local checks, lease observation, renew/release/cycle, stale timestamps, timeout policy, route attribution และ optional DNS/Internet probes

ส่งมอบ: diagnostic panel พร้อม evidence/time/target และ export diagnostic report แบบ redact

ผ่านเมื่อ: DHCP ปิด/ไม่มี link/no lease/APIPA/lease ปกติ/DNS เสีย/route หาย แสดงแยกกัน; หลาย NIC ไม่ทำให้ผล Internet NIC อื่นถูกอ้างว่าเป็น NIC เป้าหมาย; renew ไม่ใช้ output text ตัดสิน; unknown บอกได้อย่างตรงไปตรงมา

### CP7 — Legacy import, CLI และความสะดวก

งาน: INI preview/import/conflicts, CLI --data-dir/--profile-id/--adapter/--dry-run/--json, exit-code policy, optional adapter enable/disable, theme/tray และ startup behavior

ส่งมอบ: fixtures migration, CLI usage examples และ tray/settings flows

ผ่านเมื่อ: import ไม่ Apply เอง, boolean 1/true ตรงกัน, custom data path ใช้จริง, ชื่อไทย/วงเล็บ/literal tokens ไม่สูญโดยเงียบ, CLI validation fail คืน nonzero และปิด GUI ขณะ helper ทำงานเป็นไปตาม policy

### CP8 — Packaging และ acceptance

งาน: Windows x64 release build, dependency/license inventory, portable layout, version metadata, own icon, README/known limitations, checksums และ manual acceptance run

ส่งมอบ: portable ZIP, executable, user guide และ test summary Installer/code signing เป็นรายการเสริมที่ต้องระบุสถานะ ไม่แสร้งว่าทำแล้ว

ผ่านเมื่อ: clean launch บน Windows test environment, core tasks ผ่านจากต้นจนจบ, ไม่มี console เด้ง, DPI/keyboard/Thai ผ่าน, errors actionable, source hashes เดิมตรง และไม่มี release blocker ใน Apply/Restore/identity

## 10. ชุดทดสอบที่จำเป็น

### Unit และ fixture tests

- IP/mask/prefix/gateway validation รวม /31 /32 และ invalid input ตาม policy
- typed DHCP/DNS modes, alternate-only, empty DNS handling, profile round trip, migration true/1/token ambiguity
- operation transitions: precondition changed, fail stage แรก, partial, reboot, timeout, adapter gone, cancelled ก่อน/หลัง write
- stale refresh results, frozen operation target, no-change diff และ snapshot/restore DHCP semantics
- malformed protocol/unknown operation/oversized body และ config path failure

ไม่เขียน tests เพื่อทำซ้ำ UI layout code ทุกบรรทัด ให้เน้นกฎที่ผิดแล้วทำข้อมูลหรือเครือข่ายเสีย

### Windows lab matrix

| มิติ | อย่างน้อยต้องมี |
|---|---|
| Target | physical/test Ethernet, Wi-Fi, disabled NIC, virtual/VPN read-only |
| Address | DHCP lease, no lease, APIPA, static, duplicate/tentative เมื่อจำลองได้, multiple IP unsupported |
| DNS | auto, manual 1/2 entries, alternate-only input, unreachable DNS, IPv6 DNS coexistence |
| Gateway | present/none, stale static gateway หลัง DHCP, multiple NIC default routes |
| Privilege | normal read, UAC cancel, access denied, helper unavailable, alternate admin account behavior |
| Race/failure | NIC renamed/removed, dropdown changed, external config changed, timeout, app closes/crashes |
| Storage | Unicode path, read-only directory, malformed JSON/INI, unknown schema, failed write/replace |
| UI | DPI 100/125/150/200, small laptop viewport, Thai/IME, keyboard-only, system light/dark |

บันทึก expected/actual ก่อน/หลังต่อ target และระบุกรณีที่จำลองไม่ได้ ไม่เติมช่องว่าผ่านจากการคาดเดา Windows 11 x64 เป็น support target แรก Windows 10/ARM64/Server เป็น “ยังไม่ทดสอบ” จนมี environment จริง

### คุณภาพ build

ใช้ formatting, clippy และ unit tests ที่เหมาะกับโค้ดใหม่ในแต่ละ checkpoint Windows integration writes เป็น opt-in lab suite ไม่รันในการตรวจทั่วไปหรือ CI ของเครื่องอื่นโดยปริยาย

เป้าหมาย UI: ไม่มี Windows/COM/process call บล็อก render thread, แสดง acknowledgement ของ action ภายในรอบ UI ปกติ, จำกัดการ refresh ไม่ให้ซ้อน และไม่มี busy-loop ขณะ idle เก็บตัวเลขจริงก่อนตั้ง budget ของ memory/startup/CPU

## 11. ความเสี่ยง การตัดสิน และเกณฑ์หยุด

| ความเสี่ยง | วิธีรับมือ | checkpoint |
|---|---|---|
| GUI แสดง/แก้ไทยหรือ DPI ไม่ผ่าน | fixture เดียวทดสอบ egui ก่อน; เปลี่ยน UI library ก่อนสร้าง backend UX เต็ม | CP1 |
| MSVC/SDK ไม่พร้อม | ระบุส่วนที่ขาดจาก build log; ติดตั้งเฉพาะ dependency ที่จำเป็นตาม scope ที่อนุญาต | CP1 |
| Windows data sources ไม่ตรงกัน | แยก configured กับ observed, บันทึก source/time และ unknown | CP2 |
| provider ล้างค่าเกิน scope | snapshot preflight, limit simple NIC, lab proof; เปลี่ยน provider ก่อน release | CP5 |
| return code ไม่สะท้อนผลครบ | native code + readback + operation policy; ไม่ตัดสินจาก text | CP5 |
| UAC/IPC ทำให้ helper รับคำสั่งกว้างเกิน | typed allowlist, identity validation, restrictive pipe ACL, revalidate | CP4 |
| renew timeout ทั้งที่ระบบยังทำงาน | unknown/reconcile และไม่ retry อัตโนมัติ | CP6 |
| VPN/หลาย NIC ทำให้ probe ผิดเป้าหมาย | bind interface หรือระบุ system-wide; แยก unsupported | CP6 |
| legacy file สูญข้อมูลเมื่อ migrate | preview, backup, unknown fields report และไม่แก้ต้นฉบับ | CP7 |
| ไม่มี lab environment | ทำ CP1–CP4 และ mock tests ต่อได้; ยังไม่เปิด Apply write-enabled | CP5 |

Release blockers: adapter identity ผิด, mutation กระทบ NIC/IPv6 ที่ไม่อยู่ในแผน, silent profile loss, false success ที่ระบบไม่ตรง, helper รับ arbitrary command และ Restore อ้างว่าทำได้ทั้งที่ snapshot ไม่ครบ ประเด็นเหล่านี้ต้องแก้ก่อนส่ง write-enabled release

## 12. วิธีทำงานแบบ checkpoint และบันทึกต่อเนื่อง

เริ่มแต่ละรอบด้วยอ่าน plan + checkpoint ล่าสุด, Git status, และไฟล์ที่เกี่ยวข้อง ยืนยัน baseline ก่อน mutation ของโปรเจกต์ ไม่ข้ามจาก GUI mock ไปทดสอบบนอะแดปเตอร์หลัก

บันทึก `docs/checkpoints/CPn.md` หลังแต่ละช่วงด้วยรายการ:

1. เป้าหมายและ scope ที่ทำจริง
2. ไฟล์ที่สร้าง/แก้ และ decision ที่เลือก
3. วิธีตรวจพร้อม environment/expected/actual
4. สิ่งที่ผ่าน สิ่งที่ไม่ผ่าน และสิ่งที่ยังไม่ทดสอบ
5. source baseline status และ operation lab evidence ถ้ามี
6. งานต่อไปกับเกณฑ์ที่ต้องผ่าน ห้ามใช้คำว่าเสร็จสำหรับส่วนที่ยังเป็น mock

ถ้าความรู้ใหม่ทำให้ scope เปลี่ยน ให้แก้ plan/decision log ก่อนขยาย implementation ไม่เลือกทางซับซ้อนเพิ่มโดยไม่มีผลต่อเกณฑ์ผ่าน และไม่เริ่ม auto-update/cloud/service ถาวรแทรกใน core network work

ลำดับพึ่งพา: CP0 → CP1 → CP2 → CP3 → CP4 → CP5 → CP6 → CP7 → CP8 งานเอกสาร/fixtures ที่ไม่ขึ้นกับกันทำสลับภายในช่วงได้ Core beta review หลัง CP6; รุ่นใช้งานแรกหลัง CP8 เวลาจริงประเมินใหม่หลัง CP1 และ CP5 ซึ่งเป็นจุดเสี่ยงเรื่อง toolchain/UI และ Windows provider ไม่มีการรับประกันวันส่งจากแผนที่ยังไม่ build

**งานแรกที่จะทำเมื่อเริ่ม implementation:** ตรวจ source hashes/Git status → minimal Rust build → UI prototype ด้วยข้อมูลจำลองและภาษาไทย → เลือก GUI พร้อมบันทึก CP1 ยังไม่มีการเปลี่ยนค่าเครือข่ายในช่วงนี้

