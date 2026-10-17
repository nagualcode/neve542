//! Editor: the NEVE542 front panel (laid out after the 542), re-drawn as a modern device.
//!
//! Each channel is one row that reads left to right in signal-flow order,
//! like the hardware module reads top to bottom: INPUT (Trim)  ›  TAPE
//! (Tape In, IPS, Saturation + Drive meter)  ›  BLEND  ›  LEVEL meter (pre-Silk)
//!  ›  SILK (Off/Red/Blue + Texture)  ›  OUTPUT. The look borrows Ableton Live's
//! flat dark panels, thin value arcs and segmented switches.
//!
//! LINK follows the RND manual: it keeps the relative offset between the
//! channels (a change on one channel is applied as the same delta to the
//! other). In STEREO mode channel A drives both channels, so row B is
//! shown dimmed.

use std::f32::consts::PI;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};
use truce::prelude::*;
use truce_core::editor::{PluginContext, PluginContextReadF64};
use truce_egui::{EditorUi, EguiEditor};

use crate::channel::LEVEL_LEDS_DBU;
use crate::{Neve542Params, Neve542ParamsParamId as P};

pub const W: f32 = 1060.0;
pub const H: f32 = 500.0;

// Palette: Ableton Live dark theme family.
const BG: Color32 = Color32::from_rgb(0x15, 0x15, 0x15);
const HEADER: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x0E);
const PANEL: Color32 = Color32::from_rgb(0x24, 0x24, 0x24);
const PANEL_HI: Color32 = Color32::from_rgb(0x2E, 0x2E, 0x2E);
const LINE: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
const TRACK: Color32 = Color32::from_rgb(0x45, 0x45, 0x45);
const TEXT: Color32 = Color32::from_rgb(0xDC, 0xDC, 0xDC);
const DIM: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
const FAINT: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x96, 0x2B);
const LIME: Color32 = Color32::from_rgb(0xB2, 0xE0, 0x4E);
const YELLOW: Color32 = Color32::from_rgb(0xF5, 0xCD, 0x38);
const RED: Color32 = Color32::from_rgb(0xFF, 0x52, 0x46);
const BLUE: Color32 = Color32::from_rgb(0x4C, 0xA8, 0xFF);
const CYAN: Color32 = Color32::from_rgb(0x2E, 0xD3, 0xC6);
const LED_OFF: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);

#[derive(Clone, Copy)]
struct Ids {
    trim: P,
    tape: P,
    sat: P,
    ips: P,
    blend: P,
    silk: P,
    tex: P,
    out: P,
    drive: P,
    level: P,
}

const CH: [Ids; 2] = [
    Ids {
        trim: P::TrimA,
        tape: P::TapeInA,
        sat: P::SaturationA,
        ips: P::IpsA,
        blend: P::BlendA,
        silk: P::SilkA,
        tex: P::TextureA,
        out: P::OutputA,
        drive: P::DriveMeterA,
        level: P::LevelMeterA,
    },
    Ids {
        trim: P::TrimB,
        tape: P::TapeInB,
        sat: P::SaturationB,
        ips: P::IpsB,
        blend: P::BlendB,
        silk: P::SilkB,
        tex: P::TextureB,
        out: P::OutputB,
        drive: P::DriveMeterB,
        level: P::LevelMeterB,
    },
];

fn twin(id: P) -> P {
    for (a, b) in [
        (CH[0].trim, CH[1].trim),
        (CH[0].tape, CH[1].tape),
        (CH[0].sat, CH[1].sat),
        (CH[0].ips, CH[1].ips),
        (CH[0].blend, CH[1].blend),
        (CH[0].silk, CH[1].silk),
        (CH[0].tex, CH[1].tex),
        (CH[0].out, CH[1].out),
    ] {
        if id == a {
            return b;
        }
        if id == b {
            return a;
        }
    }
    id
}

type Ctx = PluginContext<Neve542Params>;

fn sk(w: f32, c: Color32) -> Stroke {
    Stroke::new(w, c)
}

/// Per-frame interaction context.
struct Panel<'a> {
    st: &'a Ctx,
    /// Twin parameter to move along (LINK), if active.
    link: bool,
    enabled: bool,
}

