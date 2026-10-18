#!/usr/bin/env python3
"""Convert the RND 542 plug-in's factory and artist presets into NEVE542
factory presets (truce `.preset` TOML files in neve542/presets/).

Source: the preset folder shipped with the RND plug-in (RND_542_Tape/). The
AU presets are canonical; presets that exist only as VST3 are taken from the
.vstpreset. Both carry the plug-in's JUCE state as XML, so only parameter
values are read - the plug-in itself is not touched.

Mapping (RND raw value -> NEVE542 plain value):
  Saturation / Blend / Texture  0..100 %   -> 0..1 (same pot rotation; NEVE542
                                              is calibrated at the hardware's
                                              MIN / 10 / 2 o'clock / MAX = 0 / .3 / .7 / 1)
  Trim, Output                  dB         -> dB (same ranges, ±12 / ±15)
  IPS                           0=15, 1=30 -> same index
  Texture Mode                  0 Off, 1 Red, 2 Blue -> Silk, same index
  Mode                          0 Dual, 1 Stereo, 2 M/S (verified by crosstalk
                                with different A/B settings; pedalboard's text
                                labels for this parameter are in another order)
  Tape, Link                    bool       -> bool
  Sat Enable, VU Mode           ignored (Sat Enable has no audible effect;
                                VU Mode is display only)

Usage: python3 tools/presets/import_rnd.py [RND_542_Tape] [neve542/presets]
"""
import glob, os, plistlib, re, sys

SRC = sys.argv[1] if len(sys.argv) > 1 else 'RND_542_Tape'
DST = sys.argv[2] if len(sys.argv) > 2 else 'neve542/presets'


def parse_xml(x):
    return {m.group(1): float(m.group(2)) for m in re.finditer(r'name="([^"]+)" value="([^"]+)"', x)}


def read_au(path):
    blob = plistlib.load(open(path, 'rb'))['jucePluginState']
    return parse_xml(blob[8:].rstrip(b'\0').decode())  # 'VC2!' + u32 size + XML


def read_vst(path):
    b = open(path, 'rb').read()
    return parse_xml(b[b.find(b'<?xml'):b.find(b'</CONTROLSTATE>')].decode(errors='replace'))


# The AU bank spells one preset "Black Vox Tape"; the VST3 bank's "Back Vox
# Tape" (backing vocals, next to the lead-vocal presets) is the same preset.
ALIASES = {'Black Vox Tape': 'Back Vox Tape'}


def collect():
    """{(bank, folder, name): values}; AU first, VST3-only presets added."""
    out = {}
    au_root = os.path.join(SRC, 'Presets_AU', 'RND 542 Tape')
    for f in sorted(glob.glob(os.path.join(au_root, '**', '*.aupreset'), recursive=True)):
        bank, folder = os.path.relpath(f, au_root).split(os.sep)[:2]
        name = os.path.basename(f)[:-9].strip()
        out[(bank, folder, ALIASES.get(name, name))] = read_au(f)
    vst_root = os.path.join(SRC, 'Presets_VST', 'RND 542 Tape Emulator')
    have = {(b, fo, n.lower()) for b, fo, n in out}
    for f in sorted(glob.glob(os.path.join(vst_root, '**', '*.vstpreset'), recursive=True)):
        bank, folder = os.path.relpath(f, vst_root).split(os.sep)[:2]
        name = os.path.basename(f)[:-10].strip()
        if (bank, folder, name.lower()) not in have:
            out[(bank, folder, name)] = read_vst(f)
    return out


def to_params(v):
    p = {
        'mode': int(v['Mode']),
        'link': bool(v['Link']),
    }
    for c in 'ab':
        C = c.upper()
        p[f'trim_{c}'] = round(v[f'Trim {C}'], 3)
        p[f'tape_in_{c}'] = bool(v[f'Tape {C}'])
        p[f'saturation_{c}'] = round(v[f'Saturation {C}'] / 100.0, 4)
        p[f'ips_{c}'] = int(v[f'IPS {C}'])
        p[f'blend_{c}'] = round(v[f'Blend {C}'] / 100.0, 4)
        p[f'silk_{c}'] = int(v[f'Texture Mode {C}'])
        p[f'texture_{c}'] = round(v[f'Texture {C}'] / 100.0, 4)
        p[f'output_{c}'] = round(v[f'Output {C}'], 3)
    return p


def toml_value(x):
    if isinstance(x, bool):
        return 'true' if x else 'false'
    if isinstance(x, int):
        return str(x)
    return repr(float(x))


def main():
    presets = collect()
    os.makedirs(DST, exist_ok=True)
    for (bank, folder, name), v in sorted(presets.items()):
        artist = bank == 'Artist Presets'
        category = 'Artist' if artist else folder
        title = name.replace('default-stereo', 'Default')
        stem = re.sub(r'[^A-Za-z0-9]+', '_', f'{folder}_{name}' if artist else name).strip('_')
        path = os.path.join(DST, category, stem + '.preset')
        uuid = None
        if os.path.exists(path):  # keep the uuid truce stamped on first install
            m = re.search(r'^uuid = "([^"]+)"', open(path).read(), re.M)
            uuid = m.group(1) if m else None
        os.makedirs(os.path.dirname(path), exist_ok=True)
        lines = []
        if uuid:
            lines.append(f'uuid = "{uuid}"')
        lines += [f'name = "{title}"', f'category = "{category}"']
        if artist:
            lines.append(f'comment = "Settings from the RND 542 plug-in artist bank ({folder})"')
        else:
            lines.append('comment = "Settings from the RND 542 plug-in factory bank"')
        if title == 'Default':
            lines.append('default = true')
        lines += ['', '[params]']
        lines += [f'{k} = {toml_value(x)}' for k, x in to_params(v).items()]
        open(path, 'w').write('\n'.join(lines) + '\n')
        print(path)


if __name__ == '__main__':
    main()
