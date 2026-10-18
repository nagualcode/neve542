#!/usr/bin/env python3
"""Stepped-sine analyzer for VST3 plugins (via pedalboard).

Emulates an Audio Precision fundamental-tracking sweep: for each frequency a
sine is played, the plugin is allowed to settle, and a phase-free
least-squares fit extracts the fundamental gain and harmonics H2..H10.

Calibration: 0 dBu == -18 dBFS (same convention as the RND plug-in, whose
Drive meter reads "2" at 0 dBu / Saturation MIN).

IMPORTANT: the RND plug-in ramps its parameters for ~0.6 s after a reset
(output swings by up to +36 dB). Every measurement therefore warms the
plug-in up with silence after parameters change, and never uses reset=True
between tones.
"""
import numpy as np
import pedalboard

CAL_DBFS_AT_0DBU = -18.0
SR = 96000
RND = "/Library/Audio/Plug-Ins/VST3/RND_542_Tape.vst3"
import os
OURS = os.path.expanduser("~/Library/Audio/Plug-Ins/Components/NEVE542.component")


def dbu_to_amp(dbu):
    return 10 ** ((dbu + CAL_DBFS_AT_0DBU) / 20.0)


class Dut:
    """A device under test: plugin + parameter-name translation."""

    def __init__(self, path, sr=SR):
        self.p = pedalboard.load_plugin(path)
        self.sr = sr
        self.is_rnd = "RND" in path
        self._dirty = True

    # ---- unified parameter API (both channels unless ch given) ----------
    def set(self, ch="ab", **kw):
        for c in ch:
            for k, v in kw.items():
                name, val = self._translate(k, v, c)
                if name is not None:
                    setattr(self.p, name, val)
        self._dirty = True

    def set_global(self, **kw):
        for k, v in kw.items():
            if self.is_rnd:
                if k == "mode":
                    # The RND plug-in's text labels for this parameter do not
                    # match its behaviour: the value labelled "Dual" processes
                    # mid/side (an L-only input leaks into R when A and B
                    # differ) and "Mid/Side" keeps the channels apart.
                    v = {"Dual Mono": "Mid/Side", "Stereo": "Stereo", "M/S": "Dual"}[v]
                if k == "link":
                    v = "Link 1" if v else "Off"
                setattr(self.p, k, v)
            else:
                name = {"mode": "processing_mode", "link": "link_channels"}.get(k, k)
                setattr(self.p, name, v)
        self._dirty = True

    def _translate(self, k, v, c):
        if self.is_rnd:
            m = {
                "tape": lambda: (f"tape_{c}", bool(v)),
                "sat": lambda: (f"saturation_{c}", float(v) * 100.0),
                "ips": lambda: (f"ips_{c}", f"{int(v)} ips"),
                "blend": lambda: (f"blend_{c}", float(v) * 100.0),
                "trim": lambda: (f"trim_{c}_db", float(v)),
                "silk": lambda: (f"texture_mode_{c}", v),
                "texture": lambda: (f"texture_{c}", float(v) * 100.0),
                "out": lambda: (f"output_{c}_db", float(v)),
            }
            if k == "sat":
                setattr(self.p, f"sat_enable_{c}", True)
            return m[k]()
        names = {
            "tape": f"tape_in_{c}",
            "sat": f"saturation_{c}",
            "ips": f"speed_{c}",
            "blend": f"blend_{c}",
            "trim": f"trim_{c}_db",
            "silk": f"silk_{c}",
            "texture": f"texture_{c}",
            "out": f"output_{c}_db",
        }
        name = names[k]
        if name not in self.p.parameters and name.endswith("_db"):
            name = name[:-3]  # AU exposes trim_a / output_a without the unit suffix
        param = self.p.parameters[name]
        if k == "ips":
            v = next(s for s in param.valid_values if str(int(v)) in s)
        elif k in ("sat", "blend", "texture"):
            lo, hi, _ = param.range
            v = float(v) * (hi - lo) + lo
        elif k == "tape":
            v = bool(v)
        return name, v

    # ---- processing -------------------------------------------------------
    def warm(self, seconds=0.8):
        z = np.zeros((2, int(seconds * self.sr)), np.float32)
        self.p.process(z, self.sr, reset=False)
        self._dirty = False

    def run(self, x2):
        """Process a (2, n) block, warming up first if params changed."""
        if self._dirty:
            self.warm()
        return self.p.process(x2.astype(np.float32), self.sr, reset=False)


def harmonics_fit(y, f, sr, nh=10):
    n = len(y)
    t = np.arange(n) / sr
    cols, hs = [], []
    for h in range(1, nh + 1):
        if f * h >= sr / 2 * 0.98:
            break
        cols += [np.sin(2 * np.pi * f * h * t), np.cos(2 * np.pi * f * h * t)]
        hs.append(h)
    cols.append(np.ones(n))
    A = np.stack(cols, axis=1)
    coef, *_ = np.linalg.lstsq(A, y, rcond=None)
    amps = np.array([np.hypot(coef[2 * i], coef[2 * i + 1]) for i in range(len(hs))])
    out = np.zeros(nh)
    out[: len(amps)] = amps
    return out


def tone(dut, f, level_dbu, ch=0, other=None, cycles=40, min_s=0.25, settle_s=0.15):
    """Play one tone; return harmonic amplitudes (relative to input amplitude)."""
    sr = dut.sr
    amp = dbu_to_amp(level_dbu)
    dur = max(min_s, cycles / f) + settle_s
    n = int(dur * sr)
    t = np.arange(n) / sr
    x = amp * np.sin(2 * np.pi * f * t)
    x2 = np.stack([x, x if other is None else other(t)])
    y = dut.run(x2)[ch].astype(np.float64)
    s = int(settle_s * sr)
    # Fit an integer number of cycles of the tail for leakage-free results
    tail = y[s:]
    ncyc = int(len(tail) * f / sr)
    tail = tail[: int(ncyc * sr / f)] if ncyc > 4 else tail
    return harmonics_fit(tail, f, sr) / amp


def sweep(dut, freqs, level_dbu=0.0, ch=0):
    """Returns dict with gain_db, thd_pct, h (n_freq x 10, relative to fundamental)."""
    g, thd, H = [], [], []
    for f in freqs:
        a = tone(dut, f, level_dbu, ch)
        g.append(20 * np.log10(max(a[0], 1e-12)))
        rel = a / max(a[0], 1e-12)
        thd.append(100 * np.sqrt(np.sum(rel[1:] ** 2)))
        H.append(rel)
    return {"gain_db": np.array(g), "thd_pct": np.array(thd), "h": np.array(H)}


AP_FREQS = [10, 12.5, 16, 20, 25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315,
            400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300,
            8000, 10000, 12500, 16000, 20000, 25000, 31500, 40000]
