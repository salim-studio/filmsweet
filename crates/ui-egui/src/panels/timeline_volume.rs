//! The Volume line on audio clips (#223): the clip's Volume level over time, with its keyframes,
//! on the same scale as the track faders.

use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use filmcraft_project::{Param, TrackItem};
use filmcraft_time::Tick;

use super::mixer::db_to_pos;
use crate::icons::{self, Icon};

const KEY_COL: Color32 = Color32::from_rgb(0xc8, 0xc8, 0xc8);

/// Where the line goes in a clip drawn at `body`: under the name, down to the bottom edge.
fn band(body: Rect) -> Rect {
    Rect::from_min_max(pos2(body.min.x, body.min.y + 17.0), pos2(body.max.x, body.max.y - 3.0))
}

fn level(it: &TrackItem) -> Option<&Param> {
    it.effect("volume")?.param("level")
}

/// The level (dB) the clip plays at timeline time `t`.
fn level_at(it: &TrackItem, t: Tick) -> f64 {
    level(it).map_or(0.0, |p| p.f64_at(it.source_time_at(t)))
}

/// Timeline time of media time `m`, the inverse of [`TrackItem::moving_source_time_at`].
fn timeline_time(it: &TrackItem, m: Tick) -> Tick {
    let rel = if it.reverse { it.source_out().0.saturating_sub(1).saturating_sub(m.0) } else { m.0.saturating_sub(it.source_in.0) };
    Tick(it.start.0.saturating_add((rel as f64 / it.speed.abs().max(1e-6)).round() as i64))
}

fn x_of(it: &TrackItem, body: Rect, t: Tick) -> f32 {
    body.min.x + (t.0.saturating_sub(it.start.0) as f64 / it.duration.0.max(1) as f64) as f32 * body.width()
}

fn t_at(it: &TrackItem, body: Rect, x: f32) -> Tick {
    let f = ((x - body.min.x) / body.width().max(1.0)).clamp(0.0, 1.0) as f64;
    Tick(it.start.0.saturating_add((f * it.duration.0 as f64) as i64))
}

fn y_of(body: Rect, db: f64) -> f32 {
    let b = band(body);
    b.max.y - db_to_pos(db) * b.height()
}

/// The line from `x0` to `x1`; keyframes bend it, so it then gets a point every 2 px.
fn line(it: &TrackItem, body: Rect, x0: f32, x1: f32) -> Vec<Pos2> {
    let n = if level(it).is_some_and(Param::is_animated) { (((x1 - x0) / 2.0).ceil() as usize).clamp(1, 4096) } else { 1 };
    (0..=n)
        .map(|i| {
            let x = x0 + (x1 - x0) * i as f32 / n as f32;
            pos2(x, y_of(body, level_at(it, t_at(it, body, x))))
        })
        .collect()
}

/// The keyframes: media time and where each is drawn.
fn keys(it: &TrackItem, body: Rect) -> Vec<(Tick, Pos2)> {
    let Some(p) = level(it) else { return Vec::new() };
    p.keyframes.iter().map(|k| (k.time, pos2(x_of(it, body, timeline_time(it, k.time)), y_of(body, k.value.as_f64().unwrap_or(0.0))))).collect()
}

/// Draw the line over an audio clip drawn at `body`: white with a dark shadow.
pub fn paint(p: &egui::Painter, body: Rect, it: &TrackItem) {
    let vis = p.clip_rect().intersect(body);
    if band(body).height() < 6.0 || vis.width() <= 0.0 {
        return;
    }
    let p = p.with_clip_rect(vis);
    let pts = line(it, body, vis.min.x, vis.max.x);
    p.add(egui::Shape::line(pts.iter().map(|q| *q + vec2(0.0, 1.0)).collect(), Stroke::new(1.0, Color32::BLACK)));
    p.add(egui::Shape::line(pts, Stroke::new(1.0, Color32::WHITE)));
    for (_, c) in keys(it, body) {
        icons::paint(&p, Rect::from_center_size(c, vec2(9.0, 9.0)), Icon::Keyframe, KEY_COL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filmcraft_project::{ClipId, ItemId, Label};

    fn clip(speed: f64, reverse: bool) -> TrackItem {
        TrackItem {
            id: ClipId(1),
            item: ItemId(1),
            name: String::new(),
            label: Label::Iris,
            start: Tick(1_000),
            duration: Tick(4_000),
            source_in: Tick(500),
            speed,
            reverse,
            enabled: true,
            link: None,
            group: None,
            effects: filmcraft_project::effect::intrinsic_audio(),
            markers: Vec::new(),
            gain_db: 0.0,
            frame_hold: None,
            scale_to_frame: false,
            essential: None,
            multicam: None,
            time_interpolation: Default::default(),
            hold_filters: false,
            field_options: None,
            source_channels: Vec::new(),
            audio_stream: 0,
            graphic: None,
        }
    }

    #[test]
    fn keyframes_sit_where_their_media_time_plays() {
        for (speed, reverse) in [(1.0, false), (2.0, false), (0.5, false), (1.0, true), (2.0, true)] {
            let it = clip(speed, reverse);
            for t in [1_000, 1_001, 2_500, 4_999] {
                let m = it.moving_source_time_at(Tick(t));
                assert!((timeline_time(&it, m).0 - t).abs() <= 1, "speed {speed} reverse {reverse}: {t}");
            }
        }
    }

    #[test]
    fn the_line_follows_the_level() {
        let mut it = clip(1.0, false);
        let body = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 80.0));
        let flat = line(&it, body, 0.0, 400.0);
        assert_eq!(flat.len(), 2);
        assert!(flat.iter().all(|p| (p.y - y_of(body, 0.0)).abs() < 1e-3));
        let p = it.effect_mut("volume").and_then(|e| e.param_mut("level")).unwrap();
        p.put_keyframe(Tick(500), filmcraft_project::ParamValue::Float(0.0));
        p.put_keyframe(Tick(4_500), filmcraft_project::ParamValue::Float(-20.0));
        let ramp = line(&it, body, 0.0, 400.0);
        assert!(ramp.len() > 100);
        assert!(ramp.windows(2).all(|w| w[1].y >= w[0].y), "goes down as the level falls");
        assert!(ramp.last().unwrap().y > y_of(body, -19.0));
        // damaged geometry draws nothing silly rather than looping or panicking
        assert_eq!(line(&it, body, 0.0, f32::NAN).len(), 2);
        assert!(line(&it, body, 0.0, f32::INFINITY).len() <= 4097);
    }
}
