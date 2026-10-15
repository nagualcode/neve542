//! Rupert Neve 542 Tape Circuit Emulation.
//! Models the record head transformer, magnetic core saturation, tape speed equalization (15/30 IPS),
//! soft-clipping circuit, and replay recovery amplifier with 2x oversampling.

use crate::biquad::{Biquad, Coefficients};
use crate::oversampler::Oversampler2x;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapeSpeed {
    Ips15,
    Ips30,
}

#[derive(Clone, Copy, Debug)]
pub struct TapeCircuit {
    speed: TapeSpeed,
    saturation: f32, // 0.0 .. 1.0
    sr: f32,

    oversampler: Oversampler2x,

    // Pre-emphasis & De-emphasis filters (run at base sample rate)
    pre_emphasis: Biquad,
    de_emphasis: Biquad,

    // Transformer core flux-weighting filters (1/f low-frequency saturation emphasis/de-emphasis)
    flux_weight: Biquad,
    flux_deweight: Biquad,

    // Tape Speed Equalization (Head Bump & Head Contour)
    head_bump: Biquad,
    sub_shelf: Biquad,
    high_shelf: Biquad,

    // Drive meter peak tracker
    drive_peak: f32,
}

impl Default for TapeCircuit {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl TapeCircuit {
    pub fn new(sr: f32) -> Self {
        let mut t = Self {
            speed: TapeSpeed::Ips15,
            saturation: 0.0,
            sr,
            oversampler: Oversampler2x::new(),
            pre_emphasis: Biquad::default(),
            de_emphasis: Biquad::default(),
            flux_weight: Biquad::default(),
            flux_deweight: Biquad::default(),
            head_bump: Biquad::default(),
            sub_shelf: Biquad::default(),
            high_shelf: Biquad::default(),
            drive_peak: 0.0,
        };
        t.update_filters();
        t
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.oversampler.reset();
        self.update_filters();
    }

    pub fn set_speed_and_saturation(&mut self, speed: TapeSpeed, saturation: f32) {
        let saturation = saturation.clamp(0.0, 1.0);
        if self.speed != speed || (self.saturation - saturation).abs() > 0.001 {
            self.speed = speed;
            self.saturation = saturation;
            self.update_filters();
        }
    }

    pub fn reset(&mut self) {
        self.oversampler.reset();
        self.pre_emphasis.reset();
        self.de_emphasis.reset();
        self.flux_weight.reset();
        self.flux_deweight.reset();
        self.head_bump.reset();
        self.sub_shelf.reset();
        self.high_shelf.reset();
        self.drive_peak = 0.0;
    }

    pub fn drive_meter_value(&self) -> f32 {
        // Maps drive peak to 1..8 LED segments:
        // Level 2 corresponds to nominal 0 dBu input with Saturation MIN.
        // Level 5 is the soft-clip threshold (~ -12 dBFS / +6 dBu).
        // Returns 0.0 .. 1.0 for DAW meters.
        let peak_db = 20.0 * (self.drive_peak.max(1e-6)).log10();
        // Nominal 0 dBu is calibrated around -18 dBFS.
        // Range: -30 dBFS (Drive 1) to -6 dBFS (Drive 8)
        let normalized = ((peak_db + 30.0) / 24.0).clamp(0.0, 1.0);
        normalized
    }

