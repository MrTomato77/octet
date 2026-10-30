//! Adapter-read benchmark for the self-improve loop (sealed: the loop must not edit it).
//! `sh scripts/bench_read.sh` prints one JSON line: `{"primary": <median ms>, "sub_scores": {...}}`.
//! Read only: it never changes the network. Correctness is `adapters::tests::native_matches_powershell`.

#[test]
#[ignore]
fn read_bench() {
    use std::time::Instant;
    let first = format!("{:?}", crate::adapters::read().expect("read failed"));
    for _ in 0..5 {
        crate::adapters::read().unwrap();   // warm up: DLLs, COM, caches the OS keeps anyway
    }
    let mut ms: Vec<f64> = (0..300)
        .map(|_| {
            let t = Instant::now();
            let got = crate::adapters::read().unwrap();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            // The same answer every time (unless the network itself changed mid-run, which fails the run).
            assert_eq!(format!("{got:?}"), first, "read() changed between calls");
            ms
        })
        .collect();
    ms.sort_by(|a, b| a.total_cmp(b));
    let (min, med, p90) = (ms[0], ms[ms.len() / 2], ms[ms.len() * 9 / 10]);
    println!("{{\"primary\": {med:.3}, \"sub_scores\": {{\"min\": {min:.3}, \"p90\": {p90:.3}}}}}");
}
