//! Transformer / head magnetic-core model.
//!
//! The 542 builds its "record head → tape → replay head" with a custom
//! interstage transformer, and every stage is transformer coupled. All
//! three cores are modelled with the same physics, in the flux domain:
//!
//! 1. **Faraday's law.** A voltage-driven winding forces the core flux
//!    `Φ = ∫v dt / τ`. For a given voltage the flux grows as `1/f`, which is
//!    why the hardware's distortion rises towards low frequencies
//!    (RND AP plot: 0.16 % @ 1 kHz → 4.7 % @ 20 Hz at Saturation MIN).
//! 2. **Hysteresis.** Irreversible domain-wall motion is a rate-independent
//!    hysteresis operator. We use a discrete Preisach model in its
//!    Prandtl–Ishlinskii form: a weighted bank of *play* (backlash)
//!    operators with uniformly spread thresholds. With a uniform weight
//!    density this reproduces Rayleigh's law exactly — the loop area grows
//!    with the square of the flux amplitude, so the (mostly 3rd-harmonic)
//!    distortion is proportional to flux, i.e. to `1/f` — the slope measured
//!    on the hardware. It also gives the memory/phase-lag that a static
//!    waveshaper cannot.
//! 3. **Saturation.** The reversible + irreversible magnetisation is passed
//!    through a smooth anhysteretic saturation curve (`x/(1+|x|^p)^(1/p)`,
//!    a Langevin-like knee whose sharpness depends on the alloy).
//! 4. **Induction on the output winding.** The secondary voltage is the time
//!    derivative of the coupled flux. The integrator/differentiator pair is
//!    an exact discrete inverse, so with an ideal (linear) core the block is
//!    bit-transparent; every deviation is produced by the core physics.
//!
//! Units: voltages are in volts (0 dBu = 0.7746 V rms); flux is expressed
//! in "volt-equivalent at 1/τ rad/s", so `sat` and the play thresholds read
//! as the voltage amplitude at which a sine of `1/(2πτ)` Hz reaches them.

