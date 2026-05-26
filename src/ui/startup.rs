use std::time::Instant;

use crate::monitor::startup::{self, StartupEntry, StartupSource};
use crate::theme;

pub struct State {
    pub entries: Vec<StartupEntry>,
    pub last_loaded: Instant,
    pub filter: String,
    pub last_error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            entries: startup::collect(),
            last_loaded: Instant::now(),
            filter: String::new(),
            last_error: None,
        }
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.heading("Startup apps");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Reload").clicked() {
                state.entries = startup::collect();
                state.last_loaded = Instant::now();
                state.last_error = None;
            }
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text("Filter by name or time (e.g. >0.5)")
                    .desired_width(220.0),
            );
        });
    });
    ui.label(
        egui::RichText::new(
            "Sorted by boot time (descending). Units managed by systemd dependencies are protected and cannot be disabled.",
        )
        .color(theme::TEXT_DIM),
    );
    ui.add_space(10.0);

    if let Some(err) = &state.last_error {
        ui.colored_label(theme::ERR, err);
        ui.add_space(8.0);
    }

    let raw_filter = state.filter.trim();
    let time_filter = parse_time_filter(raw_filter);
    let text_filter = if time_filter.is_some() {
        String::new()
    } else {
        raw_filter.to_lowercase()
    };
    let mut to_toggle: Vec<(usize, bool)> = Vec::new();

    // Partition: critical (locked) first, then the rest.
    let mut critical_idx: Vec<usize> = Vec::new();
    let mut normal_idx: Vec<usize> = Vec::new();
    for (idx, e) in state.entries.iter().enumerate() {
        if let Some((op, secs)) = time_filter {
            let ms = e.boot_time_ms.unwrap_or(0) as f64 / 1_000.0;
            if !op.matches(ms, secs) {
                continue;
            }
        } else if !text_filter.is_empty()
            && !e.name.to_lowercase().contains(&text_filter)
            && !e.exec.to_lowercase().contains(&text_filter)
        {
            continue;
        }
        if e.critical {
            critical_idx.push(idx);
        } else {
            normal_idx.push(idx);
        }
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        if !normal_idx.is_empty() {
            ui.label(
                egui::RichText::new("Startup apps & services")
                    .color(theme::TEXT)
                    .strong(),
            );
            ui.add_space(6.0);
            for idx in &normal_idx {
                render_row(ui, &state.entries[*idx], *idx, &mut to_toggle, false);
            }
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
        }

        if !critical_idx.is_empty() {
            ui.label(
                egui::RichText::new("Protected by systemd")
                    .color(theme::WARN)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "These services are pulled in by dependency (state: static/generated/alias). \
                     `systemctl disable` has no effect on them — they are managed by other units.",
                )
                .color(theme::TEXT_DIM)
                .small(),
            );
            ui.add_space(6.0);
            for idx in &critical_idx {
                render_row(ui, &state.entries[*idx], *idx, &mut to_toggle, true);
            }
        }
    });

    for (idx, enabled) in to_toggle {
        let entry = state.entries[idx].clone();
        match startup::set_enabled(&entry, enabled) {
            Ok(()) => {
                state.entries[idx].enabled = enabled;
                state.last_error = None;
            }
            Err(e) => {
                state.last_error = Some(format!("Failed to toggle {}: {e}", entry.name));
            }
        }
    }
}

fn render_row(
    ui: &mut egui::Ui,
    e: &StartupEntry,
    idx: usize,
    to_toggle: &mut Vec<(usize, bool)>,
    locked: bool,
) {
    egui::Frame::new()
        .fill(theme::CARD_BG)
        .inner_margin(egui::Margin::same(10))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(if e.name.is_empty() {
                                e.path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            } else {
                                e.name.clone()
                            })
                            .strong()
                            .size(14.0),
                        );
                        ui.label(
                            egui::RichText::new(scope_badge(&e.source))
                                .color(theme::TEXT_DIM)
                                .small(),
                        );
                        if locked {
                            ui.label(
                                egui::RichText::new("protected")
                                    .color(theme::WARN)
                                    .small()
                                    .strong(),
                            );
                        }
                    });
                    if !e.comment.is_empty() {
                        ui.label(
                            egui::RichText::new(&e.comment)
                                .color(theme::TEXT_DIM)
                                .small(),
                        );
                    }
                    ui.label(
                        egui::RichText::new(&e.exec)
                            .color(theme::TEXT_DIM)
                            .monospace()
                            .small(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut enabled = e.enabled;
                    let label = if locked {
                        "Protected"
                    } else if enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    };
                    ui.add_enabled_ui(!locked, |ui| {
                        if ui.toggle_value(&mut enabled, label).changed() {
                            to_toggle.push((idx, enabled));
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format_boot_time(e.boot_time_ms))
                            .monospace()
                            .color(if e.boot_time_ms.is_some() {
                                theme::TEXT
                            } else {
                                theme::TEXT_DIM
                            }),
                    );
                });
            });
        });
    ui.add_space(6.0);
}

fn scope_badge(s: &StartupSource) -> &'static str {
    match s {
        StartupSource::UserAutostart => "user · autostart",
        StartupSource::SystemAutostart => "system · autostart",
        StartupSource::SystemdSystem => "system · systemd",
        StartupSource::SystemdUser => "user · systemd",
    }
}

#[derive(Copy, Clone)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
}

impl Op {
    fn matches(self, value: f64, target: f64) -> bool {
        match self {
            Op::Ge => value >= target,
            Op::Gt => value > target,
            Op::Le => value <= target,
            Op::Lt => value < target,
            Op::Eq => (value - target).abs() < 0.05,
        }
    }
}

fn parse_time_filter(s: &str) -> Option<(Op, f64)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (op, rest) = if let Some(r) = s.strip_prefix(">=") {
        (Op::Ge, r)
    } else if let Some(r) = s.strip_prefix("<=") {
        (Op::Le, r)
    } else if let Some(r) = s.strip_prefix('>') {
        (Op::Gt, r)
    } else if let Some(r) = s.strip_prefix('<') {
        (Op::Lt, r)
    } else if let Some(r) = s.strip_prefix('=') {
        (Op::Eq, r)
    } else {
        (Op::Ge, s)
    };
    let rest = rest.trim().trim_end_matches('s').trim();
    rest.parse::<f64>().ok().map(|v| (op, v))
}

fn format_boot_time(ms: Option<u64>) -> String {
    match ms {
        None => "—".into(),
        Some(0) => "<1 ms".into(),
        Some(ms) if ms < 1_000 => format!("{ms} ms"),
        Some(ms) if ms < 60_000 => format!("{:.2} s", ms as f64 / 1_000.0),
        Some(ms) => {
            let total_s = ms / 1_000;
            let min = total_s / 60;
            let s = total_s % 60;
            format!("{min}m {s}s")
        }
    }
}
