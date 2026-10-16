//! The 542 "True Tape" circuit, following the RND block diagram:
//!
//! ```text
//! SAT ENGAGE → DRIVE amp (pre-emphasis selected by IPS, diode soft clip)
//!            → SATURATION TRANSFORMER ("record head", Drive meter here)
//!            → RECOVERY amp (de-emphasis + IPS replay contour)
//! ```
//!
//! DRIVE and RECOVERY are a counter-ganged dual pot (dotted line on the
//! block diagram): turning SATURATION up pushes more flux into the head
//! while the replay gain drops by the same amount, so the level stays put
//! and only the *amount of magnetic non-linearity* changes.
//!
//! The replay contour (sub-bass loss, head bump, HF gap loss) was fitted to
//! the published Audio Precision plots at the four documented pot positions
//! (MIN, 10 o'clock, 2 o'clock, MAX), see `tools/fit/fit_tape_eq.py`.

use crate::amp::DiodeClipper;
use crate::filters::{Analog, Biquad, Cascade, Smoother};
use crate::magnetics::{Core, CoreSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapeSpeed {
    Ips15,
    Ips30,
}

/// Replay contour parameters at one pot position:
/// `[fh, Qh, f_bump, G_bump(dB), Q_bump, f_lp, Q_lp, f_shelf, G_shelf(dB)]`.
/// The 1st-order HF shelf compensates the clip's compression of the
/// pre-emphasised highs at the 0 dBu reference level (≈ 0 dB below MAX).
pub type Contour = [f64; 9];

/// All tunable constants of the tape path, kept together so the probe
/// (`examples/probe.rs`) can override them during calibration.
#[derive(Clone, Copy, Debug)]
pub struct TapeCal {
    /// Pot positions at which contours/drive were measured.
    /// Contours: `tools/fit/refit_contour.py` (non-linear model in the loop);
    /// everything non-linear: `tools/fit/fit_thd.py`. Run both with
    /// `tools/fit/calibrate.sh`.
    pub knots: [f64; 4],
    /// DRIVE gain (dB) at each knot; RECOVERY is its inverse. The Drive meter
    /// (3 dB LEDs) reads 2, 2, 3, 6 at the knots, so each gain is fitted
    /// inside its LED window.
    pub drive_db: [f64; 4],
    pub contour15: [Contour; 4],
    pub contour30: [Contour; 4],
    /// Record pre-emphasis time constants (zero, pole) per speed.
    pub emph15: (f64, f64),
    pub emph30: (f64, f64),
    /// Diode clip input attenuation (sets the "Drive > 5" threshold).
    pub clip_att: f64,
    /// Record head / interstage core.
    pub head: CoreSpec,
    /// Curvature of the replay amplifier (broadband H2 floor) [1/V].
    pub a2_rep: f64,
    /// Share (in dB) of the contour's LF bump realised in the record path,
    /// i.e. in the flux. 0 = all in replay, 1 = all before the core.
    pub bump_pre: f64,
}

impl Default for TapeCal {
    fn default() -> Self {
        TapeCal {
            knots: [0.0, 0.3, 0.7, 1.0],
            // Drive meter reads 2, 2, 3, 6 at these positions (RND AP plot legend).
            drive_db: [0.0, 2.99, 5.99, 13.87],
            contour15: [
                [7.08629, 0.551993, 66.8413, 5.35447, 0.319849, 60023.5, 0.3, 6336.91, 0.172814],
                [5.96891, 0.479335, 65.6721, 4.27497, 0.34493, 60231.8, 0.3, 31951.9, -1.0],
                [6.61846, 0.526207, 62.9143, 3.03521, 0.388997, 55380.6, 0.3, 24347.9, -0.999878],
                [4.5813, 0.3, 64.9256, 2.00928, 0.523055, 50995.7, 0.3, 5707.2, 2.33868],
            ],
            contour30: [
                [5.51912, 0.300984, 130.333, 3.88642, 0.281762, 161705.0, 0.300001, 2000.0, 0.141805],
                [5.72655, 0.3, 136.411, 3.07236, 0.311961, 66318.0, 0.665242, 3506.66, 0.542125],
                [5.2773, 0.3, 146.714, 2.06216, 0.382446, 58309.1, 0.715021, 2000.0, 0.388053],
                [5.31566, 0.3, 162.065, 1.28465, 0.378831, 44102.6, 0.900932, 3105.33, 0.533262],
            ],
            // NAB 15 ips (50 µs) and AES 30 ips (17.5 µs) record pre-emphasis,
            // shelved by the drive amplifier's feedback network; the 15 ips
            // shelf and the clip threshold are fitted to the Saturation MAX
            // THD rise above 1 kHz together with its frequency response.
            emph15: (50e-6, 2.924e-05),
            emph30: (17.5e-6, 12e-6),
            clip_att: 0.05073,
            // Fitted by tools/fit/fit_thd.py to all four THD curves. Flux unit:
            // 0 dBu (1.0954 V peak) at 100 Hz, so tau = 1/(2π·100).
            head: CoreSpec {
                tau: 1.0 / (2.0 * std::f64::consts::PI * 100.0),
                hyst: 0.25111,
                r_max: 14.003,
                shape: 4.0,
                power: 0.055807,
                sat: 56.562,
                knee: 1.7336,
                leak_hz: 0.5,
                plays: crate::magnetics::PLAYS,
            },
            a2_rep: 1.6e-3,
            bump_pre: 0.3985,
        }
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl TapeCal {
    fn segment(&self, p: f64) -> (usize, f64) {
        let p = p.clamp(0.0, 1.0);
        let mut i = 0;
        while i < 2 && p > self.knots[i + 1] {
            i += 1;
        }
        let t = (p - self.knots[i]) / (self.knots[i + 1] - self.knots[i]);
        (i, t.clamp(0.0, 1.0))
    }

    pub fn drive_db_at(&self, p: f64) -> f64 {
        let (i, t) = self.segment(p);
        lerp(self.drive_db[i], self.drive_db[i + 1], t)
    }

    pub fn contour_at(&self, speed: TapeSpeed, p: f64) -> Contour {
        let tab = match speed {
            TapeSpeed::Ips15 => &self.contour15,
            TapeSpeed::Ips30 => &self.contour30,
        };
        let (i, t) = self.segment(p);
        let (a, b) = (&tab[i], &tab[i + 1]);
        let mut c = [0.0; 9];
        for k in 0..9 {
            // Frequencies interpolate geometrically, the rest linearly.
            c[k] = if k == 0 || k == 2 || k == 5 || k == 7 {
                (a[k].ln() + (b[k].ln() - a[k].ln()) * t).exp()
            } else {
                lerp(a[k], b[k], t)
            };
        }
        c
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TapeCircuit {
    pub cal: TapeCal,
    fs: f64,
    speed: TapeSpeed,
    pot: f64,
    pot_smooth: Smoother,
    pre: Biquad,
    de: Biquad,
    bump_rec: Biquad,
    contour: Cascade<4>,
    clip: DiodeClipper,
    pub head: Core,
    /// Peak follower of the record level (volts at the head).
    pub drive_peak: f64,
    peak_release: f64,
    countdown: usize,
    drive: f64,
    drive_pot: f64,
    contour_pot: f64,
}

impl TapeCircuit {
    pub fn new(fs: f64, cal: TapeCal) -> Self {
        let mut t = TapeCircuit {
            cal,
            fs,
            speed: TapeSpeed::Ips15,
            pot: 0.0,
            pot_smooth: Smoother::new(0.03, fs, 0.0),
            pre: Biquad::default(),
            de: Biquad::default(),
            bump_rec: Biquad::default(),
            contour: Cascade::default(),
            clip: DiodeClipper::new(cal.clip_att),
            head: Core::new(cal.head, fs),
            drive_peak: 0.0,
            peak_release: 0.0,
            countdown: 0,
            drive: 1.0,
            drive_pot: -1.0,
            contour_pot: -1.0,
        };
        t.set_rate(fs);
        t
    }

    pub fn set_rate(&mut self, fs: f64) {
        self.fs = fs;
        self.pot_smooth.set_time(0.03, fs);
        self.head.configure(self.cal.head, fs);
        // LED meter: instant attack, ~13 dB/s fall (classic LED bar ballistics).
        self.peak_release = (-1.0 / (0.35 * fs)).exp();
        self.update_emphasis();
        self.update_contour();
    }

    pub fn reset(&mut self) {
        self.pre.reset();
        self.de.reset();
        self.bump_rec.reset();
        self.contour.reset();
        self.clip.reset();
        self.head.reset();
        self.drive_peak = 0.0;
        self.pot_smooth.value = self.pot;
    }

    pub fn set_controls(&mut self, speed: TapeSpeed, pot: f64) {
        if speed != self.speed {
            self.speed = speed;
            self.update_emphasis();
            self.update_contour();
        }
        self.pot = pot.clamp(0.0, 1.0);
    }

    fn update_emphasis(&mut self) {
        let (t1, t2) = match self.speed {
            TapeSpeed::Ips15 => self.cal.emph15,
            TapeSpeed::Ips30 => self.cal.emph30,
        };
        let e = Analog::emphasis(t1, t2);
        let fw = 1.0 / (2.0 * std::f64::consts::PI * t1);
        self.pre.c = e.to_digital(self.fs, fw);
        self.de.c = e.inverse().to_digital(self.fs, fw);
    }

    fn update_contour(&mut self) {
        self.contour_pot = self.pot_smooth.value;
        let c = self.cal.contour_at(self.speed, self.contour_pot);
        self.contour.set(
            &[
                (Analog::hp2(c[0], c[1]), c[0]),
                (Analog::peak(c[2], c[3] * (1.0 - self.cal.bump_pre), c[4]), c[2]),
                (Analog::lp2(c[5], c[6]), c[5].min(0.4 * self.fs)),
                (Analog::shelf1(c[7] / 10f64.powf(c[8] / 40.0), c[7] * 10f64.powf(c[8] / 40.0)), c[7].min(0.4 * self.fs)),
            ],
            self.fs,
        );
        self.bump_rec.c = Analog::peak(c[2], c[3] * self.cal.bump_pre, c[4]).to_digital(self.fs, c[2]);
    }

    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let pot = self.pot_smooth.next(self.pot);
        // Re-design the contour only while the pot is actually moving.
        if self.countdown == 0 {
            if (pot - self.contour_pot).abs() > 1e-6 {
                self.update_contour();
            }
            self.countdown = 32;
        }
        self.countdown -= 1;

        if pot != self.drive_pot {
            self.drive_pot = pot;
            self.drive = 10f64.powf(self.cal.drive_db_at(pot) / 20.0);
        }
        let drive = self.drive;

        // DRIVE amplifier with IPS pre-emphasis and its diode soft clip.
        let rec = self.clip.process(self.pre.process(self.bump_rec.process(x * drive)));

        // Drive meter taps the record level sent to the head.
        let a = rec.abs();
        self.drive_peak = if a > self.drive_peak { a } else { self.drive_peak * self.peak_release };

        // Saturation transformer: flux-domain hysteresis + saturation.
        let replay = self.head.process(rec);

        // RECOVERY amplifier: inverse gain, de-emphasis, replay contour.
        let out = self.contour.process(self.de.process(replay / drive));
        out + self.cal.a2_rep * out * out
    }
}
