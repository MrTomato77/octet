pub fn field(thai: bool, text: &str) -> &str {
    if !thai {
        return text;
    }
    match text {
        "Profile name" => "ชื่อโปรไฟล์",
        "Subnet mask" | "Subnet / prefix" => "Subnet / prefix",
        "Preferred DNS" => "DNS หลัก",
        "Alternate DNS" => "DNS สำรอง",
        "Settings" => "การตั้งค่า",
        "IP mode" => "โหมด IP",
        "DNS mode" => "โหมด DNS",
        "Not assigned" => "ไม่มีค่า",
        "Manual" => "กำหนดเอง",
        "Automatic" => "อัตโนมัติ",
        _ => text,
    }
}

pub fn message(thai: bool, text: &str) -> &str {
    if !thai {
        return text;
    }
    match text {
        "Enter a profile name." => "ระบุชื่อโปรไฟล์",
        "Use a host address, not the subnet or broadcast address." => {
            "ใช้ IP ของเครื่อง ไม่ใช่เลขเครือข่ายหรือ broadcast"
        }
        "Gateway must differ from the IP address." => "Gateway ต้องไม่ซ้ำกับ IP ของเครื่อง",
        "Gateway must be in the same subnet in this version." => {
            "รุ่นนี้ต้องใช้ Gateway ใน subnet เดียวกัน"
        }
        "Gateway must be a host address." => "Gateway ต้องเป็น IP ของเครื่องที่ใช้ได้",
        "Select manual DNS for a static IP configuration." => "เมื่อกำหนด IP เอง ต้องกำหนด DNS เองด้วย",
        "Use a different DNS server, or leave this field empty." => "ใช้ DNS ที่ไม่ซ้ำกัน หรือเว้นช่องนี้ว่าง",
        "Enter at least one DNS server. Empty fields do not mean automatic DNS." => {
            "ระบุ DNS อย่างน้อยหนึ่งตัว ช่องว่างไม่ได้หมายถึงรับอัตโนมัติ"
        }
        "The configuration is incomplete." => "ข้อมูลการตั้งค่ายังไม่ครบ",
        "Enter a usable IPv4 address, for example 192.0.2.10." => "ระบุ IPv4 ที่ใช้ได้ เช่น 192.0.2.10",
        "Enter an IPv4 DNS server address." => "ระบุ IPv4 ของ DNS server",
        "Prefix must be between 0 and 32." => "Prefix ต้องอยู่ระหว่าง 0 ถึง 32",
        "Enter a subnet mask or a prefix from 0 to 32." => {
            "ระบุ subnet mask หรือ prefix ตั้งแต่ 0 ถึง 32"
        }
        "Subnet mask bits must be contiguous." => {
            "Subnet mask ต้องมีบิตเครือข่ายต่อเนื่องกัน เช่น 255.255.255.0"
        }
        _ => text,
    }
}

pub fn input_id(field: &str) -> &'static str {
    match field {
        "Profile name" => "profile_name",
        "IP address" => "address",
        "Subnet mask" => "subnet",
        "Gateway" => "gateway",
        "Preferred DNS" | "DNS" => "preferred_dns",
        "Alternate DNS" => "alternate_dns",
        _ => "profile_name",
    }
}

pub fn status(thai: bool, text: &str) -> &str {
    for (en, th) in [
        (
            "Ready. Profiles and drafts are stored locally.",
            "พร้อมใช้งาน โปรไฟล์และร่างเก็บในเครื่อง",
        ),
        (
            "Profile saved. Network settings have not changed.",
            "บันทึกโปรไฟล์แล้ว ค่าระบบเครือข่ายยังไม่เปลี่ยน",
        ),
        (
            "Check the settings listed below before saving.",
            "ตรวจสอบค่าที่ระบุก่อนบันทึก",
        ),
        (
            "Change preview prepared. No changes were applied.",
            "เตรียมตัวอย่างการเปลี่ยนค่าแล้ว ยังไม่มีการเปลี่ยนค่าจริง",
        ),
        (
            "Some settings need attention. Nothing has been applied.",
            "มีค่าที่ต้องตรวจสอบ ยังไม่มีการเปลี่ยนค่า",
        ),
        (
            "Sample snapshot refreshed. Your draft is unchanged.",
            "รีเฟรชข้อมูลจำลองแล้ว ค่าที่กำลังแก้ยังอยู่ครบ",
        ),
        (
            "New profile created. Edit its name and settings.",
            "สร้างโปรไฟล์ใหม่แล้ว แก้ชื่อและค่าได้เลย",
        ),
        (
            "Copied as manual settings. Nothing applied.",
            "คัดลอกเป็นค่ากำหนดเองแล้ว ยังไม่เปลี่ยนเครือข่าย",
        ),
        (
            "Storage failed. Changes remain in memory; see the storage notice.",
            "บันทึกลงไฟล์ไม่สำเร็จ ข้อมูลยังอยู่ในหน่วยความจำ ดูรายละเอียดการบันทึก",
        ),
    ] {
        if text == en || text == th {
            return if thai { th } else { en };
        }
    }
    text
}
