# Jardim Sonoro — Neve 542 Dual Tape Emulator

> Component-level model of the Rupert Neve Designs 542 Tape Emulator (500 series), dual channel, in Rust on the [truce](https://github.com/truce-audio/truce) framework. Calibrated against the **hardware's** published Audio Precision measurements, not against another plug-in.

Formats: CLAP · VST3 · Standalone. Modes: Dual Mono · Stereo · Mid/Side, with offset-preserving Link.

![Editor](docs/editor.png)

---

## Results

All numbers come from `tools/compare.py`: a stepped-sine analyzer (phase-free least-squares fit of H1–H10, plug-in warmed up before each measurement) run on the installed VST3 at 96 kHz, 0 dBu = −18 dBFS. The hardware curves are digitized from the AP plots in the 542 operations manual (`tools/hw/rnd542_ap_curves.json`). "RND plug-in" is the official Rupert Neve Designs 542 plug-in measured the same way.

**Frequency response at 0 dBu: error against the hardware (RMS / max, 10 Hz – 40 kHz)**

| Setting | This model | RND plug-in |
| :-- | :-- | :-- |
| 15 IPS, Sat MIN | 0.21 / 0.59 dB | 1.57 / 6.29 dB |
| 15 IPS, Sat 10 o'clock | 0.20 / 0.50 dB | 1.46 / 5.49 dB |
| 15 IPS, Sat 2 o'clock | 0.22 / 0.56 dB | 3.42 / 12.0 dB |
| 15 IPS, Sat MAX | 0.23 / 0.45 dB | 3.73 / 13.0 dB |
| 30 IPS, Sat MIN | 0.09 / 0.19 dB | 0.71 / 2.75 dB |
| 30 IPS, Sat 10 o'clock | 0.25 / 0.49 dB | 0.99 / 3.14 dB |
| 30 IPS, Sat 2 o'clock | 0.22 / 0.40 dB | 2.86 / 10.7 dB |
| 30 IPS, Sat MAX | 0.18 / 0.45 dB | 3.11 / 11.3 dB |
| SILK Red, Texture MAX (tape out) | 0.15 dB RMS | 6.10 dB RMS |
| SILK Blue, Texture MAX (tape out) | 0.10 dB RMS | 4.15 dB RMS |

**THD at 0 dBu, 15 IPS, Saturation MIN (%)**

| Hz | 20 | 40 | 60 | 100 | 200 | 500 | 1k | 3k | 10k |
| :-- | --: | --: | --: | --: | --: | --: | --: | --: | --: |
| Hardware | 4.74 | 2.64 | 2.02 | 1.45 | 0.86 | 0.34 | 0.16 | 0.09 | 0.10 |
| This model | 6.53 | 3.31 | 1.99 | 1.13 | 0.65 | 0.33 | 0.18 | 0.09 | 0.07 |
| RND plug-in | 0.01 | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |

The RND plug-in is essentially distortion-free below about +6 dBu and then clips, with the same THD at 1 kHz and 8 kHz. The hardware distorts at normal levels and its distortion rises as `1/f`, which is the signature of the magnetic core. That low-level, frequency-dependent behaviour is what this model reproduces.

Known gaps:
- At Saturation MAX the high-frequency THD is lower than the plot shows (1.2 % vs 3.2 % at 10 kHz). The soft-clip and pre-emphasis settings were picked to match the frequency response. The plot's measurement bandwidth is unknown, so its HF THD is less certain.
- THD below 30 Hz at Saturation MIN is about 1.5 dB high.
- The hardware publishes no harmonic breakdown for SILK, so SILK harmonic levels follow RND's description (Red: upper mids and highs, mostly H2; Blue: lows and low mids, H2 + H3), not a measurement.

Validation: `pluginval --strictness-level 5` passes. `examples/stress.rs` drives every mode at 44.1–192 kHz with silence, DC, noise, +12 dBFS square bursts and denormal-range input; the output stays finite throughout.

CPU: about 0.6 % (tape out) to about 7 % (Saturation MAX + SILK) of one Apple-silicon core per channel at 48 kHz, measured on a loaded machine.

---

## The model

### Sources
- RND 542 operations manual: block diagram, specifications, and the four AP plots (`tape_emulator.pdf`).
- RND 542 plug-in user guide (`manual.pdf`).
- Sound On Sound reviews of the Portico 5042 and the RND 500 series. Key points: a constant-current drive into inductively coupled coils; LF distortion is "predominantly third harmonic"; the soft clip adds 2nd harmonic at higher levels; Silk Red adds about +1.5 dB overall and +3 dB above 5 kHz; Silk Blue adds about +1.5 dB at 30 Hz.
- RND's own description of Silk: it reduces the negative feedback taken from the output transformer and changes that feedback's frequency response.
- No schematic of the 542 or 5042 has been published. Earlier versions of this README cited "US Patent 7,454,025" for Silk. **That was wrong.** US 7,454,025 is a Panasonic loudspeaker patent and has nothing to do with Neve.

### Signal chain (per channel, `neve542/src/channel.rs`)

```
IN → input transformer (mu-metal: 0.8 Hz HP, tiny hysteresis) → Class-A TRIM stage (±16 V rails)
   ├── dry ──────────────────────────────────────────────────────────────────────┐
   └── DRIVE amp: IPS pre-emphasis (NAB 50 µs / AES 17.5 µs, shelved)             │
       → diode soft clip (anti-parallel, mismatched junctions, Newton-solved)    │
       → SATURATION TRANSFORMER "record head" (flux-domain Preisach hysteresis)  │
       → RECOVERY amp: 1/DRIVE (counter-ganged), de-emphasis, replay contour ─────┤
                                                                         BLEND ──┘
   → blend amp → [LEVEL meter] → output stage + output transformer with SILK → OUT
```

Everything non-linear runs at ≥ 176.4 kHz. The oversampler is a zero-latency polyphase IIR half-band using the elliptic design from *hiir*: passband flat to 20 kHz at 44.1 kHz, images below −90 dB.

**Magnetic cores (`magnetics.rs`).** A voltage-driven winding sets the flux, Φ = ∫v dt, so flux grows as 1/f. Hysteresis is a discrete Preisach model in Prandtl–Ishlinskii form: 32 play operators on a quadratic threshold grid with a fitted coercivity density. A uniform density gives Rayleigh's law, so distortion is proportional to flux, mostly H3. The model carries memory and phase lag, and its permeability rises with amplitude, which produces the low-frequency "bloom". A Langevin-like anhysteretic knee follows. The secondary voltage is dΦ/dt, implemented as the exact discrete inverse of the integrator, so an ideal core is bit-transparent. The record-head parameters were fitted to the THD-versus-flux relation in the hardware plot (`tools/fit/fit_head_core.py`). The hardware data shows THD depends on flux (drive/f): points with the same flux at different frequencies give the same THD.

**Replay contour (`tape.rs`).** A 2nd-order high-pass, a head-bump peak and a 2nd-order gap-loss low-pass at each documented pot position, interpolated between positions. They were fitted with the full non-linear model in the loop, because the AP curves were taken at 0 dBu where the core already adds level-dependent gain (`tools/fit/refit_contour.py`).

**SILK (`silk.rs`).** A feedback model. Each fitted Red/Blue section N/D is opened by the Texture fraction t, giving N / ((1−t)N + tD): flat at t = 0, the measured curve at t = 1. The open-loop distortion of the single-ended output stage (H2, proportional to level) and of the output-transformer core (H3, proportional to 1/f) is computed on the signal and injected through the loop sensitivity S = 1/(1+T). Red raises S in the upper mids and highs, Blue in the lows.

**Meters.** DRIVE uses 8 LEDs, 3 dB apart: LED 2 = nominal, and soft clip starts above LED 5. LEVEL is the hardware's −10 / −2 / 4 / 10 / 14 / 18 / 20 / 22 dBu scale, taken pre-SILK. Both have LED ballistics.

### Interface
The panel is drawn with egui. Each channel is one row in signal-flow order: INPUT → TAPE (Tape In, IPS, Saturation, Drive meter) → BLEND → LEVEL → SILK → OUTPUT, styled after Ableton Live's flat dark panels. Rows are labelled A/B or MID/SIDE depending on mode. In Stereo mode row B is dimmed because channel A controls both. LINK keeps the relative offset between channels, as the RND manual specifies. Controls: drag, shift-drag for fine, double-click to reset, scroll wheel.

---

## Building

```bash
cargo install cargo-truce
cd neve542
cargo truce install --user            # CLAP + VST3 into ~/Library/Audio/Plug-Ins
cargo run --bin neve542-standalone --features standalone
```

## Measuring and calibrating

```bash
python3 tools/compare.py                                   # ours vs RND plug-in vs hardware
cargo run --release --example probe -- ips=15 sat=1 level=0   # DSP-level AP sweep
cargo run --release --example stress                       # robustness + CPU
cargo test --release --lib                                 # filter/oversampler/core/stage unit tests
python3 tools/fit/fit_head_core.py                         # refit record-head core
python3 tools/fit/refit_contour.py                         # refit replay contour (model in loop)
python3 tools/fit/fit_silk.py                              # refit SILK curves
```

## Credits & license
Hardware design: Rupert Neve (1926–2021) and Rupert Neve Designs. This is an independent model, not affiliated with RND. DSP, Rust implementation and UI: Jardim Sonoro ([nagualcode](https://github.com/nagualcode)). MIT license.
