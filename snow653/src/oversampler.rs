//! Zero-latency polyphase IIR half-band oversampling (2x / 4x).
//!
//! Each 2x stage is a pair of allpass chains (Valenzuela–Constantinides
//! structure). Coefficients are computed at construction time with the
//! elliptic design used by Laurent de Soras' *hiir*, so the transition band
//! can be chosen per stage: the first stage is steep (it protects the audio
//! band), the second one can be lazy (everything above 2·fs is already
//! inaudible headroom). Analog hardware is minimum-phase anyway, so the
//! IIR phase response is not a fidelity concern and no latency is reported.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
struct HalfBand<const N: usize> {
    coef: [f64; N],
    x: [f64; N],
    y: [f64; N],
}

impl<const N: usize> HalfBand<N> {
    fn new(transition: f64) -> Self {
        HalfBand { coef: design(N, transition), x: [0.0; N], y: [0.0; N] }
    }

    fn reset(&mut self) {
        self.x = [0.0; N];
        self.y = [0.0; N];
    }

    /// Runs the even-indexed chain on `a` and the odd-indexed chain on `b`.
    #[inline(always)]
    fn run(&mut self, mut a: f64, mut b: f64) -> (f64, f64) {
        let mut i = 0;
        while i + 1 < N {
            let (t0, t1) = (self.x[i], self.x[i + 1]);
            self.x[i] = a;
            self.x[i + 1] = b;
            a = (a - self.y[i]) * self.coef[i] + t0;
            b = (b - self.y[i + 1]) * self.coef[i + 1] + t1;
            self.y[i] = a;
            self.y[i + 1] = b;
            i += 2;
        }
        if i < N {
            let t0 = self.x[i];
            self.x[i] = a;
            a = (a - self.y[i]) * self.coef[i] + t0;
            self.y[i] = a;
        }
        (a, b)
    }

    #[inline(always)]
    fn up(&mut self, x: f64) -> (f64, f64) {
        self.run(x, x)
    }

    #[inline(always)]
    fn down(&mut self, s0: f64, s1: f64) -> f64 {
        // Newest sample goes through the even chain (hiir convention).
        let (a, b) = self.run(s1, s0);
        0.5 * (a + b)
    }
}

