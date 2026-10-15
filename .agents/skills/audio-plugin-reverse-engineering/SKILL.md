---
name: audio-plugin-reverse-engineering
description: >-
  Methodology and technical rigor for reverse-engineering analog audio hardware and legacy software plugins into high-precision, zero-allocation DSP plugins. Use this skill whenever researching, modeling, or implementing analog circuit emulations, component-level physics, hardware patent analysis, transformer non-linearities, Class-A discrete topologies, tape saturation, or harmonic measurement and validation.
---

# Audio Hardware & Plugin Reverse Engineering (Component-Level Modeling)

This skill codifies the complete, rigorous methodology for reverse-engineering analog audio hardware, legacy software plugins, and patented circuit designs into authentic, zero-allocation DSP plugins.

---

## 🏛️ Core Philosophy: Physical & Forensic Modeling

Never rely on superficial curve-fitting, black-box approximations, or generic memoryless waveshapers (such as bare `tanh(x)`). Real analog circuitry behaves non-linearly, dynamically, and asymmetrically due to the physical properties of iron cores, discrete transistor bias points, and feedback topologies.

An authentic analog emulation requires a **forensic investigation** that bridges:
1. **Physical Circuit Documentation** (Schematics, service manuals, component datasheets, Audio Precision factory plots).
2. **Patent Mining** (USPTO, Espacenet, Google Patents for the designer's disclosed secrets).
3. **Metallurgy & Semiconductor Physics** translated into closed-form mathematics and discrete-time DSP.
4. **Empirical Stepped-Sine Verification** (measuring individual harmonic profiles $H_1, H_2, H_3$ across frequency).
5. **Zero-Allocation Audio Thread Code** (guaranteed memory safety and zero audio dropouts).

---

## 🔎 Stage 1: The Hardware Forensic Investigation

When modeling any piece of hardware or vintage plugin:

### 1. Document Extraction
* Locate the **Operations Manual**, **Service Manual**, and **Schematics**.
* Extract factory **Audio Precision (AP)** test curves:
  * Frequency response sweeps at nominal levels (e.g. $0\text{ dBu} = -18\text{ dBFS}$).
  * Total Harmonic Distortion + Noise ($\text{THD+N}$) by level and frequency.
  * Roll-off and resonance frequencies (head bumps, transformer resonance, subsonic DC filters).

### 2. Active & Passive Component Analysis
* **Transformer Metallurgy:**
  * **Mu-Metal / High-Nickel (~80% Ni):** Used in input transformers. Features huge magnetic permeability ($\mu_r > 50,000$), low saturation flux ($B_{sat} \sim 0.7\text{ T}$), and near-zero coercivity. Delivers pristine linearity and low noise ($< 0.0025\%\text{ THD}$).
  * **Silicon Steel Laminations:** Used in tape heads, interstage inductors, and output transformers. Features high saturation flux ($B_{sat} \sim 1.5 - 2.0\text{ T}$), noticeable magnetic hysteresis, and progressive saturation that begins in low frequencies.
* **Amplifier Topology:**
  * **Single-Ended Class-A (e.g. Rupert Neve vintage/modern circuits):** Discrete bipolar transistors biased along an exponential transconductance curve ($I_c \propto e^{V_{be}/V_t}$). Compresses asymmetrical half-cycles unequally, generating the **2nd harmonic ($H_2$)** as the primary musical coloration.
  * **Symmetric Push-Pull / Op-Amps:** Symmetric clipping producing predominantly odd-order harmonics ($H_3, H_5$). Avoid using symmetric functions when modeling Class-A hardware!
* **Passive Filter Networks:**
  * Polypropylene/polyester film capacitors (e.g. WIMA, Vishay) used in equalization networks maintain phase transparency and rapid transient recovery.

### 3. Patent Mining (The Secret Weapon)
Manufacturers often keep their core innovation proprietary in marketing brochures, but **must fully disclose the circuit mechanisms in their patents**:
* **How to search:** Search USPTO, Google Patents, or Espacenet by designer name (e.g. *Rupert Neve*, *David Blackmer*, *George Massenburg*, *Ray Dolby*) or assignee.
* **What to extract:**
  * Exact feedback loop topologies (e.g. US Patent 7,454,025 for Neve SILK texture).
  * Diode/transistor limiter arrangements and thresholds.
  * State equations and circuit component values.

---

## ⚙️ Stage 2: Translating Physics into DSP Mathematics

### 1. Faraday's Law & Transformer Core Saturation ($1/f$ Weighting)
In an iron-core transformer or magnetic tape head, induced voltage is $V(t) = N \frac{d\Phi}{dt}$. Consequently, magnetic flux is:
$$\Phi(t) = \frac{1}{N} \int V(t) \, dt \propto \frac{V}{\omega}$$
* **The Physical Effect:** For an equal input voltage, a $50\text{ Hz}$ tone forces **10× more magnetic flux** through the core than a $500\text{ Hz}$ tone. Therefore, bass saturates first, creating low-end bloom and compression while high frequencies stay transparent.
* **DSP Implementation:**
  * Implement a matched **flux-weighting filter pair**:
    * Pre-saturation: Low-shelf emphasis boost ($+3\text{ dB}$ to $+6\text{ dB}$ below $150\text{ Hz}$, scaled by drive/saturation).
    * Core: Non-linear magnetic saturator.
    * Post-saturation: Matching low-shelf de-emphasis (exact mathematical inverse of the pre-filter).
  * **Transparency guarantee:** For small linear signals, $H(z) \cdot H^{-1}(z) \equiv 1$ (100% transparent and bit-clean). For large signals, bass pushes into saturation first.

### 2. Discrete Class-A Asymmetry & Second-Harmonic ($H_2$) Generation
To produce genuine analog second-harmonic warmth, the saturation transfer function **must contain an even-order non-linear term**:
* **Mathematical form:**
  $$y = x - \alpha_{\text{even}} \cdot \frac{x^2}{1 + |x|} - \beta_{\text{odd}} \cdot x^3$$
* Notice that $x^2 / (1 + |x|)$ is an **even function** ($f(-x) = f(x)$), which generates true $H_2$.
* Always follow with a subsonic high-pass filter ($5\text{ Hz}$, Butterworth Q=0.707) to remove the tiny DC bias generated by even-order rectification.

### 3. Variable Negative Feedback (The Patent-Derived SILK Model)
Do not model harmonic controls as additive static EQs. Model the closed-loop feedback amplifier:
$$y(t) = \frac{A(x(t))}{1 + A(x(t)) \cdot \beta(f, \text{Texture})}$$
* **Silk Off:** Full negative feedback ($\beta = 1.0$), suppressing distortion across all frequencies.
* **Silk Red:** Feedback path includes a high-pass network, reducing negative feedback at high frequencies ($3.5\text{ kHz} - 5\text{ kHz}$). Result: High-frequency lift accompanied by dynamic $H_2$ and $H_3$ harmonic generation.
* **Silk Blue:** Feedback path includes a low-pass network, reducing negative feedback at low frequencies ($< 120\text{ Hz}$). Result: Bass warmth accompanied by rich magnetic core bloom.

### 4. Dedicated Soft-Clipper at Elevated Drive
* When drive exceeds nominal headroom (e.g. Drive meter level $> 5$ / above $-12\text{ dBFS}$), engage an anti-parallel diode/transistor soft-knee limiter before inductive/core stages:
  $$x_{\text{clip}} = \begin{cases} x & \text{if } |x| \le x_{\text{thresh}} \\ \left(x_{\text{thresh}} + k \cdot \tanh\left(\frac{|x| - x_{\text{thresh}}}{k}\right)\right) \cdot \operatorname{sgn}(x) & \text{if } |x| > x_{\text{thresh}} \end{cases}$$

### 5. Tape Speed Equalization & Replay Compensation
* **Pre/De-emphasis:** Pair a high-shelf pre-emphasis filter with its exact inverse de-emphasis filter.
* **Replay Recovery Gain:** Scale output inversely with core compression:
  $$\text{Gain}_{\text{recovery}} = \frac{1.0}{1.0 + \kappa \cdot \sqrt{\text{Sat}}}$$
  Maintains the $1\text{ kHz}$ fundamental level steady within $\pm 0.3\text{ dB}$ across the full saturation sweep.
* **Head Bump Biquads:** Calibrate peaking filters to Audio Precision specs (e.g. $15\text{ IPS} \to 61.87\text{ Hz}$ at $+6.1\text{ dB}$; $30\text{ IPS} \to 101.67\text{ Hz}$ at $+4.3\text{ dB}$).

---

## 🎛️ Stage 3: Multi-Channel Topologies (Stereo, Dual Mono, M/S)

Provide comprehensive channel routing flexibility:
1. **Dual Mono:** Fully decoupled, independent channel strips for Left and Right.
2. **Stereo:** Channel A parameters govern both channels synchronously.
3. **Mid/Side (M/S):**
   * Power-preserving encode:
     $$M = \frac{L + R}{\sqrt{2}} \approx 0.70710678 \cdot (L + R), \quad S = \frac{L - R}{\sqrt{2}} \approx 0.70710678 \cdot (L - R)$$
   * Process Mid through Channel A, Side through Channel B.
   * Power-preserving decode:
     $$L = \frac{M + S}{\sqrt{2}}, \quad R = \frac{M - S}{\sqrt{2}}$$
   * Verify zero crosstalk ($< 10^{-6}$) between channels.
4. **Channel Linking (Link):** Synchronize parameter updates while preserving relative offsets.

---

## ⚡ Stage 4: Zero-Allocation Real-Time Audio Programming

In audio DSP programming (e.g. using Rust + Truce / nih-plug / JUCE):
* **Zero Allocations on Audio Thread:** Never call `Vec::new()`, `Box::new()`, `String`, or formatting macros inside `process()`.
* **Stack Frames:** Use stack-allocated per-frame buffers:
  ```rust
  buffer.for_each_frame_io::<2, 2, _>(|frame_in, frame_out| { ... });
  ```
* **Oversampling:** Wrap non-linear stages with minimum 2x or 4x polyphase half-band oversamplers to eliminate aliasing foldback.
* **Real-time verification:** Enable runtime allocation guards (e.g. `truce::enable_rt_paranoid!()`).

---

## 🔬 Stage 5: Empirical Verification & Validation Protocol

Always construct an automated test suite (e.g. using Python `pedalboard` + `pluginval`):

1. **Pluginval Compliance:**
   Run Steinberg `pluginval` at **Strictness Level 5**:
   ```bash
   pluginval --strictness-level 5 --skip-gui-tests --validate <plugin_path>
   ```
   Must pass with 0 errors.

2. **Stepped-Sine Harmonic Sweep:**
   * Feed single-frequency sine waves from $20\text{ Hz}$ to $40\text{ kHz}$.
   * Extract fundamental gain and harmonic amplitudes ($H_2, H_3, \dots, H_{10}$) using a phase-independent least-squares sinusoidal fit.
   * Verify frequency response against hardware manual specifications within $\pm 0.3\text{ dB}$.

3. **Harmonic Ratio Verification:**
   * Verify that Class-A and Silk modes produce genuine second-harmonic ($H_2$) dominance over third-harmonic ($H_3$).

4. **Hard Bypass Verification:**
   * Verify that engaging bypass produces bit-clean wire passthrough ($0.0000\%\text{ THD}$, $0.00\text{ dB}$ gain).

---

## 📐 Layout & Interface Architecture

* **Side-by-Side Dual Strip:** Place Channel A on the left, Master/Routing in the center, and Channel B on the right for ergonomic 19" rack-mount proportionality.
* **Height Clamping:** Set `.resizable(false)` or enforce explicit row/column clamps so the window height never grows vertically into empty margins inside the DAW.
