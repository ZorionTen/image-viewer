use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureOptions};
use eframe::epaint::TextureHandle;

use crate::loader::{Job, Loader, Msg};
use crate::meta::{self, Meta};

const EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "ico", "pnm",
];

const PANEL_W: f32 = 276.0;
const PANEL_FADE: f32 = 44.0;
const ITEM_W: f32 = 120.0;
const ITEM_H: f32 = 96.0;
const THUMB_H: f32 = 62.0;

const ARROW: f32 = 52.0;
const THUMB_CACHE_MAX: usize = 400;
const PANEL_IDLE_SECS: f64 = 2.0;

pub struct Viewer {
    dir: PathBuf,
    files: Vec<PathBuf>,
    idx: usize,
    generation: u64,
    loading: bool,
    image: Option<(TextureHandle, Meta)>,
    bg: Option<TextureHandle>,
    error: Option<String>,
    loader: Loader,
    thumbs: HashMap<PathBuf, TextureHandle>,
    pending: HashSet<PathBuf>,
    dead: HashSet<PathBuf>,
    lru: VecDeque<PathBuf>,
    carousel: bool,
    arrow: f32,
    panel: f32,
    last_move: f64,
    last_pos: Option<Pos2>,
    scroll_to: bool,
}

impl Viewer {
    pub fn new(_cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut v = Self {
            dir: PathBuf::new(),
            files: Vec::new(),
            idx: 0,
            generation: 0,
            loading: false,
            image: None,
            bg: None,
            error: None,
            loader: Loader::new(_cc.egui_ctx.clone()),
            thumbs: HashMap::new(),
            pending: HashSet::new(),
            dead: HashSet::new(),
            lru: VecDeque::new(),
            carousel: true,
            arrow: 0.0,
            panel: 0.0,
            last_move: -1e9,
            last_pos: None,
            scroll_to: false,
        };
        if let Some(p) = path {
            v.open_path(&p);
        }
        v
    }

