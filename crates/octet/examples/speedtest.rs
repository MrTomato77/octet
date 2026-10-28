//! POC: a speed test against Cloudflare's endpoints that leaves through one chosen adapter.
//!
//!   cargo run --release -p octet --example speedtest -- <ifIndex> <sourceIP> [--no-pin] [--secs N] [--watch i,j,..]
//!
//! The socket is pinned to the adapter with IP_UNICAST_IF and bound to its address; TLS is Windows' own
//! (SChannel, via native-tls). Every adapter in --watch has its byte counters read before and after, so
//! where the traffic really went is measured, not assumed. Nothing is sent to Cloudflare but the test itself.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HOST: &str = "speed.cloudflare.com";
const CHUNK: u64 = 25_000_000; // the SDK's largest request; the endpoint refuses ~100 MB
const UP_CHUNK: usize = 10_000_000;
const STREAMS: usize = 6;

type Tls = native_tls::TlsStream<TcpStream>;

#[derive(Clone, Copy)]
struct Via {
    index: u32,
    src: Ipv4Addr,
    pin: bool,
}

/// A TCP socket that can only leave through `via`, wrapped in TLS.
fn open(via: Via, to: SocketAddr) -> Result<Tls, String> {
    use std::os::windows::io::FromRawSocket;
    use windows_sys::Win32::Networking::WinSock::*;
    unsafe {
        let mut wsa: WSADATA = std::mem::zeroed();
        WSAStartup(0x0202, &mut wsa);
        let s = socket(AF_INET as i32, SOCK_STREAM, IPPROTO_TCP);
        if s == INVALID_SOCKET {
            return Err(format!("socket: {}", WSAGetLastError()));
        }
        if via.pin {
            // IPv4 wants the index in network byte order.
            let idx = via.index.to_be();
            if setsockopt(s, IPPROTO_IP, IP_UNICAST_IF, (&idx as *const u32).cast(), 4) != 0 {
                let e = WSAGetLastError();
                closesocket(s);
                return Err(format!("IP_UNICAST_IF: {e}"));
            }
        }
        let mut sa: SOCKADDR_IN = std::mem::zeroed();
        sa.sin_family = AF_INET;
        sa.sin_addr.S_un.S_addr = u32::from_ne_bytes(via.src.octets());
        if bind(s, (&sa as *const SOCKADDR_IN).cast(), size_of::<SOCKADDR_IN>() as i32) != 0 {
            let e = WSAGetLastError();
            closesocket(s);
            return Err(format!("bind {}: {e}", via.src));
        }
        let SocketAddr::V4(t) = to else { return Err("IPv4 only".into()) };
        let mut ta: SOCKADDR_IN = std::mem::zeroed();
        ta.sin_family = AF_INET;
        ta.sin_port = t.port().to_be();
        ta.sin_addr.S_un.S_addr = u32::from_ne_bytes(t.ip().octets());
        if connect(s, (&ta as *const SOCKADDR_IN).cast(), size_of::<SOCKADDR_IN>() as i32) != 0 {
            let e = WSAGetLastError();
            closesocket(s);
            return Err(format!("connect: {e}"));
        }
        let tcp = TcpStream::from_raw_socket(s as u64);
        tcp.set_read_timeout(Some(Duration::from_secs(15))).ok();
        tcp.set_write_timeout(Some(Duration::from_secs(15))).ok();
        tcp.set_nodelay(true).ok();
        native_tls::TlsConnector::new().map_err(|e| e.to_string())?.connect(HOST, tcp).map_err(|e| format!("tls: {e}"))
    }
}

/// Reads one response; returns (status, body bytes, time to first byte). `count` sees body bytes as they arrive.
fn response(c: &mut Tls, t0: Instant, count: &dyn Fn(u64)) -> Result<(u16, u64, Duration), String> {
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    let mut ttfb = None;
    while !head.ends_with(b"\r\n\r\n") {
        let n = c.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("closed".into());
        }
        ttfb.get_or_insert_with(|| t0.elapsed());
        head.push(b[0]);
    }
    let h = String::from_utf8_lossy(&head).to_ascii_lowercase();
    let status = h.get(9..12).and_then(|s| s.parse().ok()).unwrap_or(0);
    let len: u64 = h.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let mut left = len;
    let mut buf = vec![0u8; 256 * 1024];
    while left > 0 {
        let want = left.min(buf.len() as u64) as usize;
        let n = c.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("closed mid-body".into());
        }
        left -= n as u64;
        count(n as u64);
    }
    // As the SDK does: the time Cloudflare spent on its side (Server-Timing) is not network latency.
    let ttfb = ttfb.unwrap_or_default().saturating_sub(Duration::from_secs_f64(server_ms(&h) / 1e3));
    Ok((status, len, ttfb))
}

