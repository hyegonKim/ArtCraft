//! Multi-row New Tracks dialog (`Track › New…`) with Pro Tools-compatible keyboard chords.

use egui::{Key, Modifiers, os::OperatingSystem};
use serde_json::{Value, json};
use soundcraft_model::{ChannelFormat, TrackKind};

/// Updates `kind` to `next` and replaces `name` when it was empty or still matched the previous
/// track kind's default name, preserving any custom name typed by the user.
pub fn set_new_track_kind(kind: &mut String, name: &mut String, next: TrackKind) {
    let previous_default = TrackKind::from_id(kind).map(TrackKind::default_name);
    if name.is_empty() || previous_default == Some(name.as_str()) {
        *name = next.default_name().to_string();
    }
    *kind = next.id().to_string();
}

/// One row in the New Tracks dialog.
#[derive(Debug, Clone)]
pub struct NewTrackRow {
    pub count: u32,
    pub format: String,
    pub kind: String,
    pub timebase: String,
    pub name: String,
    count_text: String,
    synced_count: u32,
}

impl Default for NewTrackRow {
    fn default() -> Self {
        Self {
            count: 1,
            format: ChannelFormat::Mono.label().to_string(),
            kind: TrackKind::Audio.id().to_string(),
            timebase: "samples".to_string(),
            name: TrackKind::Audio.default_name().to_string(),
            count_text: "1".to_string(),
            synced_count: 1,
        }
    }
}

impl NewTrackRow {
    fn sync_from_count(&mut self) {
        let clamped = self.count.clamp(1, 128);
        if clamped != self.count || self.count != self.synced_count {
            self.count = clamped;
            self.synced_count = clamped;
            self.count_text = clamped.to_string();
        }
    }

    fn set_count(&mut self, next: u32) {
        let clamped = next.clamp(1, 128);
        self.count = clamped;
        self.synced_count = clamped;
        self.count_text = clamped.to_string();
    }

    fn has_format(&self) -> bool {
        self.kind != TrackKind::Video.id()
    }

    fn has_timebase(&self) -> bool {
        self.kind != TrackKind::Video.id()
    }
}

/// State of the multi-row New Tracks modal dialog.
#[derive(Debug, Clone)]
pub struct NewTracks {
    pub rows: Vec<NewTrackRow>,
    active: usize,
    focused_initial: bool,
}

impl Default for NewTracks {
    fn default() -> Self {
        Self { rows: vec![NewTrackRow::default()], active: 0, focused_initial: false }
    }
}

impl NewTracks {
    /// Returns the currently active row in the dialog.
    pub fn active_row(&self) -> Option<&NewTrackRow> {
        self.rows.get(self.active).or_else(|| self.rows.first())
    }

