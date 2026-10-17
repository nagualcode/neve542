//! Linear filters designed from analog (s-domain) prototypes.
//!
//! Every network in the 542 model is described the way it exists on the
//! circuit board: as a rational transfer function in `s`. The digital
//! filter is obtained with the bilinear transform, pre-warped at the
//! frequency that matters most for that network, and runs in Direct Form II
//! Transposed with `f64` state (sub-10 Hz corners at 192 kHz need it).

use std::f64::consts::PI;

/// Second-order analog prototype `(b0 + b1 s + b2 s²) / (a0 + a1 s + a2 s²)`
/// with `s` in rad/s.
#[derive(Clone, Copy, Debug)]
pub struct Analog {
    pub b: [f64; 3],
    pub a: [f64; 3],
}

impl Analog {
    pub const IDENTITY: Analog = Analog { b: [1.0, 0.0, 0.0], a: [1.0, 0.0, 0.0] };

    pub fn gain(g: f64) -> Self {
        Analog { b: [g, 0.0, 0.0], a: [1.0, 0.0, 0.0] }
    }

    /// 2nd-order high-pass, corner `fc`, quality `q`.
    pub fn hp2(fc: f64, q: f64) -> Self {
        let w = 2.0 * PI * fc;
        Analog { b: [0.0, 0.0, 1.0], a: [w * w, w / q, 1.0] }
    }

    /// 2nd-order low-pass, corner `fc`, quality `q`.
    pub fn lp2(fc: f64, q: f64) -> Self {
        let w = 2.0 * PI * fc;
        Analog { b: [w * w, 0.0, 0.0], a: [w * w, w / q, 1.0] }
    }

    /// 1st-order high-pass.
    pub fn hp1(fc: f64) -> Self {
        let w = 2.0 * PI * fc;
        Analog { b: [0.0, 1.0, 0.0], a: [w, 1.0, 0.0] }
    }

    /// 1st-order low-pass.
    pub fn lp1(fc: f64) -> Self {
        let w = 2.0 * PI * fc;
        Analog { b: [w, 0.0, 0.0], a: [w, 1.0, 0.0] }
    }

    /// Constant-Q peaking section (RBJ analog prototype).
    pub fn peak(fc: f64, gain_db: f64, q: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * PI * fc;
        Analog { b: [w * w, w * a / q, 1.0], a: [w * w, w / (a * q), 1.0] }
    }

    /// RC emphasis network `(1 + s·t1) / (1 + s·t2)` (time constants in s).
    /// `t1 > t2` boosts high frequencies (pre-emphasis), the reciprocal cuts.
    pub fn emphasis(t1: f64, t2: f64) -> Self {
        Analog { b: [1.0, t1, 0.0], a: [1.0, t2, 0.0] }
    }

    /// 1st-order shelf (zero at `fz`, pole at `fp`, unity at DC).
    pub fn shelf1(fz: f64, fp: f64) -> Self {
        Self::emphasis(1.0 / (2.0 * PI * fz), 1.0 / (2.0 * PI * fp))
    }

    pub fn inverse(&self) -> Self {
        Analog { b: self.a, a: self.b }
    }

    /// Bilinear transform, pre-warped at `fw` (clamped below Nyquist).
    pub fn to_digital(&self, fs: f64, fw: f64) -> Coefs {
        let fw = fw.clamp(1.0, 0.42 * fs);
        let w = 2.0 * PI * fw;
        let k = w / (w / (2.0 * fs)).tan();
        let [b0, b1, b2] = self.b;
        let [a0, a1, a2] = self.a;
        if b2 == 0.0 && a2 == 0.0 {
            // First-order section: keep it first order (no z = -1 pole/zero pair).
            let n0 = b0 + b1 * k;
            let n1 = b0 - b1 * k;
            let d0 = a0 + a1 * k;
            let d1 = a0 - a1 * k;
            return Coefs { b0: n0 / d0, b1: n1 / d0, b2: 0.0, a1: d1 / d0, a2: 0.0 };
        }
        let k2 = k * k;
        let n0 = b0 + b1 * k + b2 * k2;
        let n1 = 2.0 * (b0 - b2 * k2);
        let n2 = b0 - b1 * k + b2 * k2;
        let d0 = a0 + a1 * k + a2 * k2;
        let d1 = 2.0 * (a0 - a2 * k2);
        let d2 = a0 - a1 * k + a2 * k2;
        Coefs { b0: n0 / d0, b1: n1 / d0, b2: n2 / d0, a1: d1 / d0, a2: d2 / d0 }
    }

