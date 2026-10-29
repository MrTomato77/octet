//! Speed test against Cloudflare's endpoints (speed.cloudflare.com), out through one adapter only.
//!
//! The order is the one the Speed test view draws: who the line is (meta), then unloaded latency, then
//! LEG seconds of download and LEG of upload, each on STREAMS connections at once with a separate one
//! timing latency under that load. Events go to the view as they happen (shape in ui/index.html, speedRun).
//! Nothing is sent to Cloudflare but the test itself; TLS is the system's own (SChannel on Windows).

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HOST: &str = "speed.cloudflare.com";
const CHUNK: u64 = 25_000_000; // the largest download the endpoint serves in one go
const UP_CHUNK: usize = 10_000_000;
const STREAMS: usize = 6;
const LEG: Duration = Duration::from_secs(8); // the view's progress ring assumes this (LEG_MS)
const WARM: Duration = Duration::from_secs(1); // TCP slow start: not counted in the speed
const TICK: Duration = Duration::from_millis(250);

/// Each test takes the next number; stopping (or a newer test) moves it on, and a test whose number
/// is no longer current winds down without another word to the view.
static RUN: AtomicU64 = AtomicU64::new(0);

pub fn stop() {
    RUN.fetch_add(1, Relaxed);
}

type Tls = native_tls::TlsStream<TcpStream>;

#[derive(Clone, Copy)]
struct Via {
    index: u32,
    src: Ipv4Addr,
    to: SocketAddr,
}

/// A TCP socket that can only leave through the adapter (IP_UNICAST_IF, bound to its address), wrapped in TLS.
#[cfg(windows)]
fn tcp(via: Via) -> Result<TcpStream, String> {
    use std::os::windows::io::FromRawSocket;
    use windows_sys::Win32::Networking::WinSock::*;
    // Winsock is already started: resolving the host (std) did it.
    let SocketAddr::V4(t) = via.to else { return Err("IPv4 only".into()) };
    let addr = |ip: Ipv4Addr, port: u16| {
        let mut a: SOCKADDR_IN = unsafe { std::mem::zeroed() };
        a.sin_family = AF_INET;
        a.sin_port = port.to_be();
        a.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
        a
    };
    unsafe {
        let s = socket(AF_INET as i32, SOCK_STREAM, IPPROTO_TCP);
        if s == INVALID_SOCKET {
            return Err(format!("socket: {}", WSAGetLastError()));
        }
        let fail = |what: &str| { let e = WSAGetLastError(); closesocket(s); Err(format!("{what}: {e}")) };
        let idx = via.index.to_be(); // IPv4 wants the index in network byte order
        if setsockopt(s, IPPROTO_IP, IP_UNICAST_IF, (&idx as *const u32).cast(), 4) != 0 {
            return fail("IP_UNICAST_IF");
        }
        let (from, to) = (addr(via.src, 0), addr(*t.ip(), t.port()));
        if bind(s, (&from as *const SOCKADDR_IN).cast(), size_of::<SOCKADDR_IN>() as i32) != 0 {
            return fail("bind");
        }
        // Connect with a deadline: a blocking one waits out every SYN retry (some 21 s) on a line that goes nowhere.
        let mut nb = 1u32;
        ioctlsocket(s, FIONBIO, &mut nb);
        if connect(s, (&to as *const SOCKADDR_IN).cast(), size_of::<SOCKADDR_IN>() as i32) != 0 && WSAGetLastError() != WSAEWOULDBLOCK {
            return fail("connect");
        }
        let mut p = WSAPOLLFD { fd: s, events: POLLWRNORM, revents: 0 };
        match WSAPoll(&mut p, 1, 10_000) {
            0 => { closesocket(s); return Err("connect: timed out".into()) }
            n if n < 0 => return fail("connect"),
            _ => {}
        }
        let (mut e, mut len) = (0i32, 4i32);
        getsockopt(s, SOL_SOCKET, SO_ERROR, (&mut e as *mut i32).cast(), &mut len);
        if e != 0 || p.revents & (POLLERR | POLLHUP) != 0 {
            closesocket(s);
            return Err(format!("connect: {e}"));
        }
        nb = 0;
        ioctlsocket(s, FIONBIO, &mut nb);
        Ok(TcpStream::from_raw_socket(s as u64))
    }
}

// ponytail: not pinned to the adapter off Windows; IP_BOUND_IF (macOS) when the test ships there.
#[cfg(not(windows))]
fn tcp(via: Via) -> Result<TcpStream, String> {
    TcpStream::connect_timeout(&via.to, Duration::from_secs(10)).map_err(|e| e.to_string())
}

