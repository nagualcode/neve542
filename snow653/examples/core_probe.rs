//! THD of a bare magnetic core vs flux amplitude (rate independent, so a
//! single test frequency suffices). Prints: flux  gain_dB  THD%  H3%
//! cargo run --release --example core_probe -- hyst r_max shape sat knee flux1,flux2,...
use snow653::magnetics::{Core, CoreSpec};
use std::f64::consts::PI;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a[i].parse::<f64>().unwrap();
    let f = 100.0;
    let fs = 96_000.0;
    let tau = 1.0 / (2.0 * PI * f); // flux amplitude == voltage amplitude at f
    let spec = CoreSpec { tau, hyst: n(0), r_max: n(1), shape: n(2), power: 0.0, sat: n(3), knee: n(4), leak_hz: 0.05, plays: snow653::magnetics::PLAYS };
    for fl in a[5].split(',').map(|x| x.parse::<f64>().unwrap()) {
        let mut c = Core::new(spec, fs);
        let per = (fs / f) as usize;
        let mut h = [(0.0f64, 0.0f64); 6];
        for i in 0..per * 30 {
            let w = 2.0 * PI * f * i as f64 / fs;
            let y = c.process(fl * w.sin());
            if i >= per * 20 {
                for k in 0..6 {
                    h[k].0 += y * ((k + 1) as f64 * w).sin();
                    h[k].1 += y * ((k + 1) as f64 * w).cos();
                }
            }
        }
        let m: Vec<f64> = h.iter().map(|(s, c)| s.hypot(*c) * 2.0 / (per * 10) as f64).collect();
        let thd = m[1..].iter().map(|v| v * v).sum::<f64>().sqrt() / m[0];
        println!("{} {} {} {}", fl, 20.0 * (m[0] / fl).log10(), 100.0 * thd, 100.0 * m[2] / m[0]);
    }
}
