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
    let label = RichText::new(field.label.as_str());
    let label = if differs {
        label.color(crate::theme::of(ui).accent)
    } else {
        label
    };
    // The label takes what the control and the reset leave, and is cut with an ellipsis when it is longer: a column of fixed width keeps
    // the panel from growing to fit the longest label (Plex Mono is wide). The full label and the help are on hover.
    // The control column of a group is as wide as its widest control, a drop-down of 100 points, plus the gaps of the grid.
    let reserve = 126.0 + if differs { 70.0 } else { 0.0 };
    let label_width = (crate::theme::body_width(ui) - reserve).max(96.0);
    ui.allocate_ui(
        egui::vec2(label_width, ui.spacing().interact_size.y),
        |ui| {
            ui.add(egui::Label::new(label).truncate())
                .on_hover_text(format!("{}\n{}", field.label.as_str(), field.help.as_str()));
        },
    );
    let next = control(ui, field, current);
    let mut next = next;
    if differs {
        if ui
            .small_button("default")
            .on_hover_text("Reset to the size default")
            .clicked()
        {
            next = default.cloned();
        }
    } else {
        ui.label("");
    }
    ui.end_row();
    next
}

/// Just the control of a parameter (no label, no reset), in whatever layout the caller is in: a drag value, a check box or a
/// drop-down, with the parameter's help on hover. Returns the new value when the user changed it.
pub fn control(ui: &mut Ui, field: &ParamField, current: &Value) -> Option<Value> {
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
            if ui.add(drag).on_hover_text(field.help.as_str()).changed() {
                next = Some(if *integer {
                    json!(v.round() as i64)
                } else {
                    json!(v)
                });
            }
        }
        FieldKind::Boolean => {
            let mut b = current.as_bool().unwrap_or(false);
            if ui
                .checkbox(&mut b, "")
                .on_hover_text(field.help.as_str())
                .changed()
            {
                next = Some(json!(b));
            }
        }
        FieldKind::Choice(options) => {
            let text = json_value_text(current);
            let shown = options
                .iter()
                .find(|(v, _)| *v == text)
                .map_or(text.as_str(), |(_, n)| n.as_str());
            egui::ComboBox::from_id_salt(&field.path)
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
        }
    }
    next
}
