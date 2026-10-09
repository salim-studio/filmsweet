//! Audio of video clips (#223): a selected video clip shows the Volume, Channel Volume and Panner
//! of the audio linked to it as well, as in Premiere.

use std::sync::mpsc::{Sender, channel};

use egui_kittest::Harness;
use filmcraft_engine::Session;
use filmcraft_project::{ClipId, TrackItem};
use filmcraft_ui_egui::FilmcraftApp;
use filmcraft_ui_egui::control::ControlRequest;
use serde_json::{Value, json};

struct Driver {
    harness: Harness<'static, FilmcraftApp>,
    tx: Sender<ControlRequest>,
}

/// The demo's first clip: its video on V1 and the audio linked to it on A1.
struct Pair {
    video: u64,
    audio: u64,
}

impl Driver {
    fn demo(panels: &[&str]) -> (Self, Pair) {
        let mut session = Session::default();
        session.execute("file.openDemoProject", json!({})).expect("demo project");
        let (tx, rx) = channel();
        let app = FilmcraftApp::new(session).with_control(rx);
        let harness = Harness::builder().with_size(egui::vec2(1600.0, 1100.0)).with_max_steps(10_000).build_eframe(move |_cc| app);
        let mut d = Driver { harness, tx };
        d.frames(4);
        let seq = d.exec("sequence.inspect", json!({}));
        let v = &seq["video"][0]["items"][0];
        let a = seq["audio"][0]["items"].as_array().unwrap().iter().find(|a| a["link"] == v["link"]).expect("linked audio").clone();
        let pair = Pair { video: v["clip"].as_u64().unwrap(), audio: a["clip"].as_u64().unwrap() };
        for panel in panels {
            d.ok("ui.panel.show", json!({"panel": panel}));
        }
        d.frames(3);
        (d, pair)
    }
    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let ctx = self.harness.ctx.clone();
            let mut raw = std::mem::take(self.harness.input_mut());
            eframe::App::raw_input_hook(self.harness.state_mut(), &ctx, &mut raw);
            *self.harness.input_mut() = raw;
            self.harness.step();
        }
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        let (req, reply) = ControlRequest::new(method, params.clone());
        self.tx.send(req).unwrap();
        for _ in 0..600 {
            self.frames(1);
            if let Ok(v) = reply.try_recv() {
                return v;
            }
        }
        panic!("no reply to {method} {params}");
    }
    fn ok(&mut self, method: &str, params: Value) -> Value {
        let r = self.call(method, params.clone());
        assert_eq!(r["ok"], true, "{method} {params}: {r}");
        r["result"].clone()
    }
    fn exec(&mut self, command: &str, params: Value) -> Value {
        self.ok("engine.execute", json!({"command": command, "params": params}))
    }
    fn find(&mut self, id: &str) -> Option<[f64; 4]> {
        self.frames(2);
        let v = self.ok("ui.elements", json!({"prefix": id}));
        let e = v.as_array().unwrap().iter().find(|e| e["id"] == id)?;
        let r: Vec<f64> = e["rect"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        Some([r[0], r[1], r[2], r[3]])
    }
    fn rect(&mut self, id: &str) -> [f64; 4] {
        self.find(id).unwrap_or_else(|| panic!("no element {id}"))
    }
    fn click(&mut self, id: &str) {
        self.rect(id);
        self.ok("ui.click", json!({"id": id}));
        self.frames(3);
    }
    fn item(&self, clip: u64) -> TrackItem {
        self.harness.state().session.active_sequence().unwrap().find_item(ClipId(clip)).unwrap().1.clone()
    }
}

#[test]
fn effect_controls_show_the_audio_of_a_video_clip() {
    let (mut d, pair) = Driver::demo(&["EffectControls"]);
    d.exec("timeline.select", json!({"clips": [pair.video]}));
    let motion = d.rect("effectControls.effect.motion");
    let volume = d.rect("effectControls.effect.volume");
    assert!(volume[1] > motion[1], "Audio comes after Video: {volume:?} {motion:?}");
    d.rect("effectControls.effect.channel_volume");
    d.rect("effectControls.effect.panner");

    // the audio rows edit the audio clip
    for fx in ["motion", "opacity", "time_remap"] {
        d.click(&format!("effectControls.effect.{fx}"));
    }
    d.click("effectControls.volume.level.stopwatch");
    assert!(d.item(pair.audio).effect("volume").unwrap().param("level").unwrap().is_animated());
    assert!(d.item(pair.video).effect("volume").is_none());

    // only the video selected: no audio
    d.exec("sequence.linkedSelection", json!({"on": false}));
    d.exec("timeline.select", json!({"clips": [pair.video]}));
    assert!(d.find("effectControls.effect.volume").is_none());
    d.rect("effectControls.effect.motion");
}

#[test]
fn properties_show_the_audio_of_a_video_clip() {
    let (mut d, pair) = Driver::demo(&["Properties"]);
    d.exec("timeline.select", json!({"clips": [pair.video]}));
    let scale = d.rect("properties.motion.scale.addKeyframe");
    let level = d.rect("properties.volume.level.addKeyframe");
    assert!(level[1] > scale[1]);
    d.click("properties.volume.level.addKeyframe");
    assert_eq!(d.item(pair.audio).effect("volume").unwrap().param("level").unwrap().keyframes.len(), 1);

    d.exec("sequence.linkedSelection", json!({"on": false}));
    d.exec("timeline.select", json!({"clips": [pair.audio]}));
    d.rect("properties.volume.level.addKeyframe");
    assert!(d.find("properties.motion.scale.addKeyframe").is_none());
}
