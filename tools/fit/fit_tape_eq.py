#!/usr/bin/env python3
"""Fit the 542 tape-path linear response (analog prototype) to the digitized
Audio Precision curves in tools/hw/rnd542_ap_curves.json.

Model per (IPS, Saturation pot position):
    H(s) = HP2(fh, Qh) * Peak(fb, Gb, Qb) * LP2(fl, Ql)
HP2  : low-frequency loss of the replay chain / interstage core inductance
Peak : head-bump contour (replay-head wavelength effect, LF compensation)
LP2  : high-frequency gap / spacing loss
"""
import json, sys
import numpy as np
from scipy.optimize import least_squares

HW = json.load(open('tools/hw/rnd542_ap_curves.json'))
POT = {'cyan': 0.0, 'green': 0.3, 'red': 0.7, 'magenta': 1.0}   # 7 o'clock..5 o'clock pot


def hp2(f, fc, q):
    s = 1j * f / fc
    return s * s / (s * s + s / q + 1)


def lp2(f, fc, q):
    s = 1j * f / fc
    return 1 / (s * s + s / q + 1)


def peak(f, fc, gdb, q):
    A = 10 ** (gdb / 40)
    s = 1j * f / fc
    return (s * s + s * A / q + 1) / (s * s + s / (A * q) + 1)


def shelf(f, fc, gdb):
    g = 10 ** (gdb / 40)
    return (1 + 1j * f / (fc / g)) / (1 + 1j * f / (fc * g))


def warp(f, fw, fs):
    """Analog frequency seen by a bilinear section prewarped at fw (as in filters.rs)."""
    if fs is None:
        return f
    fw = min(fw, 0.4 * fs)
    return fw / np.tan(np.pi * fw / fs) * np.tan(np.pi * np.asarray(f) / fs)


def model(p, f, fs=None):
    """Contour response in dB; with fs, the digital (bilinear) response at that internal rate."""
    fh, qh, fb, gb, qb, fl, ql = p[:7]
    h = hp2(warp(f, fh, fs), fh, qh) * peak(warp(f, fb, fs), fb, gb, qb) * lp2(warp(f, fl, fs), fl, ql)
    if len(p) > 7:  # recovery HF shelf (compensates clip compression at 0 dBu)
        h = h * shelf(warp(f, p[7], fs), p[7], p[8])
    return 20 * np.log10(np.abs(h))


def fit(f, y, p0, fmax):
    m = (f >= 10) & (f <= fmax)
    f, y = f[m], y[m]
    lo = [2, 0.3, 20, 0, 0.2, 15e3, 0.3]
    hi = [40, 2.0, 300, 12, 3.0, 400e3, 1.5]
    r = least_squares(lambda p: model(p, f) - y, p0, bounds=(lo, hi))
    return r.x, np.sqrt(np.mean(r.fun ** 2)), np.max(np.abs(r.fun))


if __name__ == '__main__':
  out = {}
  for ips, fmax in (('15ips', 40e3), ('30ips', 100e3)):
    out[ips] = {}
    for col, pot in POT.items():
        pts = np.array(HW[ips][col])
        f, y = pts[:, 0], pts[:, 1]
        p0 = [10, 0.7, 60 if ips == '15ips' else 120, 6, 0.6, 35e3 if ips == '15ips' else 120e3, 0.7]
        p, rms, mx = fit(f, y, p0, fmax)
        out[ips][pot] = list(map(float, p))
        print(f'{ips} pot={pot:.1f}: fh={p[0]:6.2f} Qh={p[1]:.3f} fb={p[2]:6.1f} Gb={p[3]:5.2f} '
              f'Qb={p[4]:.3f} fl={p[5]/1e3:6.1f}k Ql={p[6]:.3f}  rms={rms:.3f} max={mx:.3f} dB')
  json.dump(out, open('tools/fit/tape_eq_fit.json', 'w'), indent=1)