fn open(via: Via) -> Result<Tls, String> {
    let t = tcp(via)?;
    t.set_read_timeout(Some(Duration::from_secs(15))).ok();
    t.set_write_timeout(Some(Duration::from_secs(15))).ok();
    t.set_nodelay(true).ok();
    native_tls::TlsConnector::new().map_err(|e| e.to_string())?.connect(HOST, t).map_err(|e| format!("TLS: {e}"))
}

/// Reads one response: (status, body, time to first byte). `count` sees body bytes as they arrive;
/// the body is kept only when `keep` (meta), the rest is read and dropped.
fn response(c: &mut Tls, t0: Instant, keep: bool, count: &dyn Fn(u64)) -> Result<(u16, Vec<u8>, Duration), String> {
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    let mut ttfb = None;
    while !head.ends_with(b"\r\n\r\n") {
        if c.read(&mut b).map_err(|e| e.to_string())? == 0 {
            return Err("Cloudflare closed the connection".into());
        }
        ttfb.get_or_insert_with(|| t0.elapsed());
        head.push(b[0]);
    }
    let h = String::from_utf8_lossy(&head).to_ascii_lowercase();
    let status = h.get(9..12).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut left: u64 = h.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let (mut buf, mut body) = (vec![0u8; 256 * 1024], Vec::new());
    while left > 0 {
        let want = left.min(buf.len() as u64) as usize;
        let n = c.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Cloudflare closed the connection".into());
        }
        left -= n as u64;
        count(n as u64);
        if keep {
            body.extend_from_slice(&buf[..n]);
        }
    }
    // The time Cloudflare spent on its side (Server-Timing) is not network latency (as their own test does).
    let ttfb = ttfb.unwrap_or_default().saturating_sub(Duration::from_secs_f64(server_ms(&h) / 1e3));
    Ok((status, body, ttfb))
}

/// cfRequestDuration if given, else the sum of every cfSpeed* entry.
fn server_ms(h: &str) -> f64 {
    let parts: Vec<(&str, f64)> = h.lines().filter_map(|l| l.strip_prefix("server-timing:")).flat_map(|v| v.split(','))
        .filter_map(|p| { let (k, d) = p.trim().split_once(";dur=")?; Some((k, d.trim().parse().ok()?)) }).collect();
    parts.iter().find(|(k, _)| *k == "cfreqdur" || *k == "cfrequestduration").map(|p| p.1)
        .unwrap_or_else(|| parts.iter().filter(|(k, _)| k.starts_with("cfspeed")).map(|p| p.1).sum())
}

fn get(c: &mut Tls, path: &str, keep: bool, count: &dyn Fn(u64)) -> Result<(u16, Vec<u8>, Duration), String> {
    let t0 = Instant::now();
    // /meta answers 403 without a Referer from Cloudflare's own page.
    write!(c, "GET {path} HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: Octet/{}\r\nReferer: https://{HOST}/\r\n\r\n", env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
    response(c, t0, keep, count)
}

fn post(c: &mut Tls, body: &[u8], count: &dyn Fn(u64)) -> Result<u16, String> {
    write!(c, "POST /__up HTTP/1.1\r\nHost: {HOST}\r\nUser-Agent: Octet/{}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
        env!("CARGO_PKG_VERSION"), body.len()).map_err(|e| e.to_string())?;
    for part in body.chunks(64 * 1024) {
        c.write_all(part).map_err(|e| e.to_string())?;
        count(part.len() as u64);
    }
    response(c, Instant::now(), false, &|_| {}).map(|r| r.0)
}

/// An empty download's time to first byte, in ms: latency without the line's own speed in it.
fn ping(c: &mut Tls) -> Result<f64, String> {
    get(c, "/__down?bytes=0", false, &|_| {}).map(|r| r.2.as_secs_f64() * 1e3)
}

/// The name people know a Thai ISP by. Cloudflare gives the network's registered description instead
/// ("Dynamic IP assignment for broadband service" for NT); holders checked against RIPEstat.
fn brand(asn: u64) -> Option<&'static str> {
    Some(match asn {
        23969 | 38040 | 4651 | 9931 | 131293 | 56120 => "National Telecom",   // NT: TOT and CAT as one
        7470 | 17552 | 38082 => "True Online",
        132061 => "TrueMove H",
        24378 => "dtac",
        45758 | 45629 => "3BB",
        133481 => "AIS Fibre",
        131445 | 45430 => "AIS",
        _ => return None,
    })
}