/// cfRequestDuration if given, else the sum of every cfSpeed* entry (BandwidthEngine.ts, cfGetServerTime).
fn server_ms(h: &str) -> f64 {
    let parts: Vec<(&str, f64)> = h.lines().filter_map(|l| l.strip_prefix("server-timing:")).flat_map(|v| v.split(','))
        .filter_map(|p| { let (k, d) = p.trim().split_once(";dur=")?; Some((k, d.trim().parse().ok()?)) }).collect();
    parts.iter().find(|(k, _)| *k == "cfreqdur" || *k == "cfrequestduration").map(|p| p.1)
        .unwrap_or_else(|| parts.iter().filter(|(k, _)| k.starts_with("cfspeed")).map(|p| p.1).sum())
}

fn get(c: &mut Tls, bytes: u64, count: &dyn Fn(u64)) -> Result<(u16, Duration), String> {
    let t0 = Instant::now();
    write!(c, "GET /__down?bytes={bytes} HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: octet-poc\r\n\r\n").map_err(|e| e.to_string())?;
    response(c, t0, count).map(|(s, _, t)| (s, t))
}

fn post(c: &mut Tls, body: &[u8], count: &dyn Fn(u64)) -> Result<u16, String> {
    write!(c, "POST /__up HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: octet-poc\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| e.to_string())?;
    for part in body.chunks(64 * 1024) {
        c.write_all(part).map_err(|e| e.to_string())?;
        count(part.len() as u64);
    }
    response(c, Instant::now(), &|_| {}).map(|(s, _, _)| s)
}

/// Unloaded latency: time to first byte of an empty download on a warm connection.
fn latency(via: Via, to: SocketAddr, n: usize) -> Result<Vec<f64>, String> {
    let mut c = open(via, to)?;
    get(&mut c, 0, &|_| {})?;
    (0..n).map(|_| get(&mut c, 0, &|_| {}).map(|(_, t)| t.as_secs_f64() * 1e3)).collect()
}

fn jitter(v: &[f64]) -> f64 {
    if v.len() < 2 { 0.0 } else { v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (v.len() - 1) as f64 }
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if s.is_empty() { f64::NAN } else { s[s.len() / 2] }
}

/// STREAMS connections pull (or push) for `secs`; a separate connection measures latency every 250 ms meanwhile.
/// Speed counts only after a 1 s warm-up, so TCP slow start doesn't drag it down.
fn bandwidth(via: Via, to: SocketAddr, secs: u64, up: bool) -> Result<(f64, Vec<f64>, u64), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));
    let counted = Arc::new(AtomicU64::new(0));
    let warm = Arc::new(AtomicBool::new(false));
    let err = Arc::new(Mutex::new(None::<String>));
    let body = Arc::new(vec![0u8; UP_CHUNK]);
    let socks = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
    let workers: Vec<_> = (0..STREAMS)
        .map(|_| {
            let (stop, total, counted, warm, err, body, socks) = (stop.clone(), total.clone(), counted.clone(), warm.clone(), err.clone(), body.clone(), socks.clone());
            std::thread::spawn(move || {
                let count = |n: u64| {
                    total.fetch_add(n, Relaxed);
                    if warm.load(Relaxed) {
                        counted.fetch_add(n, Relaxed);
                    }
                };
                let run = || -> Result<(), String> {
                    let mut c = open(via, to)?;
                    socks.lock().unwrap().push(c.get_ref().try_clone().map_err(|e| e.to_string())?);
                    while !stop.load(Relaxed) {
                        let s = if up { post(&mut c, &body, &count)? } else { get(&mut c, CHUNK, &count)?.0 };
                        if s != 200 {
                            return Err(format!("HTTP {s}"));
                        }
                    }
                    Ok(())
                };
                // A stream cut by the stop (read timeout, server close) is expected; anything before it is an error.
                if let Err(e) = run() {
                    if !stop.load(Relaxed) {
                        err.lock().unwrap().get_or_insert(e);
                    }
                }
            })
        })
        .collect();
    let mut loaded = Vec::new();
    let mut probe = open(via, to)?;
    let t0 = Instant::now();
    let mut warm_at = None;
    while t0.elapsed() < Duration::from_secs(secs + 1) {
        if warm_at.is_none() && t0.elapsed() >= Duration::from_secs(1) {
            warm.store(true, Relaxed);
            warm_at = Some(Instant::now());
        }
        if let Some(e) = err.lock().unwrap().clone() {
            stop.store(true, Relaxed);
            socks.lock().unwrap().iter().for_each(|s| { s.shutdown(std::net::Shutdown::Both).ok(); });
            return Err(e);
        }
        if let Ok((_, t)) = get(&mut probe, 0, &|_| {}) {
            loaded.push(t.as_secs_f64() * 1e3);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let window = warm_at.map(|w| w.elapsed().as_secs_f64()).unwrap_or(1.0);
    let bits = counted.load(Relaxed) as f64 * 8.0;
    stop.store(true, Relaxed);
    // Cut every stream now: a 25 MB request left to finish costs data nobody counts.
    for s in socks.lock().unwrap().iter() {
        s.shutdown(std::net::Shutdown::Both).ok();
    }
    for w in workers {
        w.join().ok();
    }
    Ok((bits / window / 1e6, loaded, total.load(Relaxed)))
}

/// Bytes in and out of an adapter so far (GetIfEntry2).
fn octets(index: u32) -> Option<(u64, u64)> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};
    let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
    row.InterfaceIndex = index;
    (unsafe { GetIfEntry2(&mut row) } == 0).then_some((row.InOctets, row.OutOctets))
}

