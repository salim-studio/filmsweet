//! Golden-image tests for FilmSweet's renderer (test-only crate; the library is empty).
//!
//! `tests/golden.rs` builds small procedural projects (no media files), renders one frame of each
//! through the CPU reference compositor (`filmsweet-render`) and compares it with a committed PNG
//! in `goldens/` using `filmsweet_testkit::golden`. The same scenes are composited on the GPU
//! (`filmsweet-gpu`) when an adapter is available and compared with the CPU result.
//!
//! Regenerate references with `FILMSWEET_BLESS=1 cargo test -p filmsweet-golden`.
