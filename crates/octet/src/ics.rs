//! Internet Connection Sharing through the documented NetSharingManager COM API.
//! `set` runs in the elevated copy.

use serde::Serialize;
use windows::core::{GUID, IUnknown, Interface};
use windows::Win32::NetworkManagement::WindowsFirewall::{
    ICSSHARINGTYPE_PRIVATE, ICSSHARINGTYPE_PUBLIC, INetConnection,
    INetSharingConfiguration, INetSharingManager, NetSharingManager,
};
use crate::win::Com;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, IDispatch};
use windows::Win32::System::Ole::IEnumVARIANT;
use windows::Win32::System::Variant::VARIANT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role { Public, Private }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub guid: String,
    pub index: Option<u32>,
    pub name: String,
    pub role: Option<Role>,
}


struct Entry { state: Connection, config: INetSharingConfiguration }

fn interface_index(guid: GUID) -> Option<u32> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{ConvertInterfaceGuidToLuid, ConvertInterfaceLuidToIndex};
    use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
    let guid = windows_sys::core::GUID { data1: guid.data1, data2: guid.data2, data3: guid.data3, data4: guid.data4 };
    let mut luid: NET_LUID_LH = unsafe { std::mem::zeroed() };
    let mut index = 0;
    (unsafe { ConvertInterfaceGuidToLuid(&guid, &mut luid) == 0 && ConvertInterfaceLuidToIndex(&luid, &mut index) == 0 }).then_some(index)
}

fn role(config: &INetSharingConfiguration) -> windows::core::Result<Option<Role>> {
    unsafe {
        if !config.SharingEnabled()?.as_bool() { return Ok(None); }
        match config.SharingConnectionType()? {
            ICSSHARINGTYPE_PUBLIC => Ok(Some(Role::Public)),
            ICSSHARINGTYPE_PRIVATE => Ok(Some(Role::Private)),
            _ => Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_UNEXPECTED)),
        }
    }
}

fn entries() -> Result<Vec<Entry>, String> {
    let read = || -> windows::core::Result<Vec<Entry>> { unsafe {
        let manager: INetSharingManager = CoCreateInstance(&NetSharingManager, None, CLSCTX_ALL)?;
        if !manager.SharingInstalled()?.as_bool() {
            return Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOTIMPL));
        }
        let enumerator: IEnumVARIANT = manager.EnumEveryConnection()?._NewEnum()?.cast()?;
        let mut out = Vec::new();
        loop {
            let mut value = [VARIANT::default()];
            let mut fetched = 0;
            enumerator.Next(&mut value, &mut fetched).ok()?;
            if fetched == 0 { break; }
            let connection: INetConnection = if let Ok(v) = IUnknown::try_from(&value[0]) { v.cast()? }
                else { IDispatch::try_from(&value[0])?.cast()? };
            let properties = manager.get_NetConnectionProps(&connection)?;
            let guid_text = properties.Guid()?.to_string();
            let guid = GUID::try_from(guid_text.trim_matches(['{', '}']))?;
            let config = manager.get_INetSharingConfigurationForINetConnection(&connection)?;
            let state = Connection { guid: guid_text, index: interface_index(guid), name: properties.Name()?.to_string(), role: role(&config)? };
            out.push(Entry { state, config });
        }
        Ok(out)
    }};
    read().map_err(|e| format!("Could not read Internet Sharing: {e}"))
}

fn validate(public: u32, private: u32, connections: &[Connection]) -> Result<[usize; 2], String> {
    if public == 0 || private == 0 || public == private { return Err("Sharing needs two different adapters".into()); }
    let find = |index| connections.iter().position(|c| c.index == Some(index)).ok_or_else(|| format!("Sharing adapter {index} is gone"));
    let pair = [find(public)?, find(private)?];
    for (i, c) in connections.iter().enumerate() {
        if c.role.is_some() && !pair.contains(&i) {
            return Err(format!("{} already participates in Internet Sharing; disable that sharing first", c.name));
        }
    }
    for (i, expected) in pair.into_iter().zip([Role::Public, Role::Private]) {
        if connections[i].role.is_some_and(|r| r != expected) {
            return Err(format!("{} already has a different sharing role; disable that sharing first", connections[i].name));
        }
    }
    Ok(pair)
}

/// Public = the adapter with the internet, private = the one the device hangs off. Turning on, a failed
/// private side turns the public side back off; turning off disables both. Windows sets the private
/// adapter's address itself (usually 192.168.137.1); the UI puts the old one back after Stop.
pub fn set(public: u32, private: u32, on: bool, scope: Option<&str>) -> Result<(), String> {
    let _com = Com::new()?;
    let entries = entries()?;
    let states: Vec<_> = entries.iter().map(|e| Connection { guid: String::new(), index: e.state.index, name: e.state.name.clone(), role: e.state.role }).collect();
    let [p, q] = validate(public, private, &states)?;
    let (p, q) = (&entries[p], &entries[q]);
    let fail = |e: &Entry, err: windows::core::Error| format!("Could not change sharing on {}: {err}", e.state.name);
    unsafe {
        if !on {
            q.config.DisableSharing().map_err(|e| fail(q, e))?;
            return p.config.DisableSharing().map_err(|e| fail(p, e));
        }
        if let Some(ip) = scope {
            set_scope(ip)?;
        }
        if p.state.role.is_none() {
            p.config.EnableSharing(ICSSHARINGTYPE_PUBLIC).map_err(|e| fail(p, e))?;
        }
        if q.state.role.is_none() && let Err(e) = q.config.EnableSharing(ICSSHARINGTYPE_PRIVATE) {
            let _ = p.config.DisableSharing();
            return Err(fail(q, e));
        }
    }
    Ok(())
}

/// The private side's address (/24) Windows hands out from. ICS reads it when sharing starts, so it goes
/// in before EnableSharing; the value stays for the next share, which is how Windows' own UI keeps it.
fn set_scope(ip: &str) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, REG_SZ, RegSetKeyValueW};
    let key = windows::core::HSTRING::from(r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters");
    let data: Vec<u16> = ip.encode_utf16().chain([0]).collect();
    for name in ["ScopeAddress", "ScopeAddressBackup"] {
        let name = windows::core::HSTRING::from(name);
        let code = unsafe { RegSetKeyValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32) };
        if code != 0 { return Err(format!("Could not set the sharing address (error {code})")); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(index: u32, role: Option<Role>) -> Connection { Connection { guid: String::new(), index: Some(index), name: index.to_string(), role } }
    #[test]
    fn refuses_conflicts_before_writing() {
        assert!(validate(1, 1, &[]).is_err());
        assert!(validate(1, 2, &[c(1, None)]).is_err(), "private adapter gone");
        assert!(validate(1, 2, &[c(1, None), c(2, None), c(3, Some(Role::Public))]).is_err(), "someone else's sharing");
        assert!(validate(1, 2, &[c(1, Some(Role::Private)), c(2, None)]).is_err(), "roles swapped");
        assert_eq!(validate(1, 2, &[c(2, None), c(1, Some(Role::Public))]), Ok([1, 0]));
    }
}
