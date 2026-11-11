//! Windows setters the elevated copy runs in-process: WMI for the primary address and for
//! enable/disable, IP Helper plus the CIM persistent store for the interface metric.
//! No shell, no undocumented NSI.

use windows::Win32::System::{Com::*, Rpc::*, Variant::{InitVariantFromStringArray, VARIANT}, Wmi::*};
use windows::core::{BSTR, HSTRING, PCWSTR};

type Result<T> = std::result::Result<T, String>;

/// COM for this thread; uninitialized on drop only if this call started it.
pub struct Com(bool);

impl Com {
    pub fn new() -> Result<Self> {
        let r = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if r == windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            return Ok(Self(false)); // already an STA here: usable as it is
        }
        r.ok().map_err(|e| format!("Could not start COM: {e}"))?;
        Ok(Self(true))
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Field order matters: `svc` is released before `_com` uninitializes.
struct Wmi {
    svc: IWbemServices,
    _com: Com,
}

impl Wmi {
    fn connect(namespace: &str) -> Result<Self> {
        let com = Com::new()?;
        unsafe {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
            let svc = locator
                .ConnectServer(&namespace.into(), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)
                .map_err(|e| format!("Could not reach WMI: {e}"))?;
            CoSetProxyBlanket(&svc, RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE)
                .map_err(|e| e.to_string())?;
            Ok(Self { svc, _com: com })
        }
    }

    fn one(&self, wql: &str, ctx: Option<&IWbemContext>) -> Result<IWbemClassObject> {
        let mut got = self.query(wql, ctx)?;
        if got.len() != 1 {
            return Err("Windows does not list that adapter (it may be gone)".into());
        }
        Ok(got.remove(0))
    }

    fn query(&self, wql: &str, ctx: Option<&IWbemContext>) -> Result<Vec<IWbemClassObject>> {
        unsafe {
            let rows = self.svc.ExecQuery(&"WQL".into(), &wql.into(), WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, ctx).map_err(|e| e.to_string())?;
            let (mut got, mut n) = (Vec::new(), 0);
            loop {
                let mut row = [None];
                rows.Next(WBEM_INFINITE, &mut row, &mut n).ok().map_err(|e| e.to_string())?;
                match row[0].take() {
                    Some(r) if n == 1 => got.push(r),
                    _ => break,
                }
            }
            Ok(got)
        }
    }

