#!/usr/bin/env python3
"""Re-fit the replay contour with the full non-linear model in the loop.

The AP curves were taken at 0 dBu, where the record-head core already adds
level-dependent gain (Rayleigh bloom at LF, clip compression at HF). The
contour sits after the core, so: target = hardware − (model − contour).
"""
import json, subprocess, sys
import numpy as np
from scipy.optimize import least_squares
sys.path.insert(0, 'tools/fit')
from fit_tape_eq import model, POT

FS_INT = 192000.0  # probe runs at sr=96k -> 2x oversampled tape path


def contour_db(p, f):
    return model(p, f, FS_INT)

PROBE = 'neve542/target/release/examples/probe'
EXTRA = [a for a in sys.argv[1:] if '=' in a]
QUIET = '--quiet' in sys.argv
HW = json.load(open('tools/hw/rnd542_ap_curves.json'))
cur = {}
src = open('neve542/src/tape.rs').read()


def run(ips, pot, freqs, tabs):
    args = [PROBE, 'sr=96000', *EXTRA, f'ips={ips}', f'sat={pot}', 'freqs=' + ','.join(f'{f:.2f}' for f in freqs)]
    for key in ('c15', 'c30'):
        if key in tabs:
            args.append(f'{key}=' + ','.join(f'{v:.6g}' for row in tabs[key] for v in row))
    out = subprocess.run(args, capture_output=True, text=True, check=True).stdout.splitlines()[1:]
    return np.array([float(l.split()[1]) for l in out])


init = json.load(open('tools/fit/tape_eq_fit.json'))
tabs = {'c15': [init['15ips'][str(p)] + [8000.0, 0.0] for p in (0.0, 0.3, 0.7, 1.0)],
        'c30': [init['30ips'][str(p)] + [8000.0, 0.0] for p in (0.0, 0.3, 0.7, 1.0)]}
for it in range(2):
    new = {'c15': [], 'c30': []}
    for ips, key, fmax in ((15, 'c15', 40e3), (30, 'c30', 40e3)):  # sr=96k: internal 192 kHz, as in a DAW
        for col, pot in POT.items():
            pts = np.array(HW[f'{ips}ips'][col])
            pts = pts[(pts[:, 0] >= 10) & (pts[:, 0] <= fmax)][::2]
            f, hw = pts[:, 0], pts[:, 1]
            p_cur = tabs[key][[0.0, 0.3, 0.7, 1.0].index(pot)]
            m = run(ips, pot, f, tabs)
            nl = m - contour_db(p_cur, f)
            tgt = hw - nl
            lo = [2, 0.3, 20, 0, 0.15, 15e3, 0.3, 2e3, -1.0]
            hi = [40, 2.0, 300, 12, 3.0, 400e3, 1.5, 40e3, 3.0]
            r = least_squares(lambda p: contour_db(p, f) - tgt, p_cur, bounds=(lo, hi))
            new[key].append(list(map(float, r.x)))
            err = m - hw
            if not QUIET or (it == 1 and pot == 1.0): print(f'iter{it} {ips}ips pot={pot}: model-vs-hw rms {np.sqrt(np.mean(err**2)):.3f} '
                  f'max {np.abs(err).max():.3f} dB -> refit rms {np.sqrt(np.mean(r.fun**2)):.3f}')
    tabs = new
for key in ('c15', 'c30') if not QUIET else ():
    print(f'            contour{key[1:]}: [')
    for row in tabs[key]:
        print('                [' + ', '.join(f'{v:.5g}' if i not in (5,) else f'{v:.4g}' for i, v in enumerate(row)) + '],')
    print('            ],')
if not QUIET:
    json.dump(tabs, open('tools/fit/contour_refit.json', 'w'), indent=1)
