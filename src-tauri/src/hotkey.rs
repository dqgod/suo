use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

pub struct RegisteredShortcutChange {
    previous: Shortcut,
    next: Shortcut,
    previous_was_registered: bool,
}

pub fn default_shortcut() -> String {
    #[cfg(target_os = "macos")]
    {
        "super+Space".into()
    }

    #[cfg(not(target_os = "macos"))]
    {
        "alt+Space".into()
    }
}

pub fn normalize_shortcut(value: &str) -> Result<String, String> {
    parse_shortcut(value).map(|shortcut| shortcut.to_string())
}

pub fn register_initial(app: &AppHandle, value: &str) -> String {
    let label = shortcut_label(value).unwrap_or_else(|_| value.trim().to_string());
    match parse_shortcut(value).and_then(|shortcut| {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())
    }) {
        Ok(()) => format!("{label} 已就绪"),
        Err(error) => format!("{label} 注册失败：{error}"),
    }
}

impl RegisteredShortcutChange {
    pub fn apply(app: &AppHandle, previous: &str, next: &str) -> Result<Option<Self>, String> {
        let previous = parse_shortcut(previous)?;
        let next = parse_shortcut(next)?;
        if previous == next {
            return Ok(None);
        }

        let shortcuts = app.global_shortcut();
        let previous_was_registered = shortcuts.is_registered(previous);
        shortcuts.register(next).map_err(|error| {
            format!(
                "快捷键 {} 注册失败：{error}；原快捷键保持不变",
                display_shortcut(next)
            )
        })?;

        if previous_was_registered {
            if let Err(error) = shortcuts.unregister(previous) {
                let _ = shortcuts.unregister(next);
                return Err(format!(
                    "无法停用原快捷键 {}：{error}；已撤销新快捷键",
                    display_shortcut(previous)
                ));
            }
        }

        Ok(Some(Self {
            previous,
            next,
            previous_was_registered,
        }))
    }

    pub fn rollback(self, app: &AppHandle) -> Result<(), String> {
        let shortcuts = app.global_shortcut();
        if self.previous_was_registered {
            shortcuts.register(self.previous).map_err(|error| {
                format!(
                    "配置保存失败，且无法恢复原快捷键 {}：{error}",
                    display_shortcut(self.previous)
                )
            })?;
        }
        shortcuts.unregister(self.next).map_err(|error| {
            format!(
                "配置保存失败，且无法撤销新快捷键 {}：{error}",
                display_shortcut(self.next)
            )
        })
    }
}

pub fn shortcut_label(value: &str) -> Result<String, String> {
    parse_shortcut(value).map(display_shortcut)
}

fn parse_shortcut(value: &str) -> Result<Shortcut, String> {
    let value = value.trim();
    let shortcut = value
        .parse::<Shortcut>()
        .map_err(|error| format!("全局快捷键无效：{error}"))?;
    let activation_modifiers = Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER;
    if !shortcut.mods.intersects(activation_modifiers) {
        return Err("全局快捷键必须包含 Command、Ctrl、Alt 或 Windows 键".into());
    }
    Ok(shortcut)
}

fn display_shortcut(shortcut: Shortcut) -> String {
    let mut parts = Vec::new();
    if shortcut.mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if shortcut.mods.contains(Modifiers::ALT) {
        parts.push(if cfg!(target_os = "macos") {
            "Option".to_string()
        } else {
            "Alt".to_string()
        });
    }
    if shortcut.mods.contains(Modifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    if shortcut.mods.contains(Modifiers::SUPER) {
        parts.push(if cfg!(target_os = "macos") {
            "Command".to_string()
        } else {
            "Win".to_string()
        });
    }
    parts.push(display_key(shortcut.key));
    parts.join(" + ")
}

fn display_key(key: Code) -> String {
    let value = key.to_string();
    value
        .strip_prefix("Key")
        .or_else(|| value.strip_prefix("Digit"))
        .unwrap_or(&value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_labels_supported_shortcuts() {
        assert_eq!(
            normalize_shortcut("Ctrl + Shift + K").unwrap(),
            "shift+control+KeyK"
        );
        assert_eq!(
            shortcut_label("Ctrl + Shift + K").unwrap(),
            "Ctrl + Shift + K"
        );
    }

    #[test]
    fn rejects_shortcuts_without_an_activation_modifier() {
        assert!(normalize_shortcut("KeyK").is_err());
        assert!(normalize_shortcut("Shift+KeyK").is_err());
    }

    #[test]
    fn platform_default_is_valid() {
        assert!(normalize_shortcut(&default_shortcut()).is_ok());
    }
}
