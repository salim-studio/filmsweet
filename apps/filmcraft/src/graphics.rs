//! Graphics start-up: the wgpu backends and adapter the window is created with, and the fallback
//! when a start fails (docs/gaps.md G4, #820).
//!
//! A graphics driver can crash inside wgpu instance or device creation (Intel's Vulkan driver
//! `igvk64.dll`, #512 #413; AMD's OpenGL driver `atio6axx.dll`, #477), where no Rust error
//! handling runs, so every launch died the same way until the user set `WGPU_BACKEND` by hand.
//! Each start now leaves a marker in the data folder ([`STATE_FILE`]) that is cleared once the
//! window has painted. A marker still pending at the next launch means that start died: the launch
//! moves on to the next [`Mode`] of the platform's [`LADDER`] and keeps it. A start-up error that
//! wgpu does report (no adapter, device creation failed) moves on within the same process. An
//! explicit `WGPU_BACKEND` leaves the marker alone: the user is in charge.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::{
    NativeOptions,
    egui_wgpu::WgpuSetup,
    wgpu::{self, Backends},
};

/// The start marker in the data folder: `{"mode": "<mode>", "pending": <bool>}`. Delete it to
/// start with the default graphics mode again.
pub const STATE_FILE: &str = "graphics-start.json";

/// How the window's graphics device is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The platform default: Vulkan or DirectX 12 on Windows (no OpenGL, see [`apply`]), Metal on
    /// macOS, Vulkan or OpenGL elsewhere.
    Default,
    /// DirectX 12 only (Windows), for Vulkan drivers that crash at start-up.
    Dx12,
    /// A software adapter (WARP on Windows, lavapipe / llvmpipe elsewhere); the preview then
    /// composites on the CPU (`FilmcraftApp::set_wgpu`).
    Software,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Default => "default",
            Mode::Dx12 => "dx12",
            Mode::Software => "software",
        }
    }

    fn parse(name: &str) -> Option<Mode> {
        [Mode::Default, Mode::Dx12, Mode::Software].into_iter().find(|m| m.name() == name)
    }
}

/// The modes tried in order on this platform. macOS has only Metal, and no software adapter.
pub const LADDER: &[Mode] = if cfg!(target_os = "windows") {
    &[Mode::Default, Mode::Dx12, Mode::Software]
} else if cfg!(target_os = "macos") {
    &[Mode::Default]
} else {
    &[Mode::Default, Mode::Software]
};

/// The mode after `mode` in `ladder`, if any.
fn next_in(ladder: &[Mode], mode: Mode) -> Option<Mode> {
    let i = ladder.iter().position(|m| *m == mode)?;
    ladder.get(i.checked_add(1)?).copied()
}

/// This launch's mode from the saved marker (`(mode, pending)`; `None` when there is none or it
/// is unreadable), and whether it moved on because the last start died.
fn launch_mode(saved: Option<(Mode, bool)>, ladder: &[Mode]) -> (Mode, bool) {
    let first = ladder.first().copied().unwrap_or(Mode::Default);
    match saved {
        // A mode this platform does not use (a data folder copied from another OS): start over.
        Some((mode, _)) if !ladder.contains(&mode) => (first, false),
        Some((mode, true)) => match next_in(ladder, mode) {
            Some(next) => (next, true),
            // Nothing left to try: the last mode is still the best chance.
            None => (mode, false),
        },
        Some((mode, false)) => (mode, false),
        None => (first, false),
    }
}

/// `(mode, pending)` from the marker's text; `None` for anything else.
fn parse_state(text: &str) -> Option<(Mode, bool)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    Some((Mode::parse(v.get("mode")?.as_str()?)?, v.get("pending")?.as_bool()?))
}

fn read_state(path: &Path) -> Option<(Mode, bool)> {
    // The marker is a few bytes; anything longer is not ours.
    let mut text = String::new();
    std::fs::File::open(path).ok()?.take(4096).read_to_string(&mut text).ok()?;
    parse_state(&text)
}

