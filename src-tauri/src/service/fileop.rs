/// File operations service
pub struct Service;

impl Service {
    /// Create a temp file with the given text and open it in the default editor.
    /// Uses unified jpaste2 temp directory (cleaned up on app exit).
    pub fn preview_text(content: &str) -> Result<(), String> {
        let ext = detect_format(content);
        let suffix = if ext.is_empty() { ".txt".to_string() } else { format!(".{}", ext) };

        // Create temp file in unified jpaste2 directory
        let temp_dir = crate::util::jpaste_temp_dir();
        let filename = format!("edit_{}{}", uuid::Uuid::new_v4(), suffix);
        let temp_path = temp_dir.join(&filename);

        log::debug!("fileop::preview_text: writing temp file {:?}", temp_path);

        if let Err(e) = std::fs::write(&temp_path, content.as_bytes()) {
            log::error!("fileop::preview_text: failed to write temp file: {}", e);
            return Err(format!("Failed to write temp file: {}", e));
        }

        let path_str = temp_path.to_string_lossy().to_string();

        // Try VS Code via vscode:// URI protocol (no cmd black box)
        if is_vscode_available() {
            if let Err(e) = open_vscode_uri(&path_str) {
                log::error!("fileop::preview_text: failed to open VS Code via URI, falling back to Notepad: {}", e);
                std::process::Command::new("notepad")
                    .arg(&path_str)
                    .spawn()
                    .map_err(|e| format!("Failed to open in Notepad: {}", e))?;
            }
        } else {
            std::process::Command::new("notepad").arg(&path_str).spawn()
                .map_err(|e| format!("Failed to open in Notepad: {}", e))?;
        }

        Ok(())
    }
}

/// Open VS Code via vscode://file/ URI — no cmd.exe black box.
#[cfg(windows)]
fn open_vscode_uri(path: &str) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // ShellExecuteW can open registered URI protocol handlers directly
    let normalized = path.replace('\\', "/");
    let uri = format!("vscode://file/{}", normalized);
    let wide: Vec<u16> = uri.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(w!("open").as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // HINSTANCE > 32 indicates success; cast to isize for comparison.
    if (result.0 as isize) > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed (code {})", result.0 as isize))
    }
}

#[cfg(not(windows))]
fn open_vscode_uri(path: &str) -> Result<(), String> {
    std::process::Command::new("code").arg(path).spawn()
        .map_err(|e| format!("Failed to spawn code: {}", e))?;
    Ok(())
}

/// Check if VS Code is available (result is cached).
fn is_vscode_available() -> bool {
    use once_cell::sync::OnceCell;
    static VS_CODE_CACHE: OnceCell<bool> = OnceCell::new();
    *VS_CODE_CACHE.get_or_init(|| {
        match std::process::Command::new("where").arg("code").output() {
            Ok(output) => {
                let available = output.status.success();
                log::debug!("is_vscode_available: available={}", available);
                available
            }
            Err(e) => {
                log::error!("is_vscode_available: 'where code' failed: {}", e);
                false
            }
        }
    })
}

/// Detect file format from content by examining the first 8KB.
/// Returns the file extension without dot (e.g., "json", "xml"), or "" if unknown.
fn detect_format(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }

    let sniff_size = 8 * 1024;
    let sample = if content.len() > sniff_size {
        &content[..sniff_size]
    } else {
        content
    };

    let trimmed = sample.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let first = trimmed.as_bytes()[0];

    if first == b'{' || first == b'[' {
        return "json".to_string();
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("<!doctype") || lower.starts_with("<html") {
        return "html".to_string();
    }

    if first == b'<' {
        return "xml".to_string();
    }

    let upper = trimmed.to_uppercase();
    let sql_keywords = ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "CREATE ", "ALTER ", "DROP "];
    for kw in &sql_keywords {
        if upper.starts_with(kw) {
            return "sql".to_string();
        }
    }

    if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
        return "md".to_string();
    }

    if first == b'[' && trimmed.contains(']') {
        return "ini".to_string();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() >= 2 {
        let first_commas = lines[0].matches(',').count();
        if first_commas > 0 {
            let mut match_count = 0;
            for line in &lines[1..] {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.matches(',').count() != first_commas {
                    break;
                }
                match_count += 1;
            }
            if match_count > 0 {
                return "csv".to_string();
            }
        }
    }

    let yaml_lines = trimmed
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty() && !l.starts_with('#') && l.contains(": ") && !l.starts_with('-')
        })
        .count();
    if yaml_lines >= 2 {
        return "yaml".to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json_object() {
        assert_eq!(detect_format(r#"{"key": "value"}"#), "json");
    }

    #[test]
    fn test_detect_json_array() {
        assert_eq!(detect_format("[1, 2, 3]"), "json");
    }

    #[test]
    fn test_detect_html() {
        assert_eq!(detect_format("<!DOCTYPE html><html></html>"), "html");
    }

    #[test]
    fn test_detect_xml() {
        assert_eq!(detect_format("<?xml version=\"1.0\"?><root/>"), "xml");
    }

    #[test]
    fn test_detect_sql() {
        assert_eq!(detect_format("SELECT * FROM users"), "sql");
    }

    #[test]
    fn test_detect_markdown() {
        assert_eq!(detect_format("# Heading\n## Subheading"), "md");
    }

    #[test]
    fn test_detect_csv() {
        assert_eq!(detect_format("a,b,c\n1,2,3\n4,5,6"), "csv");
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(detect_format("key: value\nother: data"), "yaml");
    }

    #[test]
    fn test_detect_empty() {
        assert_eq!(detect_format(""), "");
    }

    #[test]
    fn test_detect_plain_text() {
        assert_eq!(detect_format("just some random text"), "");
    }
}
