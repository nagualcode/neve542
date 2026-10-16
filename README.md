# Jardim Sonoro — Neve 542 Dual Tape Emulator

> Component-level model of the Rupert Neve Designs 542 Tape Emulator (500 series), dual channel, in Rust on the [truce](https://github.com/truce-audio/truce) framework. Calibrated against the **hardware's** published Audio Precision measurements, not against another plug-in.

Formats: AU · VST3 · CLAP · Standalone (macOS). Modes: Dual Mono · Stereo · Mid/Side, with offset-preserving Link.

![Editor](docs/editor.png)

---

## Results

All numbers come from `tools/compare.py`: a stepped-sine analyzer (phase-free least-squares fit of H1–H10, plug-in warmed up before each measurement) run on the installed AU at 96 kHz, 0 dBu = −18 dBFS. The hardware curves are digitized from the AP plots in the 542 operations manual (`tools/hw/rnd542_ap_curves.json`). "RND plug-in" is the official Rupert Neve Designs 542 plug-in measured the same way.

**Frequency response at 0 dBu: error against the hardware (RMS / max, 10 Hz – 40 kHz)**

| Setting | This model | RND plug-in |
| :-- | :-- | :-- |
| 15 IPS, Sat MIN | 0.14 / 0.47 dB | 1.57 / 6.29 dB |
| 15 IPS, Sat 10 o'clock | 0.13 / 0.44 dB | 1.46 / 5.49 dB |
| 15 IPS, Sat 2 o'clock | 0.15 / 0.51 dB | 3.42 / 12.0 dB |
| 15 IPS, Sat MAX | 0.31 / 1.02 dB¹ | 3.73 / 13.0 dB |
| 30 IPS, Sat MIN | 0.09 / 0.27 dB | 0.71 / 2.75 dB |
| 30 IPS, Sat 10 o'clock | 0.07 / 0.25 dB | 0.99 / 3.14 dB |
| 30 IPS, Sat 2 o'clock | 0.12 / 0.41 dB | 2.86 / 10.7 dB |
| 30 IPS, Sat MAX | 0.21 / 0.68 dB¹ | 3.11 / 11.3 dB |
| SILK Red, Texture MAX (tape out) | 0.15 dB RMS | 6.10 dB RMS |
| SILK Blue, Texture MAX (tape out) | 0.10 dB RMS | 4.15 dB RMS |

¹ The largest error is above 20 kHz, at the edge of the oversampler's half-band filter (96 kHz host). Up to 20 kHz the error stays within ±0.33 dB.

**THD at 0 dBu, 15 IPS (%), all four Saturation positions from the hardware plot**

| Saturation | | 20 | 30 | 40 | 60 | 100 | 200 | 500 | 1k | 3k | 10k |
| :-- | :-- | --: | --: | --: | --: | --: | --: | --: | --: | --: | --: |
| MIN | Hardware | 4.74 | 3.29 | 2.64 | 2.02 | 1.45 | 0.86 | 0.34 | 0.16 | 0.09 | 0.10 |
| | This model | 4.98 | 3.72 | 2.95 | 2.14 | 1.39 | 0.78 | 0.41 | 0.23 | 0.13 | 0.08 |
| 10 o'clock | Hardware | 5.77 | 4.42 | 3.68 | 2.83 | 1.98 | 1.10 | 0.47 | 0.36 | 0.15 | 0.05 |
| | This model | 5.63 | 4.18 | 3.38 | 2.54 | 1.77 | 1.04 | 0.48 | 0.30 | 0.16 | 0.06 |
| 2 o'clock | Hardware | 6.68 | 5.16 | 4.30 | 3.26 | 2.25 | 1.23 | 0.66 | 0.58 | 0.26 | 0.05 |
| | This model | 6.43 | 4.64 | 3.78 | 2.96 | 2.20 | 1.40 | 0.63 | 0.37 | 0.14 | 0.05 |
| MAX | Hardware | >10 | >10 | 5.06 | 3.95 | 3.76 | 2.87 | 1.48 | 1.05 | 1.87 | 3.20 |
| | This model | 14.7 | 8.49 | 7.10 | 4.48 | 3.33 | 2.52 | 1.48 | 1.08 | 1.49 | 3.00 |
| any | RND plug-in | 0.00–3.1 | 0.00–4.7 | 0.01–5.6 | 0.00–6.4 | 0.00–6.3 | 0.00–4.8 | 0.00–3.3 | 0.00–3.1 | 0.00–3.2 | 0.00–2.9 |

RMS error of this model against the hardware THD, in dB: 1.7 / 0.9 / 2.2 / 1.4 for MIN / 10 / 2 o'clock / MAX. The previous version scored 2.0 / 1.8 / 3.3 / 5.2. At MIN, most of the remaining error comes from the 1–3 kHz points, where both values sit near the analyzer floor (≈ 0.1 %). The RND plug-in reads ≈ 0 % at the first three positions and roughly flat 3–6 % at MAX.

The RND plug-in is essentially distortion-free below about +6 dBu and then clips, with the same THD at 1 kHz and 8 kHz. The hardware distorts at normal levels and its distortion rises as `1/f`, which is the signature of the magnetic core. That low-level, frequency-dependent behaviour is what this model reproduces.

Known gaps:
- On the 10 and 2 o'clock traces, the hardware THD steps down sharply near 2 kHz, from a 0.36 / 0.58 % plateau. No smooth mechanism reproduces that step; it looks like an analyzer range change. The model stays about 2–4 dB below that plateau at 1 kHz.
- At Saturation MAX, 30–40 Hz is about 3 dB high: the model enters hard core saturation a little early. The hardware trace leaves the plot's 10 % scale below 29 Hz.
- 30 IPS has no published THD plot, so its non-linear behaviour follows from the 15 IPS fit and the 30 IPS frequency response.
- The hardware publishes no harmonic breakdown for SILK, so SILK harmonic levels follow RND's description (Red: upper mids and highs, mostly H2; Blue: lows and low mids, H2 + H3), not a measurement.

Validation: `auval` and `pluginval --strictness-level 5` pass. `examples/stress.rs` drives every mode at 44.1–192 kHz with silence, DC, noise, +12 dBFS square bursts and denormal-range input; the output stays finite throughout.

CPU: about 0.4 % (tape out) to about 4.5 % (Saturation MAX + SILK) of one Apple-silicon core per channel at 48 kHz.

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
   └── DRIVE amp: record share of the LF bump, IPS pre-emphasis (NAB 50 µs /      │
       AES 17.5 µs, shelved) → diode soft clip (anti-parallel, Newton-solved)    │
       → SATURATION TRANSFORMER "record head" (flux-domain Preisach hysteresis)  │
       → RECOVERY amp: 1/DRIVE (counter-ganged), de-emphasis, replay contour ─────┤
                                                                         BLEND ──┘
   → blend amp → [LEVEL meter] → output stage + output transformer with SILK → OUT
```

Everything non-linear runs at ≥ 176.4 kHz. The oversampler is a zero-latency polyphase IIR half-band using the elliptic design from *hiir*: passband flat to 20 kHz at 44.1 kHz, images below −90 dB.

**Magnetic cores (`magnetics.rs`).** A voltage-driven winding sets the flux, Φ = ∫v dt, so flux grows as 1/f. Hysteresis is a discrete Preisach model in Prandtl–Ishlinskii form: 32 play operators on a quadratic threshold grid with a fitted coercivity density. A uniform density gives Rayleigh's law, with distortion proportional to flux, mostly H3. The hardware rises more slowly, about as flux^0.7 over a 50:1 range, until hard saturation. The fitted density therefore has a power-law tail, p(r) ∝ r^−γ, the broad coercivity distribution of soft steels. The model carries memory and phase lag, and its permeability rises with amplitude, which produces the low-frequency "bloom". A Langevin-like anhysteretic knee follows. The secondary voltage is dΦ/dt, implemented as the exact discrete inverse of the integrator, so an ideal core is bit-transparent. All non-linear parameters of the record path are fitted jointly to all four THD curves and to the Saturation MAX frequency response (`tools/fit/fit_thd.py`): core, clip threshold, 15 IPS pre-emphasis shelf, the drive gain at each pot position (inside its Drive-meter LED window) and the record/replay split of the head bump.

**Where the head bump lives.** A bump placed entirely after the core raises the H3 of a 20 Hz tone (at 60 Hz) relative to its fundamental. THD would then climb much faster than 1/f below 100 Hz, and the hardware's doesn't. The fit puts about 40 % of the bump (in dB) in the record path, where it also shapes the flux. As Saturation rises, the core compresses that part of the bump, which matches the hardware's bump getting smaller at higher Saturation.

**Clip and HF THD.** At Saturation MAX the hardware reaches 3.2 % THD at 10 kHz, from the soft clip acting on the pre-emphasised highs. An exponential diode clip always compresses the fundamental along with it, about 1.4 dB for 3 % THD whatever its knee. The recovery amp's 1st-order HF shelf in the replay contour compensates that at the 0 dBu reference level, the same model-in-the-loop principle used for the LF bloom. At 15 IPS MAX the fitted shelf is +2.3 dB, centred at 5.7 kHz. Everywhere else it stays within ±1 dB.

**Replay contour (`tape.rs`).** A 2nd-order high-pass, a head-bump peak, a 2nd-order gap-loss low-pass and a 1st-order HF shelf at each documented pot position, interpolated between positions. They were fitted with the full non-linear model in the loop, because the AP curves were taken at 0 dBu where the core already adds level-dependent gain (`tools/fit/refit_contour.py`). The refit evaluates the exact digital (prewarped bilinear) response at the plug-in's real internal rate of 176.4/192 kHz.

**SILK (`silk.rs`).** A feedback model. Each fitted Red/Blue section N/D is opened by the Texture fraction t, giving N / ((1−t)N + tD): flat at t = 0, the measured curve at t = 1. The open-loop distortion of the single-ended output stage (H2, proportional to level) and of the output-transformer core (H3, proportional to 1/f) is computed on the signal and injected through the loop sensitivity S = 1/(1+T). Red raises S in the upper mids and highs, Blue in the lows.

**Meters.** DRIVE uses 8 LEDs, 3 dB apart: LED 2 = nominal, and soft clip starts above LED 5. LEVEL is the hardware's −10 / −2 / 4 / 10 / 14 / 18 / 20 / 22 dBu scale, taken pre-SILK. Both have LED ballistics.

### Interface
The panel is drawn with egui. Each channel is one row in signal-flow order: INPUT → TAPE (Tape In, IPS, Saturation, Drive meter) → BLEND → LEVEL → SILK → OUTPUT, styled after Ableton Live's flat dark panels. Rows are labelled A/B or MID/SIDE depending on mode. In Stereo mode row B is dimmed because channel A controls both. LINK keeps the relative offset between channels, as the RND manual specifies. Controls: drag, shift-drag for fine, double-click to reset, scroll wheel.

---

## Building

```bash
cargo install cargo-truce
cd neve542
cargo truce install --au2             # AU into ~/Library/Audio/Plug-Ins/Components
cargo truce build --au2 --vst3 --clap # all bundles into target/bundles/
cargo run --bin neve542-standalone --features standalone
```

## Measuring and calibrating

```bash
python3 tools/compare.py                                   # ours (AU) vs RND plug-in vs hardware
cargo run --release --example probe -- ips=15 sat=1 level=0   # DSP-level AP sweep
cargo run --release --example stress                       # robustness + CPU
cargo test --release --lib                                 # filter/oversampler/core/stage unit tests
tools/fit/calibrate.sh                                     # full tape-path calibration:
                                                           #   fit_thd.py (non-linear) <-> refit_contour.py
python3 tools/fit/fit_silk.py                              # refit SILK curves
```

## Credits & license
Hardware design: Rupert Neve (1926–2021) and Rupert Neve Designs. This is an independent model, not affiliated with RND. DSP, Rust implementation and UI: Jardim Sonoro ([nagualcode](https://github.com/nagualcode)). MIT license.