impl Panel<'_> {
    fn get(&self, id: P) -> f64 {
        PluginContextReadF64::get_param(self.st, id)
    }

    fn default_norm(&self, id: P) -> f64 {
        let id: u32 = id.into();
        self.st
            .params()
            .param_infos()
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.range.normalize(i.default_plain))
            .unwrap_or(0.0)
    }

    fn begin(&self, id: P) {
        self.st.begin_edit(id);
        if self.link {
            self.st.begin_edit(twin(id));
        }
    }

    fn end(&self, id: P) {
        self.st.end_edit(id);
        if self.link {
            self.st.end_edit(twin(id));
        }
    }

    /// Set a value; with LINK the twin moves by the same delta.
    fn set(&self, id: P, v: f64) {
        let v = v.clamp(0.0, 1.0);
        let old = self.get(id);
        self.st.set_param(id, v);
        if self.link {
            let t = twin(id);
            self.st.set_param(t, (self.get(t) + v - old).clamp(0.0, 1.0));
        }
    }

    fn set_once(&self, id: P, v: f64) {
        self.begin(id);
        self.set(id, v);
        self.end(id);
    }
}

fn label(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, size: f32, color: Color32) {
    p.text(pos, align, text, FontId::proportional(size), color);
}

fn arc(center: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() / PI) * 48.0).ceil().max(2.0) as usize;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            center + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

const K_START: f32 = PI * 0.75;
const K_SWEEP: f32 = PI * 1.5;

/// Rotary control. `bipolar` draws the value arc from 12 o'clock.
fn knob(ui: &mut egui::Ui, pn: &Panel, id: P, c: Pos2, r: f32, name: &str, color: Color32, bipolar: bool) {
    let rect = Rect::from_center_size(c, Vec2::splat(r * 2.0 + 10.0));
    let uid = ui.id().with(("knob", u32::from(id)));
    let sense = if pn.enabled { Sense::click_and_drag() } else { Sense::hover() };
    let resp = ui.interact(rect, uid, sense);
    let v = pn.get(id);
    if pn.enabled {
        if resp.drag_started() {
            pn.begin(id);
        }
        if resp.dragged() {
            let fine = ui.input(|i| i.modifiers.shift);
            let dv = -resp.drag_delta().y as f64 / if fine { 900.0 } else { 220.0 };
            pn.set(id, v + dv);
        }
        if resp.drag_stopped() {
            pn.end(id);
        }
        if resp.double_clicked() {
            pn.set_once(id, pn.default_norm(id));
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                pn.set_once(id, v + scroll as f64 / 2000.0);
            }
        }
    }

    let p = ui.painter();
    let v = pn.get(id) as f32;
    let hot = resp.hovered() || resp.dragged();
    let col = if pn.enabled { color } else { FAINT };
    p.add(Shape::line(arc(c, r, K_START, K_START + K_SWEEP), sk(3.5, TRACK)));
    let (a0, a1) = if bipolar {
        let mid = K_START + K_SWEEP * 0.5;
        let a = K_START + K_SWEEP * v;
        (mid.min(a), mid.max(a))
    } else {
        (K_START, K_START + K_SWEEP * v)
    };
    if a1 - a0 > 0.01 {
        p.add(Shape::line(arc(c, r, a0, a1), sk(3.5, col)));
    }
    // Body and pointer.
    p.circle_filled(c, r - 7.0, if hot { PANEL_HI } else { Color32::from_rgb(0x1C, 0x1C, 0x1C) });
    p.circle_stroke(c, r - 7.0, sk(1.0, LINE));
    let a = K_START + K_SWEEP * v;
    let dir = vec2(a.cos(), a.sin());
    p.line_segment([c + dir * (r * 0.18), c + dir * (r - 9.0)], sk(2.0, if pn.enabled { TEXT } else { DIM }));

    label(p, pos2(c.x, c.y - r - 9.0), Align2::CENTER_BOTTOM, name, 10.0, DIM);
    let value = pn.st.format_param(id);
    label(p, pos2(c.x, c.y + r + 4.0), Align2::CENTER_TOP, &value, 11.0, if hot { TEXT } else { DIM });
}

/// Latching push button (bool parameter).
fn button(ui: &mut egui::Ui, pn: &Panel, id: P, rect: Rect, text: &str, on_color: Color32) {
    let uid = ui.id().with(("btn", u32::from(id)));
    let resp = ui.interact(rect, uid, if pn.enabled { Sense::click() } else { Sense::hover() });
    let on = pn.get(id) > 0.5;
    if pn.enabled && resp.clicked() {
        pn.begin(id);
        pn.st.set_param(id, if on { 0.0 } else { 1.0 });
        if pn.link {
            let t = twin(id);
            let tv = pn.get(t) > 0.5;
            pn.st.set_param(t, if tv { 0.0 } else { 1.0 });
        }
        pn.end(id);
    }
    let on = pn.get(id) > 0.5;
    let p = ui.painter();
    let fill = if on {
        if pn.enabled { on_color } else { on_color.gamma_multiply(0.4) }
    } else if resp.hovered() {
        PANEL_HI
    } else {
        Color32::from_rgb(0x1C, 0x1C, 0x1C)
    };
    p.rect_filled(rect, CornerRadius::same(3), fill);
    p.rect_stroke(rect, CornerRadius::same(3), sk(1.0, if on { fill } else { LINE }), StrokeKind::Inside);
    label(p, rect.center(), Align2::CENTER_CENTER, text, 11.0, if on { Color32::from_rgb(0x12, 0x12, 0x12) } else { DIM });
}

