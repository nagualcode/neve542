#!/usr/bin/env python3
"""Stepped-sine measurement harness for VST3 plugins (via pedalboard).

Measures, at a given input level, the fundamental gain (dB) and THD (%)
for a list of frequencies -- the same way an Audio Precision sweep in the
542 datasheet was taken (Input @ 0 dBu, fundamental-tracking response).

Calibration: 0 dBu == -18 dBFS (CAL_DBFS_AT_0DBU).
"""
import sys
import json
import numpy as np
import pedalboard

CAL_DBFS_AT_0DBU = -18.0
SR = 96000

OLD = "/Library/Audio/Plug-Ins/VST3/RND_542_Tape.vst3"


def dbu_to_amp(dbu):
    return 10 ** ((dbu + CAL_DBFS_AT_0DBU) / 20.0)


def load(path):
    return pedalboard.load_plugin(path)


def set_params(p, params):
    for k, v in params.items():
        setattr(p, k, v)


def harmonics_fit(y, f, sr, nh=10):
    """Least-squares fit of fundamental + harmonics; returns amplitudes."""
    n = len(y)
    t = np.arange(n) / sr
    cols = []
    hs = []
    for h in range(1, nh + 1):
        fh = f * h
        if fh >= sr / 2 * 0.98:
            break
        cols.append(np.sin(2 * np.pi * fh * t))
        cols.append(np.cos(2 * np.pi * fh * t))
        hs.append(h)
    cols.append(np.ones(n))
    A = np.stack(cols, axis=1)
    coef, *_ = np.linalg.lstsq(A, y, rcond=None)
    amps = [np.hypot(coef[2 * i], coef[2 * i + 1]) for i in range(len(hs))]
    return np.array(amps)


def measure(p, freqs, level_dbu=0.0, sr=SR, tone_s=0.35, settle_s=0.2):
    amp = dbu_to_amp(level_dbu)
    gains, thds, h2, h3 = [], [], [], []
    for f in freqs:
        n = int(tone_s * sr)
        t = np.arange(n) / sr
        x = (amp * np.sin(2 * np.pi * f * t)).astype(np.float32)
        x2 = np.stack([x, x])
        y = p.process(x2, sr, reset=True)[0].astype(np.float64)
        s = int(settle_s * sr)
        # latency-agnostic: fit is phase-free
        a = harmonics_fit(y[s:], f, sr)
        g = 20 * np.log10(max(a[0], 1e-12) / amp)
        thd = 100 * np.sqrt(np.sum(a[1:] ** 2)) / max(a[0], 1e-12)
        gains.append(g)
        thds.append(thd)
        h2.append(100 * (a[1] / a[0]) if len(a) > 1 else 0)
        h3.append(100 * (a[2] / a[0]) if len(a) > 2 else 0)
    return np.array(gains), np.array(thds), np.array(h2), np.array(h3)


def old_params(tape=True, sat=0.0, ips=15, blend=100.0, trim=0.0, silk="Off", texture=0.0):
    d = {}
    for ch in ("a", "b"):
        d[f"tape_{ch}"] = tape
        d[f"saturation_{ch}"] = sat
        d[f"ips_{ch}"] = f"{ips} ips"
        d[f"blend_{ch}"] = blend
        d[f"trim_{ch}_db"] = trim
        d[f"texture_mode_{ch}"] = silk
        d[f"texture_{ch}"] = texture
    return d


FREQS = [10, 12.5, 16, 20, 25, 31.5, 40, 50, 60, 80, 100, 125, 160, 200, 250, 315, 400,
         500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000,
         10000, 12500, 16000, 20000, 25000, 28000, 31500, 40000]

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else OLD
    p = load(path)
    print([k for k in p.parameters.keys()])
