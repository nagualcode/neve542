//! Output stage, output transformer and SILK / TEXTURE.
//!
//! RND: "Silk reduces the negative feedback on the output transformer and
//! adjusts this feedback's frequency response" (the feedback is taken from
//! one of the transformer's secondary windings). We model exactly that, in
//! two parts that both follow from feedback theory (Black, 1934):
//!
//! * **Closed-loop response.** With loop gain `T ≫ 1` the gain is set by the
//!   feedback network. Each SILK feedback-shaping section `N/D` (fitted to
//!   the hardware AP plot at TEXTURE MAX) is opened by the Texture fraction
//!   `t`, giving `N / ((1−t)·N + t·D)`: unity at t = 0, the measured Red/Blue
//!   curve at t = 1, and the physically correct path in between.
//! * **Distortion.** Distortion generated inside the loop is reduced by the
//!   sensitivity `S = 1/(1+T)`. The open-loop distortion of the Class-A
//!   output stage (2nd harmonic, ∝ level) and of the output transformer core
//!   (Rayleigh hysteresis, 3rd harmonic, ∝ 1/f) is computed on the actual
//!   signal and injected through `S(s)`. SILK OFF keeps `S` tiny (spec:
//!   < 0.0025 % THD @ 1 kHz +20 dBu); RED opens the loop in the upper mids
//!   and highs, BLUE in the lows and low mids, so the harmonics bloom in
//!   those regions only — as RND describes the two colours.