fn write_state(path: &Path, mode: Mode, pending: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::json!({ "mode": mode.name(), "pending": pending }).to_string();
    // Written beside and renamed, so a start that dies mid-write leaves the old marker whole.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// This launch's graphics mode and its start marker.
#[derive(Clone, Debug)]
pub struct GraphicsStart {
    /// The marker; `None` without a data folder or with `WGPU_BACKEND` set.
    path: Option<PathBuf>,
    mode: Mode,
}

impl GraphicsStart {
    /// Pick this launch's mode from the marker in `data_dir` and mark the start pending.
    /// `explicit_backend` (`WGPU_BACKEND` set) starts in [`Mode::Default`] and leaves the marker
    /// alone.
    pub fn begin(data_dir: Option<&Path>, explicit_backend: bool) -> Self {
        let path = if explicit_backend { None } else { data_dir.map(|d| d.join(STATE_FILE)) };
        let saved = path.as_deref().and_then(read_state);
        let (mode, moved_on) = launch_mode(saved, LADDER);
        let file = path.as_deref().map(|p| p.display().to_string()).unwrap_or_default();
        if moved_on {
            let failed = saved.map_or("default", |(m, _)| m.name());
            log::warn!(
                "the last start did not get its window up with graphics mode {failed}; starting with {} (delete {file} to try the default again)",
                mode.name()
            );
        } else if mode != Mode::Default {
            log::info!("graphics mode {} after an earlier failed start (delete {file} to try the default again)", mode.name());
        }
        let start = Self { path, mode };
        start.save(true);
        start
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Move on to the next mode after a start-up error wgpu reported; `false` when none is left.
    pub fn step_down(&mut self) -> bool {
        let Some(next) = next_in(LADDER, self.mode) else {
            return false;
        };
        log::warn!("graphics mode {} failed to start; trying {}", self.mode.name(), next.name());
        self.mode = next;
        self.save(true);
        true
    }

    /// The window is up: keep this mode for the next launches.
    pub fn confirm(&self) {
        self.save(false);
    }

    fn save(&self, pending: bool) {
        if let Some(path) = &self.path
            && let Err(e) = write_state(path, self.mode, pending)
        {
            log::warn!("can't write {}: {e}", path.display());
        }
    }
}

/// Configure `options` for `mode`, before eframe creates the wgpu instance. `backend_override`
/// is normally `Backends::from_env()`, i.e. `WGPU_BACKEND`.
pub fn apply(options: &mut NativeOptions, mode: Mode, backend_override: Option<Backends>) {
    apply_for(options, mode, backend_override, cfg!(target_os = "windows"));
}

fn apply_for(options: &mut NativeOptions, mode: Mode, backend_override: Option<Backends>, windows: bool) {
    let WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup else {
        return;
    };
    match mode {
        // eframe's default (`WGPU_BACKEND`, else PRIMARY | GL), except on Windows: there it
        // includes GL. Creating that backend can crash inside AMD's atio6axx.dll before adapter
        // selection or any Rust error handling runs, so the window never appears. Choosing an
        // adapter afterwards is too late: only leaving GL out of the instance avoids it. Keep
        // Vulkan and DX12 both: forcing DX12 alone sends shaders through FXC (which rejects
        // fx.wgsl, silently falling back to the CPU) and leaves machines without DX12 with no
        // backend at all.
        Mode::Default if windows => create.instance_descriptor.backends = backend_override.unwrap_or(Backends::PRIMARY),
        Mode::Default => {}
        // Without Vulkan, whose driver is what crashes; GPU effects may fall back to the CPU.
        Mode::Dx12 => create.instance_descriptor.backends = Backends::DX12,
        Mode::Software => {
            create.instance_descriptor.backends = software_backends(windows);
            create.native_adapter_selector = Some(Arc::new(pick_software));
        }
    }
}

/// The backends a software adapter is looked for in: WARP is a DX12 adapter, and GL stays out on
/// Windows for the reason in [`apply_for`].
fn software_backends(windows: bool) -> Backends {
    if windows { Backends::DX12 } else { Backends::VULKAN | Backends::GL }
}

/// The first software adapter that can draw to the window's surface.
fn pick_software(adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, String> {
    adapters
        .iter()
        .find(|a| filmcraft_ui_egui::software_rasterizer(&a.get_info()).is_some() && surface.is_none_or(|s| a.is_surface_supported(s)))
        .cloned()
        .ok_or_else(|| format!("no software graphics adapter (WARP, lavapipe, llvmpipe) among {} adapters", adapters.len()))
}

/// Whether `err` from `eframe::run_native` is a graphics start-up failure that another mode may
/// avoid (as opposed to a display, event-loop or app error).
pub fn is_graphics_failure(err: &eframe::Error) -> bool {
    matches!(err, eframe::Error::Wgpu(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS: &[Mode] = &[Mode::Default, Mode::Dx12, Mode::Software];
    const LINUX: &[Mode] = &[Mode::Default, Mode::Software];
    const MACOS: &[Mode] = &[Mode::Default];

    fn configured(mode: Mode, backend_override: Option<Backends>, windows: bool) -> (Backends, bool) {
        let mut options = NativeOptions::default();
        apply_for(&mut options, mode, backend_override, windows);
        let WgpuSetup::CreateNew(create) = options.wgpu_options.wgpu_setup else {
            panic!("default native options must create a wgpu instance");
        };
        (create.instance_descriptor.backends, create.native_adapter_selector.is_some())
    }

    #[test]
    fn windows_default_leaves_out_gl() {
        // eframe's own default (PRIMARY | GL) would initialize OpenGL as well.
        let WgpuSetup::CreateNew(eframe_default) = NativeOptions::default().wgpu_options.wgpu_setup else {
            panic!("default native options must create a wgpu instance");
        };
        assert!(eframe_default.instance_descriptor.backends.contains(Backends::GL));
        let (backends, selector) = configured(Mode::Default, None, true);
        assert_eq!(backends, Backends::PRIMARY);
        assert!(!backends.contains(Backends::GL));
        assert!(backends.contains(Backends::DX12) && backends.contains(Backends::VULKAN));
        assert!(!selector);
    }

    #[test]
    fn explicit_backend_override_is_preserved() {
        for backends in [Backends::VULKAN, Backends::GL, Backends::DX12, Backends::VULKAN | Backends::DX12, Backends::empty()] {
            assert_eq!(configured(Mode::Default, Some(backends), true).0, backends);
        }
    }

    #[test]
    fn fallback_modes_leave_out_the_crashing_backends() {
        // DX12 only: Intel's and AMD's Vulkan drivers are what crashed (#512, #413).
        assert_eq!(configured(Mode::Dx12, None, true), (Backends::DX12, false));
        // Software on Windows is WARP (DX12), never GL (atio6axx.dll, #477).
        assert_eq!(configured(Mode::Software, None, true), (Backends::DX12, true));
        assert_eq!(configured(Mode::Software, None, false), (Backends::VULKAN | Backends::GL, true));
        // Off Windows the default is eframe's own.
        let WgpuSetup::CreateNew(eframe_default) = NativeOptions::default().wgpu_options.wgpu_setup else {
            panic!("default native options must create a wgpu instance");
        };
        assert_eq!(configured(Mode::Default, None, false), (eframe_default.instance_descriptor.backends, false));
    }

    #[test]
    fn a_start_that_died_moves_on_to_the_next_mode_and_keeps_it() {
        // first launch
        assert_eq!(launch_mode(None, WINDOWS), (Mode::Default, false));
        // the default start died in the driver: DX12 next, then software
        assert_eq!(launch_mode(Some((Mode::Default, true)), WINDOWS), (Mode::Dx12, true));
        assert_eq!(launch_mode(Some((Mode::Dx12, true)), WINDOWS), (Mode::Software, true));
        assert_eq!(launch_mode(Some((Mode::Default, true)), LINUX), (Mode::Software, true));
        // a confirmed mode is kept
        assert_eq!(launch_mode(Some((Mode::Dx12, false)), WINDOWS), (Mode::Dx12, false));
        assert_eq!(launch_mode(Some((Mode::Default, false)), WINDOWS), (Mode::Default, false));
        // nothing left to try: stay
        assert_eq!(launch_mode(Some((Mode::Software, true)), WINDOWS), (Mode::Software, false));
        assert_eq!(launch_mode(Some((Mode::Default, true)), MACOS), (Mode::Default, false));
        // a mode this platform has not: start over
        assert_eq!(launch_mode(Some((Mode::Dx12, false)), LINUX), (Mode::Default, false));
        assert_eq!(launch_mode(Some((Mode::Software, true)), MACOS), (Mode::Default, false));
        assert_eq!(launch_mode(None, &[]), (Mode::Default, false));
    }

    #[test]
    fn the_marker_round_trips_and_rejects_anything_else() {
        for mode in WINDOWS {
            for pending in [false, true] {
                let text = serde_json::json!({ "mode": mode.name(), "pending": pending }).to_string();
                assert_eq!(parse_state(&text), Some((*mode, pending)));
            }
        }
        for bad in [
            "",
            "{",
            "null",
            "[]",
            r#"{"mode":"vulkan","pending":true}"#,
            r#"{"mode":"dx12"}"#,
            r#"{"mode":1,"pending":true}"#,
            r#"{"mode":"dx12","pending":"yes"}"#,
        ] {
            assert_eq!(parse_state(bad), None, "{bad}");
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("filmcraft-graphics-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn begin_marks_the_start_pending_and_confirm_clears_it() {
        let dir = temp_dir("marker");
        let file = dir.join(STATE_FILE);
        let start = GraphicsStart::begin(Some(&dir), false);
        assert_eq!(start.mode(), Mode::Default);
        assert_eq!(read_state(&file), Some((Mode::Default, true)));
        start.confirm();
        assert_eq!(read_state(&file), Some((Mode::Default, false)));
        // a confirmed start is kept on the next launch
        assert_eq!(GraphicsStart::begin(Some(&dir), false).mode(), Mode::Default);
        // ... and one that never confirmed moves on (Windows and Linux; macOS has nowhere to go)
        let next = GraphicsStart::begin(Some(&dir), false);
        assert_eq!(next.mode(), LADDER.get(1).copied().unwrap_or(Mode::Default));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_or_huge_marker_starts_with_the_default() {
        let dir = temp_dir("corrupt");
        let file = dir.join(STATE_FILE);
        let _ = std::fs::create_dir_all(&dir);
        for text in ["garbage".to_string(), format!(r#"{{"mode":"dx12","pending":true,"pad":"{}"}}"#, "x".repeat(10_000))] {
            let _ = std::fs::write(&file, text);
            let start = GraphicsStart::begin(Some(&dir), false);
            assert_eq!(start.mode(), Mode::Default);
            // and it is replaced with a well-formed pending marker
            assert_eq!(read_state(&file), Some((Mode::Default, true)));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_explicit_backend_leaves_the_marker_alone() {
        let dir = temp_dir("explicit");
        let file = dir.join(STATE_FILE);
        let _ = std::fs::create_dir_all(&dir);
        let _ = write_state(&file, Mode::Software, true);
        let mut start = GraphicsStart::begin(Some(&dir), true);
        assert_eq!(start.mode(), Mode::Default);
        start.confirm();
        let _ = start.step_down();
        assert_eq!(read_state(&file), Some((Mode::Software, true)));
        // no data folder: nothing is written, the default still starts
        assert_eq!(GraphicsStart::begin(None, false).mode(), Mode::Default);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn step_down_walks_the_ladder_and_records_each_attempt() {
        let dir = temp_dir("step");
        let file = dir.join(STATE_FILE);
        let mut start = GraphicsStart::begin(Some(&dir), false);
        for expected in LADDER.iter().skip(1) {
            assert!(start.step_down());
            assert_eq!(start.mode(), *expected);
            assert_eq!(read_state(&file), Some((*expected, true)));
        }
        assert!(!start.step_down(), "nothing after the last mode");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_software_selector_only_picks_a_software_adapter() {
        if wgpu::Instance::enabled_backend_features().is_empty() {
            return;
        }
        // only the backends software mode uses: no driver this test loads is one the app avoids
        let backends = software_backends(cfg!(target_os = "windows"));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapters = pollster::block_on(instance.enumerate_adapters(backends));
        match pick_software(&adapters, None) {
            Ok(adapter) => assert!(filmcraft_ui_egui::software_rasterizer(&adapter.get_info()).is_some()),
            Err(why) => {
                assert!(adapters.iter().all(|a| filmcraft_ui_egui::software_rasterizer(&a.get_info()).is_none()), "{why}");
            }
        }
        assert!(pick_software(&[], None).is_err());
    }
}
