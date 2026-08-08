import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";
import {
  aliasesFromText,
  AppConfig,
  AppConfigView,
  applyAppearance,
  loadAppConfig,
  ScriptCommandConfig,
  ScriptRuntime,
  WebSearchConfig,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import "./Settings.css";

const settingsWindow = getCurrentWindow();
const t = zhCN.settings;
type Section = "general" | "search" | "commands" | "services" | "appearance";

const sectionCopy: Record<Section, { title: string; description: string }> = {
  general: { title: zhCN.general, description: t.generalDescription },
  search: { title: zhCN.searchAndIndex, description: t.searchDescription },
  commands: { title: zhCN.commands, description: t.commandDescription },
  services: { title: t.services, description: t.serviceDescription },
  appearance: { title: zhCN.appearance, description: t.appearanceDescription },
};

function createId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function Settings() {
  const [section, setSection] = useState<Section>("general");
  const [view, setView] = useState<AppConfigView | null>(null);
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  const close = useCallback(async () => {
    try {
      await settingsWindow.hide();
    } catch (closeError) {
      setError(String(closeError));
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await loadAppConfig();
      setView(next);
      setDraft(next.config);
      applyAppearance(next.config.appearance);
      if (next.configLoadWarning) setError(next.configLoadWarning);
    } catch (loadError) {
      setError(`${t.loadFailed}：${String(loadError)}`);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") void close();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close, refresh]);

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("save_app_config", { config: draft });
      setView(next);
      setDraft(next.config);
      applyAppearance(next.config.appearance);
      setStatus(zhCN.saved);
      window.setTimeout(() => setStatus(""), 1600);
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setSaving(false);
    }
  };

  const saveApiKey = async () => {
    setError("");
    try {
      const next = await invoke<AppConfigView>("set_translation_api_key", { apiKey });
      setView(next);
      setApiKey("");
      setStatus(zhCN.saved);
    } catch (saveError) {
      setError(String(saveError));
    }
  };

  const clearApiKey = async () => {
    setError("");
    try {
      const next = await invoke<AppConfigView>("clear_translation_api_key");
      setView(next);
      setStatus(zhCN.saved);
    } catch (clearError) {
      setError(String(clearError));
    }
  };

  const rebuildIndex = async () => {
    try {
      await invoke("rebuild_file_index");
      setStatus(t.rebuildStarted);
    } catch (rebuildError) {
      setError(String(rebuildError));
    }
  };

  const updateScript = (index: number, patch: Partial<ScriptCommandConfig>) => {
    setDraft((current) => {
      if (!current) return current;
      const scriptCommands = current.scriptCommands.map((command, itemIndex) =>
        itemIndex === index ? { ...command, ...patch } : command,
      );
      return { ...current, scriptCommands };
    });
  };

  const updateWebSearch = (index: number, patch: Partial<WebSearchConfig>) => {
    setDraft((current) => {
      if (!current) return current;
      const webSearches = current.webSearches.map((search, itemIndex) =>
        itemIndex === index ? { ...search, ...patch } : search,
      );
      return { ...current, webSearches };
    });
  };

  const updateAppearance = (appearance: AppConfig["appearance"]) => {
    applyAppearance(appearance);
    setDraft((current) => (current ? { ...current, appearance } : current));
  };

  const addScript = () => {
    setDraft((current) =>
      current
        ? {
            ...current,
            scriptCommands: [
              ...current.scriptCommands,
              {
                id: createId("script"),
                name: "新脚本",
                keyword: `cmd${current.scriptCommands.length + 1}`,
                aliases: [],
                enabled: true,
                runtime: "python",
                scriptPath: "",
                immediate: false,
                timeoutMs: 3000,
              },
            ],
          }
        : current,
    );
  };

  const addWebSearch = () => {
    setDraft((current) =>
      current
        ? {
            ...current,
            webSearches: [
              ...current.webSearches,
              {
                id: createId("web"),
                name: "新搜索",
                keyword: `web${current.webSearches.length + 1}`,
                aliases: [],
                enabled: true,
                urlTemplate: "https://example.com/search?q={query}",
              },
            ],
          }
        : current,
    );
  };

  const hotkey = /Mac/i.test(navigator.platform) ? "Command + Space" : "Alt + Space";

  return (
    <main className="settings-stage">
      <header className="settings-titlebar" data-tauri-drag-region>
        <div className="settings-brand" data-tauri-drag-region>
          <span aria-hidden="true">◇</span>
          <strong data-tauri-drag-region>{zhCN.settingsTitle}</strong>
        </div>
        <button type="button" onClick={() => void close()} aria-label={zhCN.closeSettings}>×</button>
      </header>

      <div className="settings-layout">
        <aside>
          {(Object.keys(sectionCopy) as Section[]).map((key) => (
            <button
              type="button"
              key={key}
              className={section === key ? "settings-nav-active" : ""}
              onClick={() => setSection(key)}
            >
              {sectionCopy[key].title}
            </button>
          ))}
        </aside>

        <section className="settings-content">
          <div className="settings-heading">
            <div>
              <h1>{sectionCopy[section].title}</h1>
              <p>{sectionCopy[section].description}</p>
            </div>
            <div className="settings-actions">
              {status && <span className="saved-indicator visible">{status}</span>}
              <button className="primary-button" type="button" disabled={!draft || saving || view?.configReadOnly} onClick={() => void save()}>
                {saving ? t.saving : t.save}
              </button>
            </div>
          </div>

          {!draft ? (
            <div className="settings-note">{t.loading}</div>
          ) : (
            <>
              {section === "general" && (
                <div className="settings-card">
                  <div className="setting-row">
                    <div><strong>{zhCN.globalHotkey}</strong><small>{zhCN.globalHotkeyDescription}</small></div>
                    <kbd className="hotkey-value">{hotkey}</kbd>
                  </div>
                  <label className="setting-row">
                    <div><strong>{zhCN.closeOnBlur}</strong><small>{zhCN.closeOnBlurDescription}</small></div>
                    <input className="switch" type="checkbox" checked={draft.launcher.closeOnBlur} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, closeOnBlur: event.target.checked } })} />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.keepLastInputSetting}</strong><small>{zhCN.keepLastInputDescription}</small></div>
                    <input className="switch" type="checkbox" checked={draft.launcher.keepLastInput} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, keepLastInput: event.target.checked } })} />
                  </label>
                  <div className="setting-row">
                    <div><strong>{zhCN.trayIcon}</strong><small>{zhCN.trayIconDescription}</small></div>
                    <span className="setting-status">{zhCN.enabled}</span>
                  </div>
                </div>
              )}

              {section === "search" && (
                <div className="settings-card compact-card">
                  <div className="setting-row">
                    <div><strong>{zhCN.searchAndIndex}</strong><small>{t.searchDescription}</small></div>
                    <button className="secondary-button" type="button" onClick={() => void rebuildIndex()}>{t.rebuildNow}</button>
                  </div>
                </div>
              )}

              {section === "commands" && (
                <div className="editor-stack">
                  {draft.scriptCommands.map((command, index) => (
                    <article className="editor-card" key={command.id}>
                      <div className="editor-card-heading">
                        <label className="inline-toggle"><input type="checkbox" checked={command.enabled} onChange={(event) => updateScript(index, { enabled: event.target.checked })} />{t.enabled}</label>
                        <button className="danger-button" type="button" onClick={() => setDraft({ ...draft, scriptCommands: draft.scriptCommands.filter((_, itemIndex) => itemIndex !== index) })}>{t.remove}</button>
                      </div>
                      <div className="form-grid">
                        <Field label={t.name}><input value={command.name} onChange={(event) => updateScript(index, { name: event.target.value })} /></Field>
                        <Field label={t.keyword}><input value={command.keyword} onChange={(event) => updateScript(index, { keyword: event.target.value })} /></Field>
                        <Field label={t.aliases}><AliasesInput value={command.aliases} onChange={(aliases) => updateScript(index, { aliases })} /></Field>
                        <Field label={t.runtime}><select value={command.runtime} onChange={(event) => updateScript(index, { runtime: event.target.value as ScriptRuntime })}><option value="python">Python</option><option value="powerShell">PowerShell</option><option value="bash">Bash</option><option value="executable">Executable</option></select></Field>
                        <Field label={t.scriptPath} wide><input value={command.scriptPath} onChange={(event) => updateScript(index, { scriptPath: event.target.value })} placeholder="D:\\scripts\\tool.py" /></Field>
                        <Field label={t.timeout}><input type="number" min={100} max={60000} value={command.timeoutMs} onChange={(event) => updateScript(index, { timeoutMs: Number(event.target.value) })} /></Field>
                        <label className="checkbox-field"><input type="checkbox" checked={command.immediate} onChange={(event) => updateScript(index, { immediate: event.target.checked })} />{t.immediate}</label>
                      </div>
                    </article>
                  ))}
                  <button className="add-button" type="button" onClick={addScript}>＋ {t.addScript}</button>
                </div>
              )}

              {section === "services" && (
                <div className="editor-stack">
                  <article className="editor-card">
                    <div className="editor-card-heading">
                      <div><strong>{t.translation}</strong><small>{t.translationDescription}</small></div>
                      <label className="inline-toggle"><input type="checkbox" checked={draft.translation.enabled} onChange={(event) => setDraft({ ...draft, translation: { ...draft.translation, enabled: event.target.checked } })} />{t.enabled}</label>
                    </div>
                    <div className="form-grid">
                      <Field label={t.keyword}><input value={draft.translation.keyword} onChange={(event) => setDraft({ ...draft, translation: { ...draft.translation, keyword: event.target.value } })} /></Field>
                      <Field label={t.aliases}><AliasesInput value={draft.translation.aliases} onChange={(aliases) => setDraft({ ...draft, translation: { ...draft.translation, aliases } })} /></Field>
                      <Field label={t.region}><input value={draft.translation.region} onChange={(event) => setDraft({ ...draft, translation: { ...draft.translation, region: event.target.value } })} placeholder="eastasia" /></Field>
                      <Field label={t.defaultTarget}><input value={draft.translation.defaultTargetLanguage} onChange={(event) => setDraft({ ...draft, translation: { ...draft.translation, defaultTargetLanguage: event.target.value } })} /></Field>
                      <Field label={t.chineseTarget}><input value={draft.translation.chineseTargetLanguage} onChange={(event) => setDraft({ ...draft, translation: { ...draft.translation, chineseTargetLanguage: event.target.value } })} /></Field>
                      <Field label={t.apiKey} wide><div className="credential-row"><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={t.apiKeyPlaceholder} /><button className="secondary-button" type="button" disabled={!apiKey.trim()} onClick={() => void saveApiKey()}>{t.saveApiKey}</button><button className="danger-button" type="button" disabled={!view?.translationApiKeyConfigured} onClick={() => void clearApiKey()}>{t.clearApiKey}</button></div><small className={view?.translationApiKeyConfigured ? "credential-ok" : "credential-missing"}>{view?.translationApiKeyConfigured ? t.apiKeyConfigured : t.apiKeyMissing}</small></Field>
                    </div>
                  </article>

                  <div className="subsection-heading"><strong>{t.webSearch}</strong><button className="add-button small" type="button" onClick={addWebSearch}>＋ {t.addWeb}</button></div>
                  {draft.webSearches.map((search, index) => (
                    <article className="editor-card" key={search.id}>
                      <div className="editor-card-heading"><label className="inline-toggle"><input type="checkbox" checked={search.enabled} onChange={(event) => updateWebSearch(index, { enabled: event.target.checked })} />{t.enabled}</label><button className="danger-button" type="button" onClick={() => setDraft({ ...draft, webSearches: draft.webSearches.filter((_, itemIndex) => itemIndex !== index) })}>{t.remove}</button></div>
                      <div className="form-grid"><Field label={t.name}><input value={search.name} onChange={(event) => updateWebSearch(index, { name: event.target.value })} /></Field><Field label={t.keyword}><input value={search.keyword} onChange={(event) => updateWebSearch(index, { keyword: event.target.value })} /></Field><Field label={t.aliases}><AliasesInput value={search.aliases} onChange={(aliases) => updateWebSearch(index, { aliases })} /></Field><Field label={t.urlTemplate} wide><input value={search.urlTemplate} onChange={(event) => updateWebSearch(index, { urlTemplate: event.target.value })} /></Field></div>
                    </article>
                  ))}
                </div>
              )}

              {section === "appearance" && (
                <div className="settings-card compact-card">
                  <label className="setting-row"><div><strong>{t.theme}</strong><small>{t.appearanceDescription}</small></div><select className="setting-select" value={draft.appearance.theme} onChange={(event) => updateAppearance({ ...draft.appearance, theme: event.target.value as AppConfig["appearance"]["theme"] })}><option value="midnight">{t.midnight}</option><option value="paper">{t.paper}</option><option value="forest">{t.forest}</option></select></label>
                  <label className="setting-row"><div><strong>{t.accent}</strong><small>{draft.appearance.accentColor}</small></div><input className="color-input" type="color" value={draft.appearance.accentColor} onChange={(event) => updateAppearance({ ...draft.appearance, accentColor: event.target.value })} /></label>
                </div>
              )}
            </>
          )}

          {view?.credentialStoreError && <div className="settings-error">{t.credentialWarning}：{view.credentialStoreError}</div>}
          {error && <div className="settings-error">{error}</div>}
        </section>
      </div>
    </main>
  );
}

function Field({ label, wide = false, children }: { label: string; wide?: boolean; children: React.ReactNode }) {
  return <label className={`form-field ${wide ? "wide" : ""}`}><span>{label}</span>{children}</label>;
}

function AliasesInput({ value, onChange }: { value: string[]; onChange: (value: string[]) => void }) {
  const [text, setText] = useState(() => value.join(", "));
  return (
    <input
      value={text}
      onChange={(event) => {
        setText(event.target.value);
        onChange(aliasesFromText(event.target.value));
      }}
    />
  );
}

export default Settings;
