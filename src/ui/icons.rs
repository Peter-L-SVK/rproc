use std::cell::OnceCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves a process to an icon URI by indexing freedesktop `.desktop` files
/// to map process names → icon names, then looking up those icon names through
/// the system's freedesktop icon theme (respects the user's chosen theme, not
/// just hicolor).
pub struct Resolver {
    name_to_icon: OnceCell<HashMap<String, String>>,
    cache: HashMap<String, Option<String>>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            name_to_icon: OnceCell::new(),
            cache: HashMap::new(),
        }
    }

    fn index(&self) -> &HashMap<String, String> {
        self.name_to_icon.get_or_init(|| {
            let mut map = HashMap::new();
            for dir in desktop_dirs() {
                scan_desktop_dir(&dir, &mut map);
            }
            map
        })
    }

    pub fn icon_uri(&mut self, proc_name: &str, exe_path: &str) -> Option<String> {
	// Standard candidates
	for cand in desktop_candidates(proc_name, exe_path) {
            if cand.is_empty() { continue; }
            if let Some(icon) = self.index().get(&cand).cloned()
		&& let Some(uri) = self.resolve_icon(&icon)
            {
		return Some(uri);
            }
	}
	// Prefix match: "brave-browser-s" should match key "brave-browser-stable"
	let proc_lower = proc_name.to_lowercase();
	let matching_icon: Option<String> = self.index().iter()
            .find(|(key, _)| key.starts_with(&proc_lower) || proc_lower.starts_with(key.as_str()))
            .map(|(_, icon)| icon.clone());
	
	if let Some(icon) = matching_icon
            && let Some(uri) = self.resolve_icon(&icon)
	{
            return Some(uri);
	}
	// Direct icon name lookup
	if let Some(uri) = self.resolve_icon(&proc_lower) {
            return Some(uri);
	}
	// Generic fallback
	if let Some(uri) = self.resolve_icon("application-x-executable") {
            return Some(uri);
	}
	None
    }
    
    pub fn has_desktop_entry(&self, proc_name: &str, exe_path: &str) -> bool {
        desktop_candidates(proc_name, exe_path)
            .iter()
            .any(|c| !c.is_empty() && self.index().contains_key(c))
    }
    
    fn resolve_icon(&mut self, icon_name: &str) -> Option<String> {
	if let Some(cached) = self.cache.get(icon_name) {
            return cached.clone();
	}
	let result = if icon_name.starts_with('/') {
            let p = PathBuf::from(icon_name);
            if p.exists() {
		Some(format!("file://{}", p.display()))
            } else {
		None
            }
	} else {
            lookup_icon(icon_name).map(|p| {
		if p.exists() {
                    format!("file://{}", p.display())
		} else {
                    // This shouldn't happen — lookup_icon already checks exists()
                    // but just in case, return empty string to skip loading
                    String::new()
		}
            }).filter(|s| !s.is_empty())
	};
	self.cache.insert(icon_name.to_string(), result.clone());
	result
    }
}

