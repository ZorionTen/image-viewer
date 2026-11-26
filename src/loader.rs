use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use eframe::egui;

use crate::meta::{self, Meta};

pub enum Job {
    Main { generation: u64, path: PathBuf },
    Thumb { path: PathBuf },
}

pub enum Msg {
    Main {
        generation: u64,
        result: Result<(egui::ColorImage, Meta), String>,
    },
    Thumb {
        path: PathBuf,
        result: Result<egui::ColorImage, String>,
    },
}

pub struct Loader {
    pub tx: Sender<Job>,
    pub rx: Receiver<Msg>,
}

impl Loader {
    pub fn new(ctx: egui::Context) -> Self {
        let (jtx, jrx) = channel::<Job>();
        let (mtx, mrx) = channel::<Msg>();
        thread::spawn(move || worker(jrx, mtx, ctx));
        Self { tx: jtx, rx: mrx }
    }
}

fn worker(jrx: Receiver<Job>, mtx: Sender<Msg>, ctx: egui::Context) {
    let mut thumbs: VecDeque<PathBuf> = VecDeque::new();
    let mut latest_main: Option<(u64, PathBuf)> = None;
    loop {
        // Block for new work only when nothing is pending.
        if latest_main.is_none() && thumbs.is_empty() {
            let Ok(job) = jrx.recv() else { return };
            match job {
                Job::Main { generation, path } => latest_main = Some((generation, path)),
                Job::Thumb { path } => thumbs.push_back(path),
            }
            while let Ok(job) = jrx.try_recv() {
                match job {
                    Job::Main { generation, path } => latest_main = Some((generation, path)),
                    Job::Thumb { path } => thumbs.push_back(path),
                }
            }
        }
        // Main image always preempts pending thumbnails.
        if let Some((generation, path)) = latest_main.take() {
            let result = load_main(&path);
            if mtx.send(Msg::Main { generation, result }).is_err() {
                return;
            }
            ctx.request_repaint();
        }
        // Then thumbnails — but if a new job arrives, yield to it so that
        // navigation always feels immediate.
        while let Some(path) = thumbs.pop_front() {
            match jrx.try_recv() {
                Ok(Job::Main { generation, path: p }) => {
                    thumbs.push_front(path);
                    latest_main = Some((generation, p));
                    break;
                }
                Ok(Job::Thumb { path: p }) => {
                    thumbs.push_front(path);
                    thumbs.push_back(p);
                    break;
                }
                Err(_) => do_thumb(&mtx, &ctx, &path),
            }
        }
    }
}

fn do_thumb(mtx: &Sender<Msg>, ctx: &egui::Context, path: &Path) {
    let result = load_thumb(path);
    if mtx.send(Msg::Thumb { path: path.to_path_buf(), result }).is_err() {
        // Receiver gone; nothing to do.
    }
    ctx.request_repaint();
}

fn load_main(path: &Path) -> Result<(egui::ColorImage, Meta), String> {
    let reader = image::ImageReader::open(path).map_err(|e| format!("Cannot open: {e}"))?;
    let img = reader.decode().map_err(|e| format!("Cannot decode: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let rgba = img.to_rgba8();
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    );
    let meta = meta::read(path, w, h);
    Ok((color, meta))
}

fn load_thumb(path: &Path) -> Result<egui::ColorImage, String> {
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
    let img = reader.decode().map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(240, 150);
    let rgba = thumb.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}
