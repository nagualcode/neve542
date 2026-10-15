//! Rupert Neve 542 500 Series Tape Emulator Audio Plugin.
//! High-precision simulation of custom transformers, Class-A discrete circuitry,
//! magnetic saturation, and patent US 7,454,025 SILK texture.
//!
//! Supports Dual Mono, Stereo, and Mid/Side (M/S) processing modes with Channel Linking.

mod biquad;
mod neve542_channel;
mod oversampler;
mod silk;
mod tape;

use std::sync::Arc;
use truce::prelude::*;
use truce_gui::IntoLayoutEditor;
use truce_gui_types::layout::{dropdown, knob, meter, toggle, widgets, GridLayout};

use neve542_channel::Neve542Channel;
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
    pub channels: [Neve542Channel; 2],
    pub sample_rate: f32,
}

impl Default for Neve542Dsp {
    fn default() -> Self {
        Self {
            channels: [Neve542Channel::new(48000.0), Neve542Channel::new(48000.0)],
            sample_rate: 48000.0,
        }
    }
}

impl PluginLogic for Neve542 {
    type Params = Neve542Params;
    type DspState = Neve542Dsp;

    fn reset(state: &mut Self::DspState, _params: &Self::Params, config: &AudioConfig) {
        state.sample_rate = config.sample_rate as f32;
        for ch in &mut state.channels {
            ch.set_sample_rate(config.sample_rate as f32);
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
        let num_channels = buffer.channels();

        // Host bypass path
        if params.bypass.value() {
            let n_in = buffer.num_input_channels();
            for ch in 0..num_channels {
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
        let link = params.link.value();

        // Channel A settings
        let trim_a = params.trim_a.value();
        let tape_in_a = params.tape_in_a.value();
        let tape_speed_a = match params.ips_a.value() {
            IpsSpeed::Ips15 => TapeSpeed::Ips15,
            IpsSpeed::Ips30 => TapeSpeed::Ips30,
        };
        let sat_a = params.saturation_a.value();
        let blend_a = params.blend_a.value();
        let silk_a = match params.silk_a.value() {
            SilkModeEnum::Off => SilkMode::Off,
            SilkModeEnum::Red => SilkMode::Red,
            SilkModeEnum::Blue => SilkMode::Blue,
        };
        let tex_a = params.texture_a.value();
        let out_a = params.output_a.value();

        // Channel B settings: in Stereo mode or with Link active, Channel A commands both
        let (trim_b, tape_in_b, tape_speed_b, sat_b, blend_b, silk_b, tex_b, out_b) =
            if mode == ProcessingMode::Stereo || link {
                (trim_a, tape_in_a, tape_speed_a, sat_a, blend_a, silk_a, tex_a, out_a)
            } else {
                let s_b = match params.ips_b.value() {
                    IpsSpeed::Ips15 => TapeSpeed::Ips15,
                    IpsSpeed::Ips30 => TapeSpeed::Ips30,
                };
                let sk_b = match params.silk_b.value() {
                    SilkModeEnum::Off => SilkMode::Off,
                    SilkModeEnum::Red => SilkMode::Red,
                    SilkModeEnum::Blue => SilkMode::Blue,
                };
                (
                    params.trim_b.value(),
                    params.tape_in_b.value(),
                    s_b,
                    params.saturation_b.value(),
                    params.blend_b.value(),
                    sk_b,
                    params.texture_b.value(),
                    params.output_b.value(),
                )
            };

        // Frame-level processing with stack-allocated frames (no audio-thread allocation)
        buffer.for_each_frame_io::<2, 2, _>(|frame_in, frame_out| {
            let mut in_0 = frame_in[0];
            let mut in_1 = frame_in[1];

            if mode == ProcessingMode::MidSide {
                // Mid/Side matrix encode (power-preserving 1/sqrt(2) ~ 0.70710678)
                let m = (in_0 + in_1) * 0.70710678;
                let s = (in_0 - in_1) * 0.70710678;
                in_0 = m;
                in_1 = s;
            }

            let proc_0 = state.channels[0].process_sample(
                in_0,
                trim_a,
                tape_in_a,
                tape_speed_a,
                sat_a,
                blend_a,
                silk_a,
                tex_a,
                out_a,
            );

            let proc_1 = state.channels[1].process_sample(
                in_1,
                trim_b,
                tape_in_b,
                tape_speed_b,
                sat_b,
                blend_b,
                silk_b,
                tex_b,
                out_b,
            );

            if mode == ProcessingMode::MidSide {
                // Mid/Side matrix decode back to Left / Right
                frame_out[0] = (proc_0 + proc_1) * 0.70710678;
                frame_out[1] = (proc_0 - proc_1) * 0.70710678;
            } else {
                frame_out[0] = proc_0;
                frame_out[1] = proc_1;
            }
        });

        // Update hardware drive & level meters for both channels
        let drive_val_a = state.channels[0].tape.drive_meter_value();
        let drive_val_b = state.channels[1].tape.drive_meter_value();
        let level_val_a = state.channels[0].level_meter_value();
        let level_val_b = state.channels[1].level_meter_value();

        context.set_meter(P::DriveMeterA, drive_val_a);
        context.set_meter(P::DriveMeterB, drive_val_b);
        context.set_meter(P::LevelMeterA, level_val_a);
        context.set_meter(P::LevelMeterB, level_val_b);

        ProcessStatus::Normal
    }

    fn editor(params: Arc<Neve542Params>) -> Box<dyn Editor> {
        // Sleek 19" rack-style side-by-side layout:
        // Left: Channel A (cols 0..4)
        // Center: Master & Routing (cols 5..6)
        // Right: Channel B (cols 7..11)
        // Exactly 2 rows high, with resizable(false) so the window height is strictly clamped.
        GridLayout::build(vec![widgets(vec![
            // --- Row 0: Channel A (Gain & Tape) ---
            knob(P::TrimA, "Trim A").at(0, 0),
            toggle(P::TapeInA, "Tape A").at(1, 0),
            knob(P::SaturationA, "Sat A").at(2, 0),
            dropdown(P::IpsA, "Speed A").at(3, 0),
            meter(&[P::DriveMeterA], "Drive A").at(4, 0),

            // --- Row 0: Master Section ---
            dropdown(P::Mode, "Mode").at(5, 0),
            toggle(P::Bypass, "Bypass").at(6, 0),

            // --- Row 0: Channel B (Gain & Tape) ---
            knob(P::TrimB, "Trim B").at(7, 0),
            toggle(P::TapeInB, "Tape B").at(8, 0),
            knob(P::SaturationB, "Sat B").at(9, 0),
            dropdown(P::IpsB, "Speed B").at(10, 0),
            meter(&[P::DriveMeterB], "Drive B").at(11, 0),

            // --- Row 1: Channel A (Output & Silk) ---
            knob(P::OutputA, "Out A").at(0, 1),
            knob(P::BlendA, "Blend A").at(1, 1),
            dropdown(P::SilkA, "Silk A").at(2, 1),
            knob(P::TextureA, "Texture A").at(3, 1),
            meter(&[P::LevelMeterA], "Level A").at(4, 1),

            // --- Row 1: Master Section ---
            toggle(P::Link, "Link A-B").at(5, 1),

            // --- Row 1: Channel B (Output & Silk) ---
            knob(P::OutputB, "Out B").at(7, 1),
            knob(P::BlendB, "Blend B").at(8, 1),
            dropdown(P::SilkB, "Silk B").at(9, 1),
            knob(P::TextureB, "Texture B").at(10, 1),
            meter(&[P::LevelMeterB], "Level B").at(11, 1),
        ])])
        .with_title("RUPERT NEVE 542")
        .with_subtitle("DUAL TAPE EMULATOR - STEREO / DUAL MONO / M-S")
        .resizable(false)
        .into_editor(&params)
    }
}

truce::plugin! {
    logic: Neve542,
    params: Neve542Params,
}

truce::enable_rt_paranoid!();