    fn refresh(&mut self, dir: &Path) {
        self.dir = dir.to_path_buf();
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
                        .unwrap_or(false)
            })
            .collect();
        files.sort_by_key(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        });
        self.files = files;
    }

    pub fn open_path(&mut self, path: &Path) {
        if path.is_dir() {
            self.refresh(path);
            if !self.files.is_empty() {
                self.set_index(0);
            }
        } else {
            let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            self.refresh(&dir);
            let pos = self
                .files
                .iter()
                .position(|p| p.as_path() == path)
                .unwrap_or(0);
            self.image = None;
            if !self.files.is_empty() {
                self.set_index(pos);
            }
        }
    }

    fn set_index(&mut self, i: usize) {
        if self.files.is_empty() {
            return;
        }
        let n = self.files.len() as i64;
        let i = (((i as i64) % n + n) % n) as usize;
        self.idx = i;
        self.generation += 1;
        self.loading = true;
        self.error = None;
        self.scroll_to = true;
        let path = self.files[self.idx].clone();
        let _ = self.loader.tx.send(Job::Main { generation: self.generation, path });
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let mut woke = false;
        while let Ok(msg) = self.loader.rx.try_recv() {
            woke = true;
            match msg {
                Msg::Main { generation, result } if generation == self.generation => {
                    self.loading = false;
                    match result {
                        Ok((img, bg_img, meta)) => {
                            let tex = ctx.load_texture(
                                format!("main-{generation}"),
                                img,
                                TextureOptions::LINEAR,
                            );
                            self.bg = Some(ctx.load_texture(
                                format!("bg-{generation}"),
                                bg_img,
                                TextureOptions::LINEAR,
                            ));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                                "{} — Image Viewer",
                                meta.name
                            )));
                            self.image = Some((tex, meta));
                            self.error = None;
                        }
                        Err(e) => {
                            self.image = None;
                            self.bg = None;
                            self.error = Some(e);
                        }
                    }
                }
                Msg::Main { .. } => {}
                Msg::Thumb { path, result } => {
                    self.pending.remove(&path);
                    match result {
                        Ok(img) => {
                            let tex =
                                ctx.load_texture(format!("t-{}", path.display()), img, TextureOptions::LINEAR);
                            if self.thumbs.insert(path.clone(), tex).is_none() {
                                self.lru.push_back(path.clone());
                            }
                            while self.thumbs.len() > THUMB_CACHE_MAX {
                                if let Some(old) = self.lru.pop_front() {
                                    self.thumbs.remove(&old);
                                } else {
                                    break;
                                }
                            }
                        }
                        Err(_) => {
                            self.dead.insert(path);
                        }
                    }
                }
            }
        }
        // Reactive repaint mode: a decoded image/thumbnail arriving on the
        // channel must wake the UI or the new frame is never shown.
        if woke {
            ctx.request_repaint();
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (left, right, toggle) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::C),
            )
        });
        if left && !self.files.is_empty() {
            self.set_index(self.idx.wrapping_sub(1));
        }
        if right && !self.files.is_empty() {
            self.set_index(self.idx + 1);
        }
        if toggle {
            self.carousel = !self.carousel;
        }
    }

    /// Blurred, darkened copy of the current image covering the whole window.
    fn paint_background(&self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let painter = ui.painter().with_clip_rect(full);
        painter.rect_filled(full, 0.0, Color32::from_rgb(22, 22, 26));
        if let Some(bg) = &self.bg {
            let (w, h) = {
                let s = bg.size();
                (s[0] as f32, s[1] as f32)
            };
            let scale = (full.width() / w).max(full.height() / h);
            let size = egui::vec2(w * scale, h * scale);
            let rect = Rect::from_center_size(full.center(), size);
            painter.image(
                bg.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::from_rgb(94, 94, 98), // darkening tint
            );
            painter.rect_filled(full, 0.0, Color32::from_black_alpha(48));
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let full = ui.max_rect();
        let now = ctx.input(|i| i.time);
        let dt = ctx.input(|i| i.stable_dt) as f64;
        #[cfg_attr(not(debug_assertions), allow(unused_mut))]
        let mut pointer = ctx.input(|i| i.pointer.latest_pos());
        // Info panel trigger: rightmost 1/8th of the window.
        let zone_x = full.right() - full.width() / 8.0;
        #[cfg(debug_assertions)]
        if let Ok(force) = std::env::var("IV_FORCE") {
            // Debug-only pointer simulation for automated screenshot tests.
            ctx.request_repaint(); // keep frames coming; egui would otherwise idle
            let jitter = ((now * 6.0).sin() * 6.0) as f32;
            let zone_c = zone_x + full.width() / 16.0;
            pointer = match force.as_str() {
                "zone" => Some(Pos2::new(zone_c + jitter, full.center().y)),
                "zone_once" => {
                    let j = if now < 1.0 { jitter } else { 0.0 };
                    Some(Pos2::new(zone_c + j, full.center().y))
                }
                "center" => Some(Pos2::new(full.center().x + jitter, full.center().y)),
                _ => pointer,
            };
        }
        let inside = pointer.is_some_and(|p| full.contains(p));
        let in_zone = pointer.is_some_and(|p| p.x >= zone_x && full.contains(p));
        if let (Some(p), Some(lp)) = (pointer, self.last_pos) {
            if p != lp && in_zone {
                self.last_move = now;
            }
        }
        self.last_pos = pointer;

        let k = (1.0 - (-dt * 10.0).exp()) as f32;
        let panel_t = if in_zone && now - self.last_move < PANEL_IDLE_SECS {
            1.0
        } else {
            0.0
        };
        let arrow_t = if inside { 1.0 } else { 0.0 };
        self.panel += (panel_t - self.panel) * k;
        self.arrow += (arrow_t - self.arrow) * k;
        // egui repaints reactively: without an explicit wake-up the fade would
        // freeze mid-animation and the idle auto-hide deadline would never fire.
        if (self.panel - panel_t).abs() > 0.005 || (self.arrow - arrow_t).abs() > 0.005 {
            ctx.request_repaint();
        }
        if self.panel > 0.005 {
            let remaining = (PANEL_IDLE_SECS - (now - self.last_move)).max(0.0);
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(remaining + 0.05));
        }

        let image = self.image.take();
        if let Some((tex, meta)) = &image {
            let rect = fit_rect(meta.w, meta.h, full.shrink(18.0));
            ui.painter().image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            self.paint_meta(ui, full, meta);
            if self.loading {
                ui.painter().text(
                    Pos2::new(full.right() - 30.0, full.top() + 14.0),
                    egui::Align2::RIGHT_TOP,
                    "…",
                    FontId::proportional(18.0),
                    Color32::from_white_alpha(140),
                );
            }
            self.paint_arrows(ui, full);
        } else {
            let (title, body) = if let Some(e) = &self.error {
                ("Failed to open", e.as_str())
            } else if self.loading {
                ("Loading…", "")
            } else {
                (
                    "No image",
                    "Pass an image file as an argument, drag & drop an image, or open one from the file manager.",
                )
            };
            ui.painter().text(
                Pos2::new(full.center().x, full.center().y - 16.0),
                egui::Align2::CENTER_CENTER,
                title,
                FontId::proportional(15.0),
                Color32::from_gray(200),
            );
            if !body.is_empty() {
                ui.painter().text(
                    Pos2::new(full.center().x, full.center().y + 14.0),
                    egui::Align2::CENTER_CENTER,
                    body,
                    FontId::proportional(13.0),
                    Color32::from_gray(150),
                );
            }
        }
        if let Some(img) = image {
            self.image = Some(img);
        }
        self.paint_toggle(ui, full);
    }

    fn paint_meta(&self, ui: &egui::Ui, full: Rect, meta: &Meta) {
        if self.panel < 0.02 {
            return;
        }
        let p = self.panel;
        let a = (255.0 * p) as u8;
        let x = full.right() - PANEL_W * p;
        let rect = Rect::from_min_size(Pos2::new(x, full.top()), egui::vec2(PANEL_W, full.height()));
        let painter = ui.painter().with_clip_rect(full);

        let base_alpha = (175.0 * p) as u8;
        // Solid part, then a smooth alpha ramp toward the left edge (no hard border).
        let solid_x = rect.min.x + PANEL_FADE;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(solid_x, rect.min.y), rect.max),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, base_alpha),
        );
        const STRIPS: usize = 24;
        for i in 0..STRIPS {
            let t = (i as f32 + 0.5) / STRIPS as f32;
            let s = t * t * (3.0 - 2.0 * t); // smoothstep
            let sx = rect.min.x + PANEL_FADE * i as f32 / STRIPS as f32;
            let sw = PANEL_FADE / STRIPS as f32 + 1.0;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(sx, rect.min.y), egui::vec2(sw, rect.height())),
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, (base_alpha as f32 * s) as u8),
            );
        }

        let lx = rect.min.x + PANEL_FADE + 8.0;
        let mut y = rect.min.y + 14.0;
        let gray = Color32::from_rgba_unmultiplied(165, 165, 165, a);
        let white = Color32::from_white_alpha(a);
        let dir_gray = Color32::from_rgba_unmultiplied(140, 140, 140, a);

        put(&painter, lx, y, &ellipsize(&meta.name, 28), FontId::proportional(14.5), white);
        y += 22.0;
        put(&painter, lx, y, &ellipsize(&meta.dir, 36), FontId::proportional(11.0), dir_gray);
        y += 24.0;

        for (k, v) in [
            ("Format", meta.ext.as_str()),
            ("Dimensions", &format!("{} × {}", meta.w, meta.h)),
            ("Size", &meta::human_size(meta.size)),
            ("Modified", meta.modified.as_deref().unwrap_or("—")),
        ] {
            if y + 34.0 > rect.max.y {
                return;
            }
            y = row(&painter, lx, y, k, v, gray, white);
        }

        if !meta.exif.is_empty() {
            if y + 24.0 > rect.max.y {
                return;
            }
            y += 6.0;
            painter.line_segment(
                [Pos2::new(lx, y), Pos2::new(rect.max.x - 16.0, y)],
                Stroke::new(1.0, Color32::from_white_alpha((36.0 * p) as u8)),
            );
            y += 14.0;
            for (k, v) in &meta.exif {
                if y + 34.0 > rect.max.y {
                    return;
                }
                y = row(&painter, lx, y, k, &ellipsize(v, 30), gray, white);
            }
        }
    }

    fn paint_arrows(&mut self, ui: &mut egui::Ui, full: Rect) {
        if self.arrow < 0.03 || self.files.is_empty() {
            return;
        }
        let a = self.arrow;
        let cy = full.center().y;
        let lr = Rect::from_center_size(
            Pos2::new(full.left() + 16.0 + ARROW / 2.0, cy),
            egui::vec2(ARROW, ARROW),
        );
        let rr = Rect::from_center_size(
            Pos2::new(full.right() - 16.0 - ARROW / 2.0, cy),
            egui::vec2(ARROW, ARROW),
        );
        let left = ui.interact(lr, egui::Id::unique("iv.nav_left"), Sense::click());
        let right = ui.interact(rr, egui::Id::unique("iv.nav_right"), Sense::click());
        if left.clicked() {
            self.set_index(self.idx.wrapping_sub(1));
        }
        if right.clicked() {
            self.set_index(self.idx + 1);
        }

        let painter = ui.painter();
        for (rect, resp, points) in [
            (lr, &left, chevron(-1.0)),
            (rr, &right, chevron(1.0)),
        ] {
            let hot = resp.hovered() || resp.has_focus();
            let bg = if hot { 58.0 } else { 34.0 };
            let stroke = if hot { 255.0 } else { 220.0 };
            let c = rect.center();
            painter.circle_filled(
                c,
                ARROW / 2.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, (bg * a) as u8),
            );
            let pts: Vec<Pos2> = points.iter().map(|d| Pos2::new(c.x + d.0, c.y + d.1)).collect();
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(3.0, Color32::from_white_alpha((stroke * a) as u8)),
            ));
        }
    }

    fn paint_toggle(&mut self, ui: &mut egui::Ui, full: Rect) {
        let rect = Rect::from_min_size(
            Pos2::new(full.right() - 42.0, full.bottom() - 42.0),
            egui::vec2(30.0, 30.0),
        );
        let resp = ui.interact(rect, egui::Id::unique("iv.toggle_carousel"), Sense::click());
        if resp.clicked() {
            self.carousel = !self.carousel;
        }
        let a = if resp.hovered() { 150.0 } else { 95.0 };
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(0, 0, 0, a as u8));
        let c = rect.center();
        let up = !self.carousel;
        let dy = if up { -4.0 } else { 4.0 };
        let pts = vec![
            Pos2::new(c.x - 7.0, c.y + dy + if up { 3.0 } else { -3.0 }),
            Pos2::new(c.x, c.y + dy + if up { -3.0 } else { 3.0 }),
            Pos2::new(c.x + 7.0, c.y + dy + if up { 3.0 } else { -3.0 }),
        ];
        painter.add(egui::Shape::line(pts, Stroke::new(2.6, Color32::from_white_alpha(235))));
    }

    fn carousel(&mut self, ui: &mut egui::Ui) {
        let n = self.files.len();
        egui::ScrollArea::horizontal()
            .id_salt("carousel_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    for i in 0..n {
                        self.item(ui, i);
                    }
                    ui.add_space(6.0);
                });
            });
    }

    fn item(&mut self, ui: &mut egui::Ui, i: usize) {
        let path = self.files[i].clone();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ITEM_W, ITEM_H), Sense::click());
        if resp.clicked() {
            self.set_index(i);
        }
        if i == self.idx && self.scroll_to {
            resp.scroll_to_me(Some(egui::Align::Center));
            self.scroll_to = false;
        }
        let painter = ui.painter();
        let thumb_rect = Rect::from_min_size(
            rect.min + egui::vec2(8.0, 4.0),
            egui::vec2(ITEM_W - 16.0, THUMB_H),
        );
        if resp.hovered() && i != self.idx {
            painter.rect_filled(rect, 4.0, Color32::from_white_alpha(22));
        }
        match self.thumbs.get(&path) {
            Some(tex) => {
                let s = tex.size();
                let r = fit_rect(s[0] as u32, s[1] as u32, thumb_rect);
                painter.image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                painter.rect_filled(thumb_rect, 3.0, Color32::from_gray(38));
                if !self.dead.contains(&path) && self.pending.insert(path.clone()) {
                    let _ = self.loader.tx.send(Job::Thumb { path: path.clone() });
                }
            }
        }
        if i == self.idx {
            painter.rect_stroke(rect, 4.0, Stroke::new(1.6, Color32::WHITE), egui::StrokeKind::Outside);
        }
        let label_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 6.0, thumb_rect.max.y + 4.0),
            egui::vec2(ITEM_W - 12.0, 16.0),
        );
        let selected = i == self.idx;
        ui.put(
            label_rect,
            egui::Label::new(
                egui::RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                    .size(11.0)
                    .color(if selected {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(165)
                    }),
            )
            .truncate(),
        );
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.tick(&ctx);
        self.keys(&ctx);

        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for d in dropped {
            let p = d.path().to_path_buf();
            self.open_path(&p);
            break;
        }

        self.paint_background(ui);

        if self.carousel {
            egui::Panel::bottom("carousel")
                .frame(egui::Frame::NONE)
                .show(ui, |ui| self.carousel(ui));
        }

        egui::CentralPanel::no_frame().show(ui, |ui| self.central(ui));
    }
}


fn fit_rect(w: u32, h: u32, avail: Rect) -> Rect {
    let (w, h) = (w.max(1) as f32, h.max(1) as f32);
    let scale = (avail.width() / w).min(avail.height() / h);
    let size = egui::vec2(w * scale, h * scale);
    Rect::from_center_size(avail.center(), size)
}

fn put(painter: &egui::Painter, x: f32, y: f32, text: &str, font: FontId, color: Color32) {
    painter.text(Pos2::new(x, y), egui::Align2::LEFT_TOP, text, font, color);
}

fn row(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    key: &str,
    val: &str,
    gray: Color32,
    white: Color32,
) -> f32 {
    put(painter, x, y, key, FontId::proportional(11.0), gray);
    put(painter, x, y + 13.0, val, FontId::proportional(13.0), white);
    y + 34.0
}

/// Chevron arrow offsets relative to center: left (-1) or right (+1).
fn chevron(dir: f32) -> [(f32, f32); 3] {
    [
        (7.0 * dir, -11.0),
        (-7.0 * dir, 0.0),
        (7.0 * dir, 11.0),
    ]
}

fn ellipsize(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
