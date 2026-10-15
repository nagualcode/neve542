//! Precision biquad filters using Direct Form II Transposed structure.

#[derive(Clone, Copy, Debug)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for Coefficients {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

impl Coefficients {
    pub fn identity() -> Self {
        Self::default()
    }

    pub fn peaking(f0: f32, gain_db: f32, q: f32, sr: f32) -> Self {
        let f0 = f0.clamp(10.0, sr * 0.49);
        let q = q.max(0.01);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sr;
        let a_gain = 10.0f32.powf(gain_db / 40.0);
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = 1.0 + alpha * a_gain;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a_gain;
        let a0 = 1.0 + alpha / a_gain;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a_gain;

        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    pub fn low_shelf(f0: f32, gain_db: f32, q: f32, sr: f32) -> Self {
        let f0 = f0.clamp(10.0, sr * 0.49);
        let q = q.max(0.01);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sr;
        let a_gain = 10.0f32.powf(gain_db / 40.0);
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let two_sqrt_a_alpha = 2.0 * a_gain.sqrt() * alpha;

        let b0 = a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a_gain * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0);
        let b2 = a_gain * ((a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0);
        let a2 = (a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha;

        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    pub fn high_shelf(f0: f32, gain_db: f32, q: f32, sr: f32) -> Self {
        let f0 = f0.clamp(10.0, sr * 0.49);
        let q = q.max(0.01);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sr;
        let a_gain = 10.0f32.powf(gain_db / 40.0);
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let two_sqrt_a_alpha = 2.0 * a_gain.sqrt() * alpha;

        let b0 = a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a_gain * ((a_gain - 1.0) + (a_gain + 1.0) * cos_w0);
        let b2 = a_gain * ((a_gain + 1.0) + (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a_gain - 1.0) - (a_gain + 1.0) * cos_w0);
        let a2 = (a_gain + 1.0) - (a_gain - 1.0) * cos_w0 - two_sqrt_a_alpha;

        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    pub fn high_pass(f0: f32, q: f32, sr: f32) -> Self {
        let f0 = f0.clamp(1.0, sr * 0.49);
        let q = q.max(0.01);
        let w0 = 2.0 * std::f32::consts::PI * f0 / sr;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = (1.0 + cos_w0) * 0.5;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) * 0.5;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    pub fn invert(&self) -> Self {
        let inv_b0 = 1.0 / self.b0;
        Self {
            b0: inv_b0,
            b1: self.a1 * inv_b0,
            b2: self.a2 * inv_b0,
            a1: self.b1 * inv_b0,
            a2: self.b2 * inv_b0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    pub coeffs: Coefficients,
    pub s1: f32,
    pub s2: f32,
}

impl Biquad {
    pub fn new(coeffs: Coefficients) -> Self {
        Self {
            coeffs,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.coeffs.b0 * x + self.s1;
        self.s1 = self.coeffs.b1 * x - self.coeffs.a1 * y + self.s2;
        self.s2 = self.coeffs.b2 * x - self.coeffs.a2 * y;
        // Avoid denormals
        if self.s1.abs() < 1e-25 {
            self.s1 = 0.0;
        }
        if self.s2.abs() < 1e-25 {
            self.s2 = 0.0;
        }
        y
    }
}
