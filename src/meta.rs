use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Metadata shown in the side panel.
pub struct Meta {
    pub name: String,
    pub dir: String,
    pub ext: String,
    pub w: u32,
    pub h: u32,
    pub size: u64,
    pub modified: Option<String>,
    pub exif: Vec<(String, String)>,
}

const EXIF_KEEP: &[&str] = &[
    "Make",
    "Model",
    "LensMake",
    "LensModel",
    "FocalLength",
    "FNumber",
    "ExposureTime",
    "ISOSpeedRatings",
    "DateTimeOriginal",
    "Artist",
    "Software",
];

pub fn read(path: &Path, w: u32, h: u32) -> Meta {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "?".into());
    let (size, modified) = match fs::metadata(path) {
        Ok(m) => (
            m.len(),
            m.modified().ok().map(fmt_time),
        ),
        Err(_) => (0, None),
    };
    Meta {
        name,
        dir,
        ext,
        w,
        h,
        size,
        modified,
        exif: read_exif(path),
    }
}

fn read_exif(path: &Path) -> Vec<(String, String)> {
    let f = match fs::File::open(path) {
        Ok(f) => std::io::BufReader::new(f),
        Err(_) => return Vec::new(),
    };
    let mut f = f;
    let exif = match exif::Reader::new().read_from_container(&mut f) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for field in exif.fields() {
        let tag = field.tag.to_string();
        if EXIF_KEEP.contains(&tag.as_str()) {
            let val = field.display_value().with_unit(&exif).to_string();
            if !val.is_empty() {
                out.push((tag, val));
            }
        }
    }
    out
}

pub fn human_size(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1 << 20) as f64)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn fmt_time(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (h, m) = (secs.rem_euclid(86_400) / 3600, secs.rem_euclid(3600) / 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}