/// Bytes the far end has acknowledged on these sockets (SIO_TCP_INFO): sent, less what's still in flight or was sent again.
#[cfg(windows)]
fn acked(socks: &[TcpStream]) -> Option<u64> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{SIO_TCP_INFO, TCP_INFO_v0, WSAIoctl};
    // A socket that can't say (closing) is left out rather than turning the whole count into writes for a tick.
    Some(socks.iter().filter_map(|s| {
        let (ver, mut info, mut got) = (0u32, TCP_INFO_v0::default(), 0u32);
        let ok = unsafe { WSAIoctl(s.as_raw_socket() as usize, SIO_TCP_INFO, (&ver as *const u32).cast(), 4,
            (&mut info as *mut TCP_INFO_v0).cast(), size_of::<TCP_INFO_v0>() as u32, &mut got, std::ptr::null_mut(), None) } == 0;
        ok.then(|| info.BytesOut.saturating_sub(info.BytesInFlight as u64 + info.BytesRetrans as u64))
    }).sum())
}

#[cfg(not(windows))]
fn acked(_: &[TcpStream]) -> Option<u64> {
    None
}

fn median(v: &[f64]) -> Option<u32> {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s.get(s.len() / 2).map(|m| m.round() as u32)
}

/// One leg: STREAMS connections pull (or push) for LEG, while a separate one times latency under that load.
/// Every TICK the view gets the speed so far, the loaded latency so far and the bytes moved in the whole test.
fn leg(via: Via, up: bool, live: &dyn Fn() -> bool, used: &Arc<AtomicU64>, send: &dyn Fn(Value)) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let err = Arc::new(Mutex::new(None::<String>));
    let socks = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
    let loaded = Arc::new(Mutex::new(Vec::<f64>::new()));
    let body = Arc::new(vec![0u8; if up { UP_CHUNK } else { 0 }]);
    for _ in 0..STREAMS {
        let (stop, err, socks, body, used) = (stop.clone(), err.clone(), socks.clone(), body.clone(), used.clone());
        std::thread::spawn(move || {
            let count = |n: u64| { used.fetch_add(n, Relaxed); };
            let run = || -> Result<(), String> {
                // A connection the far end drops (keep-alive ends) is opened again, twice at most, rather than failing the test.
                for again in 0.. {
                    let mut c = open(via)?;
                    socks.lock().unwrap().push(c.get_ref().try_clone().map_err(|e| e.to_string())?);
                    while !stop.load(Relaxed) {
                        let s = match if up { post(&mut c, &body, &count) } else { get(&mut c, &format!("/__down?bytes={CHUNK}"), false, &count).map(|r| r.0) } {
                            Err(_) if again < 2 && !stop.load(Relaxed) => break,
                            r => r?,
                        };
                        if s != 200 {
                            return Err(format!("Cloudflare answered HTTP {s}"));
                        }
                    }
                    if stop.load(Relaxed) { break }
                }
                Ok(())
            };
            // A stream cut by the stop is expected; anything before it is an error.
            if let Err(e) = run() && !stop.load(Relaxed) {
                err.lock().unwrap().get_or_insert(e);
            }
        });
    }
    {
        let (stop, loaded, socks) = (stop.clone(), loaded.clone(), socks.clone());
        std::thread::spawn(move || {
            let Ok(mut c) = open(via) else { return };
            if let Ok(s) = c.get_ref().try_clone() { socks.lock().unwrap().push(s); }
            while !stop.load(Relaxed) {
                if let Ok(ms) = ping(&mut c) { loaded.lock().unwrap().push(ms); }
                std::thread::sleep(TICK);
            }
        });
    }
    // What this leg has moved. Downloaded bytes are counted as they're read. Uploaded ones are counted as the far
    // end acknowledges them: counting writes would count what only reached the socket's send buffer, megabytes a
    // stream in the first moments, and the dial would shoot up as upload starts.
    let base = used.load(Relaxed);   // used runs over the whole test
    let moved = || if up { acked(&socks.lock().unwrap()) } else { None }.unwrap_or_else(|| used.load(Relaxed) - base);
    let t0 = Instant::now();
    let mut warm_at = None;
    let result = loop {
        std::thread::sleep(TICK);
        if let Some(e) = err.lock().unwrap().clone() {
            break Err(e);
        }
        if !live() {
            break Ok(());
        }
        // The speed counts from the end of the warm-up; before that, a rough figure so the dial moves.
        let (since, bits) = match warm_at {
            None if t0.elapsed() >= WARM => { warm_at = Some((Instant::now(), moved())); continue }
            None => (t0.elapsed(), moved()),
            Some((w, at)) => (w.elapsed(), moved().saturating_sub(at)),
        };
        let mut ev = json!({ "kind": if up { "up" } else { "down" }, "mbps": (bits as f64 * 8.0 / since.as_secs_f64() / 1e5).round() / 10.0, "bytes": used.load(Relaxed) });
        if let Some(l) = median(&loaded.lock().unwrap()) { ev["loaded"] = json!(l); }
        send(ev);
        if t0.elapsed() >= LEG {
            break Ok(());
        }
    };
    // Cut every stream now: a 25 MB request left to finish costs data nobody counts.
    // The threads aren't waited for. One still connecting or in its TLS handshake has no socket here to cut yet,
    // and waiting on it held the next leg up for its whole timeout (15-21 s, the dial frozen meanwhile).
    // Each sees the stop once its call returns and ends on its own, its socket closed as it goes.
    stop.store(true, Relaxed);
    for s in socks.lock().unwrap().iter() {
        s.shutdown(Shutdown::Both).ok();
    }
    result
}

