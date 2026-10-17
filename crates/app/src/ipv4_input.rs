use eframe::egui::{self, Align, FontId, Id, Layout, Margin, Stroke, TextEdit, Ui, Vec2};

fn segment_id(name: &str, index: usize) -> Id {
    if index == 0 {
        Id::unique(name)
    } else {
        Id::unique(("ipv4_segment", name, index))
    }
}

fn parts(value: &str, subnet: bool) -> [String; 4] {
    let mask = if subnet && !value.contains('.') {
        simple_ip_core::parse_subnet(value)
            .ok()
            .map(simple_ip_core::prefix_mask)
            .map(|mask| mask.to_string())
    } else {
        None
    };
    if subnet && !value.contains('.') && mask.is_none() {
        return std::array::from_fn(|_| String::new());
    }
    let text = mask.as_deref().unwrap_or(value);
    let mut split = text.split('.');
    std::array::from_fn(|_| split.next().unwrap_or_default().to_owned())
}

fn join(parts: &[String; 4]) -> String {
    if parts.iter().all(String::is_empty) {
        String::new()
    } else {
        parts.join(".")
    }
}

/// One IPv4 control: four octets and, for a subnet, a linked CIDR prefix.
/// The draft remains a string so incomplete edits survive profile switching.
pub fn show(
    ui: &mut Ui,
    name: &str,
    value: &mut String,
    width: f32,
    height: f32,
) -> egui::Response {
    let subnet = name == "subnet";
    let prefix_id = segment_id(name, 4);
    let focused = ui.ctx().memory(|memory| memory.focused());
    let active = (0..=if subnet { 4 } else { 3 }).find(|&i| focused == Some(segment_id(name, i)));
    let mut full_paste = false;
    if active.is_some() {
        ui.ctx().input_mut(|input| {
            input.events.retain(|event| {
                if let egui::Event::Paste(text) = event {
                    let text = text.trim();
                    if text.contains('.') || (subnet && text.starts_with('/')) {
                        // Preserve invalid full strings for normal validation, never truncate a pasted address.
                        *value = text.to_owned();
                        full_paste = true;
                        return false;
                    }
                }
                true
            });
        });
    }
    let mut octets = parts(value, subnet);
    let mut prefix = if subnet {
        simple_ip_core::parse_subnet(value)
            .map(|n| n.to_string())
            .unwrap_or_else(|_| value.strip_prefix('/').unwrap_or("").to_owned())
    } else {
        String::new()
    };
    let mut advance = None;
    if let Some(index) = active {
        if index < 3
            && ui.ctx().input_mut(|input| {
                let dot = input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Text(text) if text == "."));
                if dot {
                    input
                        .events
                        .retain(|event| !matches!(event, egui::Event::Text(text) if text == "."));
                }
                dot
            })
        {
            advance = Some(index + 1);
        }
        if index > 0
            && octets.get(index).is_some_and(String::is_empty)
            && ui
                .ctx()
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace))
        {
            advance = Some(index - 1);
        }
    }
    let mut changed = full_paste;
    let mut prefix_changed = false;
    let layout = ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let segment_width = ((width - 22.0 - 40.0 - 12.0 - 12.0) / 4.0).max(24.0);
            ui.add_space(6.0);
            let mut response: Option<egui::Response> = None;
            for (index, octet) in octets.iter_mut().enumerate() {
                if index > 0 {
                    ui.add_sized([4.0, height], egui::Label::new("."));
                }
                let before = octet.clone();
                let output = ui.add_sized(
                    [segment_width, height],
                    TextEdit::singleline(octet)
                        .id(segment_id(name, index))
                        .font(FontId::proportional(14.0))
                        .frame(egui::Frame::NONE)
                        .margin(Margin::symmetric(2, 3))
                        .align(egui::Align2::CENTER_CENTER)
                        .desired_width(segment_width)
                        .char_limit(3),
                );
                let output = output.on_hover_text(format!("{}: {} / 0–255", name, index + 1));
                if output.changed() {
                    // Retain invalid input for validation rather than silently rewriting what was entered.
                    changed = true;
                    if octet.len() == 3
                        && octet.parse::<u8>().is_ok()
                        && before.len() < 3
                        && index < 3
                    {
                        advance = Some(index + 1);
                    }
                }
                response = Some(match response {
                    None => output,
                    Some(previous) => previous.union(output),
                });
            }
            ui.add_space(4.0);
            if subnet {
                ui.label("/");
                let input = ui.add_sized(
                    [28.0, height],
                    TextEdit::singleline(&mut prefix)
                        .id(prefix_id)
                        .font(FontId::proportional(14.0))
                        .frame(egui::Frame::NONE)
                        .margin(Margin::symmetric(1, 3))
                        .char_limit(2)
                        .align(egui::Align2::LEFT_CENTER),
                );
                prefix_changed = input.changed();
                response = Some(response.unwrap().union(input));
            }
            response.unwrap()
        },
    );
    if prefix_changed {
        *value = format!("/{prefix}");
    } else if changed && !full_paste {
        *value = join(&octets);
    }
    if let Some(index) = advance {
        let id = segment_id(name, index);
        ui.ctx().memory_mut(|memory| memory.request_focus(id));
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(octets[index].len()),
            )));
        state.store(ui.ctx(), id);
    }
    let mut response = layout.inner;
    response.rect = egui::Rect::from_min_size(layout.response.rect.min, Vec2::new(width, height));
    let visuals = if response.contains_pointer() || active.is_some() {
        &ui.visuals().widgets.hovered
    } else {
        &ui.visuals().widgets.inactive
    };
    ui.painter().rect_stroke(
        response.rect,
        4.0,
        if active.is_some() {
            Stroke::new(1.0, ui.visuals().selection.stroke.color)
        } else {
            visuals.bg_stroke
        },
        egui::StrokeKind::Inside,
    );
    if changed || prefix_changed {
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(ctx: &egui::Context, name: &str, value: &mut String, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                show(ui, name, value, 280.0, 28.0);
            },
        );
        output.textures_delta.clear();
    }
    #[test]
    fn whole_address_paste_and_dot_navigation() {
        let ctx = egui::Context::default();
        let mut value = "192.0.2.1".to_owned();
        for _ in 0..3 {
            render(&ctx, "address", &mut value, vec![]);
        }
        ctx.memory_mut(|m| m.request_focus(segment_id("address", 0)));
        render(
            &ctx,
            "address",
            &mut value,
            vec![egui::Event::Paste("203.0.113.9".into())],
        );
        assert_eq!(value, "203.0.113.9");
        render(
            &ctx,
            "address",
            &mut value,
            vec![egui::Event::Text(".".into())],
        );
        assert_eq!(ctx.memory(|m| m.focused()), Some(segment_id("address", 1)));
        assert_eq!(value, "203.0.113.9");
    }
    #[test]
    fn subnet_prefix_paste_is_displayed_as_mask_without_changing_its_meaning() {
        let ctx = egui::Context::default();
        let mut value = "255.255.255.0".to_owned();
        for _ in 0..3 {
            render(&ctx, "subnet", &mut value, vec![]);
        }
        ctx.memory_mut(|m| m.request_focus(segment_id("subnet", 2)));
        render(
            &ctx,
            "subnet",
            &mut value,
            vec![egui::Event::Paste("/31".into())],
        );
        assert_eq!(simple_ip_core::parse_subnet(&value), Ok(31));
        assert_eq!(parts(&value, true), ["255", "255", "255", "254"]);
    }
    #[test]
    fn segments_preserve_partial_addresses_and_normalize_prefix_for_display() {
        assert_eq!(parts("/24", true), ["255", "255", "255", "0"]);
        assert_eq!(join(&parts("192..2.", false)), "192..2.");
        assert_eq!(join(&parts("", false)), "");
    }
}
