//! Calibration probe: runs AP-style stepped-sine measurements directly on the
//! DSP (no plug-in host) and prints gain / THD / H2 / H3 per frequency.
//!
//! cargo run --release --example probe -- [sr=96000] [ips=15] [sat=0] [level=0]
//!     [tape=1] [silk=off|red|blue] [tex=0] [blend=1] [trim=0] [freqs=20,50,...]
//!     [cal.<field>=v ...]
use neve542::channel::{Channel, Controls};
use neve542::silk::{SilkCal, SilkMode};
use neve542::tape::{TapeCal, TapeSpeed};
use std::f64::consts::PI;

fn fit(y: &[f64], f: f64, sr: f64) -> [f64; 6] {
    // Orthogonal projection on an integer number of cycles.
    let n = y.len();
    let mut out = [0.0; 6];
    for h in 1..=6 {
        let (mut s, mut c) = (0.0, 0.0);
        for (i, v) in y.iter().enumerate() {
            let w = 2.0 * PI * f * h as f64 * i as f64 / sr;
            s += v * w.sin();
            c += v * w.cos();
        }
        out[h - 1] = 2.0 * (s * s + c * c).sqrt() / n as f64;
    }
    out
}

fn main() {
    let mut sr = 96_000.0;
    let mut ctl = Controls::default();
    let mut level = 0.0;
    let mut tape = TapeCal::default();
    let mut silk = SilkCal::default();
    let mut freqs: Vec<f64> = vec![20., 31.5, 50., 100., 200., 500., 1000., 3000., 10000., 20000.];
    for a in std::env::args().skip(1) {
        let (k, v) = a.split_once('=').expect("key=value");
        let num = || v.parse::<f64>().unwrap();
        match k {
            "sr" => sr = num(),
            "ips" => ctl.speed = if num() > 20.0 { TapeSpeed::Ips30 } else { TapeSpeed::Ips15 },
            "sat" => ctl.saturation = num(),
            "level" => level = num(),
            "tape" => ctl.tape_in = num() > 0.5,
            "blend" => ctl.blend = num(),
            "trim" => ctl.trim_db = num(),
            "tex" => ctl.texture = num(),
            "silk" => {
                ctl.silk = match v {
                    "red" => SilkMode::Red,
                    "blue" => SilkMode::Blue,
                    _ => SilkMode::Off,
                }
            }
            "freqs" => freqs = v.split(',').map(|x| x.parse().unwrap()).collect(),
            "head.tau" => tape.head.tau = num(),
            "head.hyst" => tape.head.hyst = num(),
            "head.r_max" => tape.head.r_max = num(),
            "head.sat" => tape.head.sat = num(),
            "head.knee" => tape.head.knee = num(),
            "head.shape" => tape.head.shape = num(),
            "a2_rep" => tape.a2_rep = num(),
            "emph15" => tape.emph15 = (num(), tape.emph15.1),
            "emph15b" => tape.emph15.1 = num(),
            "emph30b" => tape.emph30.1 = num(),
            "emph30" => tape.emph30 = (num(), tape.emph30.1),
            "c15" | "c30" => {
                let d: Vec<f64> = v.split(',').map(|x| x.parse().unwrap()).collect();
                let tab = if k == "c15" { &mut tape.contour15 } else { &mut tape.contour30 };
                for (i, row) in tab.iter_mut().enumerate() {
                    row.copy_from_slice(&d[i * 7..i * 7 + 7]);
                }
            }
            "clip_att" => tape.clip_att = num(),
            "drive" => {
                let d: Vec<f64> = v.split(',').map(|x| x.parse().unwrap()).collect();
                tape.drive_db.copy_from_slice(&d);
            }
            "silk.a2" => silk.a2_out = num(),
            "silk.s0" => silk.s0 = num(),
            "silk.red_k" => silk.red_k = num(),
            "silk.red_f" => silk.red_f = num(),
            "silk.blue_k" => silk.blue_k = num(),
            "silk.blue_f" => silk.blue_f = num(),
            "silk.tau" => silk.core.tau = num(),
            "silk.hyst" => silk.core.hyst = num(),
            "silk.r_max" => silk.core.r_max = num(),
            "silk.sat" => silk.core.sat = num(),
            _ => panic!("unknown key {k}"),
        }
    }
    let amp = 10f64.powf((level - 18.0) / 20.0);
    println!("{:>8} {:>8} {:>7} {:>7} {:>7}", "Hz", "gain dB", "THD %", "H2 %", "H3 %");
    for &f in &freqs {
        let mut ch = Channel::with_cal(sr, tape, silk);
        ch.set_controls(ctl);
        ch.reset();
        ch.snap();
        let settle = ((0.4 * sr) as usize).max((8.0 * sr / f) as usize);
        let cycles = (f * 0.25).max(20.0).round();
        let n = (cycles * sr / f).round() as usize;
        let mut y = Vec::with_capacity(n);
        for i in 0..settle + n {
            let x = amp * (2.0 * PI * f * i as f64 / sr).sin();
            let o = ch.process(x as f32) as f64;
            if i >= settle {
                y.push(o);
            }
        }
        // Re-phase: fit uses samples starting at `settle`.
        let h = fit(&y, f, sr);
        let thd = (h[1..].iter().map(|v| v * v).sum::<f64>()).sqrt() / h[0];
        println!(
            "{:8.1} {:+8.2} {:7.3} {:7.3} {:7.3}",
            f,
            20.0 * (h[0] / amp).log10(),
            100.0 * thd,
            100.0 * h[1] / h[0],
            100.0 * h[2] / h[0]
        );
    }
}
