//! Rupert Neve SILK Saturation & Texture Circuit.
//! Models the Class-A transformer negative feedback manipulation (Off, Red, Blue).

use crate::biquad::{Biquad, Coefficients};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilkMode {
    Off,
    Red,
    Blue,
}

#[derive(Clone, Copy, Debug)]
pub struct SilkStage {
    mode: SilkMode,
    texture: f32, // 0.0 .. 1.0
    sr: f32,

    // EQ shaping for Red mode (presence shelf + subtle low dip)
    red_high_shelf: Biquad,
    red_low_shelf: Biquad,

    // EQ shaping for Blue mode (warm low shelf)
    blue_low_shelf: Biquad,

    // Filter for Silk harmonic extraction / weighting
    harmonic_filter: Biquad,
}

impl Default for SilkStage {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl SilkStage {
    pub fn new(sr: f32) -> Self {
        let mut s = Self {
            mode: SilkMode::Off,
            texture: 0.0,
            sr,
            red_high_shelf: Biquad::default(),
            red_low_shelf: Biquad::default(),
            blue_low_shelf: Biquad::default(),
            harmonic_filter: Biquad::default(),
        };
        s.update_filters();
        s
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.update_filters();
    }

    pub fn set_mode_and_texture(&mut self, mode: SilkMode, texture: f32) {
        let texture = texture.clamp(0.0, 1.0);
        if self.mode != mode || (self.texture - texture).abs() > 0.001 {
            self.mode = mode;
            self.texture = texture;
            self.update_filters();
        }
    }

    pub fn reset(&mut self) {
        self.red_high_shelf.reset();
        self.red_low_shelf.reset();
        self.blue_low_shelf.reset();
        self.harmonic_filter.reset();
    }

    fn update_filters(&mut self) {
        let t = self.texture;
        match self.mode {
            SilkMode::Off => {
                self.red_high_shelf.coeffs = Coefficients::identity();
                self.red_low_shelf.coeffs = Coefficients::identity();
                self.blue_low_shelf.coeffs = Coefficients::identity();
                self.harmonic_filter.coeffs = Coefficients::identity();
            }
            SilkMode::Red => {
                // High shelf boost up to +2.85 dB at 100% texture
                let hi_gain = 2.85 * t;
                // Low shelf dip down to -2.1 dB at 100% texture
                let lo_gain = -2.10 * t;
                self.red_high_shelf.coeffs = Coefficients::high_shelf(3500.0, hi_gain, 0.707, self.sr);
                self.red_low_shelf.coeffs = Coefficients::low_shelf(100.0, lo_gain, 0.707, self.sr);

                // Upper-mid / high bandpass for Silk Red harmonic excitation
                self.harmonic_filter.coeffs = Coefficients::peaking(4500.0, 6.0, 0.8, self.sr);
            }
            SilkMode::Blue => {
                // Warm low shelf boost up to +0.35 dB at 100% texture
                let lo_gain = 0.35 * t;
                self.blue_low_shelf.coeffs = Coefficients::low_shelf(80.0, lo_gain, 0.707, self.sr);

                // Low-mid peaking filter for Silk Blue harmonic excitation
                self.harmonic_filter.coeffs = Coefficients::peaking(120.0, 6.0, 0.8, self.sr);
            }
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.mode == SilkMode::Off || self.texture < 0.001 {
            return input;
        }

        let t = self.texture;
        match self.mode {
            SilkMode::Off => input,
            SilkMode::Red => {
                // EQ frequency response shaping
                let eq_sig = self.red_low_shelf.process(self.red_high_shelf.process(input));

                // Harmonic distortion injection based on Rupert Neve Patent US 7,454,025:
                // Reduced negative feedback at high frequencies allows natural transformer + Class-A
                // distortion to emerge (predominantly musical 2nd harmonic, with complementary 3rd harmonic).
                let side = self.harmonic_filter.process(input);
                let x = side * (0.8 + 1.2 * t);
                // Genuine even-order quadratic Class-A term (produces true H2 2nd harmonic)
                let h2 = 0.045 * t * (x * x / (1.0 + x.abs()));
                // Odd-order transformer core magnetic term (produces H3 3rd harmonic)
                let h3 = 0.065 * t * (x * x * x) / (1.0 + x * x);
                let harmonics = h2 + h3;

                eq_sig + harmonics * 0.15
            }
            SilkMode::Blue => {
                let eq_sig = self.blue_low_shelf.process(input);

                // Low harmonic content based on US 7,454,025:
                // Reduced negative feedback in low frequencies allows transformer core bloom.
                let side = self.harmonic_filter.process(input);
                let x = side * (0.8 + 1.2 * t);
                let h2 = 0.040 * t * (x * x / (1.0 + x.abs()));
                let h3 = 0.060 * t * (x * x * x) / (1.0 + x * x);
                let harmonics = h2 + h3;

                eq_sig + harmonics * 0.15
            }
        }
    }
}
