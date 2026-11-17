use crate::json_value_text;
use eframe::egui::{self, DragValue, RichText, Ui};
use serde_json::{json, Value};
pub use tapiti_schema::{groups, FieldKind, ParamField, ParamGroup};

/// One parameter as a grid row: label with hover help, the control, and a reset when it differs from the size default.
/// Returns the new value when the user changed it.
pub fn field_row(
    ui: &mut Ui,
    field: &ParamField,
    current: &Value,
    default: Option<&Value>,
) -> Option<Value> {
    let differs = default.is_some_and(|d| d != current);
    // A setting that differs from the size's default is marked: an amber square before the label and the label in amber.
    let amber = crate::theme::of(ui).accent;
    let label = RichText::new(field.label.as_str());
    let label = if differs { label.color(amber) } else { label };
    // The row has an exact width: a share for the label, which is cut with an ellipsis when it is longer (the full label and the help
    // are on hover), and the rest for the control, which sits at the right with the reset beside it. Columns sized by their widest
    // content would make the panel as wide as its longest label and its widest drop-down.
    let width = crate::theme::body_width(ui);
    let label_width = (width * 0.52).max(96.0);
    let control_width = (width - label_width - 14.0).max(80.0);
    let height = ui.spacing().interact_size.y;
    let mut label_id = None;
    ui.allocate_ui(egui::vec2(label_width, height), |ui| {
        // A cell takes the width of what it holds unless told otherwise, and every group of rows would then have its own columns.
        ui.set_min_width(label_width);
        if differs {
            let (mark, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
            ui.painter().rect_filled(mark, 0.0, amber);
        }
        label_id = Some(
            ui.add(egui::Label::new(label).truncate())
                .on_hover_text(format!("{}\n{}", field.label.as_str(), field.help.as_str()))
                .id,
        );
    });
    let mut next = None;
    ui.allocate_ui_with_layout(
        egui::vec2(control_width, height),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.set_min_width(control_width);
            if differs {
                let reset = ui.small_button("×").on_hover_text(format!(
                    "Reset to the size default: {}",
                    default.map_or_else(String::new, json_value_text)
                ));
                reset.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("Reset {}", field.label.as_str()),
                    )
                });
                if reset.clicked() {
                    next = default.cloned();
                }
            }
            if let Some(value) = control(ui, field, current, label_id) {
                next = Some(value);
            }
        },
    );
    ui.end_row();
    next
}

/// Just the control of a parameter (no label, no reset), in whatever layout the caller is in: a drag value, a check box or a
/// drop-down, with the parameter's help on hover. Returns the new value when the user changed it.
pub fn control(
    ui: &mut Ui,
    field: &ParamField,
    current: &Value,
    label: Option<egui::Id>,
) -> Option<Value> {
    // The control is named by the label beside it, for assistive technology.
    let named = |response: egui::Response| match label {
        Some(id) => response.labelled_by(id),
        None => response,
    };
    let mut next = None;
    match &field.kind {
        FieldKind::Number { min, max, integer } => {
            let mut v = current.as_f64().unwrap_or(0.0);
            let speed = if *integer {
                0.2
            } else {
                (v.abs() * 0.02).max((max - min) / 2000.0).clamp(0.001, 5.0)
            };
            let mut drag = DragValue::new(&mut v)
                .range(*min..=*max)
                .speed(speed)
                .update_while_editing(false);
            drag = if *integer {
                drag.fixed_decimals(0)
            } else {
                drag.max_decimals(3)
            };
            if named(ui.add(drag).on_hover_text(field.help.as_str())).changed() {
                next = Some(if *integer {
                    json!(v.round() as i64)
                } else {
                    json!(v)
                });
            }
        }
        FieldKind::Boolean => {
            let mut b = current.as_bool().unwrap_or(false);
            if named(ui.checkbox(&mut b, "").on_hover_text(field.help.as_str())).changed() {
                next = Some(json!(b));
            }
        }
        FieldKind::Choice(options) => {
            let text = json_value_text(current);
            let shown = options
                .iter()
                .find(|(v, _)| *v == text)
                .map_or(text.as_str(), |(_, n)| n.as_str());
            let combo = egui::ComboBox::from_id_salt(&field.path)
                .width(100.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (value, name) in options {
                        if ui.selectable_label(*value == text, name.as_str()).clicked()
                            && *value != text
                        {
                            next = Some(
                                if matches!(field.path.as_str(), "m" | "discPorts" | "maxBends") {
                                    value.parse::<i64>().map(Value::from).unwrap_or(Value::Null)
                                } else {
                                    Value::String(value.clone())
                                },
                            );
                        }
                    }
                });
            named(combo.response);
        }
    }
    next
}
