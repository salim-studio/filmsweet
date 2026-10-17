//! The FilmSweet wordmark, drawn in code — no image assets, no trademarks from anywhere else.
//!
//! A small film frame (rounded accent tile, two sprocket holes, play triangle) followed by the
//! name: "Film" in the UI ink colour and "Sweet" in the FilmSweet accent, so it reads on dark
//! and light themes alike.

use egui::{Color32, FontFamily, FontId, Pos2, Rect, Ui, vec2};

/// The FilmSweet accent: warm rose, used for the "Sweet" half of the name and the mark.
pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x4d, 0x8e);
/// Ink for light themes.
pub const INK_DARK: Color32 = Color32::from_rgb(0x1b, 0x1c, 0x1f);
/// Ink for dark themes.
pub const INK_LIGHT: Color32 = Color32::from_rgb(0xf4, 0xf4, 0xf6);

/// The ink colour the wordmark uses on a `dark` (or light) theme.
pub fn ink(dark: bool) -> Color32 {
    if dark { INK_LIGHT } else { INK_DARK }
}

/// Draw the wordmark `height` points tall with its left edge at `left_center` (vertically
/// centred). Returns the drawn rect.
pub fn paint_wordmark(ui: &Ui, left_center: Pos2, height: f32, dark: bool) -> Option<Rect> {
    let font = FontId::new(height * 0.95, FontFamily::Proportional);
    let film = ui.painter().layout_no_wrap("Film".into(), font.clone(), ink(dark));
    let sweet = ui.painter().layout_no_wrap("Sweet".into(), font, ACCENT);
    let text_w = film.size().x + sweet.size().x;

    let mark = height;
    let gap = height * 0.34;
    let min = Pos2::new(left_center.x, left_center.y - height / 2.0);
    let rect = Rect::from_min_size(min, vec2(mark + gap + text_w, height));
    let p = ui.painter();

    // Film frame: a rounded accent tile, two sprocket holes, a play triangle in ink.
    p.rect_filled(rect, height * 0.24, ACCENT);
    let hole = height * 0.11;
    p.circle_filled(Pos2::new(min.x + height * 0.26, min.y + height * 0.24), hole, ink(dark));
    p.circle_filled(Pos2::new(min.x + height * 0.26, min.y + height * 0.76), hole, ink(dark));
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(min.x + height * 0.44, min.y + height * 0.30),
            Pos2::new(min.x + height * 0.44, min.y + height * 0.70),
            Pos2::new(min.x + height * 0.78, min.y + height * 0.50),
        ],
        ink(dark),
        egui::Stroke::NONE,
    ));

    // Name: "Film" in ink, "Sweet" in the accent.
    let base = Pos2::new(min.x + mark + gap, left_center.y);
    let film_w = film.size().x;
    p.galley(Pos2::new(base.x, base.y - film.size().y / 2.0), film, Color32::PLACEHOLDER);
    p.galley(Pos2::new(base.x + film_w, base.y - sweet.size().y / 2.0), sweet, Color32::PLACEHOLDER);
    Some(rect)
}