    fn clamp_active(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(NewTrackRow::default());
        }
        if self.active >= self.rows.len() {
            self.active = self.rows.len().saturating_sub(1);
        }
    }

    /// Renders the New Tracks form and returns `track.new` command parameters when confirmed.
    pub fn show(&mut self, ui: &mut egui::Ui, enter: bool) -> Option<Value> {
        self.clamp_active();
        for row in &mut self.rows {
            row.sync_from_count();
        }

        let mac = ui.ctx().os() == OperatingSystem::Mac;
        let row_shortcut = self.handle_keys(ui, mac);

        let mut add_after = None;
        let mut remove_idx = None;
        let row_count = self.rows.len();

        for idx in 0..row_count {
            let Some(row) = self.rows.get_mut(idx) else { continue };
            let is_active = idx == self.active;
            ui.push_id(idx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Create");
                    let mut count_out = egui::TextEdit::singleline(&mut row.count_text).desired_width(40.0).show(ui);
                    let count_resp = crate::widgets::named_text_field(count_out.response.response.clone(), "Number of tracks");
                    if idx == 0 && !self.focused_initial {
                        count_out.response.request_focus();
                        let all =
                            egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(row.count_text.chars().count()));
                        count_out.state.cursor.set_char_range(Some(all));
                        count_out.state.store(ui.ctx(), count_out.response.id);
                        self.focused_initial = true;
                    }
                    if count_resp.gained_focus() || count_resp.clicked() {
                        self.active = idx;
                    }
                    if count_out.response.changed() {
                        let digits: String = row.count_text.chars().filter(|c| c.is_ascii_digit()).collect();
                        if let Ok(parsed) = digits.parse::<u32>() {
                            let clamped = parsed.clamp(1, 128);
                            row.count = clamped;
                            row.synced_count = clamped;
                            if parsed != clamped {
                                row.count_text = clamped.to_string();
                            } else {
                                row.count_text = digits;
                            }
                        } else if digits.is_empty() {
                            row.count_text.clear();
                        }
                    } else if !count_out.response.has_focus() && row.count_text.is_empty() {
                        row.count_text = row.count.to_string();
                    }

                    if row.has_format() {
                        let fmt = craft_ui::ComboBox::from_id_salt(("nt_fmt", idx)).selected_text(row.format.as_str()).show_ui(ui, |ui| {
                            for f in ChannelFormat::ALL {
                                if ui.selectable_label(row.format == f.label(), f.label()).clicked() {
                                    row.format = f.label().to_string();
                                    self.active = idx;
                                }
                            }
                        });
                        fmt.response.accessible_name("Channel format");
                    }

                    let kind_label = TrackKind::from_id(&row.kind).map_or("Audio Track", TrackKind::label);
                    let kind_combo = craft_ui::ComboBox::from_id_salt(("nt_kind", idx)).selected_text(kind_label).show_ui(ui, |ui| {
                        for k in TrackKind::ALL {
                            if ui.selectable_label(row.kind == k.id(), k.label()).clicked() {
                                set_new_track_kind(&mut row.kind, &mut row.name, k);
                                self.active = idx;
                            }
                        }
                    });
                    kind_combo.response.accessible_name("Track kind");

                    if row.has_timebase() {
                        ui.label("in");
                        let tb_text = if row.timebase == "ticks" { "Ticks" } else { "Samples" };
                        let tb = craft_ui::ComboBox::from_id_salt(("nt_tb", idx)).selected_text(tb_text).show_ui(ui, |ui| {
                            if ui.selectable_label(row.timebase == "samples", "Samples").clicked() {
                                row.timebase = "samples".to_string();
                                self.active = idx;
                            }
                            if ui.selectable_label(row.timebase == "ticks", "Ticks").clicked() {
                                row.timebase = "ticks".to_string();
                                self.active = idx;
                            }
                        });
                        tb.response.accessible_name("Timebase");
                    }

                    ui.label("Name");
                    let name_resp = crate::widgets::named_text_field(
                        ui.add(egui::TextEdit::singleline(&mut row.name).desired_width(240.0).min_size(egui::vec2(220.0, 0.0))),
                        "Name",
                    );
                    if name_resp.gained_focus() || name_resp.clicked() {
                        self.active = idx;
                    }

                    if ui.add_enabled(row_count < 128, egui::Button::new("+")).clicked() {
                        add_after = Some(idx);
                    }
                    if ui.add_enabled(row_count > 1, egui::Button::new("−")).clicked() {
                        remove_idx = Some(idx);
                    }
                    let _ = is_active;
                });
            });
        }

        if let Some(idx) = add_after
            && self.rows.len() < 128
        {
            let insert_at = (idx + 1).min(self.rows.len());
            self.rows.insert(insert_at, NewTrackRow::default());
            self.active = insert_at;
        }
        if let Some(idx) = remove_idx
            && self.rows.len() > 1
            && idx < self.rows.len()
        {
            self.rows.remove(idx);
            self.clamp_active();
        }

        let total: u32 = self.rows.iter().map(|r| r.count).sum();
        let ready = (1..=128).contains(&total);
        if super::buttons_enabled(ui, "Create", enter && !row_shortcut, ready) {
            let rows: Vec<Value> = self
                .rows
                .iter()
                .map(|r| {
                    let mut obj = json!({
                        "count": r.count,
                        "kind": r.kind,
                        "name": r.name,
                    });
                    if r.has_format() {
                        obj["format"] = json!(r.format);
                    }
                    if r.has_timebase() {
                        obj["timebase"] = json!(r.timebase);
                    }
                    obj
                })
                .collect();
            if rows.len() == 1 {
                return rows.into_iter().next();
            }
            return Some(json!({ "rows": rows }));
        }
        None
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui, mac: bool) -> bool {
        let mut row_shortcut = false;
        let arrows: Vec<(Key, Modifiers)> = ui.input_mut(|i| {
            let mut matched = Vec::new();
            i.events.retain(|e| {
                let egui::Event::Key { key, pressed: true, modifiers, .. } = e else {
                    return true;
                };
                if !matches!(key, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight) {
                    return true;
                }
                let primary = if mac { modifiers.mac_cmd || modifiers.command } else { modifiers.ctrl || modifiers.command };
                let bare_up_down = matches!(key, Key::ArrowUp | Key::ArrowDown)
                    && !modifiers.command
                    && !modifiers.mac_cmd
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && !modifiers.shift;
                if primary || bare_up_down {
                    matched.push((*key, *modifiers));
                    false
                } else {
                    true
                }
            });
            matched
        });

        for (key, mods) in arrows {
            let primary = if mac { mods.mac_cmd || mods.command } else { mods.ctrl || mods.command };
            if !primary {
                if let Some(row) = self.rows.get_mut(self.active) {
                    match key {
                        Key::ArrowUp => row.set_count(row.count.saturating_add(1)),
                        Key::ArrowDown => row.set_count(row.count.saturating_sub(1)),
                        _ => {}
                    }
                }
                continue;
            }

            if mods.shift {
                row_shortcut = true;
                match key {
                    Key::ArrowDown if self.rows.len() < 128 => {
                        self.rows.push(NewTrackRow::default());
                        self.active = self.rows.len().saturating_sub(1);
                    }
                    Key::ArrowUp if self.rows.len() > 1 => {
                        self.rows.pop();
                        self.clamp_active();
                    }
                    _ => {}
                }
                continue;
            }

            let Some(row) = self.rows.get_mut(self.active) else { continue };
            if mods.alt {
                if row.has_timebase() {
                    match key {
                        Key::ArrowDown | Key::ArrowUp => {
                            row.timebase = if row.timebase == "ticks" { "samples".to_string() } else { "ticks".to_string() };
                        }
                        _ => {}
                    }
                }
                continue;
            }

            match key {
                Key::ArrowLeft | Key::ArrowRight if row.has_format() => {
                    let all = ChannelFormat::ALL;
                    if !all.is_empty() {
                        let cur = all.iter().position(|f| f.label() == row.format).unwrap_or(0);
                        let next = if key == Key::ArrowRight { (cur + 1) % all.len() } else { (cur + all.len() - 1) % all.len() };
                        if let Some(fmt) = all.get(next) {
                            row.format = fmt.label().to_string();
                        }
                    }
                }
                Key::ArrowUp | Key::ArrowDown => {
                    let all = TrackKind::ALL;
                    if !all.is_empty() {
                        let cur = all.iter().position(|k| k.id() == row.kind).unwrap_or(0);
                        let next = if key == Key::ArrowDown { (cur + 1) % all.len() } else { (cur + all.len() - 1) % all.len() };
                        if let Some(k) = all.get(next) {
                            set_new_track_kind(&mut row.kind, &mut row.name, *k);
                        }
                    }
                }
                _ => {}
            }
        }
        row_shortcut
    }
}
