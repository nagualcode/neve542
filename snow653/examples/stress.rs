//! Robustness + CPU check: every control combination at every common rate,
//! fed with silence, DC, full-scale noise, +12 dBFS square bursts and
//! denormal-range input. Fails on NaN/Inf or runaway output.
use snow653::channel::{Channel, Controls};
use snow653::silk::SilkMode;
use snow653::tape::TapeSpeed;
use std::time::Instant;

fn main() {
    let mut worst = 0.0f32;
    let mut seed = 1u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0
    };
    for &sr in &[44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for silk in [SilkMode::Off, SilkMode::Red, SilkMode::Blue] {
            for speed in [TapeSpeed::Ips15, TapeSpeed::Ips30] {
                for &sat in &[0.0, 1.0] {
                    let mut ch = Channel::new(sr);
                    let ctl = Controls { speed, saturation: sat, silk, texture: 1.0, trim_db: 12.0, output_db: 15.0, ..Default::default() };
                    ch.set_controls(ctl);
                    ch.reset();
                    let n = sr as usize;
                    for i in 0..n * 2 {
                        let x = match i * 5 / n {
                            0 => 0.0,
                            1 => 1.0,
                            2 => rnd(),
                            3 => if (i / 50) % 2 == 0 { 4.0 } else { -4.0 },
                            _ => 1e-30,
                        };
                        let y = ch.process(x);
                        assert!(y.is_finite(), "non-finite at sr={sr} {silk:?} {speed:?} sat={sat} i={i}");
                        worst = worst.max(y.abs());
                    }
                }
            }
        }
    }
    println!("all finite; worst |y| = {worst:.2} (= {:.1} dBFS)", 20.0 * worst.log10());
    // CPU per channel at 48 kHz (4x OS), 10 s of pink-ish noise, per setting.
    let sr = 48_000.0;
    for (name, ctl) in [
        ("tape out, silk off", Controls { tape_in: false, ..Default::default() }),
        ("tape sat 0, silk off", Controls::default()),
        ("tape sat 1, silk off", Controls { saturation: 1.0, ..Default::default() }),
        ("tape sat 1, silk blue", Controls { saturation: 1.0, silk: SilkMode::Blue, texture: 1.0, ..Default::default() }),
    ] {
        let mut el = f64::MAX;
        let mut acc = 0.0f32;
        for _ in 0..3 {
            let mut ch = Channel::new(sr);
            ch.set_controls(ctl);
            ch.reset();
            let n = (sr * 10.0) as usize;
            let t = Instant::now();
            for _ in 0..n {
                acc += ch.process(rnd() * 0.3);
            }
            el = el.min(t.elapsed().as_secs_f64());
        }
        println!("CPU {name:24}: {:.2}% of one core per channel [{acc:.1}]", 100.0 * el / 10.0);
    }
}
