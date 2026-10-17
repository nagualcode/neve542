#!/usr/bin/env python3
"""Joint fit of the record path's non-linear parameters to the hardware
'THD by Saturation Level' AP plot (all four pot positions at once).

Free parameters: record-head core (hyst, r_max, shape, sat, knee), diode clip
attenuation and the 15 ips pre-emphasis shelf pole. The probe (full DSP) is in
the loop. Run tools/fit/refit_contour.py afterwards: the replay contour sits
after the core and restores the frequency response.

Usage: python3 tools/fit/fit_thd.py [key=value ...]   (extra probe overrides)
"""
import json, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
import numpy as np
from scipy.optimize import least_squares

PROBE = 'snow653/target/release/examples/probe'
EXTRA = [a for a in sys.argv[1:] if '=' in a]
HW = json.load(open('tools/hw/rnd542_ap_curves.json'))['thd']
POTS = (('red', 0.0), ('green', 0.3), ('cyan', 0.7), ('magenta', 1.0))
F = np.array([20, 25, 31.5, 40, 60, 100, 200, 500, 1000, 1500, 3000, 5000, 10000], float)
# The 10 o'clock / 2 o'clock traces have a step near 2 kHz that no smooth
# mechanism reproduces (likely an analyzer range change); de-weight 1-5 kHz there.
W = {0.0: np.ones(len(F)), 1.0: np.ones(len(F))}
W[0.3] = W[0.7] = np.where((F >= 1500) & (F <= 5000), 0.4, 1.0)
NAMES = ['head.hyst', 'head.r_max', 'head.shape', 'head.power', 'head.sat', 'head.knee', 'clip_att', 'emph15b', 'bump_pre',
         'drive1', 'drive2', 'drive3']
X0 = [0.28642, 11.175, 1.11837, 0.3, 81.7797, 1.90407, 0.0629563, 3.24913e-05, 0.8, 2.5, 4.5, 12.5]
# Drive meter (3 dB LEDs) reads 2, 2, 3, 6 at MIN, 10, 2 o'clock, MAX: the
# drive gains are only known to within their LED window.
LO = [0.02, 0.5, 0.0, 0.0, 5.0, 1.2, 0.02, 15e-6, 0.0, 0.3, 3.0, 9.0]
HI = [0.8, 60.0, 4.0, 0.9, 300.0, 6.0, 0.2, 49e-6, 1.0, 2.99, 5.99, 14.99]
try:  # warm start from the previous fit
    _prev = json.load(open('tools/fit/thd_fit.json'))
    X0 = [_prev.get(n, x) for n, x in zip(NAMES, X0)]
except FileNotFoundError:
    pass
SCALE = np.array(X0)


def hw(col):
    a = np.array(HW[col])
    return np.interp(np.log(F), np.log(a[:, 0]), a[:, 1], left=10.0)


TGT = {pot: hw(col) for col, pot in POTS}


# Saturation MAX frequency response (LF core saturation and 2-20 kHz clip
# compression, both speeds): the mechanisms that make the THD also shape the
# fundamental, so fit both together.
HWR = json.load(open('tools/hw/rnd542_ap_curves.json'))
FR = np.array([10, 12.5, 16, 20, 25, 31.5, 40, 50, 2000, 3150, 5000, 8000, 10000, 12500, 16000, 20000], float)
FR_TOL_DB = 0.15


def hw_resp(ips):
    a = np.array(HWR[f'{ips}ips']['magenta'])
    return np.interp(np.log(FR), np.log(a[:, 0]), a[:, 1])


TGT_R = {ips: hw_resp(ips) for ips in (15, 30)}


def probe(pot, x, ips=15, freqs=F, col=2):
    args = [PROBE, *EXTRA, f'ips={ips}', f'sat={pot}', 'freqs=' + ','.join(f'{f:g}' for f in freqs)]
    args += [f'{n}={v:.8g}' for n, v in zip(NAMES, x) if not n.startswith('drive')]
    args.append('drive=0,' + ','.join(f'{v:.6g}' for v in x[-3:]))
    out = subprocess.run(args, capture_output=True, text=True, check=True).stdout.splitlines()[1:]
    return np.array([float(l.split()[col]) for l in out])


def thd_all(x, resp=False):
    jobs = [lambda p=p: probe(p, x) for _, p in POTS]
    if resp:
        jobs += [lambda i=i: probe(1.0, x, ips=i, freqs=FR, col=1) for i in (15, 30)]
    with ThreadPoolExecutor(6) as ex:
        res = list(ex.map(lambda j: j(), jobs))
    out = dict(zip([p for _, p in POTS], res[:4]))
    if resp:
        out['r15'], out['r30'] = res[4], res[5]
    return out


def resid(u):
    m = thd_all(u * SCALE, resp=True)
    r = [W[p] * (np.log(m[p]) - np.log(TGT[p])) for _, p in POTS]
    # Saturation MAX is off the plot's 10 % full scale below ~29 Hz.
    r[-1][F < 29] = np.minimum(0.0, np.log(m[1.0][F < 29]) - np.log(10.0))
    r += [(m[f'r{i}'] - TGT_R[i]) / FR_TOL_DB * 0.1 for i in (15, 30)]
    return np.concatenate(r)


if __name__ == '__main__':
    r = least_squares(resid, np.ones(len(X0)), bounds=(np.array(LO) / SCALE, np.array(HI) / SCALE),
                      diff_step=0.03, max_nfev=60, verbose=1)
    x = r.x * SCALE
    print('fitted:', ' '.join(f'{n}={v:.6g}' for n, v in zip(NAMES, x)))
    m = thd_all(x, resp=True)
    for i in (15, 30):
        print(f'Sat MAX {i} ips response error (dB) @', ' '.join(f'{f:g}' for f in FR), ':',
              ' '.join(f'{v:+.2f}' for v in m[f'r{i}'] - TGT_R[i]))
    print('Hz      ' + ' '.join(f'{f:6.0f}' for f in F))
    for _, p in POTS:
        print(f'hw  {p:.1f} ' + ' '.join(f'{v:6.2f}' for v in TGT[p]))
        print(f'fit {p:.1f} ' + ' '.join(f'{v:6.2f}' for v in m[p]))
    json.dump(dict(zip(NAMES, map(float, x))), open('tools/fit/thd_fit.json', 'w'), indent=1)
