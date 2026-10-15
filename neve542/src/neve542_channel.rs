//! Complete single-channel Rupert Neve 542 Tape Emulator processing strip.

use crate::biquad::{Biquad, Coefficients};
use crate::silk::{SilkMode, SilkStage};
use crate::tape::{TapeCircuit, TapeSpeed};

#[derive(Clone, Copy, Debug)]
pub struct Neve542Channel {
    dc_block: Biquad,
    pub tape: TapeCircuit,
    pub silk: SilkStage,

    // Meter peak trackers
    level_peak: f32,
}

impl Default for Neve542Channel {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Neve542Channel {
    pub fn new(sr: f32) -> Self {
        let mut ch = Self {
            dc_block: Biquad::new(Coefficients::high_pass(5.0, 0.707, sr)),
            tape: TapeCircuit::new(sr),
            silk: SilkStage::new(sr),
            level_peak: 0.0,
        };
        ch.set_sample_rate(sr);
        ch
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.dc_block.coeffs = Coefficients::high_pass(5.0, 0.707, sr);
        self.tape.set_sample_rate(sr);
        self.silk.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.dc_block.reset();
        self.tape.reset();
        self.silk.reset();
        self.level_peak = 0.0;
    }

    pub fn level_meter_value(&self) -> f32 {
        // Output level meter pre-Silk
        // 8 segments: -10, -2, 4, 10, 14, 18, 20, 22 dBu
        // 0 dBu = -18 dBFS.
        let peak_db = 20.0 * (self.level_peak.max(1e-6)).log10();
        let dbu = peak_db + 18.0;
        let normalized = ((dbu + 10.0) / 32.0).clamp(0.0, 1.0);
        normalized
    }

    /// Process a single sample through the channel strip.
    #[inline(always)]
    pub fn process_sample(
        &mut self,
        input: f32,
        trim_db: f32,
        tape_in: bool,
        tape_speed: TapeSpeed,
        saturation_norm: f32,
        blend_norm: f32,
        silk_mode: SilkMode,
        texture_norm: f32,
        output_db: f32,
    ) -> f32 {
        // 1. Input Stage: subsonic DC filter
        let clean = self.dc_block.process(input);

        // 2. Pre-tape TRIM (+/- 12 dB)
        let trim_gain = 10.0f32.powf(trim_db / 20.0);
        let pre_tape = clean * trim_gain;

        // 3. Tape Circuit & Blend
        let blended = if tape_in {
            let sat = saturation_norm.clamp(0.0, 1.0);
            self.tape.set_speed_and_saturation(tape_speed, sat);
            let post_tape = self.tape.process(pre_tape);

            let blend = blend_norm.clamp(0.0, 1.0);
            (1.0 - blend) * pre_tape + blend * post_tape
        } else {
            pre_tape
        };

        // 4. Output level tracking (Pre-Silk Level Meter, as per manual spec)
        let abs_blended = blended.abs();
        self.level_peak = self.level_peak * 0.9992 + abs_blended * 0.0008;

        // 5. Output Stage: SILK Circuit & Texture
        let tex = texture_norm.clamp(0.0, 1.0);
        self.silk.set_mode_and_texture(silk_mode, tex);
        let post_silk = self.silk.process(blended);

        // 6. Output Trim (+/- 15 dB)
        let out_gain = 10.0f32.powf(output_db / 20.0);
        post_silk * out_gain
    }
}
