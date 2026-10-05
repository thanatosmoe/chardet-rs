//! Micro-benchmark for the Rust detector.
//!
//! Run with:
//!     cargo run --release --example bench
//!
//! Prints per-case throughput (MiB/s) and files/s for a set of representative
//! inputs, plus a large-input run to exercise the max_bytes path.

use std::time::Instant;

fn cp1251_bytes() -> Vec<u8> {
    // "Съешь же ещё этих мягких французских булок, да выпей чаю."
    let hex = "d1fae5f8fc20e6e520e5f9b820fdf2e8f520ecffe3eae8f520f4f0e0edf6f3e7f1eae8f520e1f3ebeeea2c20e4e020e2fbefe5e920f7e0fe2e";
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn repeat_to(unit: Vec<u8>, target: usize) -> Vec<u8> {
    if unit.is_empty() {
        return unit;
    }
    let mut out = Vec::with_capacity(target + unit.len());
    while out.len() < target {
        out.extend_from_slice(&unit);
    }
    out
}

fn bench_case(name: &str, data: &[u8], iters: usize) {
    // Warm up (also loads the models once).
    for _ in 0..3 {
        std::hint::black_box(chardet::detect(data));
    }
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(chardet::detect(data));
    }
    let elapsed = start.elapsed().as_secs_f64();
    let per = elapsed / iters as f64;
    let mib = data.len() as f64 / (1024.0 * 1024.0);
    let mbps = if per > 0.0 { mib / per } else { 0.0 };
    println!(
        "{name:<14} {:>9} B  {iters:>5} iters  {:.3} ms/iter  {:>8.1} MiB/s",
        data.len(),
        per * 1000.0,
        mbps
    );
}

fn main() {
    let target = 256 * 1024;

    let ascii = repeat_to(
        b"The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. "
            .to_vec(),
        target,
    );
    let utf8 = repeat_to(
        "The naïve café — произведение 日本語. ".as_bytes().to_vec(),
        target,
    );
    let cp1251 = repeat_to(cp1251_bytes(), target);
    let binary = repeat_to((0..=255u8).collect(), target);

    println!("chardet-rs micro-benchmark (256 KiB inputs)\n");
    bench_case("ascii", &ascii, 50);
    bench_case("utf-8", &utf8, 50);
    bench_case("cp1251", &cp1251, 50);
    bench_case("binary", &binary, 50);

    // Large input exercises the max_bytes (200 KB) window.
    let large = repeat_to(utf8.clone(), 8 * 1024 * 1024);
    bench_case("large (8 MiB)", &large, 20);
}
