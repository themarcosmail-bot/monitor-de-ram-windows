use std::{env, fs, io, path::PathBuf};

pub const MINUTES_MAX: u32 = 1440;
pub const THRESHOLD_MAX_MIB: u32 = 4096;
pub const THRESHOLD_MIN_MIB: u32 = 256;

#[derive(Clone)]
pub struct Settings {
    pub minutes: u32,
    pub threshold_mib: u32,
    pub automatic: bool,
    pub targets: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            minutes: 1,
            threshold_mib: 1024,
            automatic: false,
            targets: Vec::new(),
        }
    }
}

pub fn parse_number(text: &str, minimum: u32, maximum: u32) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|value| (minimum..=maximum).contains(value))
}

impl Settings {
    pub fn load() -> Self {
        config_path()
            .and_then(fs::read_to_string)
            .map(|text| Self::from_text(&text))
            .unwrap_or_default()
    }

    fn from_text(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("minutes=") {
                if let Some(minutes) = parse_number(value, 1, MINUTES_MAX) {
                    settings.minutes = minutes;
                }
            } else if let Some(value) = line.strip_prefix("threshold_mib=") {
                if let Some(threshold) = parse_number(value, THRESHOLD_MIN_MIB, THRESHOLD_MAX_MIB) {
                    settings.threshold_mib = threshold;
                }
            } else if let Some(value) = line.strip_prefix("automatic=") {
                settings.automatic = value == "1";
            } else if let Some(path) = line.strip_prefix("target=") {
                settings.add_target(path);
            }
        }
        settings
    }

    pub fn add_target(&mut self, path: &str) -> bool {
        let path = path.to_lowercase();
        if path.contains(['\n', '\r'])
            || !std::path::Path::new(&path).is_absolute()
            || !path.ends_with(".exe")
            || self.targets.contains(&path)
        {
            return false;
        }
        self.targets.push(path);
        true
    }

    pub fn save(&self) -> io::Result<()> {
        let file = config_path()?;
        let parent = file
            .parent()
            .ok_or_else(|| io::Error::other("pasta inválida"))?;
        fs::create_dir_all(parent)?;
        let mut text = format!(
            "minutes={}\nthreshold_mib={}\nautomatic={}\n",
            self.minutes,
            self.threshold_mib,
            u8::from(self.automatic)
        );
        for target in &self.targets {
            text.push_str("target=");
            text.push_str(target);
            text.push('\n');
        }
        let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, text)?;
        if let Err(error) = fs::rename(&temporary, &file) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }
}

fn config_path() -> io::Result<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(|path| {
            PathBuf::from(path)
                .join("Monitor de RAM")
                .join("config.txt")
        })
        .ok_or_else(|| io::Error::other("LOCALAPPDATA indisponível"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_intervals_and_deduplicates_targets() {
        let settings = Settings::from_text(concat!(
            "minutes=0\nthreshold_mib=2048\nautomatic=0\n",
            "target=C:\\Program Files\\VLC\\vlc.exe\n",
            "target=c:\\program files\\vlc\\VLC.EXE\n",
            "target=calc.exe\n",
        ));
        assert_eq!(settings.minutes, 1);
        assert_eq!(settings.threshold_mib, 2048);
        assert!(!settings.automatic);
        assert_eq!(settings.targets.len(), 1);
    }
}