/// Segmented switch for an enum parameter with `labels.len()` steps.
fn segmented(ui: &mut egui::Ui, pn: &Panel, id: P, rect: Rect, labels: &[&str], colors: &[Color32]) {
    let n = labels.len();
    let cur = (pn.get(id) * (n - 1) as f64).round() as usize;
    let w = rect.width() / n as f32;
    for (i, text) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + w * i as f32, rect.top()), vec2(w, rect.height()));
        let uid = ui.id().with(("seg", u32::from(id), i));
        let resp = ui.interact(r.shrink(0.5), uid, if pn.enabled { Sense::click() } else { Sense::hover() });
        if pn.enabled && resp.clicked() && i != cur {
            let nv = i as f64 / (n - 1) as f64;
            pn.begin(id);
            let old = pn.get(id);
            pn.st.set_param(id, nv);
            if pn.link {
                let t = twin(id);
                let step = 1.0 / (n - 1) as f64;
                let tv = ((pn.get(t) + nv - old) / step).round() * step;
                pn.st.set_param(t, tv.clamp(0.0, 1.0));
            }
            pn.end(id);
        }
        let sel = i == cur;
        let p = ui.painter();
        let fill = if sel {
            if pn.enabled { colors[i] } else { colors[i].gamma_multiply(0.35) }
        } else if resp.hovered() {
            PANEL_HI
        } else {
            Color32::from_rgb(0x1C, 0x1C, 0x1C)
        };
        let cr = match (i, n) {
            (0, _) => CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 },
            (k, n) if k == n - 1 => CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 },
            _ => CornerRadius::ZERO,
        };
        p.rect_filled(r, cr, fill);
        let txt = if sel { Color32::from_rgb(0x12, 0x12, 0x12) } else { DIM };
        label(p, r.center(), Align2::CENTER_CENTER, text, 10.5, txt);
    }
    ui.painter().rect_stroke(rect, CornerRadius::same(3), sk(1.0, LINE), StrokeKind::Inside);
}

/// Vertical 8-LED bar like the hardware: LED k is lit when value >= k/8.
fn led_meter(p: &egui::Painter, rect: Rect, value: f32, scale: &[String; 8], colors: &[Color32; 8], title: &str) {
    let gap = 3.0;
    let lw = 22.0;
    let lh = (rect.height() - 7.0 * gap) / 8.0;
    for k in 0..8 {
        let y = rect.bottom() - (k as f32 + 1.0) * lh - k as f32 * gap;
        let r = Rect::from_min_size(pos2(rect.left(), y), vec2(lw, lh));
        let lit = value >= (k as f32 + 1.0) / 8.0 - 1e-4;
        let c = if lit { colors[k] } else { LED_OFF };
        p.rect_filled(r, CornerRadius::same(2), c);
        if lit {
            p.rect_filled(r.expand(1.5), CornerRadius::same(3), c.gamma_multiply(0.18));
        }
        label(p, pos2(r.right() + 6.0, r.center().y), Align2::LEFT_CENTER, &scale[k], 9.5, if lit { TEXT } else { FAINT });
    }
    label(p, pos2(rect.left() + lw * 0.5, rect.top() - 8.0), Align2::CENTER_BOTTOM, title, 10.0, DIM);
}

fn section(p: &egui::Painter, rect: Rect, title: &str, accent: Color32) {
    p.rect_filled(rect, CornerRadius::same(4), PANEL);
    p.rect_stroke(rect, CornerRadius::same(4), sk(1.0, LINE), StrokeKind::Inside);
    p.line_segment(
        [pos2(rect.left() + 10.0, rect.top() + 22.0), pos2(rect.right() - 10.0, rect.top() + 22.0)],
        sk(1.0, LINE),
    );
    p.circle_filled(pos2(rect.left() + 14.0, rect.top() + 11.5), 3.0, accent);
    label(p, pos2(rect.left() + 23.0, rect.top() + 11.5), Align2::LEFT_CENTER, title, 10.5, TEXT);
}

#[derive(Default)]
pub struct Ui542;

