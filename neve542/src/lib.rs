//! NEVE542 (Jardim Sonoro) — dual-channel vintage tape emulator. Starts
//! from a component-level model of the Rupert Neve Designs 542 Tape Emulator
//! (500 series), calibrated to its published AP plots, and adds the
//! character of an aged, hand-matched unit where no measurement exists.
//!
//! Signal chain per channel (see `channel.rs`): mu-metal input transformer →
//! Class-A TRIM stage → tape circuit (DRIVE amp with IPS pre-emphasis and
//! diode soft clip → hysteretic "record head" transformer → counter-ganged
//! RECOVERY amp with de-emphasis and replay contour) → BLEND amp → output
//! stage + output transformer with SILK (frequency-shaped negative-feedback
//! reduction). Dual Mono, Stereo and Mid/Side operation.

pub mod amp;
pub mod channel;
pub mod filters;
pub mod magnetics;
pub mod oversampler;
pub mod silk;
pub mod tape;
mod ui;

use std::sync::Arc;
use truce::prelude::*;

use channel::{Channel, Controls};
pub use silk::SilkMode;
pub use tape::TapeSpeed;

#[derive(ParamEnum)]
pub enum ProcessingMode {
    #[name = "Dual Mono"]
    DualMono,
    #[name = "Stereo"]
    Stereo,
    #[name = "M/S"]
    MidSide,
}

#[derive(ParamEnum)]
pub enum IpsSpeed {
    #[name = "15 IPS"]
    Ips15,
    #[name = "30 IPS"]
    Ips30,
}

#[derive(ParamEnum)]
pub enum SilkModeEnum {
    #[name = "Off"]
    Off,
    #[name = "Red"]
    Red,
    #[name = "Blue"]
    Blue,
}

#[derive(Params)]
pub struct Neve542Params {
    // --- Master / Routing Controls ---
    #[param(name = "Processing Mode", short_name = "Mode")]
    pub mode: EnumParam<ProcessingMode>,

    #[param(name = "Link Channels", short_name = "Link", default = false)]
    pub link: BoolParam,

    #[param(name = "Bypass", short_name = "Bypass", default = false)]
    pub bypass: BoolParam,

    // --- Channel A (Left / Mid) Controls ---
    #[param(
        name = "Trim A",
        short_name = "Trim A",
        range = "linear(-12, 12)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub trim_a: FloatParam,

    #[param(name = "Tape In A", short_name = "Tape A", default = true)]
    pub tape_in_a: BoolParam,

    #[param(
        name = "Saturation A",
        short_name = "Sat A",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub saturation_a: FloatParam,

    #[param(name = "Speed A", short_name = "IPS A")]
    pub ips_a: EnumParam<IpsSpeed>,

    #[param(
        name = "Blend A",
        short_name = "Blend A",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub blend_a: FloatParam,

    #[param(name = "Silk A", short_name = "Silk A")]
    pub silk_a: EnumParam<SilkModeEnum>,

    #[param(
        name = "Texture A",
        short_name = "Tex A",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub texture_a: FloatParam,

    #[param(
        name = "Output A",
        short_name = "Out A",
        range = "linear(-15, 15)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub output_a: FloatParam,

    // --- Channel B (Right / Side) Controls ---
    #[param(
        name = "Trim B",
        short_name = "Trim B",
        range = "linear(-12, 12)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub trim_b: FloatParam,

    #[param(name = "Tape In B", short_name = "Tape B", default = true)]
    pub tape_in_b: BoolParam,

    #[param(
        name = "Saturation B",
        short_name = "Sat B",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub saturation_b: FloatParam,

    #[param(name = "Speed B", short_name = "IPS B")]
    pub ips_b: EnumParam<IpsSpeed>,

    #[param(
        name = "Blend B",
        short_name = "Blend B",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub blend_b: FloatParam,

    #[param(name = "Silk B", short_name = "Silk B")]
    pub silk_b: EnumParam<SilkModeEnum>,

    #[param(
        name = "Texture B",
        short_name = "Tex B",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        smooth = "exp(10)"
    )]
    pub texture_b: FloatParam,

    #[param(
        name = "Output B",
        short_name = "Out B",
        range = "linear(-15, 15)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(10)"
    )]
    pub output_b: FloatParam,

    // --- Meters ---
    #[meter]
    pub drive_meter_a: MeterSlot,

    #[meter]
    pub drive_meter_b: MeterSlot,

    #[meter]
    pub level_meter_a: MeterSlot,

    #[meter]
    pub level_meter_b: MeterSlot,
}

