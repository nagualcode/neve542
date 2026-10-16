//! One 542 channel, end to end, in the circuit's own units (volts).
//!
//! ```text
//! in ─ INPUT XFMR ─ INPUT AMP (TRIM) ─┬──────────────────────────┐ dry
//!                                     └─ TAPE (drive/head/replay) ┘ wet
//!        BLEND ─ BLEND AMP ─(LEVEL meter)─ OUTPUT STAGE + XFMR + SILK ─ out
//! ```
//!
//! Everything non-linear runs oversampled (≥ 176.4 kHz). Calibration:
//! 0 dBu (0.7746 V rms) = −18 dBFS, so full scale is +18 dBu and the
//! hardware's +23.5 dBu clip point sits 5.5 dB above 0 dBFS, as in a real
//! studio with a −18 dBFS line-up.

use crate::amp::ClassA;
use crate::filters::{Analog, Biquad, Smoother};
use crate::magnetics::{Core, CoreSpec};
use crate::oversampler::{MAX_FACTOR, Oversampler};
use crate::silk::{SilkCal, SilkMode, SilkStage};
use crate::tape::{TapeCal, TapeCircuit, TapeSpeed};

/// Volts per unit of digital full scale (0 dBu peak = 1.0954 V at −18 dBFS).
pub const VOLTS_PER_FS: f64 = 1.095_445 / 0.125_892_54;
/// 0 dBu sine peak, in volts.
pub const DBU0_PEAK: f64 = 1.095_445;

/// Mu-metal input transformer: very high permeability, tiny loop
/// (spec: 0.07 % THD @ 20 Hz, +20 dBu, tape out).
pub const INPUT_CORE: CoreSpec = CoreSpec { tau: 8.0e-3, hyst: 0.004, r_max: 1.0, shape: 0.0, sat: 30.0, knee: 4.0, leak_hz: 0.2, plays: 8 };

#[derive(Clone, Copy, Debug)]
pub struct Controls {
    pub trim_db: f64,
    pub tape_in: bool,
    pub speed: TapeSpeed,
    pub saturation: f64,
    pub blend: f64,
    pub silk: SilkMode,
    pub texture: f64,
    pub output_db: f64,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            trim_db: 0.0,
            tape_in: true,
            speed: TapeSpeed::Ips15,
            saturation: 0.0,
            blend: 1.0,
            silk: SilkMode::Off,
            texture: 0.0,
            output_db: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Channel {
    os: Oversampler,
    fs_os: f64,
    ctl: Controls,
    in_hp: Biquad,
    in_core: Core,
    in_lp: Biquad,
    trim: Smoother,
    wet: Smoother,
    out_gain: Smoother,
    trim_target: f64,
    out_target: f64,
    tape_idle: bool,
    pub tape: TapeCircuit,
    pub silk: SilkStage,
    out_hp: Biquad,
    /// Peak follower at the blend-amp output (pre-SILK LEVEL meter), volts.
    pub level_peak: f64,
    level_release: f64,
}

impl Channel {
    pub fn new(fs: f64) -> Self {
        Self::with_cal(fs, TapeCal::default(), SilkCal::default())
    }

    pub fn with_cal(fs: f64, tape: TapeCal, silk: SilkCal) -> Self {
        let os = Oversampler::new(fs);
        let fs_os = fs * os.factor() as f64;
        let mut c = Channel {
            os,
            fs_os,
            ctl: Controls::default(),
            in_hp: Biquad::default(),
            in_core: Core::new(INPUT_CORE, fs_os),
            in_lp: Biquad::default(),
            trim: Smoother::new(0.02, fs_os, 1.0),
            wet: Smoother::new(0.02, fs_os, 1.0),
            out_gain: Smoother::new(0.02, fs_os, 1.0),
            trim_target: 1.0,
            out_target: 1.0,
            tape_idle: false,
            tape: TapeCircuit::new(fs_os, tape),
            silk: SilkStage::new(fs_os, silk),
            out_hp: Biquad::default(),
            level_peak: 0.0,
            level_release: 0.0,
        };
        c.configure();
        c
    }

    fn configure(&mut self) {
        let fs = self.fs_os;
        self.in_hp.c = Analog::hp1(0.8).to_digital(fs, 10.0);
        // Transformer + line amp HF limit (-2 dB @ 120 kHz, flat ±0.25 dB to 60 kHz).
        self.in_lp.c = Analog::lp2(150e3, 0.6).to_digital(fs, 60e3);
        self.out_hp.c = Analog::hp1(1.2).to_digital(fs, 10.0);
        self.level_release = (-1.0 / (0.35 * fs)).exp();
    }

