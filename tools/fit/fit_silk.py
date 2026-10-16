#!/usr/bin/env python3
"""Fit the SILK closed-loop response change (Texture MAX) to the hardware AP
plot 'Tape Effect Disengaged' (tools/hw/rnd542_ap_curves.json).

Each mode is a cascade of feedback-shaping sections N_i(s)/D_i(s). Reducing
negative feedback by a fraction t (Texture) turns each section into
N/((1-t)N + tD), which equals 1 at t=0 and the fitted response at t=1.
"""
import json
import numpy as np
from scipy.optimize import least_squares

HW = json.load(open('tools/hw/rnd542_ap_curves.json'))['tape_off_silk']
cy = np.array(HW['cyan'])
f = cy[:, 0]


def rel(c):
    a = np.array(HW[c])
    return np.interp(np.log(f), np.log(a[:, 0]), a[:, 1]) - cy[:, 1]


def s_(ff):
    return 2j * np.pi * ff


def sec_hp2(ff, fc, q):
    w = 2 * np.pi * fc; s = s_(ff)
    return s * s, s * s + s * w / q + w * w


def sec_hs2(ff, fc, g, q):  # analog high shelf (RBJ)
    A = 10 ** (g / 40); w = 2 * np.pi * fc; s = s_(ff) / w
    return A * (A * s * s + np.sqrt(A) / q * s + 1), (s * s + np.sqrt(A) / q * s + A)


def sec_ls2(ff, fc, g, q):
    A = 10 ** (g / 40); w = 2 * np.pi * fc; s = s_(ff) / w
    return A * (s * s + np.sqrt(A) / q * s + A), (A * s * s + np.sqrt(A) / q * s + 1)


def sec_lp1(ff, fc):
    w = 2 * np.pi * fc
    return w + 0 * ff, s_(ff) + w


def red(p, ff):
    g0, fh, qh, fs, gs, qs = p
    h = 10 ** (g0 / 20)
    for n, d in (sec_hp2(ff, fh, qh), sec_hs2(ff, fs, gs, qs)):
        h = h * n / d
    return 20 * np.log10(np.abs(h))


def blue(p, ff):
    g0, fh, qh, fl, gl, ql, fp = p
    h = 10 ** (g0 / 20)
    for n, d in (sec_hp2(ff, fh, qh), sec_ls2(ff, fl, gl, ql), sec_lp1(ff, fp)):
        h = h * n / d
    return 20 * np.log10(np.abs(h))


m = (f >= 10) & (f <= 60e3)
for name, fn, p0, lo, hi in (
    ('red', red, [1.5, 8, 0.6, 3000, 1.8, 0.6], [0, 1, .3, 300, 0, .2], [4, 40, 2, 30e3, 6, 2]),
    ('blue', blue, [0, 10, 0.7, 60, 1.6, 0.6, 150e3], [-1, 1, .3, 15, 0, .2, 20e3], [1, 40, 2, 400, 6, 2, 900e3]),
):
    y = rel(name)
    r = least_squares(lambda p: fn(p, f[m]) - y[m], p0, bounds=(lo, hi))
    print(name, 'params', np.round(r.x, 4).tolist(), 'rms', np.sqrt(np.mean(r.fun ** 2)).round(3),
          'max', np.abs(r.fun).max().round(3))
    for ff in (10, 20, 30, 50, 100, 300, 1000, 3000, 10000, 20000, 40000):
        i = np.argmin(abs(f - ff))
        print(f'   {ff:6d}: hw {y[i]:+.2f}  fit {fn(r.x, np.array([f[i]]))[0]:+.2f}')