use Neve542ParamsParamId as P;

pub struct Neve542;

pub struct Neve542Dsp {
    pub channels: [Channel; 2],
    pub sample_rate: f64,
}

impl Default for Neve542Dsp {
    fn default() -> Self {
        Self { channels: [Channel::for_unit(48_000.0, 0), Channel::for_unit(48_000.0, 1)], sample_rate: 48_000.0 }
    }
}

fn speed(v: IpsSpeed) -> TapeSpeed {
    match v {
        IpsSpeed::Ips15 => TapeSpeed::Ips15,
        IpsSpeed::Ips30 => TapeSpeed::Ips30,
    }
}

fn silk(v: SilkModeEnum) -> SilkMode {
    match v {
        SilkModeEnum::Off => SilkMode::Off,
        SilkModeEnum::Red => SilkMode::Red,
        SilkModeEnum::Blue => SilkMode::Blue,
    }
}

fn controls_a(p: &Neve542Params) -> Controls {
    Controls {
        trim_db: p.trim_a.value() as f64,
        tape_in: p.tape_in_a.value(),
        speed: speed(p.ips_a.value()),
        saturation: p.saturation_a.value() as f64,
        blend: p.blend_a.value() as f64,
        silk: silk(p.silk_a.value()),
        texture: p.texture_a.value() as f64,
        output_db: p.output_a.value() as f64,
    }
}

fn controls_b(p: &Neve542Params) -> Controls {
    Controls {
        trim_db: p.trim_b.value() as f64,
        tape_in: p.tape_in_b.value(),
        speed: speed(p.ips_b.value()),
        saturation: p.saturation_b.value() as f64,
        blend: p.blend_b.value() as f64,
        silk: silk(p.silk_b.value()),
        texture: p.texture_b.value() as f64,
        output_db: p.output_b.value() as f64,
    }
}

impl PluginLogic for Neve542 {
    type Params = Neve542Params;
    type DspState = Neve542Dsp;

    fn reset(state: &mut Self::DspState, params: &Self::Params, config: &AudioConfig) {
        state.sample_rate = config.sample_rate;
        let (a, b) = (controls_a(params), controls_b(params));
        let stereo = params.mode.value() == ProcessingMode::Stereo;
        for (i, ch) in state.channels.iter_mut().enumerate() {
            *ch = Channel::for_unit(config.sample_rate, i);
            ch.set_controls(if i == 0 || stereo { a } else { b });
            ch.reset();
        }
    }

    fn process(
        state: &mut Self::DspState,
        params: &Self::Params,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        if params.bypass.value() {
            let n_in = buffer.num_input_channels();
            for ch in 0..buffer.channels() {
                let (inp, out) = buffer.io(ch);
                if ch < n_in {
                    out.copy_from_slice(inp);
                } else {
                    out.fill(0.0);
                }
            }
            return ProcessStatus::Normal;
        }

        let mode = params.mode.value();
        let a = controls_a(params);
        // STEREO assigns CH A controls to both channels (RND manual).
        let b = if mode == ProcessingMode::Stereo { a } else { controls_b(params) };
        state.channels[0].set_controls(a);
        state.channels[1].set_controls(b);

        let ms = mode == ProcessingMode::MidSide;
        let [c0, c1] = &mut state.channels;
        buffer.for_each_frame_io::<2, 2, _>(|inp, out| {
            let (mut l, mut r) = (inp[0], inp[1]);
            if ms {
                let m = (l + r) * std::f32::consts::FRAC_1_SQRT_2;
                let s = (l - r) * std::f32::consts::FRAC_1_SQRT_2;
                l = m;
                r = s;
            }
            let (y0, y1) = (c0.process(l), c1.process(r));
            if ms {
                out[0] = (y0 + y1) * std::f32::consts::FRAC_1_SQRT_2;
                out[1] = (y0 - y1) * std::f32::consts::FRAC_1_SQRT_2;
            } else {
                out[0] = y0;
                out[1] = y1;
            }
        });

        context.set_meter(P::DriveMeterA, state.channels[0].drive_meter());
        context.set_meter(P::DriveMeterB, state.channels[1].drive_meter());
        context.set_meter(P::LevelMeterA, state.channels[0].level_meter());
        context.set_meter(P::LevelMeterB, state.channels[1].level_meter());
        ProcessStatus::Normal
    }

    fn editor(params: Arc<Neve542Params>) -> Box<dyn Editor> {
        ui::editor(params)
    }
}

truce::plugin! {
    logic: Neve542,
    params: Neve542Params,
}

truce::enable_rt_paranoid!();
