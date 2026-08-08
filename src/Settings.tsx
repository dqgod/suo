import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import { zhCN } from "./i18n/zh-CN";
import {
  LauncherPreferences,
  loadLauncherPreferences,
  saveLauncherPreferences,
} from "./preferences";
import "./Settings.css";

const settingsWindow = getCurrentWindow();

function Settings() {
  const [preferences, setPreferences] = useState(loadLauncherPreferences);
  const [saved, setSaved] = useState(false);

  const close = useCallback(async () => {
    try {
      await settingsWindow.hide();
    } catch (error) {
      console.error("Unable to hide the settings window", error);
    }
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") void close();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close]);

  const updatePreferences = async (next: LauncherPreferences) => {
    setPreferences(next);
    saveLauncherPreferences(next);
    await invoke("update_launcher_preferences", {
      closeOnBlur: next.closeOnBlur,
      keepLastInput: next.keepLastInput,
    });
    setSaved(true);
    window.setTimeout(() => setSaved(false), 1200);
  };

  const hotkey = /Mac/i.test(navigator.platform) ? "Command + Space" : "Alt + Space";

  return (
    <main className="settings-stage">
      <header className="settings-titlebar" data-tauri-drag-region>
        <div className="settings-brand" data-tauri-drag-region>
          <span aria-hidden="true">◇</span>
          <strong data-tauri-drag-region>{zhCN.settingsTitle}</strong>
        </div>
        <button type="button" onClick={close} aria-label={zhCN.closeSettings}>×</button>
      </header>

      <div className="settings-layout">
        <aside>
          <span className="settings-nav-active">{zhCN.general}</span>
          <span>{zhCN.searchAndIndex}</span>
          <span>{zhCN.commands}</span>
          <span>{zhCN.appearance}</span>
        </aside>

        <section className="settings-content">
          <div className="settings-heading">
            <div>
              <h1>{zhCN.general}</h1>
              <p>{zhCN.generalDescription}</p>
            </div>
            <span className={`saved-indicator ${saved ? "visible" : ""}`}>
              {zhCN.saved}
            </span>
          </div>

          <div className="settings-card">
            <div className="setting-row">
              <div>
                <strong>{zhCN.globalHotkey}</strong>
                <small>{zhCN.globalHotkeyDescription}</small>
              </div>
              <kbd className="hotkey-value">{hotkey}</kbd>
            </div>

            <label className="setting-row">
              <div>
                <strong>{zhCN.closeOnBlur}</strong>
                <small>{zhCN.closeOnBlurDescription}</small>
              </div>
              <input
                className="switch"
                type="checkbox"
                checked={preferences.closeOnBlur}
                onChange={(event) =>
                  void updatePreferences({
                    ...preferences,
                    closeOnBlur: event.target.checked,
                  })
                }
              />
            </label>

            <label className="setting-row">
              <div>
                <strong>{zhCN.keepLastInputSetting}</strong>
                <small>{zhCN.keepLastInputDescription}</small>
              </div>
              <input
                className="switch"
                type="checkbox"
                checked={preferences.keepLastInput}
                onChange={(event) =>
                  void updatePreferences({
                    ...preferences,
                    keepLastInput: event.target.checked,
                  })
                }
              />
            </label>

            <div className="setting-row">
              <div>
                <strong>{zhCN.trayIcon}</strong>
                <small>{zhCN.trayIconDescription}</small>
              </div>
              <span className="setting-status">{zhCN.enabled}</span>
            </div>
          </div>

          <div className="settings-note">
            <strong>{zhCN.technicalPreview}</strong>
            <span>{zhCN.moreSettingsLater}</span>
          </div>
        </section>
      </div>
    </main>
  );
}

export default Settings;
