use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const APP_ID: &str = "nzxt-cam";

pub fn autostart_path() -> Result<PathBuf> {
    Ok(crate::config::config_home()?.join(format!("autostart/{APP_ID}.desktop")))
}

pub fn desktop_entry(executable: &Path, autostart: bool) -> String {
    let exec = executable.display().to_string().replace(' ', "\\s");
    let mut text = format!(
        "[Desktop Entry]\nType=Application\nName=NZXT CAM\nComment=Refrigeração, iluminação e monitoramento\nComment[en_US]=Cooling, lighting and monitoring\nExec={exec}{}\nIcon={APP_ID}\nTerminal=false\nCategories=System;Monitor;HardwareSettings;\nStartupWMClass={APP_ID}\n",
        if autostart { " --minimized" } else { "" }
    );
    if autostart {
        text.push_str("X-GNOME-Autostart-enabled=true\n");
    }
    text
}

pub fn install_autostart() -> Result<()> {
    let path = autostart_path()?;
    fs::create_dir_all(path.parent().context("Autostart sem diretório")?)?;
    fs::write(&path, desktop_entry(&std::env::current_exe()?, true))?;
    Ok(())
}

pub fn autostart_enabled() -> Result<bool> {
    let text = match fs::read_to_string(autostart_path()?) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("Não foi possível consultar o início automático"),
    };
    Ok(parse_enabled(&text))
}

fn parse_enabled(text: &str) -> bool {
    let mut desktop_entry = false;
    let mut found = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            desktop_entry = line == "[Desktop Entry]";
            found |= desktop_entry;
        } else if desktop_entry
            && let Some((key, value)) = line.split_once('=')
            && matches!(
                (key.trim(), value.trim()),
                ("Hidden", "true") | ("X-GNOME-Autostart-enabled", "false")
            )
        {
            return false;
        }
    }
    found
}

pub fn remove_autostart() -> Result<()> {
    match fs::remove_file(autostart_path()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Não foi possível desativar o início automático"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_escapes_spaces_and_marks_autostart() {
        let entry = desktop_entry(Path::new("/opt/my apps/nzxt-cam"), true);
        assert!(entry.contains("Exec=/opt/my\\sapps/nzxt-cam --minimized"));
        assert!(parse_enabled(&entry));
        assert!(!desktop_entry(Path::new("/x"), false).contains("--minimized"));
    }

    #[test]
    fn disabled_entries_are_not_enabled() {
        assert!(!parse_enabled("[Desktop Entry]\nHidden=true\n"));
        assert!(!parse_enabled(
            "[Desktop Entry]\nX-GNOME-Autostart-enabled=false\n"
        ));
        assert!(!parse_enabled("[Other]\nName=x\n"));
        assert!(parse_enabled("[Desktop Entry]\n[Other]\nHidden=true\n"));
    }
}
