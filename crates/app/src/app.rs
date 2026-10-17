use crate::{
    language,
    storage::{State, Store},
};
use crate::{
    session::Session,
    theme::{self, Palette},
};
use eframe::egui::{
    self, Align, Button, Color32, FontId, Frame, Id, Layout, Margin, RichText, Stroke, Ui, Vec2,
};
use std::time::Instant;

fn t(thai: bool, english: &'static str, translated: &'static str) -> &'static str {
    if thai { translated } else { english }
}

#[derive(Clone, Copy)]
enum Symbol {
    Refresh,
    Settings,
    Manual,
    Copy,
    Desktop,
    Preview,
    Wired,
    Wifi,
    Warning,
}

// Phosphor Regular is embedded independently from the Noto text family.
fn paint_symbol(ui: &Ui, rect: egui::Rect, symbol: Symbol, color: Color32) {
    let glyph = match symbol {
        Symbol::Settings => '\u{e270}',
        Symbol::Refresh => '\u{e036}',
        Symbol::Manual => '\u{e3b4}',
        Symbol::Copy => '\u{e1ca}',
        Symbol::Desktop => '\u{e560}',
        Symbol::Preview => '\u{e30c}',
        Symbol::Wired => '\u{edde}',
        Symbol::Wifi => '\u{e4ea}',
        Symbol::Warning => '\u{e4e0}',
    };
    ui.painter().text(
        egui::pos2(rect.left() + 15.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::new(16.0, egui::FontFamily::Name("symbols".into())),
        color,
    );
}
fn symbol_button(
    ui: &mut Ui,
    symbol: Symbol,
    label: &str,
    primary: Option<Color32>,
) -> egui::Response {
    let color = primary.map_or(ui.visuals().text_color(), |_| Color32::WHITE);
    let mut button = Button::new(RichText::new(format!("\u{2003}    {label}")).color(color));
    if let Some(fill) = primary {
        button = button.fill(fill);
    }
    let response = ui.add(button);
    paint_symbol(ui, response.rect, symbol, color);
    response
}

fn icon_button(ui: &mut Ui, symbol: Symbol, tooltip: &str) -> egui::Response {
    let response = ui.add_sized([30.0, ROW_HEIGHT], Button::new(""));
    let color = ui.visuals().text_color();
    paint_symbol(ui, response.rect, symbol, color);
    response.on_hover_text(tooltip)
}

fn is_wireless(name: &str, description: &str) -> bool {
    let text = format!("{name} {description}").to_lowercase();
    ["wi-fi", "wifi", "wireless", "wlan", "802.11"]
        .iter()
        .any(|word| text.contains(word))
}

pub struct SimpleIpApp {
    session: Session,
    thai: bool,
    dark: bool,
    status: String,
    loading: Option<std::sync::mpsc::Receiver<Result<Vec<simple_ip_core::SampleAdapter>, String>>>,
    show_help: bool,
    scroll_to_feedback: bool,
    focus_field: Option<&'static str>,
    storage: Option<Store>,
    last_persist: Instant,
    window_size: [f32; 2],
    #[cfg(feature = "capture")]
    capture_frames: u32,
    #[cfg(test)]
    preview_button_rect: egui::Rect,
    #[cfg(test)]
    aligned_rows: Vec<[egui::Rect; 3]>,
    #[cfg(test)]
    painted_text: Vec<(String, egui::Pos2)>,
}

impl SimpleIpApp {
    pub fn new(cc: &eframe::CreationContext<'_>, thai: bool, scale: f32, store: Store) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, thai, scale);
        app.dark = store.state.dark;
        app.window_size = store.state.size;
        theme::install_style(&cc.egui_ctx, app.dark);
        app.storage = Some(store);
        app.refresh();
        #[cfg(feature = "capture")]
        let app = {
            let mut app = app;
            if let Ok(scenario) = std::env::var("SIMPLE_IP_CAPTURE_SCENARIO") {
                app.storage = None;
                app.session = Session::default();
                app.session.load_lab_sample();
                if scenario.contains("dhcp") {
                    app.session.reset_draft();
                }
                if scenario.contains("dirty") {
                    app.session.draft_mut().address = "198.51.100.25".into();
                }
                if scenario.contains("dark") {
                    app.dark = true;
                    theme::install_style(&cc.egui_ctx, true);
                }
                if scenario.contains("preview") {
                    app.preview();
                }
                if scenario.contains("invalid") {
                    app.session.draft_mut().subnet = "255.0.255.0".into();
                    app.preview();
                }
            }
            app
        };
        app
    }

    fn with_context(ctx: &egui::Context, thai: bool, scale: f32) -> Self {
        theme::install_fonts(ctx);
        theme::install_style(ctx, false);
        ctx.set_zoom_factor(scale);
        Self {
            session: Session::default(),
            thai,
            dark: false,
            status: t(
                thai,
                "Ready. Sample data only; nothing is applied.",
                "พร้อมใช้งาน ข้อมูลจำลอง ยังไม่เปลี่ยนเครือข่ายจริง",
            )
            .into(),
            loading: None,
            show_help: false,
            scroll_to_feedback: false,
            focus_field: None,
            storage: None,
            last_persist: Instant::now(),
            window_size: [700.0, 440.0],
            #[cfg(feature = "capture")]
            capture_frames: 0,
            #[cfg(test)]
            preview_button_rect: egui::Rect::NOTHING,
            #[cfg(test)]
            aligned_rows: vec![],
            #[cfg(test)]
            painted_text: vec![],
        }
    }

    fn persist(&mut self) {
        let state = State {
            version: 2,
            thai: self.thai,
            dark: self.dark,
            size: self.window_size,
        };
        if let Some(store) = &mut self.storage
            && store.write(state).is_err()
        {
            self.status = t(
                self.thai,
                "Storage failed. Changes remain in memory; see the storage notice.",
                "บันทึกลงไฟล์ไม่สำเร็จ ข้อมูลยังอยู่ในหน่วยความจำ ดูรายละเอียดการบันทึก",
            )
            .into();
        }
        self.last_persist = Instant::now();
    }

    fn preview(&mut self) {
        let valid = self.session.make_preview();
        self.scroll_to_feedback = true;
        self.status = if valid {
            t(
                self.thai,
                "Change preview prepared. No changes were applied.",
                "เตรียมตัวอย่างการเปลี่ยนค่าแล้ว ยังไม่มีการเปลี่ยนค่าจริง",
            )
        } else {
            t(
                self.thai,
                "Some settings need attention. Nothing has been applied.",
                "มีค่าที่ต้องตรวจสอบ ยังไม่มีการเปลี่ยนค่า",
            )
        }
        .into();
    }

    /// Reads the machine's adapters on a worker thread; `poll_adapters` picks up the result.
    fn refresh(&mut self) {
        if self.loading.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::adapters::read());
        });
        self.loading = Some(rx);
        self.status = t(self.thai, "Reading network adapters…", "กำลังอ่านอะแดปเตอร์…").into();
    }

    fn poll_adapters(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.loading else { return };
        match rx.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            Ok(Ok(adapters)) if !adapters.is_empty() => {
                self.session.load_adapters(adapters);
                self.status = t(
                    self.thai,
                    "Adapters read from Windows. Nothing is applied.",
                    "อ่านค่าจาก Windows แล้ว ยังไม่มีการเปลี่ยนค่า",
                )
                .into();
            }
            Ok(Ok(_)) => {
                self.status = t(
                    self.thai,
                    "No network adapters found; showing sample data.",
                    "ไม่พบอะแดปเตอร์ แสดงข้อมูลจำลองแทน",
                )
                .into();
            }
            Ok(Err(error)) => {
                self.status = format!(
                    "{} {error}",
                    t(
                        self.thai,
                        "Cannot read adapters; showing sample data.",
                        "อ่านอะแดปเตอร์ไม่ได้ แสดงข้อมูลจำลองแทน",
                    )
                );
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
        }
        self.loading = None;
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.show_help = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            self.refresh();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
            self.preview();
        }
    }

    fn footer(&self, ui: &mut Ui, p: Palette) {
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(p.surface)
                    .inner_margin(Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(if self.session.live {
                            t(self.thai, "Live · read-only", "ข้อมูลจริง · อ่านอย่างเดียว")
                        } else {
                            t(self.thai, "Sample data", "ข้อมูลจำลอง")
                        })
                        .color(p.accent)
                        .strong()
                        .size(12.0),
                    );
                    ui.label(
                        RichText::new(language::status(self.thai, &self.status))
                            .size(12.5)
                            .color(p.muted),
                    );
                });
            });
    }

    fn adapter_list(&mut self, ui: &mut Ui, p: Palette) {
        let thai = self.thai;
        egui::Panel::left("adapters")
            .resizable(false)
            .exact_size(150.0)
            .frame(Frame::new().fill(p.surface).inner_margin(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(thai, "Adapters", "อะแดปเตอร์"))
                            .size(12.0)
                            .color(p.muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Symbol::Refresh, t(thai, "Refresh (F5)", "รีเฟรช (F5)"))
                            .clicked()
                        {
                            self.refresh();
                        }
                    });
                });
                ui.add_space(2.0);
                let before = self.session.selected_adapter;
                let dirty = self.session.is_dirty();
                for (index, adapter) in self.session.adapters.iter().enumerate() {
                    let selected = before == index;
                    let color = if adapter.connected { p.ink } else { p.muted };
                    let response = ui.add_sized(
                        [ui.available_width(), 32.0],
                        Button::new(())
                            .left_text(
                                RichText::new(format!("\u{2003}    {}", adapter.name))
                                    .size(13.0)
                                    .color(color),
                            )
                            .fill(if selected { p.selection } else { p.surface })
                            .stroke(Stroke::new(if selected { 1.0 } else { 0.0 }, p.border))
                            .corner_radius(6)
                            .truncate(),
                    );
                    paint_symbol(
                        ui,
                        response.rect,
                        if is_wireless(&adapter.name, &adapter.description) {
                            Symbol::Wifi
                        } else {
                            Symbol::Wired
                        },
                        color,
                    );
                    let response = response.on_hover_text(format!(
                        "{}
{}",
                        adapter.name,
                        if adapter.connected {
                            t(thai, "Link up", "เชื่อมต่ออยู่")
                        } else {
                            t(thai, "Link down", "ไม่เชื่อมต่อ")
                        }
                    ));
                    if response.clicked() {
                        self.session.selected_adapter = index;
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    let menu = ui.menu_button(
                        format!("\u{2003}    {}", t(thai, "Settings", "ตั้งค่า")),
                        |ui| {
                            let theme = if self.dark {
                                t(thai, "Light theme", "ธีมสว่าง")
                            } else {
                                t(thai, "Dark theme", "ธีมมืด")
                            };
                            if ui.button(theme).clicked() {
                                self.dark = !self.dark;
                                theme::install_style(ui.ctx(), self.dark);
                                ui.close();
                            }
                            if ui
                                .button(if thai {
                                    "English"
                                } else {
                                    "ภาษาไทย"
                                })
                                .clicked()
                            {
                                self.thai = !self.thai;
                                ui.close();
                            }
                            if ui.button(t(thai, "Help (F1)", "วิธีใช้ (F1)")).clicked()
                            {
                                self.show_help = true;
                                ui.close();
                            }
                        },
                    );
                    paint_symbol(ui, menu.response.rect, Symbol::Settings, p.ink);
                });
                if before != self.session.selected_adapter {
                    // Unedited forms follow the new adapter; edits are never discarded silently.
                    if dirty {
                        self.session.invalidate_preview();
                    } else {
                        self.session.reset_draft();
                    }
                }
            });
    }

    fn adapter_selector(&mut self, ui: &mut Ui, p: Palette) {
        ui.horizontal(|ui| {
            let selected = &self.session.adapters[self.session.selected_adapter];
            let adapter_symbol = if is_wireless(&selected.name, &selected.description) {
                Symbol::Wifi
            } else {
                Symbol::Wired
            };
            let label = ui.label(
                RichText::new(format!(
                    "\u{2003}    {}",
                    self.session.adapters[self.session.selected_adapter].name
                ))
                .size(16.0)
                .strong(),
            );
            paint_symbol(ui, label.rect, adapter_symbol, p.ink);
        });
        let thai = self.thai;
        let adapter = &self.session.adapters[self.session.selected_adapter];
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} · {} · DHCP {}",
                    adapter.description,
                    adapter.mac,
                    adapter
                        .dhcp_server
                        .map_or_else(|| "-".into(), |ip| ip.to_string())
                ))
                .size(12.0)
                .color(p.muted),
            );
            let (status_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            if adapter.connected {
                ui.painter()
                    .circle_filled(status_rect.center(), 3.5, p.accent);
            } else {
                ui.painter()
                    .circle_stroke(status_rect.center(), 4.0, Stroke::new(1.5, p.muted));
                ui.painter().line_segment(
                    [
                        status_rect.left_top() + egui::vec2(2.0, 2.0),
                        status_rect.right_bottom() - egui::vec2(2.0, 2.0),
                    ],
                    Stroke::new(1.5, p.muted),
                );
            }
            ui.label(
                RichText::new(if adapter.connected {
                    t(thai, "Link up", "เชื่อมต่ออยู่")
                } else {
                    t(thai, "Link down", "ไม่เชื่อมต่อ")
                })
                .size(12.0)
                .color(p.accent),
            );
        });
        ui.add_space(4.0);
    }

    fn settings_grid(&mut self, ui: &mut Ui, p: Palette) {
        let thai = self.thai;
        let adapter = self.session.adapters[self.session.selected_adapter].clone();
        let missing = t(thai, "Not assigned", "ไม่มีค่า");
        let value = |ip: Option<std::net::Ipv4Addr>| {
            ip.map(|ip| ip.to_string())
                .unwrap_or_else(|| missing.into())
        };
        let address = value(adapter.address);
        let subnet = adapter
            .address
            .map(|_| simple_ip_core::prefix_mask(adapter.prefix).to_string())
            .unwrap_or_else(|| missing.into());
        let gateway = value(adapter.gateway);
        let dns1 = value(adapter.dns.first().copied());
        let dns2 = value(adapter.dns.get(1).copied());
        let ip_mode = if matches!(adapter.config.ip, simple_ip_core::IpConfig::Dhcp) {
            "DHCP"
        } else {
            t(thai, "Manual", "กำหนดเอง")
        };
        let dns_mode = if matches!(adapter.config.dns, simple_ip_core::DnsConfig::Automatic) {
            t(thai, "Automatic", "อัตโนมัติ")
        } else {
            t(thai, "Manual", "กำหนดเอง")
        };
        let available = ui.available_width();
        // An IPv4 editor needs ~230 px; wider fields only push the columns apart.
        let widths = [
            112.0,
            130.0,
            (available - 112.0 - 130.0 - 24.0).clamp(200.0, 240.0),
        ];
        let original = self.session.draft().clone();
        let dirty = self.session.is_dirty();
        let can_copy = adapter.address.is_some();
        let mut copy_clicked = false;
        let issues = self.session.issues.clone();
        let draft = self.session.draft_mut();
        #[cfg(test)]
        let mut row_rects = Vec::new();
        egui::Grid::new("settings_matrix")
            .num_columns(3)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (label, width, symbol) in [
                    ("", widths[0], None),
                    (
                        t(thai, "Current", "ค่าปัจจุบัน"),
                        widths[1],
                        Some(Symbol::Desktop),
                    ),
                    (
                        t(thai, "Desired", "ค่าที่ต้องการ"),
                        widths[2],
                        Some(Symbol::Manual),
                    ),
                ] {
                    let text = if symbol.is_some() {
                        format!("\u{2003}    {label}")
                    } else {
                        label.into()
                    };
                    let response = grid_label(
                        ui,
                        &text,
                        width,
                        if label == t(thai, "Desired", "ค่าที่ต้องการ") {
                            p.ink
                        } else {
                            p.muted
                        },
                    );
                    if let Some(symbol) = symbol {
                        paint_symbol(ui, response.rect, symbol, p.muted);
                    }
                    if label == t(thai, "Current", "ค่าปัจจุบัน") && can_copy
                    {
                        // Copies the current column into the desired column as manual values.
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(response.rect.left() + width - 30.0, response.rect.top()),
                            Vec2::new(30.0, response.rect.height()),
                        );
                        let copy = ui
                            .interact(rect, Id::unique("copy_current"), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(t(
                                thai,
                                "Use current values as manual settings",
                                "ใช้ค่าปัจจุบันเป็นค่ากำหนดเอง",
                            ));
                        paint_symbol(
                            ui,
                            rect,
                            Symbol::Copy,
                            if copy.hovered() { p.primary } else { p.muted },
                        );
                        copy_clicked = copy.clicked();
                    }
                    if label == t(thai, "Desired", "ค่าที่ต้องการ") && dirty
                    {
                        let galley = ui.painter().layout_no_wrap(
                            t(thai, "Edited", "แก้ไขแล้ว").into(),
                            FontId::proportional(11.0),
                            p.primary,
                        );
                        let pos = egui::pos2(
                            response.rect.left() + width - galley.size().x,
                            response.rect.center().y - galley.size().y / 2.0,
                        );
                        ui.painter().galley(pos, galley, p.primary);
                        paint_symbol(
                            ui,
                            egui::Rect::from_min_size(
                                pos - egui::vec2(29.0, 0.0),
                                Vec2::new(28.0, response.rect.height()),
                            ),
                            Symbol::Manual,
                            p.primary,
                        );
                    }
                }
                ui.end_row();

                grid_label(ui, t(thai, "IP mode", "โหมด IP"), widths[0], p.ink);
                grid_label(ui, ip_mode, widths[1], p.ink);
                let ip_controls = ui.allocate_ui_with_layout(
                    Vec2::new(widths[2], ROW_HEIGHT),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.radio_value(&mut draft.automatic_ip, true, "DHCP");
                        if ui
                            .radio_value(
                                &mut draft.automatic_ip,
                                false,
                                t(thai, "Manual", "กำหนดเอง"),
                            )
                            .changed()
                            && !draft.automatic_ip
                        {
                            draft.automatic_dns = false;
                        }
                    },
                );
                difference_mark(
                    ui,
                    egui::Rect::from_min_size(
                        ip_controls.response.rect.min,
                        Vec2::new(widths[2], ROW_HEIGHT),
                    ),
                    draft.automatic_ip
                        != matches!(adapter.config.ip, simple_ip_core::IpConfig::Dhcp),
                    p,
                );
                ui.end_row();

                let rects = edit_row(
                    ui,
                    "IP address",
                    &address,
                    "192.0.2.10",
                    "address",
                    &mut draft.address,
                    !draft.automatic_ip,
                    widths,
                    p,
                    &issues,
                    thai,
                );
                #[cfg(test)]
                row_rects.push(rects);
                #[cfg(not(test))]
                let _ = rects;
                let rects = edit_row(
                    ui,
                    t(thai, "Subnet / prefix", "Subnet / prefix"),
                    &subnet,
                    "255.255.255.0",
                    "subnet",
                    &mut draft.subnet,
                    !draft.automatic_ip,
                    widths,
                    p,
                    &issues,
                    thai,
                );
                #[cfg(test)]
                row_rects.push(rects);
                #[cfg(not(test))]
                let _ = rects;
                let rects = edit_row(
                    ui,
                    "Gateway",
                    &gateway,
                    t(thai, "Optional", "ไม่บังคับ"),
                    "gateway",
                    &mut draft.gateway,
                    !draft.automatic_ip,
                    widths,
                    p,
                    &issues,
                    thai,
                );
                #[cfg(test)]
                row_rects.push(rects);
                #[cfg(not(test))]
                let _ = rects;

                grid_label(ui, t(thai, "DNS mode", "โหมด DNS"), widths[0], p.ink);
                grid_label(ui, dns_mode, widths[1], p.ink);
                let dns_controls = ui.allocate_ui_with_layout(
                    Vec2::new(widths[2], ROW_HEIGHT),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_enabled_ui(draft.automatic_ip, |ui| {
                            ui.radio_value(
                                &mut draft.automatic_dns,
                                true,
                                t(thai, "Auto", "อัตโนมัติ"),
                            )
                            .on_disabled_hover_text(t(
                                thai,
                                "Automatic DNS requires DHCP for IP.",
                                "DNS อัตโนมัติใช้ได้เมื่อโหมด IP เป็น DHCP",
                            ));
                        });
                        ui.radio_value(
                            &mut draft.automatic_dns,
                            false,
                            t(thai, "Manual", "กำหนดเอง"),
                        );
                    },
                );
                difference_mark(
                    ui,
                    egui::Rect::from_min_size(
                        dns_controls.response.rect.min,
                        Vec2::new(widths[2], ROW_HEIGHT),
                    ),
                    draft.automatic_dns
                        != matches!(adapter.config.dns, simple_ip_core::DnsConfig::Automatic),
                    p,
                );
                ui.end_row();

                let rects = edit_row(
                    ui,
                    t(thai, "Preferred DNS", "DNS หลัก"),
                    &dns1,
                    "192.0.2.53",
                    "preferred_dns",
                    &mut draft.preferred_dns,
                    !draft.automatic_dns,
                    widths,
                    p,
                    &issues,
                    thai,
                );
                #[cfg(test)]
                row_rects.push(rects);
                #[cfg(not(test))]
                let _ = rects;
                let rects = edit_row(
                    ui,
                    t(thai, "Alternate DNS", "DNS สำรอง"),
                    &dns2,
                    t(thai, "Optional", "ไม่บังคับ"),
                    "alternate_dns",
                    &mut draft.alternate_dns,
                    !draft.automatic_dns,
                    widths,
                    p,
                    &issues,
                    thai,
                );
                #[cfg(test)]
                row_rects.push(rects);
                #[cfg(not(test))]
                let _ = rects;
            });
        #[cfg(test)]
        {
            self.aligned_rows = row_rects;
        }
        if copy_clicked {
            self.session.copy_current();
            self.status = t(
                thai,
                "Copied as manual settings. Nothing applied.",
                "คัดลอกเป็นค่ากำหนดเองแล้ว ยังไม่เปลี่ยนเครือข่าย",
            )
            .into();
        }
        if *self.session.draft() != original {
            self.session.draft_changed();
        }
    }

    fn actions(&mut self, ui: &mut Ui, p: Palette) {
        let thai = self.thai;
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), ROW_HEIGHT),
            Layout::right_to_left(Align::Center),
            |ui| {
                let response = symbol_button(
                    ui,
                    Symbol::Preview,
                    t(thai, "Preview changes", "ดูการเปลี่ยนค่า"),
                    Some(p.primary),
                )
                .on_hover_text("Ctrl+Enter · Preview only");
                #[cfg(test)]
                {
                    self.preview_button_rect = response.rect;
                }
                if response.clicked() {
                    self.preview();
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(
                        RichText::new(t(thai, "◆ Different from current", "◆ ต่างจากค่าปัจจุบัน"))
                            .size(12.0)
                            .color(p.muted),
                    );
                });
            },
        );
    }
    fn feedback(&mut self, ui: &mut Ui, p: Palette) {
        let thai = self.thai;
        if self.scroll_to_feedback {
            ui.scroll_to_cursor(Some(Align::TOP));
            self.scroll_to_feedback = false;
        }
        if !self.session.issues.is_empty() {
            ui.add_space(10.0);
            ui.separator();
            let warning = ui.label(
                RichText::new(format!(
                    "\u{2003}    {}",
                    t(thai, "Check these settings", "ตรวจสอบค่าต่อไปนี้")
                ))
                .strong()
                .color(p.primary),
            );
            paint_symbol(ui, warning.rect, Symbol::Warning, p.primary);
            for issue in &self.session.issues {
                if ui
                    .add(
                        Button::new(format!(
                            "{}: {}",
                            language::field(thai, issue.field),
                            language::message(thai, issue.message)
                        ))
                        .frame(false)
                        .wrap(),
                    )
                    .clicked()
                {
                    self.focus_field = Some(language::input_id(issue.field));
                }
            }
        }
        let mut close_preview = false;
        if let Some(plan) = &self.session.preview {
            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        t(thai, "Change preview", "ตัวอย่างการเปลี่ยนค่า"),
                        plan.target_name
                    ))
                    .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    close_preview = ui.button(t(thai, "Close", "ปิด")).clicked();
                });
            });
            if plan.changes.is_empty() {
                ui.label(t(
                    thai,
                    "No configuration changes. The draft matches this sample adapter.",
                    "ไม่มีค่าที่เปลี่ยน ค่าที่เตรียมตรงกับอะแดปเตอร์จำลองนี้",
                ));
            } else {
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        t(thai, "Adapter ID", "รหัสอะแดปเตอร์"),
                        plan.target_id
                    ))
                    .size(11.0)
                    .color(p.muted),
                );
                egui::Grid::new("change_preview")
                    .num_columns(3)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        for heading in [
                            t(thai, "Setting", "รายการ"),
                            t(thai, "Before", "ก่อน"),
                            t(thai, "After", "หลัง"),
                        ] {
                            ui.strong(heading);
                        }
                        ui.end_row();
                        for change in &plan.changes {
                            ui.label(language::field(thai, change.field));
                            ui.label(
                                RichText::new(language::field(thai, &change.before)).color(p.muted),
                            );
                            ui.label(RichText::new(language::field(thai, &change.after)).strong());
                            ui.end_row();
                        }
                    });
                if self.session.draft().automatic_ip {
                    ui.label(t(
                        thai,
                        "The DHCP address is assigned by the server and cannot be predicted here.",
                        "IP จาก DHCP จะถูกกำหนดโดย server ไม่สามารถระบุค่าที่จะได้รับล่วงหน้าได้",
                    ));
                }
            }
            ui.label(
                RichText::new(t(
                    thai,
                    "The sample snapshot has not been modified.",
                    "ข้อมูลปัจจุบันจำลองไม่ได้ถูกเปลี่ยน",
                ))
                .color(p.accent)
                .size(12.0),
            );
        }
        if close_preview {
            self.session.invalidate_preview();
        }
    }

    fn help(&mut self, ctx: &egui::Context) {
        let thai = self.thai;
        egui::Window::new(t(thai, "About this preview", "เกี่ยวกับต้นแบบนี้"))
            .open(&mut self.show_help).resizable(false).default_width(430.0).show(ctx, |ui| {
                ui.label(t(thai, "CP1: native Rust single-page prototype.", "CP1: ต้นแบบ Rust หน้าเดียวบน Windows"));
                ui.label(t(thai, "Adapters are read from Windows (read-only). This build cannot change network settings yet.", "อ่านอะแดปเตอร์จาก Windows แบบอ่านอย่างเดียว รุ่นนี้ยังเปลี่ยนค่าเครือข่ายไม่ได้"));
                ui.separator();
                for (key, description) in [
                    ("Ctrl+Enter", t(thai, "Preview changes only", "ดูตัวอย่างการเปลี่ยนค่า")),
                    ("F1", t(thai, "Help", "วิธีใช้")),
                    ("F5", t(thai, "Refresh sample snapshot", "รีเฟรชข้อมูลจำลอง")),
                ] { ui.horizontal(|ui| { ui.monospace(key); ui.label(description); }); }
                ui.separator();
                ui.label(t(thai, "Enter does not apply network settings. Only language, theme and window size are stored.", "Enter ไม่เปลี่ยนเครือข่าย เก็บไว้เฉพาะภาษา ธีม และขนาดหน้าต่าง"));
            });
    }
}