    fn update_filters(&mut self) {
        let sat = self.saturation;
        let sr = self.sr;

        // Tape head pre-emphasis / de-emphasis pairing
        // Exact inverse ensures complete passband transparency for linear signals
        let pre_gain = 3.5 + 2.5 * sat;
        self.pre_emphasis.coeffs = Coefficients::high_shelf(6500.0, pre_gain, 0.707, sr);
        self.de_emphasis.coeffs = self.pre_emphasis.coeffs.invert();

        // Magnetic transformer core flux-weighting:
        // Faraday's law: low frequencies induce higher magnetic flux (1/f).
        // A low-shelf emphasis before saturation + matching de-emphasis after saturation
        // drives bass frequencies into magnetic saturation first, giving classic transformer bloom.
        let flux_boost = 3.5 * sat;
        self.flux_weight.coeffs = Coefficients::low_shelf(150.0, flux_boost, 0.707, sr);
        self.flux_deweight.coeffs = self.flux_weight.coeffs.invert();

        match self.speed {
            TapeSpeed::Ips15 => {
                // Head bump peak at 61.87 Hz
                // Calibrated to produce +6.1 dB at Min Sat, compressing down to +3.1 dB at Max Sat
                let bump_gain = 6.60 - 3.20 * sat;
                self.head_bump.coeffs = Coefficients::peaking(61.87, bump_gain, 0.485, sr);

                // Sub-bass roll-off shelf at 25 Hz
                let sub_gain = -3.80 - 2.80 * sat;
                self.sub_shelf.coeffs = Coefficients::low_shelf(25.0, sub_gain, 0.672, sr);

                // High-frequency roll-off shelf
                let hi_gain = -1.20 - 1.20 * sat;
                self.high_shelf.coeffs = Coefficients::high_shelf(22000.0, hi_gain, 0.707, sr);
            }
            TapeSpeed::Ips30 => {
                // Head bump peak at 101.67 Hz
                // Calibrated to produce +4.3 dB at Min Sat, compressing down to +2.3 dB at Max Sat
                let bump_gain = 4.45 - 2.00 * sat;
                self.head_bump.coeffs = Coefficients::peaking(101.67, bump_gain, 0.435, sr);

                // Sub-bass shelf at 18.88 Hz
                let sub_gain = -2.12 - 2.70 * sat;
                self.sub_shelf.coeffs = Coefficients::low_shelf(18.88, sub_gain, 0.972, sr);

                // 30 IPS High Frequency Extension (calibrated to stay within +/-0.2 dB to 20 kHz)
                let hi_gain = 0.20 - 0.20 * sat;
                self.high_shelf.coeffs = Coefficients::high_shelf(18000.0, hi_gain, 0.707, sr);
            }
        }
    }

    /// Process a single sample through the tape emulation engine.
    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let sat = self.saturation;

        // 1. Pre-emphasis filter + Transformer Flux weighting
        let pre = self.pre_emphasis.process(input);
        let fluxed = self.flux_weight.process(pre);

        // Drive level scaling into the record head
        let drive_mult = 1.0 + 3.2 * sat;
        let drive_signal = fluxed * drive_mult;

        // Track peak for Drive meter
        let abs_drive = drive_signal.abs();
        self.drive_peak = self.drive_peak * 0.9992 + abs_drive * 0.0008;

        // 2. 2x Oversampling for core saturation and soft-clipping
        let (u0, u1) = self.oversampler.upsample2(drive_signal);

        let sat_sample = |x: f32| -> f32 {
            // Built-in diode soft-clipper active when Drive level exceeds 5 (x.abs() > ~0.45)
            // Tames sharp transients before magnetic core saturation as described in hardware user guide
            let x_clip = if x.abs() > 0.45 {
                let excess = x.abs() - 0.45;
                (0.45 + 0.35 * (excess / 0.35).tanh()) * x.signum()
            } else {
                x
            };

            let v = x_clip * 4.2;

            // Authentic Class-A single-ended quadratic asymmetry:
            // An even term (v^2 / (1 + |v|)) without sign inversion produces the rich 2nd harmonic (H2)
            // characteristic of Rupert Neve's discrete single-ended transistor stages.
            let alpha_even = 0.042 * (1.0 + 0.5 * sat);
            let even_term = alpha_even * (v * v / (1.0 + v.abs()));

            // Core magnetic saturation (odd harmonics H3)
            let core = v.tanh() + even_term;

            // Interstage head limiting
            let v_clip = 1.25;
            let abs_core = core.abs();
            if abs_core > v_clip {
                let excess = abs_core - v_clip;
                let soft = v_clip + 0.75 * (excess / 0.75).tanh();
                soft * core.signum()
            } else {
                core
            }
        };

        let y0 = sat_sample(u0);
        let y1 = sat_sample(u1);

        let down = self.oversampler.downsample2(y0, y1);
        let post_sat = down * (1.045 / 4.2);

        // 3. De-weighting & De-emphasis filters
        let de_fluxed = self.flux_deweight.process(post_sat);
        let de_emph = self.de_emphasis.process(de_fluxed);

        // 4. Replay amplifier recovery gain compensation
        // Precisely balances drive increase and core compression so 1 kHz fundamental stays rock solid
        let recovery_gain = 1.0 / (1.00 + 1.25 * sat.sqrt());
        let recovered = de_emph * recovery_gain;

        // 5. Tape Speed Equalization (head bump, sub shelf, high shelf)
        let bumped = self.head_bump.process(recovered);
        let subbed = self.sub_shelf.process(bumped);
        let shaped = self.high_shelf.process(subbed);

        shaped
    }
}