fn lookup_icon(icon_name: &str) -> Option<PathBuf> {
    use std::sync::OnceLock;
    static THEME_CHAIN: OnceLock<Vec<PathBuf>> = OnceLock::new();
    
    let theme_dirs = THEME_CHAIN.get_or_init(|| {
        let mut dirs = Vec::new();
	
        let active_theme = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "icon-theme"])
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .trim_matches('\'')
                    .to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .unwrap_or_else(|| "hicolor".to_string());
        
        let icon_bases = {
            let mut bases = vec![
                PathBuf::from("/usr/share/icons"),
                PathBuf::from("/usr/local/share/icons"),
            ];
            if let Some(home) = std::env::var_os("HOME") {
                let home = PathBuf::from(home);
                bases.push(home.join(".local/share/icons"));
                bases.push(home.join(".icons"));
            }
            bases
        };
        
        // Collect themes following inheritance chain
        let mut themes = vec![active_theme];
        let mut seen = std::collections::HashSet::new();
        seen.insert(themes[0].clone());
        
        let mut i = 0;
        while i < themes.len() {
            let current = themes[i].clone(); // Clone to release borrow
            for base in &icon_bases {
                let index_path = base.join(&current).join("index.theme");
                if let Ok(content) = std::fs::read_to_string(&index_path) {
                    for line in content.lines() {
                        if let Some(parents) = line.strip_prefix("Inherits=") {
                            for parent in parents.split(',') {
                                let parent = parent.trim().to_string();
                                if !parent.is_empty() && seen.insert(parent.clone()) {
                                    themes.push(parent);
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }
	 if seen.insert("hicolor".to_string()) {
            themes.push("hicolor".to_string());
        }
        // Adwaita is GNOME's default and has many generic icons
         if seen.insert("Adwaita".to_string()) {
            themes.push("Adwaita".to_string());
        }
        // Build search directories
        for theme in &themes {
            for base in &icon_bases {
                dirs.push(base.join(theme));
            }
        }
        
        dirs
    });
    
    let names_to_try: [&str; 2] = [
        icon_name,
        icon_name.strip_suffix("-symbolic").unwrap_or(icon_name),
    ];
    
    let exts = ["svg", "png", "xpm"];
    let sizes = ["48x48", "64x64", "128x128", "32x32", "24x24", "22x22", "16x16", "scalable"];
    let cats = ["apps", "devices", "places", "categories", "status", "emblems", "mimetypes"];
    
    for name in &names_to_try {
        for dir in theme_dirs {
            for size in &sizes {
                for cat in &cats {
                    for ext in &exts {
                        let path = dir.join(size).join(cat).join(format!("{name}.{ext}"));
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

fn desktop_candidates(proc_name: &str, exe_path: &str) -> Vec<String> {
    let exe_base = Path::new(exe_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let proc_lower = proc_name.to_lowercase();
    let stem = proc_lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
        .to_string();
    let prefix: String = proc_lower.chars().take(10).collect();
    let mut cands = vec![exe_base, proc_lower.clone(), stem.clone()];
    if prefix != stem && prefix != proc_lower {
        cands.push(prefix);
    }
    cands
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    dirs
}

fn scan_desktop_dir(dir: &Path, out: &mut HashMap<String, String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        parse_desktop(&content, stem, out);
    }
}

fn parse_desktop(content: &str, file_stem: &str, out: &mut HashMap<String, String>) {
    let mut icon: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut try_exec: Option<String> = None;
    let mut wm_class: Option<String> = None;
    let mut in_main = false;
    let mut hidden = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "Icon" => icon = Some(v.trim().to_string()),
            "Exec" => exec = Some(v.trim().to_string()),
            "TryExec" => try_exec = Some(v.trim().to_string()),
            "StartupWMClass" => wm_class = Some(v.trim().to_string()),
            "Hidden" => hidden = v.trim().eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden {
        return;
    }
    let Some(icon) = icon else {
        return;
    };

    let mut add = |key: &str| {
        let key = key.to_lowercase();
        if key.is_empty() {
            return;
        }
        out.entry(key).or_insert_with(|| icon.clone());
    };

    if let Some(e) = exec.as_deref()
        && let Some(bin) = exec_binary(e)
        && let Some(base) = Path::new(&bin).file_name().and_then(|s| s.to_str())
    {
        add(base);
    }
    if let Some(t) = try_exec.as_deref()
        && let Some(base) = Path::new(t).file_name().and_then(|s| s.to_str())
    {
        add(base);
    }
    if let Some(w) = wm_class.as_deref() {
        add(w);
    }
    add(file_stem);
}

fn exec_binary(exec: &str) -> Option<String> {
    let mut tokens = shell_split(exec);
    while let Some(tok) = tokens.first().cloned() {
        if tok.contains('=') && !tok.starts_with('/') {
            tokens.remove(0);
            continue;
        }
        let base = Path::new(&tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&tok);
        if matches!(base, "env" | "sh" | "bash" | "dbus-run-session") {
            tokens.remove(0);
            continue;
        }
        return Some(tok);
    }
    None
}

fn shell_split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;
    for c in s.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
            }
            c => buf.push(c),
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

// Tests remain the same — the old find_icon_file tests are removed since
// that function no longer exists. The desktop-parsing and candidate tests
// are unchanged.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_candidates_lowercases_and_extracts_stem() {
        let c = desktop_candidates("Code-Insiders", "/usr/bin/Code");
        assert_eq!(c[0], "code");
        assert_eq!(c[1], "code-insiders");
        assert_eq!(c[2], "code");
    }

    #[test]
    fn desktop_candidates_empty_inputs_yield_empty_strings() {
        let c = desktop_candidates("", "");
        assert!(c.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn has_desktop_entry_matches_any_candidate() {
        let mut name_to_icon = HashMap::new();
        name_to_icon.insert("firefox".to_string(), "firefox".to_string());
        let r = Resolver {
            name_to_icon: OnceCell::from(name_to_icon),
            cache: HashMap::new(),
        };
        assert!(r.has_desktop_entry("firefox", "/usr/lib/firefox/firefox-bin"));
        assert!(!r.has_desktop_entry("kworker/0:1", ""));
    }

    #[test]
    fn shell_split_basic() {
        assert_eq!(shell_split("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn shell_split_preserves_quoted_spaces() {
        assert_eq!(
            shell_split(r#"/usr/bin/foo "an arg" bar"#),
            vec!["/usr/bin/foo", "an arg", "bar"],
        );
    }

    #[test]
    fn exec_binary_skips_env_assignments() {
        assert_eq!(exec_binary("FOO=bar baz"), Some("baz".to_string()));
    }

    #[test]
    fn exec_binary_skips_env_command_wrapper() {
        assert_eq!(
            exec_binary("env VAR=1 /usr/bin/firefox"),
            Some("/usr/bin/firefox".to_string())
        );
    }

    #[test]
    fn exec_binary_passes_through_absolute_path() {
        assert_eq!(
            exec_binary("/usr/bin/foo=weird"),
            Some("/usr/bin/foo=weird".to_string())
        );
    }

    #[test]
    fn parse_desktop_basic_entry_indexes_by_file_stem() {
        let content = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=/usr/bin/firefox %u
Icon=firefox
";
        let mut out = HashMap::new();
        parse_desktop(content, "firefox", &mut out);
        assert_eq!(out.get("firefox"), Some(&"firefox".to_string()));
    }

    #[test]
    fn parse_desktop_ignores_hidden_entries() {
        let content = "\
[Desktop Entry]
Type=Application
Name=Hidden app
Exec=/usr/bin/hidden
Icon=hidden
Hidden=true
";
        let mut out = HashMap::new();
        parse_desktop(content, "hidden", &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_desktop_no_icon_skipped() {
        let content = "\
[Desktop Entry]
Type=Application
Exec=/usr/bin/foo
";
        let mut out = HashMap::new();
        parse_desktop(content, "foo", &mut out);
        assert!(out.is_empty());
    }
}
