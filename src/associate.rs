/// Register (or unregister) as an image viewer / default-app candidate.
pub fn run(register: bool) -> Result<String, String> {
    platform::run(register)
}

#[cfg(target_os = "linux")]
const MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/bmp",
    "image/webp",
    "image/tiff",
    "image/x-icon",
    "image/x-pnm",
];

#[cfg(windows)]
const EXT_PROGIDS: &[(&str, &str)] = &[
    ("jpg", "JPEG image"),
    ("jpeg", "JPEG image"),
    ("png", "PNG image"),
    ("gif", "GIF image"),
    ("bmp", "BMP image"),
    ("webp", "WebP image"),
    ("tif", "TIFF image"),
    ("tiff", "TIFF image"),
    ("ico", "icon image"),
];

#[cfg(windows)]
mod platform {
    use super::*;
    use std::fs;

    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    pub fn run(register: bool) -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_s = exe.display().to_string();
        let exe_name = exe
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image-viewer.exe".into());
        let app_key = format!("Software\\Classes\\Applications\\{exe_name}");
        let caps_key = format!("{app_key}\\Capabilities");
        let cmd = format!("\"{exe_s}\" \"%1\"");

        let mut reg = String::from("Windows Registry Editor Version 5.00\r\n");

        if register {
            reg.push_str(&format!(
                "\r\n[HKEY_CURRENT_USER\\{app_key}]\r\n@=\"Image Viewer\"\r\n\"FriendlyAppName\"=\"Image Viewer\"\r\n"
            ));
            reg.push_str(&format!(
                "\r\n[HKEY_CURRENT_USER\\{app_key}\\shell\\open\\command]\r\n@=\"{}\"\r\n",
                esc(&cmd)
            ));
            reg.push_str(&format!(
                "\r\n[HKEY_CURRENT_USER\\{app_key}\\SupportedTypes]\r\n"
            ));
            for (ext, _) in EXT_PROGIDS {
                reg.push_str(&format!("\".{ext}\"=\"\"\r\n"));
            }

            reg.push_str(&format!(
                "\r\n[HKEY_CURRENT_USER\\Software\\RegisteredApplications]\r\n\"Image Viewer\"=\"{}\"\r\n",
                esc(&caps_key)
            ));
            reg.push_str(&format!(
                "\r\n[HKEY_CURRENT_USER\\{caps_key}]\r\n\"ApplicationName\"=\"Image Viewer\"\r\n\"ApplicationDescription\"=\"Fast, minimal image viewer\"\r\n"
            ));

            for (ext, label) in EXT_PROGIDS {
                let pid = format!("ImageViewer.{ext}");

                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\{caps_key}\\FileAssociations]\r\n\".{ext}\"=\"{pid}\"\r\n"
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Classes\\{pid}]\r\n@=\"Image Viewer {label}\"\r\n"
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Classes\\{pid}\\DefaultIcon]\r\n@=\"{}\"\r\n",
                    esc(&format!("\"{exe_s}\",0"))
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Classes\\{pid}\\shell\\open\\command]\r\n@=\"{}\"\r\n",
                    esc(&cmd)
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Classes\\.{ext}\\OpenWithProgids]\r\n\"{pid}\"=\"\"\r\n"
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.{ext}\\OpenWithProgids]\r\n\"{pid}\"=\"\"\r\n"
                ));
            }
        } else {
            for (ext, _) in EXT_PROGIDS {
                let pid = format!("ImageViewer.{ext}");
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Classes\\.{ext}\\OpenWithProgids]\r\n\"{pid}\"=-\r\n"
                ));
                reg.push_str(&format!(
                    "\r\n[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.{ext}\\OpenWithProgids]\r\n\"{pid}\"=-\r\n"
                ));
                reg.push_str(&format!(
                    "\r\n[-HKEY_CURRENT_USER\\Software\\Classes\\{pid}]\r\n"
                ));
            }
            reg.push_str(
                "\r\n[HKEY_CURRENT_USER\\Software\\RegisteredApplications]\r\n\"Image Viewer\"=-\r\n",
            );
            reg.push_str(&format!("\r\n[-HKEY_CURRENT_USER\\{app_key}]\r\n"));
        }

        let rt = reg;
        import(&rt)?;

        if register {
            Ok("Registered as an image viewer.\n\n\
                Windows 10/11 keep your default-app choice behind a user confirmation, \
                so pick \"Image Viewer\" once in Settings > Apps > Default apps, or \
                right-click an image > Open with > Image Viewer > Always."
                .into())
        } else {
            Ok("Registration removed.".into())
        }
    }

    fn import(reg_text: &str) -> Result<(), String> {
        let tmp = std::env::temp_dir().join(format!(
            "image-viewer-{}.reg",
            std::process::id()
        ));
        // "Windows Registry Editor Version 5.00" files must be UTF-16LE with a BOM;
        // a plain-ASCII file makes `reg import` fail (and the error box dings).
        let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
        for unit in reg_text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        let out = std::process::Command::new("reg")
            .arg("import")
            .arg(&tmp)
            .output()
            .map_err(|e| format!("cannot run reg.exe: {e}"))?;
        let _ = fs::remove_file(&tmp);
        if !out.status.success() {
            return Err(format!(
                "registry import failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    pub fn run(register: bool) -> Result<String, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let home = PathBuf::from(home);
        let apps = home.join(".local/share/applications");
        let desktop = apps.join("image-viewer.desktop");
        let mimeapps = home.join(".config/mimeapps.list");

        if !register {
            let _ = fs::remove_file(&desktop);
            let icon = home.join(".local/share/icons/hicolor/256x256/apps/image-viewer.png");
            let _ = fs::remove_file(&icon);
            if let Ok(content) = fs::read_to_string(&mimeapps) {
                let cleaned: String = content
                    .lines()
                    .filter(|l| !l.contains("image-viewer.desktop"))
                    .map(|l| format!("{l}\n"))
                    .collect();
                let _ = fs::write(&mimeapps, cleaned);
            }
            let _ = std::process::Command::new("update-desktop-database")
                .arg(&apps)
                .status();
            return Ok("Registration removed.".into());
        }

        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .display()
            .to_string();
        fs::create_dir_all(&apps).map_err(|e| e.to_string())?;

        // Install the icon so the desktop entry has something to point at.
        let icon_dir = home.join(".local/share/icons/hicolor/256x256/apps");
        fs::create_dir_all(&icon_dir).map_err(|e| e.to_string())?;
        fs::write(
            icon_dir.join("image-viewer.png"),
            include_bytes!("../assets/icon_256.png"),
        )
        .map_err(|e| e.to_string())?;

        let mime_line = MIME_TYPES.iter().map(|m| format!("{m};")).collect::<String>();
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Image Viewer\n\
             Comment=Fast, minimal image viewer\n\
             Exec=\"{exe}\" %f\n\
             Icon=image-viewer\n\
             Terminal=false\n\
             Categories=Graphics;Viewer;\n\
             MimeType={mime_line}\n"
        );
        fs::write(&desktop, content).map_err(|e| e.to_string())?;

        // Set defaults only where none exist — never overwrite the user's choice.
        let mut conf = fs::read_to_string(&mimeapps).unwrap_or_default();
        if !conf.ends_with('\n') && !conf.is_empty() {
            conf.push('\n');
        }
        for mime in MIME_TYPES {
            if conf.contains(&format!("{mime}=")) {
                continue;
            }
            for section in ["Default Applications", "Added Associations"] {
                let header = format!("[{section}]\n");
                let line = format!("{mime}=image-viewer.desktop;\n");
                match conf.find(&header) {
                    Some(pos) => {
                        let at = pos + header.len();
                        conf.insert_str(at, &line);
                    }
                    None => conf.push_str(&format!("\n{header}{line}")),
                }
            }
        }
        if let Some(parent) = mimeapps.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&mimeapps, conf).map_err(|e| e.to_string())?;

        let _ = std::process::Command::new("update-desktop-database")
            .arg(&apps)
            .status();

        Ok(
            "Registered.\n\nImage Viewer now appears in \"Open With\" and as a default-app \
             candidate for common image types. If your file manager does not refresh immediately, \
             log out and back in (or restart the file manager)."
                .into(),
        )
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    pub fn run(_register: bool) -> Result<String, String> {
        Err(
            "Automatic registration is supported on Windows and Linux. \
             On macOS use Finder: right click > Get Info > Open With."
                .into(),
        )
    }
}
