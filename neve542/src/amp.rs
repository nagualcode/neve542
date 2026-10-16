//! Active stages: single-ended Class-A amplifiers and the diode soft clip.

/// Single-ended Class-A gain stage (Portico / 542 "TLA" style).
///
/// A single-sided stage has no crossover region; its residual curvature is
/// the transistor's exponential transfer, which is *asymmetric* and so
/// produces mainly the 2nd harmonic, growing with level (`H2 ≈ a2·A/2`).
/// Near the ±16 V 500-series rails it runs out of swing differently on each
/// half-cycle: the positive excursion ends softly as the device approaches
/// cut-off, the negative one more abruptly in saturation. Both edges are
/// modelled with smooth knees so the clip spectrum stays band-limited.
#[derive(Clone, Copy, Debug)]
pub struct ClassA {
    /// Quadratic (H2) coefficient [1/V].
    pub a2: f64,
    /// Positive / negative swing limits [V].
    pub v_pos: f64,
    pub v_neg: f64,
    /// Knee sharpness of each rail.
    pub k_pos: f64,
    pub k_neg: f64,
}

impl ClassA {
    /// Input / blend amplifiers: 0.0025 % THD @ +20 dBu, max out ≈ +23.5 dBu.
    pub const LINE: ClassA = ClassA { a2: 4.5e-6, v_pos: 15.2, v_neg: 15.8, k_pos: 18.0, k_neg: 26.0 };

    #[inline(always)]
    fn rail(x: f64, v: f64, k: f64) -> f64 {
        let r = (x / v).abs();
        if r < 0.4 {
            return x; // (0.4)^18 < 1e-7: the rail is not in play
        }
        x / (1.0 + r.powf(k)).powf(1.0 / k)
    }

    /// Curvature-only transfer (no rail clipping), for the feedback residual.
    #[inline(always)]
    pub fn curvature(&self, x: f64) -> f64 {
        x + self.a2 * x * x
    }

    #[inline(always)]
    pub fn process(&self, x: f64) -> f64 {
        let y = self.curvature(x);
        if y >= 0.0 { Self::rail(y, self.v_pos, self.k_pos) } else { Self::rail(y, self.v_neg, self.k_neg) }
    }
}

/// Diode soft clip of the record (DRIVE) amplifier.
///
/// Series resistor into an anti-parallel diode pair, then made up by the
/// following gain: `(v_in − v)/R = Is₁(e^{v/nVt} − 1) − Is₂(e^{−v/nVt} − 1)`.
/// The two junctions are not identical (`Is₂ ≠ Is₁`), so the knee is a few
/// tens of millivolts lopsided and the clip adds 2nd harmonic on top of the
/// 3rd — Sound On Sound measured exactly that on the 542 ("soft-clipping
/// that produces second-order harmonics at higher levels").
///
/// Solved per sample with damped Newton iterations warm-started from the
/// previous sample (converges in 2–3 steps at audio rates).
#[derive(Clone, Copy, Debug)]
pub struct DiodeClipper {
    r: f64,
    is1: f64,
    is2: f64,
    nvt: f64,
    /// Amplifier-side scaling: circuit volts → diode node volts.
    pub att: f64,
    v: f64,
}

impl DiodeClipper {
    pub fn new(att: f64) -> Self {
        DiodeClipper { r: 10_000.0, is1: 2.52e-9, is2: 1.1e-9, nvt: 1.752 * 0.025_85, att, v: 0.0 }
    }

    pub fn reset(&mut self) {
        self.v = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let vin = x * self.att;
        let mut v = self.v.clamp(-1.2, 1.2);
        // Initial guess: the smaller of the input and the last solution keeps
        // Newton on the convex side of the exponential.
        if v.abs() > vin.abs() {
            v = vin;
        }
        let inv_r = 1.0 / self.r;
        for _ in 0..6 {
            let e1 = (v / self.nvt).clamp(-40.0, 40.0).exp();
            let e2 = 1.0 / e1;
            let f = (vin - v) * inv_r - self.is1 * (e1 - 1.0) + self.is2 * (e2 - 1.0);
            let df = -inv_r - (self.is1 * e1 + self.is2 * e2) / self.nvt;
            let step = f / df;
            let step = step.clamp(-0.1, 0.1);
            v -= step;
            if step.abs() < 1e-9 {
                break;
            }
        }
        self.v = v;
        v / self.att
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_a_line_stage_spec() {
        // 0.0025 % THD at +20 dBu (11 V peak): H2 = a2*A/2.
        let a = 11.0;
        let h2 = ClassA::LINE.a2 * a / 2.0;
        assert!(h2 < 0.000_03, "h2={h2}");
        // Max output just above +23 dBu (15.4 V peak) before the knee bites hard.
        let y = ClassA::LINE.process(13.0);
        assert!((y - 13.0).abs() / 13.0 < 0.03, "y={y}");
    }

    #[test]
    fn diode_clip_is_transparent_at_low_level_and_limits_high() {
        let mut d = DiodeClipper::new(0.13);
        let y = d.process(0.5);
        assert!((y - 0.5).abs() < 0.01, "y={y}");
        let mut d = DiodeClipper::new(0.13);
        let y = d.process(20.0);
        assert!(y < 8.0, "y={y}");
    }
}