    pub fn oversampling(&self) -> usize {
        self.os.factor()
    }

    pub fn reset(&mut self) {
        self.os.reset();
        self.in_hp.reset();
        self.in_core.reset();
        self.in_lp.reset();
        self.out_hp.reset();
        self.tape.reset();
        self.silk.reset();
        self.trim.value = db(self.ctl.trim_db);
        self.wet.value = self.wet_target();
        self.out_gain.value = db(self.ctl.output_db);
        self.level_peak = 0.0;
    }

    fn wet_target(&self) -> f64 {
        if self.ctl.tape_in { self.ctl.blend.clamp(0.0, 1.0) } else { 0.0 }
    }

    pub fn set_controls(&mut self, c: Controls) {
        self.ctl = c;
        self.trim_target = db(c.trim_db);
        self.out_target = db(c.output_db);
        self.tape.set_controls(c.speed, c.saturation);
        self.silk.set_controls(c.silk, c.texture);
    }

    /// Settle smoothers instantly (used after state recall / for tests).
    pub fn snap(&mut self) {
        self.trim.value = db(self.ctl.trim_db);
        self.wet.value = self.wet_target();
        self.out_gain.value = db(self.ctl.output_db);
    }

    #[inline(always)]
    fn tick(&mut self, v: f64) -> f64 {
        // Input transformer and Class-A input stage with TRIM.
        let v = self.in_lp.process(self.in_core.process(self.in_hp.process(v)));
        let dry = ClassA::LINE.process(v * self.trim.next(self.trim_target));

        // The tape circuit sleeps while it is fully out of the signal path;
        // it restarts demagnetised and BLEND/TAPE IN ramp it in without clicks.
        let target = self.wet_target();
        let wet = self.wet.next(target);
        let blended = if wet < 1e-6 && target == 0.0 {
            if !self.tape_idle {
                self.tape.reset();
                self.tape_idle = true;
            }
            ClassA::LINE.process(dry)
        } else {
            self.tape_idle = false;
            let tape = self.tape.process(dry);
            ClassA::LINE.process(dry + wet * (tape - dry))
        };

        let a = blended.abs();
        self.level_peak = if a > self.level_peak { a } else { self.level_peak * self.level_release };

        let out = self.out_hp.process(self.silk.process(blended));
        out * self.out_gain.next(self.out_target)
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut buf = [0.0; MAX_FACTOR];
        self.os.up(x as f64 * VOLTS_PER_FS, &mut buf);
        for i in 0..self.os.factor() {
            buf[i] = self.tick(buf[i]);
        }
        (self.os.down(&buf) / VOLTS_PER_FS) as f32
    }

    /// Drive meter (record level re. nominal 0 dBu @ Saturation MIN):
    /// value v lights LED k (1..8) when v >= k/8; LEDs are 3 dB apart,
    /// LED 2 = nominal, soft clip engages above LED 5 (+9 dB).
    pub fn drive_meter(&self) -> f32 {
        let d = 20.0 * (self.tape.drive_peak.max(1e-9) / DBU0_PEAK).log10();
        ((d + 6.0) / 24.0).clamp(0.0, 1.0) as f32
    }

    /// LEVEL meter (pre-SILK, dBu scale -10 -2 4 10 14 18 20 22):
    /// value v lights LED k when v >= k/8.
    pub fn level_meter(&self) -> f32 {
        level_to_meter(20.0 * (self.level_peak.max(1e-9) / DBU0_PEAK).log10()) as f32
    }
}

pub const LEVEL_LEDS_DBU: [f64; 8] = [-10.0, -2.0, 4.0, 10.0, 14.0, 18.0, 20.0, 22.0];

pub fn level_to_meter(dbu: f64) -> f64 {
    let t = &LEVEL_LEDS_DBU;
    if dbu <= t[0] {
        return ((dbu + 20.0) / 10.0).max(0.0) / 8.0;
    }
    for k in 0..7 {
        if dbu < t[k + 1] {
            return (k as f64 + 1.0 + (dbu - t[k]) / (t[k + 1] - t[k])) / 8.0;
        }
    }
    1.0
}

#[inline(always)]
fn db(x: f64) -> f64 {
    10f64.powf(x / 20.0)
}