/// Who this adapter's connection is, as Cloudflare sees it: public IP, ISP and the Cloudflare site that answered.
/// The one answer behind Info's public IP and the speed test's ISP and site, so the two can't disagree.
fn meta(c: &mut Tls) -> Result<Value, String> {
    let (status, body, _) = get(c, "/meta", true, &|_| {})?;
    let m: Value = serde_json::from_slice(&body).map_err(|_| "Unexpected answer from Cloudflare".to_string())?;
    let ip = m["clientIp"].as_str().filter(|ip| ip.parse::<std::net::IpAddr>().is_ok()).filter(|_| status == 200).ok_or("Unexpected answer from Cloudflare")?;
    let isp = m["asn"].as_u64().and_then(brand).map(Value::from).unwrap_or_else(|| m["asOrganization"].clone());
    // city: a guess at where this connection is; site: where the Cloudflare server that answered is.
    Ok(json!({ "ip": ip, "isp": isp, "asn": m["asn"], "city": m["city"], "colo": m["colo"]["iata"], "site": m["colo"]["city"] }))
}

fn resolve(index: u32, src: Ipv4Addr) -> Result<Via, String> {
    // ponytail: the name is looked up the system's way (default route); only the test itself is pinned.
    let to = (HOST, 443).to_socket_addrs().ok().and_then(|mut a| a.find(SocketAddr::is_ipv4)).ok_or("Can't look up speed.cloudflare.com")?;
    Ok(Via { index, src, to })
}

/// Info's public IP: the same answer the speed test gets, out through the same adapter.
pub fn whois(index: u32, src: Ipv4Addr) -> Result<Value, String> {
    let mut c = open(resolve(index, src)?).map_err(|e| format!("Can't reach the internet through this adapter ({e})"))?;
    meta(&mut c)
}

/// The whole test. Returns once it's over, stopped or failed; a stopped test sends nothing more.
pub fn run(index: u32, src: Ipv4Addr, send: impl Fn(Value)) -> Result<(), String> {
    let me = RUN.fetch_add(1, Relaxed) + 1;
    let live = || RUN.load(Relaxed) == me;
    let send = |v: Value| if live() { send(v) };
    let via = resolve(index, src)?;
    let used = Arc::new(AtomicU64::new(0));

    let mut c = open(via).map_err(|e| format!("Can't reach Cloudflare through this adapter ({e})"))?;
    if let Ok(mut m) = meta(&mut c) {
        m["kind"] = json!("meta");
        send(m);
    }
    ping(&mut c)?; // warms the connection; the first is slow
    for _ in 0..20 {
        if !live() {
            return Ok(());
        }
        send(json!({ "kind": "latency", "ms": ping(&mut c)?.round() }));
    }
    drop(c);
    for up in [false, true] {
        leg(via, up, &live, &used, &send)?;
        if !live() {
            return Ok(());
        }
    }
    send(json!({ "kind": "done", "bytes": used.load(Relaxed) }));
    Ok(())
}

#[cfg(test)]
mod tests {
    // Real traffic (~200 MB): OCTET_IF=<adapter index> OCTET_SRC=<its IPv4> cargo test -p octet speed -- --ignored --nocapture
    #[test]
    #[ignore]
    fn runs_a_whole_test() {
        let (Ok(i), Ok(src)) = (std::env::var("OCTET_IF"), std::env::var("OCTET_SRC")) else { return };
        let t0 = std::time::Instant::now();
        let last = std::cell::RefCell::new(String::new());
        super::run(i.parse().unwrap(), src.parse().unwrap(), |v| {
            let k = v["kind"].as_str().unwrap_or("").to_string();
            if *last.borrow() != k || k == "done" { println!("{:5.1}s {v}", t0.elapsed().as_secs_f64()); *last.borrow_mut() = k; }
        }).unwrap();
        // A line that goes nowhere fails within the connect deadline, not after every SYN retry.
        let t = std::time::Instant::now();
        assert!(super::open(super::Via { index: i.parse().unwrap(), src: src.parse().unwrap(), to: "10.255.255.1:443".parse().unwrap() }).is_err());
        println!("dead end: {:?}", t.elapsed());
        assert!(t.elapsed() < std::time::Duration::from_secs(12));
    }
}