impl SimpleIpApp {
    fn render(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if let Some(field) = self.focus_field.take() {
            ctx.data_mut(|data| data.insert_temp(Id::unique("requested_issue_field"), field));
        }
        if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
            self.window_size = [rect.width().max(700.0), rect.height().max(440.0)];
        }
        self.poll_adapters(&ctx);
        self.shortcuts(&ctx);
        let p = Palette::new(self.dark);
        self.footer(ui, p);
        self.adapter_list(ui, p);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(p.background)
                    .inner_margin(Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                let storage_error = self
                    .storage
                    .as_ref()
                    .and_then(|store| store.error.clone().map(|error| (store.path.clone(), error)));
                if let Some((path, error)) = storage_error {
                    ui.colored_label(
                        p.primary,
                        t(
                            self.thai,
                            "Storage error. Changes are not saved to disk.",
                            "มีปัญหาการบันทึก ข้อมูลที่แก้ยังไม่ถูกเก็บลงไฟล์",
                        ),
                    );
                    ui.label(format!("{}: {}", path.display(), error));
                    if ui
                        .button(t(self.thai, "Recover backup", "กู้จากสำเนาสำรอง"))
                        .clicked()
                        && let Some(store) = &mut self.storage
                    {
                        match store.recover_backup() {
                            Ok(()) => self.session.invalidate_preview(),
                            Err(error) => store.error = Some(error),
                        }
                    }
                    ui.separator();
                }
                self.adapter_selector(ui, p);
                egui::Panel::bottom("working_actions")
                    .resizable(false)
                    .frame(
                        Frame::new()
                            .fill(p.background)
                            .inner_margin(Margin::symmetric(0, 4)),
                    )
                    .show(ui, |ui| self.actions(ui, p));
                egui::ScrollArea::vertical()
                    .id_salt("working_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.settings_grid(ui, p);
                        self.feedback(ui, p);
                    });
            });
        self.help(&ctx);
        if self.last_persist.elapsed().as_secs_f32() > 1.0 {
            self.persist();
        }
        if self.storage.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        #[cfg(feature = "capture")]
        if let Some(path) = std::env::var_os("SIMPLE_IP_CAPTURE_TO") {
            // Wait for the adapter read so a live capture shows real data.
            if self.loading.is_none() {
                self.capture_frames += 1;
            }
            if self.capture_frames < 12 {
                ctx.request_repaint();
            } else if self.capture_frames == 12 {
                let close_ctx = ctx.clone();
                ctx.request_screenshot(move |pixels| {
                    let rgba: Vec<u8> = pixels.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(error) = image::save_buffer(
                        std::path::PathBuf::from(path),
                        &rgba,
                        pixels.width() as u32,
                        pixels.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Cannot save preview capture: {error}");
                    }
                    close_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                });
            }
        }
    }
}

