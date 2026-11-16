//! Small charts drawn with the painter (no plotting crate): bars and columns, square and flat like the rest of the interface.

use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Ui, Vec2};

pub struct Bar {
    pub label: String,
    pub value: f64,
    pub color: Color32,
}

impl Bar {
    pub fn new(label: impl Into<String>, value: f64, color: Color32) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

fn number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

/// One row per bar: the label on the left, the bar on a track, the value at the end. `max` is the length of a full track.
pub fn hbars(ui: &mut Ui, bars: &[Bar], max: f64) {
    let row = 24.0;
    let size = Vec2::new(ui.available_width(), row * bars.len() as f32);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let c = crate::theme::of(ui);
    let (text, track) = (c.text, c.border_variant);
    let font = FontId::proportional(12.0);
    let label_w = (rect.width() * 0.34).clamp(80.0, 150.0);
    let value_w = 40.0;
    for (i, bar) in bars.iter().enumerate() {
        let y = rect.top() + row * i as f32;
        let mid = y + row / 2.0;
        painter.text(
            Pos2::new(rect.left(), mid),
            Align2::LEFT_CENTER,
            &bar.label,
            font.clone(),
            text,
        );
        let full = Rect::from_min_max(
            Pos2::new(rect.left() + label_w, y + 5.0),
            Pos2::new(rect.right() - value_w, y + row - 5.0),
        );
        painter.rect_filled(full, 0.0, track);
        if bar.value > 0.0 && max > 0.0 {
            let w = ((bar.value / max) as f32).clamp(0.0, 1.0) * full.width();
            let filled = Rect::from_min_size(full.min, Vec2::new(w.max(3.0), full.height()));
            painter.rect_filled(filled, 0.0, bar.color);
        }
        painter.text(
            Pos2::new(rect.right(), mid),
            Align2::RIGHT_CENTER,
            number(bar.value),
            font.clone(),
            text,
        );
    }
}

/// Vertical bars side by side, each with its value above and its label below.
pub fn columns(ui: &mut Ui, bars: &[Bar], max: f64, height: f32) {
    let size = Vec2::new(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let c = crate::theme::of(ui);
    let (text, track) = (c.text, c.border_variant);
    let font = FontId::proportional(12.0);
    let n = bars.len().max(1) as f32;
    let slot = rect.width() / n;
    let top = rect.top() + 18.0;
    let bottom = rect.bottom() - 20.0;
    for (i, bar) in bars.iter().enumerate() {
        let x0 = rect.left() + slot * i as f32 + slot * 0.18;
        let x1 = rect.left() + slot * (i + 1) as f32 - slot * 0.18;
        let full = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x1, bottom));
        painter.rect_filled(full, 0.0, track);
        let h = if max > 0.0 {
            ((bar.value / max) as f32).clamp(0.0, 1.0) * full.height()
        } else {
            0.0
        };
        if bar.value > 0.0 {
            let filled = Rect::from_min_max(
                Pos2::new(full.left(), full.bottom() - h.max(3.0)),
                full.right_bottom(),
            );
            painter.rect_filled(filled, 0.0, bar.color);
        }
        painter.text(
            Pos2::new((x0 + x1) / 2.0, top - 3.0),
            Align2::CENTER_BOTTOM,
            number(bar.value),
            font.clone(),
            text,
        );
        painter.text(
            Pos2::new((x0 + x1) / 2.0, bottom + 4.0),
            Align2::CENTER_TOP,
            &bar.label,
            font.clone(),
            text,
        );
    }
}
