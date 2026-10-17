#!/usr/bin/env python3
"""Compare Snow 653 (ours) and the RND 542 plug-in against the 542 hardware
Audio Precision plots (digitized in tools/hw/rnd542_ap_curves.json).

Usage: python3 tools/compare.py [--json out.json]
"""
import json, sys
import numpy as np
sys.path.insert(0, 'tools')
import analyzer as A

HW = json.load(open('tools/hw/rnd542_ap_curves.json'))
POTS = {'cyan': 0.0, 'green': 0.3, 'red': 0.7, 'magenta': 1.0}
F_RESP = [10, 16, 20, 31.5, 50, 63, 100, 160, 250, 500, 1000, 2000, 5000, 10000, 16000, 20000, 31500, 40000]
F_THD = [20, 30, 40, 60, 100, 200, 500, 1000, 3000, 10000]


def hw_at(curve, freqs, left=None):
    a = np.array(curve)
    return np.interp(np.log(freqs), np.log(a[:, 0]), a[:, 1], left=left)


def main():
    duts = {'ours': A.Dut(A.OURS), 'rnd': A.Dut(A.RND)}
    for d in duts.values():
        d.set_global(mode='Dual Mono', link=False)
        d.set(trim=0.0, out=0.0, blend=1.0, silk='Off', texture=0.0)
    report = {}
    print('\n=== Frequency response @ 0 dBu: error vs hardware (RMS / max dB, 10 Hz-40 kHz) ===')
    for ips in (15, 30):
        for col, pot in POTS.items():
            f = np.array(F_RESP, float)
            hw = hw_at(HW[f'{ips}ips'][col], f)
            row = []
            for name, d in duts.items():
                d.set(tape=True, ips=ips, sat=pot)
                g = A.sweep(d, f)['gain_db']
                e = g - hw
                report[f'resp_{ips}_{pot}_{name}'] = g.tolist()
                row.append(f'{name}: rms {np.sqrt(np.mean(e**2)):.2f} max {np.abs(e).max():.2f}')
            print(f'{ips} ips sat {pot:.1f}  |  ' + '  |  '.join(row))
    print('\n=== THD @ 0 dBu, 15 ips (hardware plot vs models) ===')
    f = np.array(F_THD, float)
    for col, pot in (('red', 0.0), ('green', 0.3), ('cyan', 0.7), ('magenta', 1.0)):
        hw = hw_at(HW['thd'][col], f, left=np.nan)  # MAX is off-scale (>10 %) below ~29 Hz
        print(f'sat {pot:.1f}  freq : ' + ' '.join(f'{x:7.0f}' for x in f))
        print(f'         hw   : ' + ' '.join('    >10' if np.isnan(x) else f'{x:7.2f}' for x in hw))
        for name, d in duts.items():
            d.set(tape=True, ips=15, sat=pot)
            r = A.sweep(d, f)
            report[f'thd_{pot}_{name}'] = r['thd_pct'].tolist()
            print(f'         {name:5s}: ' + ' '.join(f'{x:7.2f}' for x in r['thd_pct']))
    print('\n=== SILK @ Texture MAX, tape out: response vs hardware ===')
    f = np.array([10, 20, 30, 50, 100, 300, 1000, 3000, 10000, 20000, 40000], float)
    base = hw_at(HW['tape_off_silk']['cyan'], f)
    for mode, col in (('Red', 'red'), ('Blue', 'blue')):
        hw = hw_at(HW['tape_off_silk'][col], f) - base
        print(f'{mode:4s} freq : ' + ' '.join(f'{x:6.0f}' for x in f))
        print(f'     hw   : ' + ' '.join(f'{x:+6.2f}' for x in hw))
        for name, d in duts.items():
            d.set(tape=False, silk=mode, texture=1.0)
            g = A.sweep(d, f)['gain_db']
            print(f'     {name:5s}: ' + ' '.join(f'{x:+6.2f}' for x in g) +
                  f'   rms err {np.sqrt(np.mean((g-hw)**2)):.2f}')
        for name, d in duts.items():
            d.set(tape=False, silk=mode, texture=1.0)
            r = A.sweep(d, np.array([50, 200, 1000, 5000.]), level_dbu=8)
            print(f'     {name:5s} harmonics @+8 dBu (50/200/1k/5k Hz)  H2%: ' +
                  ' '.join(f'{x*100:.2f}' for x in r['h'][:, 1]) + '  H3%: ' + ' '.join(f'{x*100:.2f}' for x in r['h'][:, 2]))
    if '--json' in sys.argv:
        json.dump(report, open(sys.argv[sys.argv.index('--json') + 1], 'w'))


if __name__ == '__main__':
    main()
