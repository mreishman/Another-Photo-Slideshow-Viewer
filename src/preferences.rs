use crate::INTERVALS;
use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

pub const MAX_RECENT: usize = 5;

pub struct Preferences {
    pub interval: u64,
    pub looping: bool,
    pub shuffled: bool,
    pub subfolders: bool,
    pub recent: Vec<PathBuf>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            interval: 5,
            looping: true,
            shuffled: false,
            subfolders: false,
            recent: Vec::new(),
        }
    }
}

impl Preferences {
    fn path() -> Option<PathBuf> {
        Some(
            PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
                .join("Another Photo Slideshow Viewer")
                .join("preferences.txt"),
        )
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or_else(Self::default, |data| Self::parse(&data))
    }

    fn parse(data: &str) -> Self {
        let mut prefs = Self::default();
        for (key, value) in data.lines().filter_map(|line| line.split_once('=')) {
            match key {
                "interval" => {
                    if let Ok(interval) = value.parse() {
                        if INTERVALS.contains(&interval) {
                            prefs.interval = interval;
                        }
                    }
                }
                "loop" => prefs.looping = value.parse().unwrap_or(prefs.looping),
                "shuffle" => prefs.shuffled = value.parse().unwrap_or(prefs.shuffled),
                "subfolders" => prefs.subfolders = value.parse().unwrap_or(prefs.subfolders),
                "folder" if prefs.recent.len() < MAX_RECENT => {
                    if let Some(path) = decode_path(value) {
                        if path.is_absolute() && !prefs.recent.contains(&path) {
                            prefs.recent.push(path);
                        }
                    }
                }
                _ => {}
            }
        }
        prefs
    }

    pub fn remember(&mut self, folder: PathBuf) {
        self.recent.retain(|path| path != &folder);
        self.recent.insert(0, folder);
        self.recent.truncate(MAX_RECENT);
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        self.save_to(&path)
    }

    fn save_to(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path.parent().expect("preferences directory"))?;
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = (|| {
            let mut file = fs::File::create(&temp)?;
            writeln!(
                file,
                "interval={}\nloop={}\nshuffle={}\nsubfolders={}",
                self.interval, self.looping, self.shuffled, self.subfolders
            )?;
            for folder in &self.recent {
                write!(file, "folder=")?;
                for unit in folder.as_os_str().encode_wide() {
                    write!(file, "{unit:04x}")?;
                }
                writeln!(file)?;
            }
            drop(file);
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

fn decode_path(text: &str) -> Option<PathBuf> {
    if text.is_empty() || !text.is_ascii() || text.len() % 4 != 0 {
        return None;
    }
    let units = (0..text.len())
        .step_by(4)
        .map(|offset| u16::from_str_radix(&text[offset..offset + 4], 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if units.contains(&0) {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&units)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_latest_preferences_and_unicode_recent_folders() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("slideshow-prefs-{}-{stamp}", std::process::id()));
        let file = dir.join("preferences.txt");
        let mut prefs = Preferences::default();
        prefs.save_to(&file).unwrap();
        for i in 0..7 {
            prefs.remember(PathBuf::from(format!(r"C:\Photos\{i}")));
        }
        let latest = PathBuf::from(r"C:\Photos\旅行 & Friends");
        prefs.remember(latest.clone());
        prefs.remember(latest.clone());
        prefs.interval = 10;
        prefs.looping = false;
        prefs.shuffled = true;
        prefs.subfolders = true;
        prefs.save_to(&file).unwrap();

        let restored = Preferences::parse(&fs::read_to_string(file).unwrap());
        assert_eq!(restored.recent, prefs.recent);
        assert_eq!(restored.recent.len(), MAX_RECENT);
        assert_eq!(restored.recent[0], latest);
        assert_eq!(restored.interval, 10);
        assert!(restored.shuffled && restored.subfolders && !restored.looping);
        fs::remove_dir_all(dir).unwrap();
    }
}