fn cpu_ms() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut a, mut b, mut k, mut u) = (z, z, z, z);
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u) };
    let f = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as f64 / 1e4;
    f(k) + f(u)
}

fn peak_mb() -> f64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut m: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    m.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) };
    m.PeakWorkingSetSize as f64 / 1048576.0
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (Some(index), Some(src)) = (a.first().and_then(|s| s.parse().ok()), a.get(1).and_then(|s| s.parse().ok())) else {
        eprintln!("usage: speedtest <ifIndex> <sourceIP> [--no-pin] [--secs N] [--watch i,j,..]");
        std::process::exit(2);
    };
    let flag = |f: &str| a.iter().position(|x| x == f).and_then(|i| a.get(i + 1)).cloned();
    let via = Via { index, src, pin: !a.iter().any(|x| x == "--no-pin") };
    let secs: u64 = flag("--secs").and_then(|s| s.parse().ok()).unwrap_or(8);
    let mut watch: Vec<u32> = flag("--watch").map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    if !watch.contains(&index) {
        watch.insert(0, index);
    }
    // ponytail: DNS goes the system's way (default route); only the test traffic is pinned.
    let to = match (HOST, 443).to_socket_addrs().map(|mut i| i.find(SocketAddr::is_ipv4)) {
        Ok(Some(t)) => t,
        _ => {
            eprintln!("cannot resolve {HOST}");
            std::process::exit(1);
        }
    };
    println!("via ifIndex {index} from {src} (pinned: {}) to {to}, {secs}s per direction, {STREAMS} streams", via.pin);
    let before: Vec<_> = watch.iter().map(|&i| (i, octets(i))).collect();
    let (t0, c0) = (Instant::now(), cpu_ms());

    let result = (|| -> Result<(), String> {
        let lat = latency(via, to, 20)?;
        println!("latency   median {:.1} ms, jitter {:.1} ms ({} samples)", median(&lat), jitter(&lat), lat.len());
        let (down, dl, db) = bandwidth(via, to, secs, false)?;
        println!("download  {down:.1} Mbps, loaded latency median {:.1} ms ({} samples), {:.1} MB", median(&dl), dl.len(), db as f64 / 1e6);
        let (up, ul, ub) = bandwidth(via, to, secs, true)?;
        println!("upload    {up:.1} Mbps, loaded latency median {:.1} ms ({} samples), {:.1} MB", median(&ul), ul.len(), ub as f64 / 1e6);
        Ok(())
    })();
    if let Err(e) = &result {
        println!("FAILED: {e}");
    }
    std::thread::sleep(Duration::from_secs(2)); // let the cut streams drain before the counters are read
    println!("wall {:.1} s, cpu {:.0} ms, peak working set {:.1} MB", t0.elapsed().as_secs_f64(), cpu_ms() - c0, peak_mb());
    for (i, b) in before {
        match (b, octets(i)) {
            (Some((i0, o0)), Some((i1, o1))) => println!("ifIndex {i:>3}: in +{:.1} MB, out +{:.1} MB", (i1 - i0) as f64 / 1e6, (o1 - o0) as f64 / 1e6),
            _ => println!("ifIndex {i:>3}: no counters"),
        }
    }
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}