    fn call(&self, obj: &IWbemClassObject, method: &str, args: &[(&str, VARIANT)]) -> Result<()> {
        unsafe {
            let path = BSTR::try_from(&get(obj, "__PATH")?).map_err(|e| e.to_string())?;
            // A method's signature lives on the class, not on the instance (an instance gives ILLEGAL_OPERATION).
            let class = BSTR::try_from(&get(obj, "__CLASS")?).map_err(|e| e.to_string())?;
            let mut cls = None;
            self.svc.GetObject(&class, WBEM_GENERIC_FLAG_TYPE(0), None, Some(&mut cls), None).map_err(|e| e.to_string())?;
            let mut sig = None;
            cls.ok_or("WMI class is missing")?.GetMethod(&HSTRING::from(method), 0, &mut sig, std::ptr::null_mut()).map_err(|e| e.to_string())?;
            let input = sig.map(|s| s.SpawnInstance(0)).transpose().map_err(|e| e.to_string())?;
            for (name, value) in args {
                put(input.as_ref().ok_or("WMI method takes no arguments")?, name, value)?;
            }
            let mut out = None;
            self.svc.ExecMethod(&path, &method.into(), WBEM_GENERIC_FLAG_TYPE(0), None, input.as_ref(), Some(&mut out), None)
                .map_err(|e| format!("{method}: {e}"))?;
            match u32::try_from(&get(&out.ok_or("WMI returned nothing")?, "ReturnValue")?).map_err(|e| e.to_string())? {
                0 => Ok(()),
                1 => Err(format!("{method} needs a restart to take effect")),
                code => Err(format!("Windows refused {method} (WMI code {code})")),
            }
        }
    }
}

fn get(obj: &IWbemClassObject, name: &str) -> Result<VARIANT> {
    let mut v = VARIANT::default();
    unsafe { obj.Get(&HSTRING::from(name), 0, &mut v, None, None) }.map_err(|e| e.to_string())?;
    Ok(v)
}

fn put(obj: &IWbemClassObject, name: &str, v: &VARIANT) -> Result<()> {
    unsafe { obj.Put(&HSTRING::from(name), 0, v, 0) }.map_err(|e| e.to_string())
}

fn strings(values: &[&str]) -> Result<VARIANT> {
    let wide: Vec<HSTRING> = values.iter().map(|v| HSTRING::from(*v)).collect();
    let ptrs: Vec<PCWSTR> = wide.iter().map(|h| PCWSTR(h.as_ptr())).collect();
    unsafe { InitVariantFromStringArray(&ptrs) }.map_err(|e| e.to_string())
}

/// The NetTCPIP cmdlets pick a store with a "PolicyStore" context value; WMI queries and writes take it too.
fn store(name: &str) -> Result<IWbemContext> {
    let ctx: IWbemContext = unsafe { CoCreateInstance(&WbemContext, None, CLSCTX_INPROC_SERVER) }.map_err(|e| e.to_string())?;
    unsafe { ctx.SetValue(&HSTRING::from("PolicyStore"), 0, &VARIANT::from(BSTR::from(name))) }.map_err(|e| e.to_string())?;
    Ok(ctx)
}

/// Deletes what a query finds, now and after a restart.
fn delete(wmi: &Wmi, wql: &str) -> Result<()> {
    for name in ["ActiveStore", "PersistentStore"] {
        let ctx = store(name)?;
        for row in wmi.query(wql, Some(&ctx))? {
            let path = BSTR::try_from(&get(&row, "__PATH")?).map_err(|e| e.to_string())?;
            unsafe { wmi.svc.DeleteInstance(&path, WBEM_GENERIC_FLAG_TYPE(0), &ctx, None) }.map_err(|e| format!("Could not remove {wql}: {e}"))?;
        }
    }
    Ok(())
}

/// A static method (Create) is called on the class itself.
fn create(wmi: &Wmi, class: &str, args: &[(&str, VARIANT)]) -> Result<()> {
    let mut cls = None;
    unsafe { wmi.svc.GetObject(&class.into(), WBEM_GENERIC_FLAG_TYPE(0), None, Some(&mut cls), None) }.map_err(|e| e.to_string())?;
    wmi.call(&cls.ok_or("WMI class is missing")?, "Create", args)
}

/// Primary address, as `netsh ... set address`: static = EnableStatic (replaces every address, DHCP off),
/// DHCP = EnableDHCP. Either way every typed-in address and default route goes, now and after a restart;
/// the Advanced lists go back on afterwards.
pub fn set_address(index: u32, static_ip: Option<(&str, &str)>) -> Result<()> {
    {
        let wmi = Wmi::connect(r"ROOT\CIMV2")?;
        let cfg = wmi.one(&format!("SELECT * FROM Win32_NetworkAdapterConfiguration WHERE InterfaceIndex={index}"), None)?;
        match static_ip {
            Some((ip, mask)) => wmi.call(&cfg, "EnableStatic", &[("IPAddress", strings(&[ip])?), ("SubnetMask", strings(&[mask])?)])?,
            None => wmi.call(&cfg, "EnableDHCP", &[])?,
        }
    }
    let wmi = Wmi::connect(r"ROOT\StandardCimv2")?;
    if static_ip.is_none() {
        delete(&wmi, &format!("SELECT * FROM MSFT_NetIPAddress WHERE InterfaceIndex={index} AND AddressFamily=2 AND PrefixOrigin=1"))?; // 1 = manual
    }
    delete(&wmi, &format!("SELECT * FROM MSFT_NetRoute WHERE InterfaceIndex={index} AND DestinationPrefix='0.0.0.0/0'"))
}

/// One more address (prefix Some) or one fewer (None). `ip` is a checked IPv4 address, safe in WQL.
pub fn extra_address(index: u32, ip: &str, prefix: Option<u8>) -> Result<()> {
    let wmi = Wmi::connect(r"ROOT\StandardCimv2")?;
    match prefix {
        Some(p) => create(&wmi, "MSFT_NetIPAddress", &[("InterfaceIndex", VARIANT::from(index as i32)), ("AddressFamily", VARIANT::from(2i32)),
            ("IPAddress", VARIANT::from(BSTR::from(ip))), ("PrefixLength", VARIANT::from(p))]),
        None => delete(&wmi, &format!("SELECT * FROM MSFT_NetIPAddress WHERE InterfaceIndex={index} AND IPAddress='{ip}'")),
    }
}

/// One more default gateway with its route metric (0 = automatic, as netsh's gwmetric=0), or one fewer (None).
pub fn gateway(index: u32, gw: &str, metric: Option<u32>) -> Result<()> {
    let wmi = Wmi::connect(r"ROOT\StandardCimv2")?;
    match metric {
        Some(m) => create(&wmi, "MSFT_NetRoute", &[("InterfaceIndex", VARIANT::from(index as i32)), ("AddressFamily", VARIANT::from(2i32)),
            ("DestinationPrefix", VARIANT::from(BSTR::from("0.0.0.0/0"))), ("NextHop", VARIANT::from(BSTR::from(gw))), ("RouteMetric", VARIANT::from(i32::from(u16::try_from(m).map_err(|_| "Gateway metric is too large")?)))]),
        None => delete(&wmi, &format!("SELECT * FROM MSFT_NetRoute WHERE InterfaceIndex={index} AND DestinationPrefix='0.0.0.0/0' AND NextHop='{gw}'")),
    }
}

/// By index, so a renamed adapter (or one named "2") cannot be confused with another.
pub fn enable(index: u32, on: bool) -> Result<()> {
    let wmi = Wmi::connect(r"ROOT\CIMV2")?;
    let nic = wmi.one(&format!("SELECT * FROM Win32_NetworkAdapter WHERE InterfaceIndex={index}"), None)?;
    wmi.call(&nic, if on { "Enable" } else { "Disable" }, &[])
}

/// Interface metric, None = automatic. IP Helper changes it now. What survives a restart is the
/// `InterfaceMetric` value under the adapter's Tcpip key (the CIM persistent store reads and writes that same
/// value): a number there is a manual metric, no value is automatic. No API empties it (Set-NetIPInterface
/// -AutomaticMetric Enabled leaves the number, and the next restart brings it back), so it is deleted here.
pub fn set_metric(index: u32, metric: Option<u32>) -> Result<()> {
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, REG_DWORD, RegDeleteKeyValueW, RegSetKeyValueW};
    let mut row = MIB_IPINTERFACE_ROW { Family: windows_sys::Win32::Networking::WinSock::AF_INET, InterfaceIndex: index, ..Default::default() };
    let (mut luid, mut guid) = Default::default();
    unsafe {
        if GetIpInterfaceEntry(&mut row) != 0 || ConvertInterfaceIndexToLuid(index, &mut luid) != 0 || ConvertInterfaceLuidToGuid(&luid, &mut guid) != 0 {
            return Err("That adapter is gone".into());
        }
        row.UseAutomaticMetric = metric.is_none();
        row.Metric = metric.unwrap_or(row.Metric);
        row.SitePrefixLength = 0; // IPv4 rows must send 0
        let code = SetIpInterfaceEntry(&mut row);
        if code != 0 {
            return Err(format!("Could not set the interface metric (error {code})"));
        }
        let key = HSTRING::from(format!(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{{{:?}}}",
            windows::core::GUID { data1: guid.data1, data2: guid.data2, data3: guid.data3, data4: guid.data4 }));
        let name = HSTRING::from("InterfaceMetric");
        let code = match metric {
            Some(m) => RegSetKeyValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), REG_DWORD, (&m as *const u32).cast(), 4),
            None => match RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr()) { 2 => 0, c => c }, // 2: none saved
        };
        if code != 0 {
            return Err(format!("Set the metric, but could not keep it for after a restart (error {code})"));
        }
    }
    Ok(())
}