impl Ui542 {
    fn header(&self, ui: &mut egui::Ui, st: &Ctx) {
        let p = ui.painter().clone();
        let hr = Rect::from_min_size(Pos2::ZERO, vec2(W, 46.0));
        p.rect_filled(hr, CornerRadius::ZERO, HEADER);
        p.line_segment([pos2(0.0, 46.0), pos2(W, 46.0)], sk(1.0, LINE));
        label(&p, pos2(18.0, 23.0), Align2::LEFT_CENTER, "542", 24.0, TEXT);
        label(&p, pos2(70.0, 17.0), Align2::LEFT_CENTER, "TAPE EMULATOR", 11.0, TEXT);
        label(&p, pos2(70.0, 31.0), Align2::LEFT_CENTER, "NEVE542 · vintage transformer & tape circuit · Jardim Sonoro", 9.5, DIM);

        let pn = Panel { st, link: false, enabled: true };
        label(&p, pos2(W * 0.5 - 132.0, 23.0), Align2::RIGHT_CENTER, "MODE", 10.0, DIM);
        segmented(
            ui,
            &pn,
            P::Mode,
            Rect::from_min_size(pos2(W * 0.5 - 124.0, 11.0), vec2(248.0, 24.0)),
            &["DUAL MONO", "STEREO", "M / S"],
            &[ORANGE, ORANGE, ORANGE],
        );
        button(ui, &pn, P::Link, Rect::from_min_size(pos2(W - 196.0, 11.0), vec2(80.0, 24.0)), "LINK", CYAN);
        button(ui, &pn, P::Bypass, Rect::from_min_size(pos2(W - 106.0, 11.0), vec2(88.0, 24.0)), "BYPASS", YELLOW);
    }