impl eframe::App for SimpleIpApp {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.persist();
    }
    fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
        self.render(ui);
    }
}

const ROW_HEIGHT: f32 = 28.0;
const FIELD_FONT: f32 = 14.0;

fn field_differs(id: &str, current: &str, desired: &str, enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    if id == "subnet" {
        return match (
            simple_ip_core::parse_subnet(current),
            simple_ip_core::parse_subnet(desired),
        ) {
            (Ok(a), Ok(b)) => a != b,
            _ => current.trim() != desired.trim(),
        };
    }
    current.parse::<std::net::Ipv4Addr>().ok() != desired.trim().parse::<std::net::Ipv4Addr>().ok()
}

fn difference_mark(ui: &Ui, rect: egui::Rect, different: bool, p: Palette) {
    if different {
        let center = egui::pos2(rect.right() - 9.0, rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(0.0, -3.0),
                center + egui::vec2(3.0, 0.0),
                center + egui::vec2(0.0, 3.0),
                center + egui::vec2(-3.0, 0.0),
            ],
            p.primary,
            Stroke::NONE,
        ));
    }
}

fn focus_input(ui: &mut Ui, field: &str, response: &egui::Response) {
    let key = Id::unique("requested_issue_field");
    if ui.ctx().data_mut(|data| data.get_temp::<&'static str>(key)) == Some(field) {
        response.request_focus();
        response.scroll_to_me(Some(Align::Center));
        ui.ctx().data_mut(|data| data.remove::<&'static str>(key));
    }
}

fn grid_label(ui: &mut Ui, text: &str, width: f32, color: Color32) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(width, ROW_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(FontId::proportional(FIELD_FONT))
                        .color(color),
                )
                .truncate(),
            )
            .on_hover_text(text)
        },
    )
    .inner
}

