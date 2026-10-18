#!/usr/bin/env python3
"""Play every imported preset through NEVE542 (AU) and the RND 542 plug-in
with the same settings and compare the signal.

Per preset and per processing path (A/B for Dual and Stereo, Mid/Side for
M/S): the frequency response at -20 dBu (below either plug-in's clip, so it
is the linear EQ), the 1 kHz gain, and THD at 0 dBu at 100 Hz and 1 kHz.

Usage: python3 tools/presets/validate.py [--json out.json]
"""
import glob, json, os, re, sys
import numpy as np
sys.path.insert(0, 'tools')
import analyzer as A

F = np.array([20, 31.5, 50, 63, 80, 100, 160, 250, 500, 1000, 2000, 4000, 8000, 12500, 16000, 20000], float)
MODES = {0: 'Dual Mono', 1: 'Stereo', 2: 'M/S'}


def load(path):
    p = {}
    for line in open(path).read().split('[params]')[1].splitlines():
        if '=' in line:
            k, v = (x.strip() for x in line.split('=', 1))
            p[k] = {'true': 1.0, 'false': 0.0}.get(v, None) if v in ('true', 'false') else float(v)
    return p


def apply(d, p):
    d.set_global(mode=MODES[int(p['mode'])], link=False)
    for c in 'ab':
        d.set(ch=c, trim=p[f'trim_{c}'], tape=p[f'tape_in_{c}'] > 0.5, ips=15 if p[f'ips_{c}'] == 0 else 30,
              sat=p[f'saturation_{c}'], blend=p[f'blend_{c}'], silk=['Off', 'Red', 'Blue'][int(p[f'silk_{c}'])],
              texture=p[f'texture_{c}'], out=p[f'output_{c}'])


def measure(d, path_kind):
    """path_kind: ('A'|'B') = output channel 0/1 with L=R input; 'M'/'S' = mid/side."""
    ch = 0 if path_kind in ('A', 'M') else 1
    other = None
    if path_kind == 'S':
        other = 'neg'
    res = {}
    for lvl in (-20, 0):
        g, thd = [], []
        for f in (F if lvl == -20 else np.array([100.0, 1000.0])):
            sr = d.sr
            amp = A.dbu_to_amp(lvl)
            n = int((max(0.25, 40 / f) + 0.15) * sr)
            t = np.arange(n) / sr
            x = amp * np.sin(2 * np.pi * f * t)
            y = d.run(np.stack([x, -x if other else x]))
            y = (y[0] - y[1]) / 2 if path_kind == 'S' else ((y[0] + y[1]) / 2 if path_kind == 'M' else y[ch])
            tail = y[int(0.15 * sr):].astype(float)
            k = int(len(tail) * f / sr)
            tail = tail[: int(k * sr / f)]
            h = A.harmonics_fit(tail, f, sr) / amp
            g.append(20 * np.log10(max(h[0], 1e-12)))
            thd.append(100 * np.sqrt(np.sum((h[1:] / h[0]) ** 2)))
        res[lvl] = (np.array(g), np.array(thd))
    return res


def main():
    ours, rnd = A.Dut(A.OURS), A.Dut(A.RND)
    report = {}
    print(f'{"preset":34s} path  FR diff @-20 dBu (rms/max dB)  1k gain ours/rnd   THD% 100Hz ours/rnd   THD% 1k ours/rnd')
    for path in sorted(glob.glob('neve542/presets/*/*.preset')):
        p = load(path)
        name = re.search(r'^name = "(.*)"', open(path).read(), re.M).group(1)
        cat = os.path.basename(os.path.dirname(path))
        mode = int(p['mode'])
        kinds = ['M', 'S'] if mode == 2 else (['A'] if mode == 1 else ['A', 'B'])
        if mode == 0 and all(p[f'{k}_a'] == p[f'{k}_b'] for k in ('trim', 'saturation', 'blend', 'texture', 'output', 'ips', 'silk')):
            kinds = ['A']
        for d in (ours, rnd):
            apply(d, p)
        for kind in kinds:
            mo, mr = measure(ours, kind), measure(rnd, kind)
            diff = mo[-20][0] - mr[-20][0]
            report[f'{cat}/{name}/{kind}'] = {'f': F.tolist(), 'ours_fr': mo[-20][0].tolist(), 'rnd_fr': mr[-20][0].tolist(),
                                             'ours_thd': mo[0][1].tolist(), 'rnd_thd': mr[0][1].tolist(),
                                             'ours_g0': mo[0][0].tolist(), 'rnd_g0': mr[0][0].tolist()}
            i1k = list(F).index(1000)
            print(f'{(cat + "/" + name)[:34]:34s}  {kind}    {np.sqrt(np.mean(diff**2)):5.2f} / {np.abs(diff).max():5.2f}'
                  f'             {mo[-20][0][i1k]:+5.1f} / {mr[-20][0][i1k]:+5.1f}'
                  f'        {mo[0][1][0]:5.2f} / {mr[0][1][0]:5.2f}       {mo[0][1][1]:5.2f} / {mr[0][1][1]:5.2f}', flush=True)
    if '--json' in sys.argv:
        json.dump(report, open(sys.argv[sys.argv.index('--json') + 1], 'w'))


if __name__ == '__main__':
    main()
