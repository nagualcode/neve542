#!/usr/bin/env python3
"""Write tools/fit/thd_fit.json + contour_refit.json into neve542/src/tape.rs."""
import json, re

P = 'neve542/src/tape.rs'
s = open(P).read()
thd = json.load(open('tools/fit/thd_fit.json'))
con = json.load(open('tools/fit/contour_refit.json'))


def lit(v, fmt='.6g'):
    t = format(v, fmt)
    return t if any(c in t for c in '.eE') else t + '.0'


def sub(pat, rep):
    global s
    s, n = re.subn(pat, rep, s, count=1, flags=re.S)
    assert n == 1, pat


for key in ('c15', 'c30'):
    rows = '\n'.join('                [' + ', '.join(lit(v) for v in r) + '],' for r in con[key])
    sub(rf'(contour{key[1:]}: \[\n).*?(\n            \],)', lambda m: m.group(1) + rows + m.group(2))
sub(r'emph15: \(50e-6, [^)]*\)', f'emph15: (50e-6, {lit(thd["emph15b"], ".4g")})')
sub(r'clip_att: [0-9.e-]+,', f'clip_att: {lit(thd["clip_att"], ".4g")},')
for k in ('hyst', 'r_max', 'shape', 'power', 'sat', 'knee'):
    sub(rf'(head: CoreSpec \{{.*?\n\s+{k}: )[^,]+,', lambda m: m.group(1) + f'{lit(thd["head." + k], ".5g")},')
sub(r'bump_pre: [0-9.e-]+,', f'bump_pre: {lit(thd["bump_pre"], ".4g")},')
sub(r'clip_rd: [0-9.e-]+,', f'clip_rd: {lit(thd.get("clip_rd", 0.0), ".4g")},')
sub(r'drive_db: \[[0-9][^]]*\]', 'drive_db: [0.0, ' + ', '.join(lit(thd[f"drive{i}"], ".4g") for i in (1, 2, 3)) + ']')
open(P, 'w').write(s)
print('updated', P)