#[allow(clippy::too_many_arguments)]
fn edit_row(
    ui: &mut Ui,
    label: &str,
    current: &str,
    hint: &str,
    id: &str,
    value: &mut String,
    enabled: bool,
    widths: [f32; 3],
    p: Palette,
    issues: &[simple_ip_core::ValidationIssue],
    thai: bool,
) -> [egui::Rect; 3] {
    let label = grid_label(ui, label, widths[0], p.ink);
    let current_text = current;
    let current = grid_label(ui, current_text, widths[1], p.muted);
    let current = current
        .interact(egui::Sense::CLICK)
        .on_hover_cursor(egui::CursorIcon::Copy)
        .on_hover_text("Click to copy / คลิกเพื่อคัดลอก");
    if current.clicked() {
        ui.ctx().copy_text(current_text.to_owned());
    }
    let different = field_differs(id, current_text, value, enabled);
    let editor = if enabled {
        crate::ipv4_input::show(ui, id, value, widths[2], ROW_HEIGHT)
            .labelled_by(label.id)
            .on_hover_text(hint)
    } else {
        // Empty disabled boxes read as missing data; say the value is assigned automatically.
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(widths[2], ROW_HEIGHT), egui::Sense::hover());
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            t(thai, "Automatic (DHCP)", "อัตโนมัติ (DHCP)"),
            FontId::proportional(FIELD_FONT),
            p.muted,
        );
        response
    };
    focus_input(ui, id, &editor);
    let field = match id {
        "address" => "IP address",
        "subnet" => "Subnet mask",
        "gateway" => "Gateway",
        "preferred_dns" => "Preferred DNS",
        _ => "Alternate DNS",
    };
    let errors: Vec<_> = issues
        .iter()
        .filter(|issue| issue.field == field || (issue.field == "DNS" && id == "preferred_dns"))
        .map(|issue| language::message(thai, issue.message))
        .collect();
    if !errors.is_empty() {
        ui.painter().rect_stroke(
            editor.rect,
            4.0,
            Stroke::new(1.5, p.primary),
            egui::StrokeKind::Inside,
        );
        paint_symbol(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(editor.rect.right() - 25.0, editor.rect.top()),
                Vec2::new(25.0, editor.rect.height()),
            ),
            Symbol::Warning,
            p.primary,
        );
        editor.clone().on_hover_text(errors.join("\n"));
    } else if different {
        let center = egui::pos2(editor.rect.right() - 9.0, editor.rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(0.0, -3.0),
                center + egui::vec2(3.0, 0.0),
                center + egui::vec2(0.0, 3.0),
                center + egui::vec2(-3.0, 0.0),
            ],
            p.primary,
            Stroke::NONE,
        ));
        editor
            .clone()
            .on_hover_text("Different from current / ต่างจากค่าปัจจุบัน");
    }
    ui.end_row();
    [label.rect, current.rect, editor.rect]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_current_value_emits_clipboard_text_without_mutating_draft() {
        let ctx = egui::Context::default();
        let mut app = SimpleIpApp::with_context(&ctx, true, 1.0);
        app.session.load_lab_sample();
        let size = Vec2::new(820.0, 620.0);
        for _ in 0..5 {
            frame(&ctx, &mut app, size, 1.0, vec![]);
        }
        let original = app.session.draft().clone();
        let position = app.aligned_rows[0][1].center();
        let mut commands = vec![];
        for pressed in [true, false] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events: vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.render(ui));
            output.textures_delta.clear();
            commands.extend(output.platform_output.commands);
        }
        assert!(commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == "192.0.2.24")
        ));
        assert_eq!(app.session.draft(), &original);
    }

    #[test]
    fn semantic_prefix_comparison_and_issue_focus() {
        assert!(!field_differs("subnet", "255.255.255.0", "/24", true));
        assert!(field_differs("subnet", "255.255.255.0", "/25", true));
        assert!(!field_differs("address", "192.0.2.1", "bad", false));
        let ctx = egui::Context::default();
        let mut app = SimpleIpApp::with_context(&ctx, true, 1.0);
        app.session.load_lab_sample();
        app.session.draft_mut().subnet = "255.0.255.0".into();
        app.preview();
        let size = Vec2::new(820.0, 620.0);
        for _ in 0..5 {
            frame(&ctx, &mut app, size, 1.0, vec![]);
        }
        app.focus_field = Some("subnet");
        for _ in 0..3 {
            frame(&ctx, &mut app, size, 1.0, vec![]);
        }
        assert_eq!(ctx.memory(|m| m.focused()), Some(Id::unique("subnet")));
        assert!(
            app.painted_text
                .iter()
                .any(|(text, _)| text.contains("บิตเครือข่ายต่อเนื่อง"))
        );
        frame(
            &ctx,
            &mut app,
            size,
            1.0,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(Id::unique(("ipv4_segment", "subnet", 1)))
        );
        for _ in 0..4 {
            frame(
                &ctx,
                &mut app,
                size,
                1.0,
                vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
            );
        }
        assert_eq!(ctx.memory(|m| m.focused()), Some(Id::unique("gateway")));
    }

    fn frame(
        ctx: &egui::Context,
        app: &mut SimpleIpApp,
        size: Vec2,
        ppp: f32,
        events: Vec<egui::Event>,
    ) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        // The headless harness inspects layout and input, not GPU textures.
        output.textures_delta.clear();
        assert!(!output.shapes.is_empty());
        fn collect(shape: &egui::epaint::Shape, text: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::epaint::Shape::Text(shape) => {
                    text.push((shape.galley.job.text.clone(), shape.pos))
                }
                egui::epaint::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, text);
                    }
                }
                _ => {}
            }
        }
        app.painted_text.clear();
        for shape in &output.shapes {
            collect(&shape.shape, &mut app.painted_text);
        }
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn primary_action_stays_inside_compact_view_at_all_density_factors() {
        for thai in [false, true] {
            for density in [1.0, 1.25, 1.5, 2.0] {
                let ctx = egui::Context::default();
                let mut app = SimpleIpApp::with_context(&ctx, thai, 1.0);
                let size = egui::vec2(900.0, 650.0);
                for _ in 0..4 {
                    frame(&ctx, &mut app, size, density, vec![]);
                }
                let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                assert!(
                    viewport.contains_rect(app.preview_button_rect),
                    "Primary action was clipped: Thai={thai}, density={density}, rect={:?}",
                    app.preview_button_rect
                );
                assert!(ctx.fonts_mut(|fonts| {
                    fonts.has_glyphs(&FontId::proportional(15.0), "ภาษาไทย น้ำ เก้าอี้ กิ๊บ")
                }));
            }
        }
    }

    #[test]
    fn compact_matrix_aligns_value_columns_above_the_action() {
        for thai in [false, true] {
            for density in [1.0, 1.25, 1.5, 2.0] {
                let ctx = egui::Context::default();
                let mut app = SimpleIpApp::with_context(&ctx, thai, 1.0);
                app.session.load_lab_sample();
                let size = egui::vec2(820.0, 620.0);
                for _ in 0..5 {
                    frame(&ctx, &mut app, size, density, vec![]);
                }
                assert_eq!(app.aligned_rows.len(), 5);
                assert!(
                    app.aligned_rows[4][2].bottom() < app.preview_button_rect.top(),
                    "Last row is clipped below the pinned action"
                );
                let first = app.aligned_rows[0];
                for row in &app.aligned_rows {
                    for column in 0..3 {
                        assert!(
                            (row[column].left() - first[column].left()).abs() < 1.0,
                            "Column shifted: {row:?}"
                        );
                        assert!(
                            (row[column].center().y - row[0].center().y).abs() < 1.0,
                            "Row does not share a text plane: {row:?}"
                        );
                    }
                    assert!(row[2].right() <= size.x - 8.0);
                }
                let text_pos = |text: &str| {
                    app.painted_text
                        .iter()
                        .find(|(value, _)| value == text)
                        .unwrap_or_else(|| panic!("Expected rendered text {text}"))
                        .1
                };
                let label_x = text_pos("IP address").x;
                for label in [
                    "Gateway",
                    "Subnet / prefix",
                    t(thai, "Preferred DNS", "DNS หลัก"),
                    t(thai, "Alternate DNS", "DNS สำรอง"),
                ] {
                    assert!(
                        (text_pos(label).x - label_x).abs() < 1.0,
                        "Painted label is not left aligned: {label}"
                    );
                }
                let current = text_pos("192.0.2.24");
                let desired = text_pos("198");
                assert!(
                    (current.y - desired.y).abs() < 1.0,
                    "Actual text origins are on different planes: {current:?}, {desired:?}"
                );
                assert!(
                    app.painted_text
                        .iter()
                        .filter(|(text, _)| text == "198")
                        .all(|(_, pos)| (pos.x - desired.x).abs() < 1.0)
                );
            }
        }
    }

    #[test]
    fn enter_does_not_preview_but_explicit_shortcut_does_without_mutation() {
        let ctx = egui::Context::default();
        let mut app = SimpleIpApp::with_context(&ctx, false, 1.0);
        app.session.load_lab_sample();
        let adapter = app.session.adapters[0].config.clone();
        let size = egui::vec2(1040.0, 800.0);
        frame(&ctx, &mut app, size, 1.0, vec![]);
        frame(
            &ctx,
            &mut app,
            size,
            1.0,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(app.session.preview.is_none());
        frame(
            &ctx,
            &mut app,
            size,
            1.0,
            vec![key(egui::Key::Enter, egui::Modifiers::CTRL)],
        );
        assert!(app.session.preview.is_some());
        assert_eq!(app.session.adapters[0].config, adapter);
    }
}