/// hiir `PolyphaseIir2Designer::compute_coefs_spec_order_tbw`.
fn design<const N: usize>(nbr: usize, transition: f64) -> [f64; N] {
    let mut k = ((1.0 - transition * 2.0) * PI / 4.0).tan();
    k *= k;
    let kksqrt = (1.0 - k * k).powf(0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e4 = e * e * e * e;
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let order = (nbr * 2 + 1) as f64;
    let mut out = [0.0; N];
    for (idx, c_out) in out.iter_mut().enumerate() {
        let c = (idx + 1) as f64;
        let mut num = 0.0;
        let mut i = 0i32;
        let mut j = 1.0;
        loop {
            let t = q.powi(i * (i + 1)) * (((i * 2 + 1) as f64) * c * PI / order).sin() * j;
            num += t;
            j = -j;
            i += 1;
            if t.abs() < 1e-100 || i > 100 {
                break;
            }
        }
        let mut den = 0.0;
        let mut i = 1i32;
        let mut j = -1.0;
        loop {
            let t = q.powi(i * i) * ((i * 2) as f64 * c * PI / order).cos() * j;
            den += t;
            j = -j;
            i += 1;
            if t.abs() < 1e-100 || i > 100 {
                break;
            }
        }
        let ww = num * q.powf(0.25) / (den + 0.5);
        let wwsq = ww * ww;
        let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
        *c_out = (1.0 - x) / (1.0 + x);
    }
    out
}

pub const MAX_FACTOR: usize = 4;

/// 1x / 2x / 4x oversampler chosen from the host sample rate so the
/// non-linear stages always run at >= 176.4 kHz.
#[derive(Clone, Copy, Debug)]
pub struct Oversampler {
    factor: usize,
    up1: HalfBand<10>,
    up2: HalfBand<4>,
    dn1: HalfBand<10>,
    dn2: HalfBand<4>,
}

impl Oversampler {
    pub fn new(base_sr: f64) -> Self {
        let factor = if base_sr < 60_000.0 {
            4
        } else if base_sr < 120_000.0 {
            2
        } else {
            1
        };
        Oversampler {
            factor,
            // Stage 1: passband to 0.4535·fs (20 kHz @ 44.1k), > 100 dB rejection.
            up1: HalfBand::new(0.0233),
            dn1: HalfBand::new(0.0233),
            // Stage 2 only has to reject images above fs·1.55 of the 2x rate.
            up2: HalfBand::new(0.20),
            dn2: HalfBand::new(0.20),
        }
    }

    pub fn factor(&self) -> usize {
        self.factor
    }

    pub fn reset(&mut self) {
        self.up1.reset();
        self.up2.reset();
        self.dn1.reset();
        self.dn2.reset();
    }

    /// Upsample one base-rate sample into `factor` samples.
    #[inline(always)]
    pub fn up(&mut self, x: f64, out: &mut [f64; MAX_FACTOR]) {
        match self.factor {
            1 => out[0] = x,
            2 => {
                let (a, b) = self.up1.up(x);
                out[0] = a;
                out[1] = b;
            }
            _ => {
                let (a, b) = self.up1.up(x);
                let (a0, a1) = self.up2.up(a);
                let (b0, b1) = self.up2.up(b);
                *out = [a0, a1, b0, b1];
            }
        }
    }

    /// Decimate `factor` oversampled samples into one base-rate sample.
    #[inline(always)]
    pub fn down(&mut self, s: &[f64; MAX_FACTOR]) -> f64 {
        match self.factor {
            1 => s[0],
            2 => self.dn1.down(s[0], s[1]),
            _ => {
                let a = self.dn2.down(s[0], s[1]);
                let b = self.dn2.down(s[2], s[3]);
                self.dn1.down(a, b)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_gain(f: f64, fs: f64) -> f64 {
        let mut os = Oversampler::new(fs);
        let n = (fs * 0.5) as usize;
        let mut buf = [0.0; MAX_FACTOR];
        let mut acc = 0.0;
        let mut cnt = 0;
        for i in 0..n {
            let x = (2.0 * PI * f * i as f64 / fs).sin();
            os.up(x, &mut buf);
            let y = os.down(&buf);
            if i > n / 2 {
                acc += y * y;
                cnt += 1;
            }
        }
        (2.0 * acc / cnt as f64).sqrt()
    }

    #[test]
    fn passband_is_flat_to_20k() {
        for fs in [44_100.0, 48_000.0, 96_000.0] {
            for f in [100.0, 1000.0, 10_000.0, 18_000.0, 20_000.0] {
                let g = 20.0 * roundtrip_gain(f, fs).log10();
                assert!(g.abs() < 0.05, "fs={fs} f={f} gain={g} dB");
            }
        }
    }

    #[test]
    fn images_are_rejected() {
        // An upsampled tone must not leave energy at its images (fs±f, 2fs-f).
        let fs = 48_000.0;
        let f = 19_000.0;
        let osr = fs * 4.0;
        for fi in [fs - f, fs + f, 2.0 * fs - f] {
            let mut os = Oversampler::new(fs);
            let n = 1 << 15;
            let mut acc = (0.0f64, 0.0f64);
            let mut buf = [0.0; MAX_FACTOR];
            let mut k = 0usize;
            for i in 0..n {
                os.up((2.0 * PI * f * i as f64 / fs).sin(), &mut buf);
                for v in buf.iter() {
                    let t = k as f64 / osr;
                    if i > 1000 {
                        acc.0 += v * (2.0 * PI * fi * t).cos();
                        acc.1 += v * (2.0 * PI * fi * t).sin();
                    }
                    k += 1;
                }
            }
            let mag = 2.0 * acc.0.hypot(acc.1) / ((n - 1001) * 4) as f64;
            let db = 20.0 * mag.log10();
            assert!(db < -90.0, "image at {fi} Hz: {db} dB");
        }
    }
}