    fn row(&self, ui: &mut egui::Ui, st: &Ctx, ch: usize, top: f32) {
        let mode = (PluginContextReadF64::get_param(st, P::Mode) * 2.0).round() as usize;
        let stereo = mode == 1;
        let link = PluginContextReadF64::get_param(st, P::Link) > 0.5 && !stereo;
        let enabled = !(stereo && ch == 1);
        let pn = Panel { st, link, enabled };
        let ids = CH[ch];
        let h = 206.0;
        let p = ui.painter().clone();

        // Channel tag.
        let tag = Rect::from_min_size(pos2(12.0, top), vec2(54.0, h));
        p.rect_filled(tag, CornerRadius::same(4), PANEL);
        p.rect_stroke(tag, CornerRadius::same(4), sk(1.0, LINE), StrokeKind::Inside);
        let (big, small) = match (mode, ch) {
            (2, 0) => ("M", "MID"),
            (2, _) => ("S", "SIDE"),
            (_, 0) => ("A", if stereo { "L + R" } else { "LEFT" }),
            _ => ("B", "RIGHT"),
        };
        label(&p, pos2(tag.center().x, top + 70.0), Align2::CENTER_CENTER, big, 30.0, if enabled { TEXT } else { FAINT });
        label(&p, pos2(tag.center().x, top + 100.0), Align2::CENTER_CENTER, small, 9.5, DIM);
        if !enabled {
            label(&p, pos2(tag.center().x, top + 150.0), Align2::CENTER_CENTER, "= A", 11.0, ORANGE);
        }

        let y0 = top;
        let cy = top + 112.0;
        let mut x = 74.0;

        // INPUT
        let r = Rect::from_min_size(pos2(x, y0), vec2(104.0, h));
        section(&p, r, "INPUT", DIM);
        knob(ui, &pn, ids.trim, pos2(r.center().x, cy), 28.0, "TRIM", TEXT, true);
        x = r.right() + 8.0;

        // TAPE
        let r = Rect::from_min_size(pos2(x, y0), vec2(352.0, h));
        section(&p, r, "TAPE", ORANGE);
        button(ui, &pn, ids.tape, Rect::from_min_size(pos2(r.left() + 14.0, y0 + 46.0), vec2(84.0, 26.0)), "TAPE IN", ORANGE);
        label(&p, pos2(r.left() + 56.0, y0 + 96.0), Align2::CENTER_BOTTOM, "IPS", 10.0, DIM);
        segmented(
            ui,
            &pn,
            ids.ips,
            Rect::from_min_size(pos2(r.left() + 14.0, y0 + 100.0), vec2(84.0, 26.0)),
            &["15", "30"],
            &[ORANGE, ORANGE],
        );
        knob(ui, &pn, ids.sat, pos2(r.left() + 182.0, cy), 46.0, "SATURATION", ORANGE, false);
        let led_scale: [String; 8] = std::array::from_fn(|k| format!("{}", k + 1));
        let drive_colors = [LIME, LIME, LIME, LIME, YELLOW, ORANGE, RED, RED];
        let m = Rect::from_min_size(pos2(r.right() - 74.0, y0 + 48.0), vec2(40.0, 142.0));
        led_meter(&p, m, st.get_meter(ids.drive), &led_scale, &drive_colors, "DRIVE");
        label(&p, pos2(r.left() + 56.0, y0 + 152.0), Align2::CENTER_CENTER, "soft clip", 9.0, FAINT);
        label(&p, pos2(r.left() + 56.0, y0 + 164.0), Align2::CENTER_CENTER, "above 5", 9.0, FAINT);
        x = r.right() + 8.0;

        // BLEND
        let r = Rect::from_min_size(pos2(x, y0), vec2(104.0, h));
        section(&p, r, "BLEND", ORANGE);
        knob(ui, &pn, ids.blend, pos2(r.center().x, cy), 28.0, "DRY · TAPE", ORANGE, false);
        x = r.right() + 8.0;

        // LEVEL meter (pre-SILK)
        let r = Rect::from_min_size(pos2(x, y0), vec2(92.0, h));
        section(&p, r, "LEVEL", LIME);
        let lvl_scale: [String; 8] = std::array::from_fn(|k| format!("{:+}", LEVEL_LEDS_DBU[k] as i32));
        let lvl_colors = [LIME, LIME, LIME, LIME, LIME, YELLOW, ORANGE, RED];
        let m = Rect::from_min_size(pos2(r.left() + 22.0, y0 + 48.0), vec2(40.0, 142.0));
        led_meter(&p, m, st.get_meter(ids.level), &lvl_scale, &lvl_colors, "dBu");
        x = r.right() + 8.0;

        // SILK
        let silk_idx = (pn.get(ids.silk) * 2.0).round() as usize;
        let silk_col = [DIM, RED, BLUE][silk_idx];
        let r = Rect::from_min_size(pos2(x, y0), vec2(206.0, h));
        section(&p, r, "SILK", silk_col);
        segmented(
            ui,
            &pn,
            ids.silk,
            Rect::from_min_size(pos2(r.left() + 14.0, y0 + 46.0), vec2(r.width() - 28.0, 26.0)),
            &["OFF", "RED", "BLUE"],
            &[DIM, RED, BLUE],
        );
        knob(ui, &pn, ids.tex, pos2(r.center().x, cy + 22.0), 34.0, "TEXTURE", if silk_idx == 0 { DIM } else { silk_col }, false);
        x = r.right() + 8.0;

        // OUTPUT
        let r = Rect::from_min_size(pos2(x, y0), vec2(W - 12.0 - x, h));
        section(&p, r, "OUTPUT", DIM);
        knob(ui, &pn, ids.out, pos2(r.center().x, cy), 28.0, "TRIM", TEXT, true);

        if !enabled {
            let full = Rect::from_min_max(pos2(74.0, top), pos2(W - 12.0, top + h));
            p.rect_filled(full, CornerRadius::same(4), Color32::from_black_alpha(110));
            label(&p, full.center(), Align2::CENTER_CENTER, "STEREO  ·  channel A controls both sides", 13.0, TEXT);
        }
    }
}

impl EditorUi<Neve542Params> for Ui542 {
    fn ui(&mut self, ui: &mut egui::Ui, st: &Ctx) {
        ui.painter().rect_filled(Rect::from_min_size(Pos2::ZERO, vec2(W, H)), CornerRadius::ZERO, BG);
        self.header(ui, st);
        self.row(ui, st, 0, 58.0);
        self.row(ui, st, 1, 58.0 + 206.0 + 10.0);
        let p = ui.painter();
        label(
            p,
            pos2(14.0, H - 12.0),
            Align2::LEFT_CENTER,
            "INPUT XFMR  ›  CLASS-A TRIM  ›  DRIVE + PRE-EMPHASIS  ›  SOFT CLIP  ›  RECORD HEAD (HYSTERESIS)  ›  RECOVERY + REPLAY EQ  ›  BLEND  ›  OUTPUT XFMR + SILK",
            9.0,
            FAINT,
        );
        label(p, pos2(W - 14.0, H - 12.0), Align2::RIGHT_CENTER, "drag · shift fine · double-click reset", 9.0, FAINT);
        ui.ctx().request_repaint();
    }
}

pub fn editor(params: Arc<Neve542Params>) -> Box<dyn Editor> {
    let mut visuals = truce_egui::theme::dark();
    visuals.panel_fill = BG;
    Box::new(EguiEditor::with_ui(params, (W as u32, H as u32), Ui542).with_visuals(visuals).resizable(false))
}