use crate::amp::ClassA;
use crate::filters::{Analog, Biquad, Cascade, Smoother};
use crate::magnetics::{Core, CoreSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilkMode {
    Off,
    Red,
    Blue,
}

#[derive(Clone, Copy, Debug)]
pub struct SilkCal {
    /// Red @ Texture MAX: [gain dB, HP fc, HP Q, high-shelf fc, gain dB, Q].
    pub red: [f64; 6],
    /// Blue @ Texture MAX: [gain dB, HP fc, HP Q, low-shelf fc, gain dB, Q, LP fc].
    /// (Shelf gain trimmed from the 1.95 dB linear fit: the output-core
    /// residual adds ~0.5 dB of in-phase LF bloom when Blue opens the loop.)
    pub blue: [f64; 7],
    /// Residual sensitivity with full feedback (SILK OFF).
    pub s0: f64,
    /// Sensitivity opened by Red (high-pass shaped) at Texture MAX.
    pub red_k: f64,
    pub red_f: f64,
    /// Sensitivity opened by Blue (low-pass shaped) at Texture MAX.
    pub blue_k: f64,
    pub blue_f: f64,
    /// Open-loop curvature of the single-ended output stage [1/V].
    pub a2_out: f64,
    pub core: CoreSpec,
}

impl Default for SilkCal {
    fn default() -> Self {
        SilkCal {
            red: [1.4716, 3.0152, 0.3, 3943.85, 1.7835, 0.5197],
            blue: [-0.1069, 2.7198, 0.3, 90.528, 1.45, 0.5101, 178_062.0],
            s0: 0.0015,
            red_k: 0.9,
            red_f: 1500.0,
            blue_k: 0.8,
            blue_f: 250.0,
            a2_out: 4.0e-3,
            core: CoreSpec { tau: 2.0e-3, hyst: 0.10, r_max: 1.5, shape: 0.0, sat: 14.0, knee: 3.0, leak_hz: 0.3, plays: 16 },
        }
    }
}

/// Analog high shelf (RBJ prototype), s in rad/s.
fn high_shelf(fc: f64, g_db: f64, q: f64) -> Analog {
    let a = 10f64.powf(g_db / 40.0);
    let w = 2.0 * std::f64::consts::PI * fc;
    let sa = a.sqrt();
    Analog { b: [a, a * sa / (q * w), a * a / (w * w)], a: [a, sa / (q * w), 1.0 / (w * w)] }
}

/// Analog low shelf (RBJ prototype), s in rad/s.
fn low_shelf(fc: f64, g_db: f64, q: f64) -> Analog {
    let a = 10f64.powf(g_db / 40.0);
    let w = 2.0 * std::f64::consts::PI * fc;
    let sa = a.sqrt();
    Analog { b: [a * a, a * sa / (q * w), a / (w * w)], a: [1.0, sa / (q * w), a / (w * w)] }
}

/// Open a feedback-shaping section by fraction `t`: `N / ((1−t)N + tD)`.
fn open(sec: Analog, t: f64) -> Analog {
    let mut d = [0.0; 3];
    for i in 0..3 {
        d[i] = (1.0 - t) * sec.b[i] + t * sec.a[i];
    }
    Analog { b: sec.b, a: d }
}

#[derive(Clone, Copy, Debug)]
pub struct SilkStage {
    pub cal: SilkCal,
    fs: f64,
    mode: SilkMode,
    target_mode: SilkMode,
    texture: f64,
    t: Smoother,
    lin: Cascade<3>,
    sens: Biquad,
    core: Core,
    countdown: usize,
}

impl SilkStage {
    pub fn new(fs: f64, cal: SilkCal) -> Self {
        let mut s = SilkStage {
            cal,
            fs,
            mode: SilkMode::Off,
            target_mode: SilkMode::Off,
            texture: 0.0,
            t: Smoother::new(0.02, fs, 0.0),
            lin: Cascade::default(),
            sens: Biquad::default(),
            core: Core::new(cal.core, fs),
            countdown: 0,
        };
        s.set_rate(fs);
        s
    }

    pub fn set_rate(&mut self, fs: f64) {
        self.fs = fs;
        self.t.set_time(0.02, fs);
        self.core.configure(self.cal.core, fs);
        self.update();
    }

    pub fn reset(&mut self) {
        self.lin.reset();
        self.sens.reset();
        self.core.reset();
        self.mode = self.target_mode;
        self.t.value = if self.mode == SilkMode::Off { 0.0 } else { self.texture };
        self.update();
    }

    pub fn set_controls(&mut self, mode: SilkMode, texture: f64) {
        self.target_mode = mode;
        self.texture = texture.clamp(0.0, 1.0);
    }

    fn update(&mut self) {
        let t = self.t.value.clamp(0.0, 1.0);
        let c = &self.cal;
        let fs = self.fs;
        match self.mode {
            SilkMode::Off => {
                self.lin.set(
                    &[(Analog::IDENTITY, 1000.0), (Analog::IDENTITY, 1000.0), (Analog::IDENTITY, 1000.0)],
                    fs,
                );
                self.sens.c = Analog::gain(c.s0).to_digital(fs, 1000.0);
            }
            SilkMode::Red => {
                let r = &c.red;
                let g = 10f64.powf(r[0] / 20.0);
                // Gain section folded into the HP section's numerator.
                let mut hp = open(Analog::hp2(r[1], r[2]), t);
                let gs = open(Analog::gain(g), t);
                hp.b = hp.b.map(|v| v * gs.b[0] / gs.a[0]);
                self.lin.set(
                    &[
                        (hp, r[1].max(20.0)),
                        (open(high_shelf(r[3], r[4], r[5]), t), r[3]),
                        (Analog::IDENTITY, 1000.0),
                    ],
                    fs,
                );
                let w = 2.0 * std::f64::consts::PI * c.red_f;
                let k = t * c.red_k;
                // S = s0 + k·s/(s+w)
                self.sens.c = Analog { b: [c.s0 * w, c.s0 + k, 0.0], a: [w, 1.0, 0.0] }.to_digital(fs, c.red_f);
            }
            SilkMode::Blue => {
                let b = &c.blue;
                let g = 10f64.powf(b[0] / 20.0);
                let mut hp = open(Analog::hp2(b[1], b[2]), t);
                let gs = open(Analog::gain(g), t);
                hp.b = hp.b.map(|v| v * gs.b[0] / gs.a[0]);
                self.lin.set(
                    &[
                        (hp, b[1].max(20.0)),
                        (open(low_shelf(b[3], b[4], b[5]), t), b[3]),
                        (open(Analog::lp1(b[6]), t), 20_000.0),
                    ],
                    fs,
                );
                let w = 2.0 * std::f64::consts::PI * c.blue_f;
                let k = t * c.blue_k;
                // S = s0 + k·w/(s+w)
                self.sens.c =
                    Analog { b: [(c.s0 + k) * w, c.s0, 0.0], a: [w, 1.0, 0.0] }.to_digital(fs, c.blue_f);
            }
        }
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        // Mode changes ramp Texture down, switch, and ramp back up (no clicks).
        let target_t = if self.mode == self.target_mode && self.mode != SilkMode::Off {
            self.texture
        } else {
            0.0
        };
        let before = self.t.value;
        let t = self.t.next(target_t);
        if self.mode != self.target_mode && t < 1e-4 {
            self.mode = self.target_mode;
            self.t.value = 0.0;
            self.countdown = 0;
        }
        if self.countdown == 0 || ((t - before).abs() > 1e-9 && self.countdown % 8 == 0) {
            self.update();
            self.countdown = 32;
        }
        self.countdown = self.countdown.saturating_sub(1);

        let y = self.lin.process(x);
        // Open-loop distortion of output stage + output transformer core ...
        let d_amp = self.cal.a2_out * y * y;
        // With SILK fully off the loop gain buries the core residual
        // (s0 · d_core < 0.001 %), so skip it and keep the core demagnetised.
        let d_core = if self.mode == SilkMode::Off && t < 1e-6 {
            0.0
        } else {
            self.core.process(y) - y
        };
        // ... reduced by the (Silk-dependent) feedback sensitivity.
        ClassA::LINE.process(y + self.sens.process(d_amp + d_core))
    }
}
