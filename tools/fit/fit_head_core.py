#!/usr/bin/env python3
"""Fit the record-head core (Preisach + anhysteretic saturation) to the THD
vs flux relation implied by the RND 542 AP plot 'THD by Saturation Level'.

Flux unit: flux amplitude of 0 dBu at 100 Hz with Saturation MIN.
The AP THD was measured after the replay contour; the targets below are
corrected to the core itself (fundamental vs 3rd-harmonic contour gains) and
the ~0.09 % broadband floor (Class-A stages) is removed in quadrature.
"""
import subprocess, numpy as np
from scipy.optimize import least_squares

BIN = 'neve542/target/release/examples/core_probe'
flux = np.array([0.1, 0.2, 0.5, 1.0, 2.0, 4.0, 7.0, 10.0, 20.0])
thd = np.array([0.13, 0.32, 0.85, 2.0, 3.0, 3.8, 4.2, 6.0, 15.0])
wt = np.array([1, 1, 1, 1.5, 1, 1, 0.7, 0.7, 0.5])


def run(p):
    hyst, rmax, shape, sat, knee = p
    out = subprocess.run([BIN, f'{hyst}', f'{rmax}', f'{shape}', f'{sat}', f'{knee}',
                          ','.join(map(str, flux))], capture_output=True, text=True).stdout
    return np.array([[float(v) for v in l.split()] for l in out.strip().splitlines()])


def resid(p):
    r = run(p)
    return wt * (np.log(r[:, 2]) - np.log(thd))


r = least_squares(resid, [0.25, 2.0, 0.5, 12.0, 2.0],
                  bounds=([0.02, 0.3, 0.0, 3.0, 1.2], [0.6, 10, 4, 60, 6]), diff_step=0.02)
print('params hyst,r_max,shape,sat,knee =', np.round(r.x, 4).tolist())
res = run(r.x)
for fl, t, (_, g, th, h3) in zip(flux, thd, res):
    print(f'flux {fl:5.1f}: target {t:5.2f}%  model {th:5.2f}% (H3 {h3:5.2f})  gain {g:+.2f} dB')
