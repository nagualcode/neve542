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
/// `[fh, Qh, f_bump, G_bump(dB), Q_bump, f_lp, Q_lp]`.
pub type Contour = [f64; 7];

/// All tunable constants of the tape path, kept together so the probe
/// (`examples/probe.rs`) can override them during calibration.
#[derive(Clone, Copy, Debug)]
pub struct TapeCal {
    /// Pot positions at which contours/drive were measured.
    /// Contours: `tools/fit/refit_contour.py` (non-linear model in the loop).
    pub knots: [f64; 4],
    /// DRIVE gain (dB) at each knot; RECOVERY is its inverse.
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
}

impl Default for TapeCal {
    fn default() -> Self {
        TapeCal {
            knots: [0.0, 0.3, 0.7, 1.0],
            // Drive meter reads 2, 2, 3, 6 at these positions (RND AP plot legend).
            drive_db: [0.0, 3.0, 5.0, 12.0],
            contour15: [
                [6.73082, 0.522224, 68.1432, 5.46458, 0.329274, 45791.9, 0.488385],
                [6.76443, 0.512768, 68.6072, 4.2873, 0.359091, 42791.4, 0.473889],
                [7.42183, 0.544154, 65.9302, 3.03133, 0.394623, 41670.7, 0.425724],
                [5.5931, 0.3, 64.1904, 1.71054, 0.530165, 39636.7, 0.446224],
            ],
            contour30: [
                [5.73618, 0.3, 132.138, 3.98252, 0.287482, 400000.0, 1.25924],
                [5.92244, 0.3, 143.357, 3.12105, 0.322554, 139225.0, 0.887691],
                [5.55588, 0.3, 155.903, 2.12192, 0.381819, 166416.0, 0.762859],
                [6.3256, 0.3, 180.476, 1.11653, 0.397277, 400000.0, 0.454707],
            ],
            // NAB 15 ips (50 µs) and AES 30 ips (17.5 µs) record pre-emphasis,
            // shelved by the drive amplifier's feedback network (+4.4 / +3.3 dB);
            // shelf limits and clip threshold chosen by refit_contour.py sweeps
            // (best match of the Saturation MAX AP curves).
            emph15: (50e-6, 30e-6),
            emph30: (17.5e-6, 12e-6),
            clip_att: 0.05,
            // Fitted by tools/fit/fit_head_core.py. Flux unit: 0 dBu (1.0954 V
            // peak) at 100 Hz, so tau = 1/(2π·100) and levels scale by 1.0954.
            head: CoreSpec {
                tau: 1.0 / (2.0 * std::f64::consts::PI * 100.0),
                hyst: 0.2379,
                r_max: 3.0626 * 1.095_445,
                shape: 0.3891,
                sat: 47.94 * 1.095_445,
                knee: 2.4208,
                leak_hz: 0.5,
                plays: crate::magnetics::PLAYS,
            },
            a2_rep: 1.6e-3,
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
        let mut c = [0.0; 7];
        for k in 0..7 {
            // Frequencies interpolate geometrically, the rest linearly.
            c[k] = if k == 0 || k == 2 || k == 5 {
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
    contour: Cascade<3>,
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
                (Analog::peak(c[2], c[3], c[4]), c[2]),
                (Analog::lp2(c[5], c[6]), c[5].min(0.4 * self.fs)),
            ],
            self.fs,
        );
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
        let rec = self.clip.process(self.pre.process(x * drive));

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
