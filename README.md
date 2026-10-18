# NEVE542 — Vintage Dual Tape Emulator (Jardim Sonoro)

> A dual-channel tape and transformer emulator in Rust on the [truce](https://github.com/truce-audio/truce) framework. At its core is a component-level model of the Rupert Neve Designs 542 Tape Emulator (500 series), calibrated against the **hardware's** published Audio Precision measurements rather than against another plug-in. Where no measurement exists, NEVE542 makes documented choices for the sound of an aged, hand-matched unit; see [Character](#character-the-vintage-unit).
>
> NEVE542 is an independent product. It is not affiliated with or endorsed by Rupert Neve Designs, and "542" and "Rupert Neve" are mentioned only to identify the hardware that was modelled.

Formats: AU · VST3 · CLAP · Standalone (macOS). Modes: Dual Mono · Stereo · Mid/Side, with offset-preserving Link.

![Editor](docs/editor.png)

**Contents:** [Results](#results) · [Signal lineage](#signal-lineage) · [Recreating the hardware](#recreating-the-hardware-sound) · [Black-box validation against the reference plug-in](#black-box-validation-against-the-reference-plug-in) · [Factory presets](#factory-presets) · [Character](#character-the-vintage-unit) · [The model](#the-model) · [Building](#building) · [Measuring](#measuring-and-calibrating)

---

## Results

All numbers come from `tools/compare.py`. It is a stepped-sine analyzer: a phase-free least-squares fit of H1–H10, with the plug-in warmed up before each measurement. It runs on the installed AU at 96 kHz, with 0 dBu = −18 dBFS. The hardware curves are digitized from the AP plots in the 542 operations manual (`tools/hw/rnd542_ap_curves.json`). "RND plug-in" is the official Rupert Neve Designs 542 plug-in, v1.0.7, measured the same way.

**Frequency response at 0 dBu: error against the hardware (RMS / max, 10 Hz – 40 kHz)**

| Setting | This model | RND plug-in |
| :-- | :-- | :-- |
| 15 IPS, Sat MIN | 0.13 / 0.49 dB | 1.57 / 6.29 dB |
| 15 IPS, Sat 10 o'clock | 0.17 / 0.52 dB | 1.46 / 5.49 dB |
| 15 IPS, Sat 2 o'clock | 0.16 / 0.51 dB | 3.42 / 12.0 dB |
| 15 IPS, Sat MAX | 0.20 / 0.59 dB¹ | 3.73 / 13.0 dB |
| 30 IPS, Sat MIN | 0.11 / 0.30 dB | 0.71 / 2.75 dB |
| 30 IPS, Sat 10 o'clock | 0.10 / 0.37 dB | 0.99 / 3.14 dB |
| 30 IPS, Sat 2 o'clock | 0.14 / 0.48 dB | 2.86 / 10.7 dB |
| 30 IPS, Sat MAX | 0.14 / 0.43 dB¹ | 3.11 / 11.3 dB |
| SILK Red, Texture MAX (tape out) | 0.15 dB RMS | 6.10 dB RMS |
| SILK Blue, Texture MAX (tape out) | 0.10 dB RMS | 4.15 dB RMS |

¹ The largest errors sit at 10 Hz and above 20 kHz, at the edge of the oversampler's half-band filter (96 kHz host). Measured on channel A, the calibrated reference; channel B carries the vintage tolerances described under Character.

**THD at 0 dBu, 15 IPS (%), all four Saturation positions from the hardware plot**

| Saturation | | 20 | 30 | 40 | 60 | 100 | 200 | 500 | 1k | 3k | 10k |
| :-- | :-- | --: | --: | --: | --: | --: | --: | --: | --: | --: | --: |
| MIN | Hardware | 4.74 | 3.29 | 2.64 | 2.02 | 1.45 | 0.86 | 0.34 | 0.16 | 0.09 | 0.10 |
| | This model | 4.99 | 3.80 | 3.06 | 2.20 | 1.42 | 0.81 | 0.42 | 0.23 | 0.13 | 0.08 |
| 10 o'clock | Hardware | 5.77 | 4.42 | 3.68 | 2.83 | 1.98 | 1.10 | 0.47 | 0.36 | 0.15 | 0.05 |
| | This model | 5.69 | 4.31 | 3.49 | 2.61 | 1.82 | 1.06 | 0.49 | 0.32 | 0.17 | 0.05 |
| 2 o'clock | Hardware | 6.68 | 5.16 | 4.30 | 3.26 | 2.25 | 1.23 | 0.66 | 0.58 | 0.26 | 0.05 |
| | This model | 6.40 | 4.84 | 3.95 | 3.05 | 2.22 | 1.40 | 0.70 | 0.37 | 0.16 | 0.04 |
| MAX | Hardware | >10 | >10 | 5.06 | 3.95 | 3.76 | 2.87 | 1.48 | 1.05 | 1.87 | 3.20 |
| | This model | 11.5 | 7.69 | 5.76 | 4.32 | 3.36 | 2.57 | 1.54 | 1.13 | 1.48 | 3.22 |
| any | RND plug-in | 0.00–3.1 | 0.00–4.7 | 0.00–5.6 | 0.00–6.4 | 0.00–6.3 | 0.00–4.8 | 0.00–3.3 | 0.00–3.1 | 0.00–3.2 | 0.00–2.9 |

RMS error of this model against the hardware THD, in dB: 1.9 / 0.7 / 2.0 / 1.0 for MIN / 10 / 2 o'clock / MAX, 1.51 overall. Earlier versions scored 1.55 (v0.4.0), 1.7 / 0.9 / 2.2 / 1.4 (v0.2.0) and 2.0 / 1.8 / 3.3 / 5.2 (v0.1.0).

These errors should be read against how precisely the target itself is known:
- The plot's THD axis is **linear**, from 0 to 10 %. One pixel is about 0.02 % and the trace is about 0.1 % thick, so a reading of 0.1 % is only known to about ±0.05 %. The fit uses an error model with an absolute and a relative part, σ = √(0.05 %² + (10 %·THD)²), and reaches 1.19 σ RMS.
- The manual's own specification table disagrees with its plot by up to about 3×. At 15 IPS MIN it gives 0.4 % at 3 kHz, where the plot shows 0.09 %. At MAX it gives 0.3 % at 1 kHz, where the plot shows 1.05 %. Units, or the measurement conditions, evidently vary by about ±3 dB.

The RND plug-in is essentially distortion-free below about +6 dBu and then clips, with the same THD at 1 kHz and 8 kHz. The hardware distorts at normal levels and its distortion rises roughly as `1/f`, which is the signature of the magnetic core. That low-level, frequency-dependent behaviour is what this model reproduces.

Tape out, the model meets the hardware specification: 0.067 % THD at 20 Hz, +20 dBu, of which H2 is 0.038 % and H3 is 0.050 % (spec: "0.07 % typical, 2nd & 3rd order"). At 1 kHz, +20 dBu it reads 0.003 % (spec: better than 0.0025 %).

Known gaps:
- On the 10 and 2 o'clock traces, the hardware THD steps down sharply near 2 kHz, from a 0.36 / 0.58 % plateau. The model stays about 2–4 dB below that plateau at 1 kHz. One white-box hypothesis was that two H2 sources cancel across the emphasis network. It was tested and rejected (see E9 below), so an analyzer range change remains the most likely explanation.
- At Saturation MAX, 40 Hz is about 1.1 dB high.
- 30 IPS has no published THD plot. Its non-linear behaviour follows from the 15 IPS fit and the 30 IPS frequency response.
- The hardware publishes no harmonic breakdown for any setting. The H2/H3 balance, the behaviour above 0 dBu and SILK's harmonics are design choices; see Character.

Validation: `auval` and `pluginval --strictness-level 5` pass. `examples/stress.rs` drives every mode at 44.1–192 kHz with silence, DC, noise, +12 dBFS square bursts and denormal-range input; the output stays finite throughout.

CPU: about 0.8 % (tape out) to about 5 % (Saturation MAX + SILK) of one Apple-silicon core per channel at 48 kHz.

---

## Signal lineage

**Design lineage.** Rupert Neve's tape emulators began at AMEK and continued at Rupert Neve Designs. There the Portico 5042 "True Tape" (two channels, 19") came first, followed in 2013 by the 542, its 500-series version. Both use the same idea. A real tape path is not reproduced with heads and tape. Instead, a custom **interstage transformer** stands in for the record head and is driven hard. A counter-ganged replay amplifier then restores the level, so that SATURATION changes how hard the iron works rather than how loud the module is. SILK, shared with the rest of the Portico range, varies the negative feedback around the output transformer.

**Signal chain.** This is the 542 block diagram (operations manual, p. 1), stage by stage, as modelled in `neve542/src/channel.rs`:

```
IN ─ INPUT XFMR ─ INPUT STAGE (±12 dB TRIM) ─┬───────────────────────────────────────────────┐ dry
                                             │                                               │
                    SAT ENGAGE ─ DRIVE amp ──┘ (IPS pre-emphasis, soft clip in its feedback)│
                                 │                                                           │
                    4 Hz coupling cap → SATURATION XFMR "record head" ──► DRIVE meter        │
                                 │                                                           │
                    RECOVERY amp (1/DRIVE, counter-ganged; de-emphasis; replay contour) ─────┤ wet
                                                                                             │
                                       BLEND pot (passive, dry ↔ wet) ─ SAT ENGAGE ─ BLEND AMP
                                       ─► LEVEL meter (pre-SILK) ─ OUTPUT STAGE ─ OUTPUT XFMR ─ OUT
                                                                      ▲            │
                                                                      └─ SILK Red/Blue + TEXTURE
                                                                         (feedback from the output
                                                                          transformer)
```

| Stage | What the hardware documents | How NEVE542 models it | Calibrated / validated against |
| :-- | :-- | :-- | :-- |
| Input transformer | Transformer-coupled input; tape-out THD 0.07 % @ 20 Hz, +20 dBu ("2nd & 3rd"); < 0.0025 % @ 1 kHz; 5 Hz–60 kHz ±0.25 dB, −2 dB @ 120 kHz | Mu-metal core (Preisach + saturation) with a little remanence, 0.8 Hz HP, 150 kHz LP | Tape-out specification (all three points) |
| Input stage, TRIM | Class-A, ±12 dB | Single-ended Class-A curvature, soft ±16 V rail knees, input noise that rises with TRIM | Spec THD / noise |
| DRIVE amp | "PRE-EMPHASIS/SOFT CLIP", IPS switch; soft clip active above Drive LED 5 | Shelved NAB 50 µs / AES 17.5 µs pre-emphasis, part of the head bump, a mismatched anti-parallel diode clip (Newton-solved) | THD at Sat MAX (HF rise), Drive meter legend of the plots |
| Saturation transformer | Custom interstage transformer as "record head" | Flux-domain Preisach hysteresis (power-law coercivity density), anhysteretic knee, remanent flux, exact integrator/differentiator pair | All four 15 IPS THD curves jointly |
| RECOVERY amp | Counter-ganged with DRIVE; DE-EMPHASIS | 1/DRIVE gain, de-emphasis, replay contour (HP, head-bump peak, gap-loss LP, 1st-order shelf) per pot position | All eight FR plots, with the non-linear model in the loop |
| BLEND | Passive pot between dry and wet nodes into a blend amp; does not affect SILK | Linear mix of the two nodes, so phase interaction at intermediate settings is kept; Class-A blend amp | Plug-in manual |
| Output stage + transformer + SILK | SILK reduces and frequency-shapes the feedback from the output transformer | Feedback model: Red/Blue sections opened by Texture; output-stage H2 and core H3 injected through the loop sensitivity | Tape-out SILK Red/Blue AP plots |

---

## Recreating the hardware sound

No 542 or 5042 schematic has been published, and no hardware unit was available to measure. The model was therefore built from three kinds of evidence:

1. **Hardware documents.** These are the operations manual (`tape_emulator.pdf`): block diagram, specification table and four AP plots. The plug-in user guide (`manual.pdf`) adds a little.
2. **Literature on the circuit family.** Sound On Sound reviews of the Portico 5042 and the 542, RND's own descriptions of SILK, and transformer theory (Whitlock / Jensen).
3. **Black-box measurement of the RND plug-in**, used for what it can tell and never as the calibration target (next section).

### White-box review of the hardware documents

A second pass read the documents as a circuit, asking what each one implies, and turned every implication into a test that can be falsified. The tests ran on the DSP directly (`examples/probe.rs`) and on the installed AU against the RND plug-in (`tools/compare.py`).

| # | Hypothesis drawn from the documents | Test | Result | Outcome |
| :-- | :-- | :-- | :-- | :-- |
| E1 | The plug-in guide says Saturation "also *controls* a low frequency compensation effect", so the bump shrinking with Saturation is a pot-linked EQ, not core compression | Response of the RND plug-in and of NEVE542 at −20, 0 and +8 dBu, every Saturation position, both speeds | The RND plug-in's bump shrinks with Saturation identically at −20 and 0 dBu: its developers made it linear. The hardware's specification table lists bump peaks (+5.0 / +2.4 / +3.15 / +1.3 dB for 15 MIN / 15 MAX / 30 MIN / 30 MAX) about 1 dB below the AP plots taken at 0 dBu. NEVE542 at −20 dBu gives +5.4 / +2.1 / +3.9 / +1.3 dB. That fits a bump with both a linear, pot-driven part and a level-dependent part (Rayleigh bloom in the head). | Kept. Pot-driven contour plus level bloom; the RND plug-in has only the first. |
| E2 | The THD plot's axis is linear, so readings below about 0.3 % are imprecise | Refit with an absolute + relative error model | 1.27 σ → 1.25 σ; the old fit was already near the structural optimum | Adopted (honest weighting) |
| E3 | The IPS switch sits only on the DRIVE amp, so the replay de-emphasis is fixed and IPS shapes only the record path | One fixed de-emphasis for both speeds; 30 IPS contour refit | The refit needs its HF shelf pinned at the +3 dB bound and the 30 IPS response error doubles (0.08 → 0.14 dB RMS) | Rejected. A block diagram omits switch poles; the data prefer a switched de-emphasis. |
| E4 | SOS: "constant-current drive into inductively coupled coils". Model the head as a circuit (source impedance into a non-linear magnetising inductance) instead of flux-domain | Simulation of both topologies with the same core law | Circuit topology: distortion = magnetising current × source impedance, so THD ∝ 1/f² (slope 2.0 per octave). Flux-domain: THD ∝ 1/f. Hardware: ≈ f^−0.74. | Rejected. The flux-domain core with a power-law coercivity density stays. |
| E5 | "PRE-EMPHASIS/SOFT CLIP" in one amp means diodes in its feedback; a resistor in series with them would reduce the slope instead of limiting | Diode-branch series resistance as a free fit parameter | Converges to 0.005 (effectively none). THD error 2.16 → 2.05 dB, mostly from the joint core refit. The HF compensation shelf stays at +0.5 dB. | The hardware clip limits. The parameter stays, at ≈ 0. |
| E6 | The tape-out specification (0.07 % @ 20 Hz with "2nd & 3rd", < 0.0025 % @ 1 kHz, both +20 dBu) is an always-on colour that should be met | Steady-state probe (6 s settle) | v0.4.0: 0.44 % at 20 Hz, H3 only; 0.015 % at 1 kHz. The input core sat too close to saturation, and four Class-A stages in series added up. | Fixed: input-core saturation and hysteresis, 2 flux units of remanence for the H2, gentler line-stage curvature, sharper rail knees, smaller SILK-off residual |
| E7 | The specification table gives THD for 30 IPS, which has no plot | Probe at those points | The table disagrees with the 15 IPS plot of the same manual (see Results), so it cannot be a target. At 30 IPS / MIN / 20 Hz it reads 5 %; the model reads 5.4 %. | Used only as an estimate of unit spread (±3 dB) |
| E9 | The 2 kHz THD step is two H2 sources (clip asymmetry before the core, replay curvature after it) cancelling across the emphasis network | Flip the sign of each H2 source | Each flip shifts the curve up or down uniformly; no step appears | Rejected; still unexplained |

One measurement pitfall turned up along the way. A sine that starts abruptly at 20 Hz leaves a DC flux in the input core, which decays over about a second, as in a real transformer. Measurements with short settling read the tape-out THD 7× too high. `examples/probe.rs` now takes `settle=<seconds>`.

---

## Black-box validation against the reference plug-in

**The reference.** The official RND 542 plug-in was developed for Rupert Neve Designs by McDSP. Its bundle id is `com.mcdsp.auvst.rnd542tape`, and it is built with JUCE. Public release v1.0.1 came out on 2026-09-10; v1.0.7 (2026-09-22) "restored harmonic modeling errantly turned off". All measurements here are of v1.0.7. The binary is stripped and wrapped in PACE Fusion, and its licence (§5.1) forbids reverse engineering, decompiling, disassembling and decrypting it. The plug-in was therefore studied **only as a black box**, through its audio, with `tools/analyzer.py`.

**What it measures like** (warmed up, with silence between tones; it ramps its parameters for about 0.6 s after any reset):
- **Distortion:** a static waveshaper with a threshold. It is clean (< 0.01 %) up to about +6 dBu at Saturation MIN, then mostly H3 + H5, the same at 1 kHz and 5 kHz.
- **No 1/f transformer distortion** at 0 dBu, although the hardware plots show 1.5–5 % below 100 Hz.
- **SMPTE IMD:** 0.003 % at Sat MIN and 0 dBu. NEVE542 gives 4.9 %, the tape-like value its core physics produces.
- **Head bump:** shrinks with Saturation identically at −20 and 0 dBu (linear and pot-driven), and is level-independent up to its clip. At Sat MAX and +8 dBu it drops about 4 dB broadband.
- **Low end:** much deeper than the hardware at high Saturation. At 15 IPS / MAX and 10 Hz it is −17 dB, where the hardware is −4.5 dB.
- **SILK:** mostly H2, about 2.3 % at +16 dBu (Red, Texture MAX, 100 Hz). The SILK frequency responses miss the hardware by 4–6 dB RMS.
- **Noise and latency:** zero latency and no noise.

**What it confirms for NEVE542:**
- The topology: separate tape and SILK sections, Blend not affecting SILK, Link keeping offsets.
- The pot law and a pot-linked LF compensation.
- That SILK's colour is predominantly second order.

**Where NEVE542 deliberately departs from it:** wherever the hardware's own plots say otherwise. That covers the low-level 1/f core distortion, the LF response at high Saturation and the SILK curves. Across the eight tape settings, the plug-in's response error against the hardware is 0.7–3.7 dB RMS, against 0.10–0.20 dB for NEVE542.

---

## Factory presets

NEVE542 ships the 30 factory and artist presets of the RND 542 plug-in, converted by `tools/presets/import_rnd.py` into truce `.preset` files (`neve542/presets/<Category>/`). `cargo truce install` turns them into native AU/VST3/CLAP presets. In AU hosts they appear as factory presets: Bass, Default, Drums, Guitar, Mastering, Vocals, and Artist.

**Conversion.** Only the parameter values are read, from the JUCE state stored in the RND preset files; the plug-in itself is not touched.
- Saturation, Blend and Texture: percent → 0–1. This is the same pot rotation, and NEVE542 is calibrated at the hardware's MIN / 10 / 2 o'clock / MAX = 0 / 0.3 / 0.7 / 1.
- Trim and Output: dB, same ranges.
- IPS, Silk mode and processing mode: index for index.
- The AU bank is canonical. Two presets exist only in the VST3 bank and are taken from there.
- RND's AU bank spells one preset "Black Vox Tape"; it is the VST3 bank's "Back Vox Tape".
- RND's hidden `Sat Enable` parameter has no audible effect and is dropped.
- The text labels of RND's processing-mode parameter don't match its behaviour. The value labelled "Dual" processes mid/side: an L-only input leaks into R when A and B differ. The conversion follows the behaviour.

**Signal check against the RND plug-in** (`tools/presets/validate.py`). Every preset is played through both plug-ins with the same settings, on each processing path (A/B, or M and S):
- **Linear response (−20 dBu):** above 31.5 Hz the two agree within ±0.7 dB in every preset. That confirms the control mapping and that both mix BLEND linearly, with the same wet/dry phase.
- **Below 31.5 Hz:** the presets differ by 2–11 dB at 20 Hz. These are the presets at high Saturation, where the RND plug-in rolls the deep bass off much harder than the hardware (at Sat MAX, −17 dB at 10 Hz, against −4.5 dB on the AP plot). NEVE542 follows the hardware.
- **Distortion:** the RND plug-in is clean below about +6 dBu. Presets that push Trim +3…+7 dB therefore measure 0.1–0.4 % THD at 100 Hz there, against 1–3 % in NEVE542, which follows the hardware's low-frequency core distortion. Presets that drive hard at Saturation MAX ("Fried", "Snare Sweetener") distort more in the RND plug-in (16–21 % against 8–11 %), because above 0 dBu NEVE542 saturates more softly. That is one of the design choices; see Character.

---

## Character: the vintage unit

The published plots fix the frequency response and the THD at 0 dBu, but not the harmonic balance, the behaviour above 0 dBu, noise, or unit variation. For those, NEVE542 makes choices drawn from the hardware literature, reviews and forum lore. The result is meant to sound like a well-used, hand-matched unit, and every choice is listed here.

| Trait | Choice | Basis |
| :-- | :-- | :-- |
| Low-frequency colour | Transformer-core 1/f distortion, predominantly H3, kept from the hardware fit | SOS on the 5042: "considerable amounts of low-frequency distortion (predominantly third harmonic)"; the hardware AP plots |
| Soft clip | Diode pair deliberately mismatched (Is ratio 1:12.5), so H2 rises with the clip before H3 takes over (at 5 kHz, Sat MAX, 0 dBu: H2 1.9 %, H3 1.6 %) | RND: the soft clip "enhances 2nd-order harmonics"; SOS 542 review. Rupert Neve, citing Moir, held that odd harmonics are more readily heard and "spoil" listening while even ones are benign, increasingly so at high frequency. A Gearspace user on the RND plug-in: "when it distorts it gets nasty". |
| Clip ↔ head coupling | A 4 Hz coupling capacitor between DRIVE amp and head. The clip's rectified DC shifts the bias for a moment instead of magnetising the head, so heavy clipping breathes with the programme. | Standard practice for any amplifier driving a transformer |
| Head remanence | A small remanent flux in the record head (an un-demagnetised head) tilts the saturation curve. It adds H2 that grows with level (50 Hz, 0 dBu: H2/H3 ≈ 0.4; +12 dBu: ≈ 0.55) while the low end stays H3-led. | Tape-machine practice of regular head demagnetising, because magnetised heads add even-order distortion and hiss |
| Input-transformer remanence | Enough remanence that the tape-out 20 Hz distortion is H2 + H3, as the specification states | 542 specification ("2nd & 3rd order harmonics") |
| SILK | Output stage curvature raised and output-core hysteresis lowered, so Texture is mostly 2nd order: about 2.4–3.3 % H2 at +16 dBu (1–5 kHz, Red), approaching 4–5 % just before the output stage reaches its rails | RND / dealer copy: "effect ranges from essentially absent to roughly 4–5 % THD (mostly second order)"; the RND plug-in's SILK is H2-dominant too |
| Channel matching | Channel A is the calibrated reference. Channel B carries the tolerances of a 40-year-old matched pair: head bump +1.8 % in frequency and +0.12 dB, DRIVE pot tracking +0.15 dB, head saturation −3 %, more remanence, slightly different diodes and SILK core. A and B differ by ≤ 0.06 dB at 0 dBu. | Unit-to-unit variation of vintage Neve modules (GroupDIY and Gearspace restoration threads); the ±3 dB spread between the 542's own spec table and plots |
| Noise | Uncorrelated hiss per channel, about −98 dBu (−116 dBFS), split between input (rises with TRIM) and output stage. That is a couple of dB above the spec's "better than −100 dBu" for an aged unit. | 542 specification; SOS and Tape Op note audible hiss when the 5042 is driven to extremes |
| IMD | Left as the core physics gives it: SMPTE 4.9 % at MIN and 0 dBu. That is tape-like, where a voltage-driven line transformer would be about ten times lower (Jensen). | The 542's head stage is a deliberately overdriven "tape" transformer, not a line transformer |

Not modelled on purpose: wow and flutter, dropouts and tape hiss modulation. The 542 has no transport, and its character is electronic and magnetic.

---

## The model

### Sources
- RND 542 operations manual: block diagram, specifications, and the four AP plots (`tape_emulator.pdf`).
- RND 542 plug-in user guide (`manual.pdf`).
- Sound On Sound reviews of the Portico 5042 and the RND 500 series. Key points: a constant-current drive into inductively coupled coils; LF distortion is "predominantly third harmonic"; the soft clip adds 2nd harmonic at higher levels; Silk Red adds about +1.5 dB overall and +3 dB above 5 kHz; Silk Blue adds about +1.5 dB at 30 Hz.
- RND's own description of Silk: it reduces the negative feedback taken from the output transformer and changes that feedback's frequency response.
- B. Whitlock, "Audio Transformers" (*Handbook for Sound Engineers*): transformer distortion as magnetising current through the source impedance (used for the rejected E4).
- Black-box measurements of the RND 542 plug-in (see above), and forum and press discussion (Gearspace, GroupDIY, Steve Hoffman forums, Tape Op, Jensen/Whitlock transformer notes) for the Character choices.
- No schematic of the 542 or 5042 has been published. Earlier versions of this README cited "US Patent 7,454,025" for Silk. **That was wrong.** US 7,454,025 is a Panasonic loudspeaker patent and has nothing to do with Neve.

Everything non-linear runs at ≥ 176.4 kHz. The oversampler is a zero-latency polyphase IIR half-band using the elliptic design from *hiir*: passband flat to 20 kHz at 44.1 kHz, images below −90 dB.

**Magnetic cores (`magnetics.rs`).**
- **Flux.** A voltage-driven winding sets the flux, Φ = ∫v dt, so flux grows as 1/f.
- **Hysteresis.** It uses a discrete Preisach model in Prandtl–Ishlinskii form: 32 play operators on a quadratic threshold grid with a fitted coercivity density.
- **Coercivity density.** A uniform density gives Rayleigh's law, with distortion proportional to flux, mostly H3. The hardware rises more slowly, about as flux^0.7 over a 50:1 range, until hard saturation. The fitted density therefore has a power-law tail, p(r) ∝ r^−γ, the broad coercivity distribution of soft steels.
- **Memory and bloom.** The model carries memory and phase lag, and its permeability rises with amplitude, which produces the low-frequency "bloom". A Langevin-like anhysteretic knee follows.
- **Output.** The secondary voltage is dΦ/dt, implemented as the exact discrete inverse of the integrator, so an ideal core is bit-transparent.
- **Why flux-domain.** The circuit-level alternative (E4) was tested and gives the wrong slope against frequency.
- **Fitting.** All non-linear parameters of the record path are fitted jointly to all four THD curves and to the Saturation MAX frequency response (`tools/fit/fit_thd.py`). That covers the core, the clip threshold and series resistance, the 15 IPS pre-emphasis shelf, the drive gain at each pot position (inside its Drive-meter LED window) and the record/replay split of the head bump.

**Where the head bump lives.** A bump placed entirely after the core raises the H3 of a 20 Hz tone (at 60 Hz) relative to its fundamental. THD would then climb much faster than 1/f below 100 Hz, and the hardware's doesn't. The fit puts about 41 % of the bump (in dB) in the record path, where it also shapes the flux. The rest follows the pot linearly through the replay contour, which E1 shows is how the reference plug-in treats it too. The record-path part makes the bump grow by 0.5–1.3 dB from −20 to 0 dBu (depending on speed and Saturation), consistent with the gap between the hardware's specification table and its 0 dBu plots.

**Clip and HF THD.** At Saturation MAX the hardware reaches 3.2 % THD at 10 kHz, from the soft clip acting on the pre-emphasised highs. An exponential diode clip always compresses the fundamental along with it. The recovery amp's 1st-order HF shelf in the replay contour compensates that at the 0 dBu reference level: +0.5 dB centred near 3 kHz at 15 IPS MAX. A series resistor in the diode branch was offered to the fit as an alternative (E5), and the fit declined it.

**Replay contour (`tape.rs`).** At each documented pot position, the contour is a 2nd-order high-pass, a head-bump peak, a 2nd-order gap-loss low-pass and a 1st-order HF shelf, interpolated between positions. They were fitted with the full non-linear model in the loop, because the AP curves were taken at 0 dBu, where the core already adds level-dependent gain (`tools/fit/refit_contour.py`). The refit evaluates the exact digital (prewarped bilinear) response at the plug-in's real internal rate of 176.4/192 kHz.

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
cargo run --release --example probe -- ips=15 sat=1 level=0   # DSP-level AP sweep (settle=<s> for steady state)
cargo run --release --example stress                       # robustness + CPU
cargo test --release --lib                                 # filter/oversampler/core/stage unit tests
tools/fit/calibrate.sh                                     # full tape-path calibration:
                                                           #   fit_thd.py (non-linear) <-> refit_contour.py
FIX_RD=0 tools/fit/calibrate.sh                            # same, with the classic limiting diode clip
python3 tools/fit/fit_thd.py --eval                        # score the current parameters (dB and sigma)
python3 tools/fit/fit_silk.py                              # refit SILK curves
python3 tools/presets/import_rnd.py                        # RND_542_Tape/ -> neve542/presets/
python3 tools/presets/validate.py                          # every preset: NEVE542 vs RND plug-in
```

## Credits & license
Modelled hardware: the 542 Tape Emulator by Rupert Neve (1926–2021) and Rupert Neve Designs. NEVE542 is an independent product, not affiliated with or endorsed by RND. DSP, Rust implementation and UI: Jardim Sonoro ([nagualcode](https://github.com/nagualcode)). MIT license.
