//! Small choices kept between runs, one file each in the config folder (`$XDG_CONFIG_HOME/tapiti/<name>`, else `~/.config/tapiti`). A file
//! that cannot be read or written only means the choice is not kept.

use std::path::PathBuf;

/// The folder the choices are kept in, for the Settings window to name.
pub fn dir() -> Option<PathBuf> {
    file("")
}

fn file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(base.join("tapiti").join(name))
}

pub fn load(name: &str) -> Option<String> {
    std::fs::read_to_string(file(name)?)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn save(name: &str, value: &str) {
    if let Some(file) = file(name) {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(file, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_is_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("tapiti-prefs-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        assert_eq!(load("view"), None);
        save("view", "iso");
        assert_eq!(load("view").as_deref(), Some("iso"));
        save("view", "solid");
        assert_eq!(load("view").as_deref(), Some("solid"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
