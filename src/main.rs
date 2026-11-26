#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod associate;
mod loader;
mod meta;

use std::path::PathBuf;

const USAGE: &str = "Image Viewer — a fast, minimal image viewer

USAGE:
    image-viewer [IMAGE]        open an image file or a folder of images
    image-viewer --register     register as a (default-app candidate) image viewer
    image-viewer --unregister   remove the registration
    image-viewer --help         show this help

KEYS:
    Left / Right    previous / next image in the current folder
    C               show / hide the bottom thumbnail strip";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--register" || a == "--unregister") {
        let register = args.iter().any(|a| a == "--register");
        match associate::run(register) {
            Ok(msg) => report(&msg, false),
            Err(e) => {
                report(&e, true);
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        report(USAGE, false);
        return;
    }

    let path = args.iter().find(|a| !a.starts_with('-')).map(PathBuf::from);
    if let Err(e) = run(path) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(path: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([560.0, 420.0])
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Image Viewer",
        options,
        Box::new(move |cc| Ok(Box::new(app::Viewer::new(cc, path)))),
    )
}

/// Icon shown in the title bar / task bar / dock (X11, Wayland, Windows, macOS).
fn window_icon() -> eframe::egui::IconData {
    let bytes: &[u8] = include_bytes!("../assets/icon_256.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            eframe::egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            }
        }
        Err(_) => eframe::egui::IconData::default(),
    }
}

#[cfg(not(target_os = "windows"))]
fn report(msg: &str, err: bool) {
    if err {
        eprintln!("{msg}");
    } else {
        println!("{msg}");
    }
}

/// Windows release builds have no console; show a message box instead.
#[cfg(target_os = "windows")]
fn report(msg: &str, err: bool) {
    use std::iter;
    #[link(name = "user32")]
    unsafe extern "system" {        fn MessageBoxW(hWnd: isize, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    }
    if cfg!(debug_assertions) {
        if err {
            eprintln!("{msg}");
        } else {
            println!("{msg}");
        }
        return;
    }
    let text: Vec<u16> = msg.encode_utf16().chain(iter::once(0u16)).collect();
    let cap: Vec<u16> = "Image Viewer".encode_utf16().chain(iter::once(0u16)).collect();
    // MB_OK (no icon) is silent; only real errors use MB_ICONERROR (which dings).
    let ty = if err { 0x10u32 } else { 0x0u32 };
    unsafe {
        MessageBoxW(0, text.as_ptr(), cap.as_ptr(), ty);
    }
}