    /// Magnitude of the analog response at `f` Hz (used by tests/calibration).
    pub fn mag(&self, f: f64) -> f64 {
        let w = 2.0 * PI * f;
        let num = (self.b[0] - self.b[2] * w * w).hypot(self.b[1] * w);
        let den = (self.a[0] - self.a[2] * w * w).hypot(self.a[1] * w);
        num / den
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Coefs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Default for Coefs {
    fn default() -> Self {
        Coefs { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 }
    }
}

/// Biquad in Direct Form II Transposed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    pub c: Coefs,
    s1: f64,
    s2: f64,
}

impl Biquad {
    pub fn new(c: Coefs) -> Self {
        Biquad { c, s1: 0.0, s2: 0.0 }
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let c = &self.c;
        let y = c.b0 * x + self.s1;
        self.s1 = c.b1 * x - c.a1 * y + self.s2;
        self.s2 = c.b2 * x - c.a2 * y;
        y
    }

    /// Instantaneous gain (`y = g·x + state`), for zero-delay loop solving.
    #[inline(always)]
    pub fn g(&self) -> f64 {
        self.c.b0
    }
}

/// A small fixed-size cascade of analog-designed sections.
#[derive(Clone, Copy, Debug)]
pub struct Cascade<const N: usize> {
    pub stages: [Biquad; N],
}

impl<const N: usize> Default for Cascade<N> {
    fn default() -> Self {
        Cascade { stages: [Biquad::default(); N] }
    }
}

impl<const N: usize> Cascade<N> {
    pub fn set(&mut self, protos: &[(Analog, f64); N], fs: f64) {
        for (st, (p, fw)) in self.stages.iter_mut().zip(protos.iter()) {
            st.c = p.to_digital(fs, *fw);
        }
    }

    pub fn reset(&mut self) {
        for s in &mut self.stages {
            s.reset();
        }
    }

    #[inline(always)]
    pub fn process(&mut self, mut x: f64) -> f64 {
        for s in &mut self.stages {
            x = s.process(x);
        }
        x
    }
}

/// One-pole smoother for control signals (per-sample, allocation free).
#[derive(Clone, Copy, Debug, Default)]
pub struct Smoother {
    pub value: f64,
    coef: f64,
}

impl Smoother {
    pub fn new(time_s: f64, fs: f64, init: f64) -> Self {
        let mut s = Smoother { value: init, coef: 0.0 };
        s.set_time(time_s, fs);
        s
    }

    pub fn set_time(&mut self, time_s: f64, fs: f64) {
        self.coef = (-1.0 / (time_s * fs)).exp();
    }

    #[inline(always)]
    pub fn next(&mut self, target: f64) -> f64 {
        self.value = target + (self.value - target) * self.coef;
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digital_mag(c: &Coefs, f: f64, fs: f64) -> f64 {
        let w = 2.0 * PI * f / fs;
        let (z1r, z1i) = (w.cos(), -w.sin());
        let (z2r, z2i) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let nr = c.b0 + c.b1 * z1r + c.b2 * z2r;
        let ni = c.b1 * z1i + c.b2 * z2i;
        let dr = 1.0 + c.a1 * z1r + c.a2 * z2r;
        let di = c.a1 * z1i + c.a2 * z2i;
        nr.hypot(ni) / dr.hypot(di)
    }

    #[test]
    fn bilinear_matches_analog_in_band() {
        let fs = 192_000.0;
        for p in [Analog::hp2(8.0, 0.6), Analog::peak(60.0, 6.0, 0.3), Analog::lp2(40e3, 0.5)] {
            let c = p.to_digital(fs, 1000.0);
            for f in [10.0, 60.0, 1000.0, 10_000.0, 20_000.0] {
                let a = 20.0 * p.mag(f).log10();
                let d = 20.0 * digital_mag(&c, f, fs).log10();
                assert!((a - d).abs() < 0.15, "f={f} analog={a} digital={d}");
            }
        }
    }

    #[test]
    fn emphasis_pair_is_transparent() {
        let fs = 96_000.0;
        let e = Analog::emphasis(35e-6, 5e-6);
        let mut pre = Biquad::new(e.to_digital(fs, 4500.0));
        let mut de = Biquad::new(e.inverse().to_digital(fs, 4500.0));
        let mut err = 0.0f64;
        for n in 0..20000 {
            let x = (n as f64 * 0.37).sin() * 0.5;
            let y = de.process(pre.process(x));
            if n > 100 {
                err = err.max((y - x).abs());
            }
        }
        assert!(err < 1e-9, "err={err}");
    }
}