pub const PLAYS: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct CoreSpec {
    /// Flux time constant τ [s]: a sine of `1/(2πτ)` Hz has flux amplitude = voltage amplitude.
    pub tau: f64,
    /// Fraction of magnetisation carried by irreversible (hysteretic) wall motion.
    pub hyst: f64,
    /// Largest play threshold (flux units). Domains with larger coercivity never move.
    pub r_max: f64,
    /// Shape of the coercivity distribution: density ∝ (1 − r/r_max)^shape
    /// (0 = uniform → pure Rayleigh up to r_max; > 0 = fewer hard domains).
    pub shape: f64,
    /// Power-law tail of the coercivity density, ∝ r^(−power) (0 ≤ power < 1).
    /// Gives THD ∝ flux^(1 − power) over a wide range instead of Rayleigh's
    /// THD ∝ flux; soft steels show such broad coercivity distributions.
    pub power: f64,
    /// Saturation flux of the anhysteretic curve (flux units).
    pub sat: f64,
    /// Knee sharpness of the saturation curve (2 = soft silicon steel, 4+ = hard nickel).
    pub knee: f64,
    /// Corner of the flux leak (keeps DC from walking the core into saturation).
    pub leak_hz: f64,
    /// Number of play operators used (<= PLAYS); fewer is cheaper and
    /// adequate for cores whose loop is tiny.
    pub plays: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Core {
    spec: CoreSpec,
    dt_over_tau: f64,
    leak: f64,
    r: [f64; PLAYS],
    w: [f64; PLAYS],
    rev: f64,
    play: [f64; PLAYS],
    n: usize,
    /// Below this |m/sat| the saturation term is < 1e-7 and is skipped.
    lin_below: f64,
    phi: f64,
    b_prev: f64,
    /// Drive scaling applied to the flux before the core law (used by the
    /// record head: more DRIVE = more flux for the same replay level).
    pub flux_gain: f64,
    /// Remanent magnetisation (flux units): a core that has been hit with
    /// DC or a big transient keeps a small offset, which tilts the
    /// saturation curve and adds even harmonics (an un-demagnetised head).
    pub bias: f64,
}

impl Core {
    pub fn new(spec: CoreSpec, fs: f64) -> Self {
        let mut c = Core {
            spec,
            dt_over_tau: 0.0,
            leak: 0.0,
            r: [0.0; PLAYS],
            w: [0.0; PLAYS],
            rev: 1.0,
            play: [0.0; PLAYS],
            n: PLAYS,
            lin_below: 0.0,
            phi: 0.0,
            b_prev: 0.0,
            flux_gain: 1.0,
            bias: 0.0,
        };
        c.configure(spec, fs);
        c
    }

    pub fn configure(&mut self, spec: CoreSpec, fs: f64) {
        self.spec = spec;
        self.dt_over_tau = 1.0 / (fs * spec.tau);
        self.leak = (-2.0 * std::f64::consts::PI * spec.leak_hz / fs).exp();
        // Preisach density on (0, r_max], sampled on a quadratic grid so the
        // small-amplitude (Rayleigh) region is resolved finely: a uniform
        // density makes the loop area grow with amplitude² near zero.
        self.n = spec.plays.clamp(1, PLAYS);
        self.lin_below = (1e-7f64).powf(1.0 / spec.knee);
        let n = self.n as f64;
        let mut total = 0.0;
        self.w = [0.0; PLAYS];
        for k in 0..self.n {
            let e0 = spec.r_max * (k as f64 / n).powi(2);
            let e1 = spec.r_max * ((k + 1) as f64 / n).powi(2);
            let rc = 0.5 * (e0 + e1);
            self.r[k] = rc;
            self.w[k] = (e1 - e0)
                * (1.0 - rc / spec.r_max).max(0.0).powf(spec.shape)
                * (rc / spec.r_max).powf(-spec.power);
            total += self.w[k];
        }
        for w in &mut self.w {
            *w *= spec.hyst / total;
        }
        self.rev = 1.0 - spec.hyst;
    }

    pub fn reset(&mut self) {
        self.play = [0.0; PLAYS];
        self.phi = 0.0;
        self.b_prev = 0.0;
    }

    #[inline(always)]
    fn saturate(&self, m: f64) -> f64 {
        let s = self.spec.sat;
        let x = m / s;
        if x.abs() < self.lin_below {
            return m;
        }
        let p = self.spec.knee;
        let u = x.abs().powf(p);
        if u < 1e-3 {
            // (1+u)^(-1/p) to 2nd order: error < 1e-9, saves a powf.
            return s * x * (1.0 - u / p + (1.0 + p) * u * u / (2.0 * p * p));
        }
        s * x / (1.0 + u).powf(1.0 / p)
    }

    /// Coupled flux for a given winding flux (hysteresis + saturation), in flux units.
    #[inline(always)]
    fn magnetise(&mut self, phi: f64) -> f64 {
        let mut m = self.rev * phi;
        for k in 0..self.n {
            let r = self.r[k];
            let p = self.play[k].clamp(phi - r, phi + r);
            self.play[k] = p;
            m += self.w[k] * p;
        }
        if self.bias == 0.0 {
            return self.saturate(m);
        }
        self.saturate(m + self.bias) - self.saturate(self.bias)
    }

    /// One sample: winding voltage in, induced secondary voltage out.
    #[inline(always)]
    pub fn process(&mut self, v: f64) -> f64 {
        // Backward-Euler flux integration with a gentle leak ...
        self.phi = self.leak * self.phi + v * self.dt_over_tau;
        let g = self.flux_gain;
        // Normalised to the initial (small-signal) permeability: tiny signals
        // pass at unity, larger flux recruits domain walls (Rayleigh bloom).
        let b = self.magnetise(self.phi * g) / (g * self.rev);
        // ... and its exact inverse on the output winding.
        let out = (b - self.leak * self.b_prev) / self.dt_over_tau;
        self.b_prev = b;
        out
    }

    /// Current flux relative to saturation (for metering / diagnostics).
    pub fn flux_ratio(&self) -> f64 {
        (self.phi * self.flux_gain / self.spec.sat).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_core_is_transparent() {
        let spec = CoreSpec { tau: 1e-3, hyst: 0.0, r_max: 1.0, shape: 0.0, power: 0.0, sat: 1e9, knee: 2.0, leak_hz: 1.0, plays: PLAYS };
        let mut c = Core::new(spec, 96_000.0);
        let mut err = 0.0f64;
        for n in 0..50_000 {
            let x = (n as f64 * 0.01).sin() + 0.3 * (n as f64 * 0.13).sin();
            let y = c.process(x);
            err = err.max((y - x).abs());
        }
        assert!(err < 1e-9, "err={err}");
    }

    fn thd(c: &mut Core, f: f64, a: f64, fs: f64) -> f64 {
        let n = (fs / f * 60.0) as usize;
        let skip = n / 3;
        let (mut h1, mut h3) = ((0.0, 0.0), (0.0, 0.0));
        for i in 0..n {
            let t = i as f64 / fs;
            let y = c.process(a * (2.0 * std::f64::consts::PI * f * t).sin());
            if i >= skip {
                let w = 2.0 * std::f64::consts::PI * f * t;
                h1.0 += y * w.sin();
                h1.1 += y * w.cos();
                h3.0 += y * (3.0 * w).sin();
                h3.1 += y * (3.0 * w).cos();
            }
        }
        f64::hypot(h3.0, h3.1) / f64::hypot(h1.0, h1.1)
    }

    #[test]
    fn rayleigh_distortion_scales_with_flux() {
        // Halving frequency doubles flux; in the Rayleigh region H3 must ~double.
        let spec = CoreSpec { tau: 1e-3, hyst: 0.1, r_max: 5.0, shape: 0.0, power: 0.0, sat: 1e3, knee: 2.0, leak_hz: 0.5, plays: PLAYS };
        let fs = 96_000.0;
        let h_100 = thd(&mut Core::new(spec, fs), 100.0, 0.5, fs);
        let h_50 = thd(&mut Core::new(spec, fs), 50.0, 0.5, fs);
        let ratio = h_50 / h_100;
        assert!(ratio > 1.6 && ratio < 2.4, "ratio={ratio} h50={h_50} h100={h_100}");
    }
}
