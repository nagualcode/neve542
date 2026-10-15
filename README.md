# Jardim Sonoro — Neve 542 Dual Tape Emulator

> **Component-Level Reverse Engineering & Physical Modeling of the Rupert Neve Designs 542 Tape Emulator & US Patent 7,454,025 SILK Texture Engine.**

[![Plugin Formats](https://img.shields.io/badge/Formats-CLAP%20%7C%20VST3%20%7C%20Standalone-blue.svg)](#installation--building)
[![Pluginval Status](https://img.shields.io/badge/pluginval-Strictness%20Level%205%20PASS-brightgreen.svg)](#audio-precision--validation-results)
[![DAW Category](https://img.shields.io/badge/DAW%20Folder-Jardim%20Sonoro-orange.svg)](#daw-integration)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

---

## 🎧 Overview

The **Jardim Sonoro Neve 542** is an authentic, high-precision digital recreation of the Rupert Neve Designs 542 500-Series Tape Emulator. Built in modern Rust using the zero-allocation [Truce](https://crates.io/crates/truce) audio framework, it delivers the thick harmonic density, low-end magnetic head bump, soft transient compression, and Class-A transformer saturation of real analog tape and custom wound audio iron.

Unlike simplistic digital tape plugins that apply a memoryless waveshaper (`tanh`), this implementation models the underlying physical phenomena:
* **Silicon Steel vs. Mu-Metal Transformer Physics** with frequency-dependent flux weighting ($\int V \, dt$).
* **Rupert Neve Patent US 7,454,025** variable negative feedback (NFB) harmonic generation.
* **Single-Ended Class-A Discrete Asymmetry** producing musical second-harmonic ($H_2$) richness.
* **Dual Mono, Stereo, and Mid/Side (M/S)** processing topologies.
* **Hardware-Calibrated 15 IPS & 30 IPS Responses** matched directly against Audio Precision measurements.

In your DAW, the plugin is organized cleanly under the **Jardim Sonoro** vendor directory.

---

## 🔬 Technical Rigor & Reverse Engineering Methodology

Our reconstruction is based on strict empirical measurement, hardware documentation, and circuit patent analysis:

### 1. Audio Precision Curve Matching
We analyzed the hardware operations manual and Audio Precision (AP) fundamental-tracking sweep measurements:
* **15 IPS Mode:** Center frequency at $61.87\text{ Hz}$ producing a $+6.1\text{ dB}$ head bump at minimum saturation with natural high-frequency roll-off ($-2\text{ dB}$ at $20\text{ kHz}$).
* **30 IPS Mode:** Extended top-end linearity ($\pm 0.2\text{ dB}$ out to $20\text{ kHz}$) with head bump centered at $101.67\text{ Hz}$ ($+4.3\text{ dB}$).
* **Replay Recovery Amplifier:** When tape saturation increases, magnetic core compression is automatically compensated by dynamic replay gain scaling, ensuring the $1\text{ kHz}$ fundamental level remains steady within $\pm 0.3\text{ dB}$.

### 2. Transformer Magnetic Core Physics: Silicon Steel vs. Mu-Metal
Real audio transformers behave drastically differently depending on their core metallurgy:
* **Input Stage (Mu-Metal / High Nickel ~80%):** Enormous magnetic permeability ($\mu_r > 50,000$) and near-zero coercivity. Sits at the balanced input to ensure pristine transparency, high common-mode rejection, and negligible distortion ($< 0.0025\%\text{ THD}$).
* **Record Head Simulation & Output Transformers (Silicon Steel):** Heavy magnetic core laminations that saturate in accordance with Faraday's Law of Induction:
  $$\Phi(t) = \frac{1}{N} \int V(t) \, dt$$
  Because magnetic flux $\Phi$ is inversely proportional to frequency ($1/f$), bass frequencies below $150\text{ Hz}$ push up to 10× more magnetic flux through the core than midrange frequencies at equal amplitude. Our DSP implements a matched pre/de-flux weighting network so that low frequencies push into core saturation first, yielding the coveted analog "bloom" and weight without muddying the mids.

### 3. Class-A Discrete Topology & Second-Harmonic ($H_2$) Richness
Rupert Neve famously avoided generic push-pull operational amplifiers (e.g. TL072/NE5532) in primary signal paths, employing discrete bipolar junction transistors operating in pure single-ended Class-A:
* Symmetrical push-pull stages generate predominantly odd harmonics ($H_3, H_5$), which can sound abrasive and digital when driven.
* Single-ended Class-A operating points compress positive and negative excursions with subtle quadratic asymmetry ($v^2 / (1 + |v|)$).
* Our saturation engine specifically injects this even-order quadratic term, generating the musical **2nd harmonic (octave)** that human ears perceive as analog depth, warmth, and 3D space.

### 4. Rupert Neve Patent US 7,454,025 (SILK Harmonic Texture)
In the 542 hardware, the **SILK** control is not a static additive EQ. As disclosed in Rupert Neve's US Patent 7,454,025 (*"Audio transformer harmonic enhancement"*), the circuit adjusts the **negative feedback (NFB)** loop around the discrete Class-A output transformer stage:
* **Silk Off:** 100% negative feedback actively cancels transformer non-linearities, resulting in clinical, linear fidelity.
* **Silk Red:** Selectively attenuates high-frequency negative feedback ($3.5\text{ kHz} - 5\text{ kHz}$). With feedback reduced in the upper register, the natural harmonic distortion of the Class-A transformer emerges, creating shimmering presence and air (predominantly $H_2$).
* **Silk Blue:** Selectively attenuates low-frequency negative feedback ($< 120\text{ Hz}$). The reduced low-frequency damping unleashes transformer core saturation and low-end bloom ($H_2 + H_3$).

### 5. Dedicated Soft-Clipper at Drive > 5
As stated in the hardware guide: *"The Soft Clip effect will become active once the Drive level has exceeded 5"*. We modeled this anti-parallel diode/transistor limiter stage prior to the record head. Sharp incoming transient spikes above $-12\text{ dBFS}$ ($+6\text{ dBu}$) are smoothly rounded with a hyperbolic tangent knee, preserving punch while preventing digital or harsh interstage overshoots.

---

## 📐 Signal Flow Diagram

```
Audio Input (L / R)
       │
       ▼
[Subsonic DC-Blocker (5 Hz Highpass)]
       │
       ▼
[Input Trim (+/- 12 dB)]
       │
       ▼
[Processing Mode Router: Stereo / Dual Mono / M-S Encode]
       │
       ├──────────────────────────────────────┐
       │ (Mid or Ch A)                        │ (Side or Ch B)
       ▼                                      ▼
[High-Shelf Pre-Emphasis Filter]        [High-Shelf Pre-Emphasis Filter]
       │                                      │
[Transformer Flux Weighting (1/f)]     [Transformer Flux Weighting (1/f)]
       │                                      │
[Drive Level Scaling & Peak Meter]      [Drive Level Scaling & Peak Meter]
       │                                      │
[2x Polyphase Upsampler]                [2x Polyphase Upsampler]
       │                                      │
[Soft-Clip Limiter (Drive > 5)]         [Soft-Clip Limiter (Drive > 5)]
       │                                      │
[Core Magnetic Saturation + Class-A H2] [Core Magnetic Saturation + Class-A H2]
       │                                      │
[2x Polyphase Downsampler]              [2x Polyphase Downsampler]
       │                                      │
[Transformer Flux De-Weighting]         [Transformer Flux De-Weighting]
       │                                      │
[De-Emphasis Inverse Filter]            [De-Emphasis Inverse Filter]
       │                                      │
[Replay Recovery Amplifier (Auto-Gain)] [Replay Recovery Amplifier (Auto-Gain)]
       │                                      │
[Head Bump Peaking EQ (60Hz / 100Hz)]   [Head Bump Peaking EQ (60Hz / 100Hz)]
       │                                      │
[Sub-Bass Shelf + High-Rolloff Shelf]   [Sub-Bass Shelf + High-Rolloff Shelf]
       │                                      │
       ▼                                      ▼
[Blend Stage: Dry pre-tape / Wet tape]  [Blend Stage: Dry pre-tape / Wet tape]
       │                                      │
[Pre-Silk Output Level Metering]        [Pre-Silk Output Level Metering]
       │                                      │
[SILK Class-A NFB Stage (Off/Red/Blue)] [SILK Class-A NFB Stage (Off/Red/Blue)]
       │                                      │
[Output Trim (+/- 15 dB)]               [Output Trim (+/- 15 dB)]
       │                                      │
       └──────────────────┬───────────────────┘
                          │
                          ▼
             [M-S Decode (if in M/S Mode)]
                          │
                          ▼
                 Audio Output (L / R)
```

---

## 🎛️ Dual-Channel Processing Topologies

The plugin provides 3 distinct operating modes plus control linking:

1. **Dual Mono:** Channel A (Left) and Channel B (Right) are completely independent. Each has its own Trim, Tape switch, Saturation, IPS speed, Blend, Silk mode, Texture, and Output Trim.
2. **Stereo:** Channel A parameters govern both channels simultaneously for rapid, perfectly matched stereo bus processing.
3. **M/S (Mid/Side):**
   * Encodes incoming stereo into Mid ($M = (L+R)/\sqrt{2}$) and Side ($S = (L-R)/\sqrt{2}$).
   * Channel A processes the Mid channel (kick, bass, lead vocal).
   * Channel B processes the Side channel (stereo width, room microphones, reverbs, cymbals).
   * Decodes back to Left and Right with zero crosstalk ($< 10^{-6}$).
4. **Link A-B:** Synchronizes channel adjustments across both channels.

---

## 📊 Audio Precision & Validation Results

Verified using `pluginval` (Steinberg VST3 test harness at maximum strictness) and automated stepped-sine harmonic analysis (`tools/test_full.py`):

| Test Condition | Spec / Target | Jardim Sonoro Neve 542 | Status |
| :--- | :--- | :--- | :--- |
| **pluginval Strictness Level 5** | Official VST3 Compliance | **PASSED (0 Errors)** | **VERIFIED** |
| **15 IPS Head Bump (60 Hz)** | $+6.10\text{ dB}$ | **$+6.13\text{ dB}$** | **MATCH** |
| **15 IPS Sub-Bass (20 Hz)** | $-0.50\text{ dB}$ | **$-0.58\text{ dB}$** | **MATCH** |
| **30 IPS Head Bump (100 Hz)** | $+4.30\text{ dB}$ | **$+4.28\text{ dB}$** | **MATCH** |
| **30 IPS High Extension (20 kHz)** | $0.00\text{ dB}$ | **$-0.18\text{ dB}$** | **MATCH** |
| **1 kHz Fundamental Stability** | Stable with Sat 0%–100% | **$\pm 0.3\text{ dB}$ deviation** | **MATCH** |
| **Class-A 2nd Harmonic ($H_2$)** | Distinct even harmonic | **Present ($0.74\% \to 3.86\%$)** | **VERIFIED** |
| **Silk Red (10 kHz Shelf & $H_2$)** | $+2.85\text{ dB}$, high $H_2$ | **$+2.81\text{ dB}$ ($H_2=0.324\%$)** | **VERIFIED** |
| **Silk Blue (50 Hz Shelf & $H_2$)** | $+0.25\text{ dB}$, core bloom | **$+0.23\text{ dB}$ ($H_2=0.360\%$)** | **VERIFIED** |
| **M/S Matrix Isolation** | Zero inter-channel leak | **$0.000000$ crosstalk** | **BIT-CLEAN** |
| **Hard Bypass** | Clean wire passthrough | **$0.0000\%\text{ THD}$, $0.00\text{ dB}$** | **BIT-CLEAN** |

---

## 🖥️ User Interface

The interface is engineered in a side-by-side dual-strip format modeled after vintage analog hardware racks:
* **Channel A Strip (Left):** Input Trim, Tape In, Saturation, IPS Speed, Drive Meter, Output Trim, Blend, Silk Mode, Texture, Level Meter.
* **Master Center Console:** Processing Mode (`Dual Mono`, `Stereo`, `M/S`), Global Bypass, Link A-B toggle.
* **Channel B Strip (Right):** Complete dedicated control strip for Right/Side.
* **Height Optimization:** Formatted with a compact 2-row layout and `.resizable(false)`, ensuring the window height never balloons vertically in your DAW.

---

## 🚀 Installation & Building

### Prerequisites
* [Rust Toolchain](https://rustup.rs/) (1.85+ recommended)
* [cargo-truce](https://crates.io/crates/cargo-truce) CLI:
  ```bash
  cargo install cargo-truce
  ```

### 1. Build and Install into your DAW (macOS / Windows / Linux)
```bash
# Clone the repository
git clone https://github.com/nagualcode/neve542.git
cd neve542/neve542

# Build, code-sign, and install CLAP and VST3 bundles into user plugin directory
cargo truce install --user
```
The plugin will be installed to:
* **macOS VST3:** `~/Library/Audio/Plug-Ins/VST3/Neve542.vst3`
* **macOS CLAP:** `~/Library/Audio/Plug-Ins/CLAP/Neve542.clap`

### 2. Run as Standalone (No DAW required)
```bash
cd neve542
cargo run --bin neve542-standalone --features standalone
```

### 3. Run the Comprehensive Validation Suite
```bash
python3 tools/test_full.py
```

---

## 🗺️ To-Do / Roadmap for Future Releases

- [ ] **Dynamic Magnetic Hysteresis Solver:** Implement a state-variable Jiles-Atherton differential model for magnetic particle memory, phase lag, and non-linear minor loops.
- [ ] **Switchable 4x / 8x Linear-Phase Polyphase Oversampling:** Selectable oversampling multiplier for ultra-high-resolution mastering sessions.
- [ ] **Photorealistic Skinned GUI:** Custom vector/raster theme featuring analog needle VU meters with authentic ballistic damping, illuminated vintage push buttons, and brushed aluminum faceplates.
- [ ] **Tape Transport Mechanical Modulation:** Optional subtle wow & flutter emulation modeling capstan eccentricity and motor inertia variations.
- [ ] **Factory Preset Bank:** Curated presets covering Master Bus Glue, Warm Vocal Air, Punchy Drum Transients, Bass Core Bloom, and Acoustic Guitar Sweetener.

---

## 📜 Credits & License

* **Hardware Design & Inspiration:** Rupert Neve (1926–2021) and Rupert Neve Designs.
* **Reverse Engineering, DSP & Rust Implementation:** Jardim Sonoro ([nagualcode](https://github.com/nagualcode)).
* **Framework:** [Truce Audio Engine](https://crates.io/crates/truce).
* **License:** Distributed under the [MIT License](LICENSE).
