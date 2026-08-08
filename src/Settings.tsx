import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  aliasesFromText,
  AppConfig,
  AppConfigView,
  applyAppearance,
  loadAppConfig,
  ScriptCommandConfig,
  ScriptRuntime,
  TranslationConfig,
  WebSearchConfig,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import { SuoIcon } from "./SuoIcon";
import "./Settings.css";

const settingsWindow = getCurrentWindow();
const t = zhCN.settings;
type Section = "general" | "search" | "configuration" | "appearance";
type ConfigurationCategory = "scripts" | "web" | "services";

type EditorState =
  | {
      kind: "script";
      id: string;
      original: ScriptCommandConfig | null;
      value: ScriptCommandConfig;
    }
  | {
      kind: "web";
      id: string;
      original: WebSearchConfig | null;
      value: WebSearchConfig;
    }
  | {
      kind: "translation";
      id: "translation";
      original: TranslationConfig;
      value: TranslationConfig;
    };

const sectionCopy: Record<Section, { title: string; description: string }> = {
  general: { title: zhCN.general, description: t.generalDescription },
  search: { title: zhCN.searchAndIndex, description: t.searchDescription },
  configuration: {
    title: t.commandsAndServices,
    description: t.configurationDescription,
  },
  appearance: { title: zhCN.appearance, description: t.appearanceDescription },
};

const runtimeLabels: Record<ScriptRuntime, string> = {
  python: "Python",
  powerShell: "PowerShell",
  bash: "Bash",
  executable: "Executable",
};

function createId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function nextAvailableKeyword(config: AppConfig, prefix: string) {
  const used = new Set<string>([
    config.translation.keyword,
    ...config.translation.aliases,
    ...config.scriptCommands.flatMap((command) => [command.keyword, ...command.aliases]),
    ...config.webSearches.flatMap((search) => [search.keyword, ...search.aliases]),
  ].map((value) => value.toLowerCase()));
  let suffix = 1;
  while (used.has(`${prefix}${suffix}`)) suffix += 1;
  return `${prefix}${suffix}`;
}

function cloneScript(command: ScriptCommandConfig): ScriptCommandConfig {
  return { ...command, aliases: [...command.aliases] };
}

function cloneWebSearch(search: WebSearchConfig): WebSearchConfig {
  return { ...search, aliases: [...search.aliases] };
}

function cloneTranslation(translation: TranslationConfig): TranslationConfig {
  return { ...translation, aliases: [...translation.aliases] };
}

function applyEditor(config: AppConfig, editor: EditorState): AppConfig {
  if (editor.kind === "translation") {
    return { ...config, translation: cloneTranslation(editor.value) };
  }
  if (editor.kind === "script") {
    const value = cloneScript(editor.value);
    const found = config.scriptCommands.some((command) => command.id === editor.id);
    return {
      ...config,
      scriptCommands: found
        ? config.scriptCommands.map((command) => command.id === editor.id ? value : command)
        : [...config.scriptCommands, value],
    };
  }
  const value = cloneWebSearch(editor.value);
  const found = config.webSearches.some((search) => search.id === editor.id);
  return {
    ...config,
    webSearches: found
      ? config.webSearches.map((search) => search.id === editor.id ? value : search)
      : [...config.webSearches, value],
  };
}

function Settings() {
  const [section, setSection] = useState<Section>("general");
  const [category, setCategory] = useState<ConfigurationCategory>("scripts");
  const [view, setView] = useState<AppConfigView | null>(null);
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const draftRevisionRef = useRef(0);

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
      setEditor(null);
      applyAppearance(next.config.appearance);
      if (next.configLoadWarning) setError(next.configLoadWarning);
    } catch (loadError) {
      setError(`${t.loadFailed}：${String(loadError)}`);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    draftRevisionRef.current += 1;
  }, [draft, editor]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (saving) return;
      if (editor) {
        event.preventDefault();
        cancelEditor();
      } else {
        void close();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close, editor, saving]);

  const save = async () => {
    if (!draft) return;
    const config = editor ? applyEditor(draft, editor) : draft;
    const startedAtRevision = draftRevisionRef.current;
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    setSaving(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("save_app_config", { config });
      setView(next);
      if (draftRevisionRef.current === startedAtRevision) {
        setDraft(next.config);
        setEditor(null);
        applyAppearance(next.config.appearance);
        setStatus(zhCN.saved);
      } else {
        setStatus(t.savedWithPendingChanges);
      }
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
    if (!window.confirm(t.confirmClearApiKey)) return;
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

  const revealScript = async (configuredPath: string) => {
    setError("");
    try {
      await invoke("reveal_script_in_folder", { configuredPath });
    } catch (revealError) {
      setError(String(revealError));
    }
  };

  const commitEditor = () => {
    if (!draft || !editor) return;
    setDraft(applyEditor(draft, editor));
    setEditor(null);
  };

  function cancelEditor() {
    if (!editor) return;
    if (editor.original === null) {
      setDraft((current) => {
        if (!current) return current;
        if (editor.kind === "script") {
          return {
            ...current,
            scriptCommands: current.scriptCommands.filter((command) => command.id !== editor.id),
          };
        }
        if (editor.kind === "web") {
          return {
            ...current,
            webSearches: current.webSearches.filter((search) => search.id !== editor.id),
          };
        }
        return current;
      });
    }
    setEditor(null);
  }

  const changeSection = (next: Section) => {
    if (draft && editor) setDraft(applyEditor(draft, editor));
    setEditor(null);
    setSection(next);
  };

  const changeCategory = (next: ConfigurationCategory) => {
    if (draft && editor) setDraft(applyEditor(draft, editor));
    setEditor(null);
    setCategory(next);
  };

  const openScript = (command: ScriptCommandConfig) => {
    if (!draft) return;
    const nextDraft = editor ? applyEditor(draft, editor) : draft;
    setDraft(nextDraft);
    if (editor?.kind === "script" && editor.id === command.id) {
      setEditor(null);
      return;
    }
    const next = nextDraft.scriptCommands.find((item) => item.id === command.id) ?? command;
    const original = cloneScript(next);
    setEditor({ kind: "script", id: command.id, original, value: cloneScript(next) });
  };

  const openWebSearch = (search: WebSearchConfig) => {
    if (!draft) return;
    const nextDraft = editor ? applyEditor(draft, editor) : draft;
    setDraft(nextDraft);
    if (editor?.kind === "web" && editor.id === search.id) {
      setEditor(null);
      return;
    }
    const next = nextDraft.webSearches.find((item) => item.id === search.id) ?? search;
    const original = cloneWebSearch(next);
    setEditor({ kind: "web", id: search.id, original, value: cloneWebSearch(next) });
  };

  const openTranslation = () => {
    if (!draft) return;
    const nextDraft = editor ? applyEditor(draft, editor) : draft;
    setDraft(nextDraft);
    if (editor?.kind === "translation") {
      setEditor(null);
      return;
    }
    const original = cloneTranslation(nextDraft.translation);
    setEditor({
      kind: "translation",
      id: "translation",
      original,
      value: cloneTranslation(original),
    });
  };

  const addScript = () => {
    if (!draft) return;
    const nextDraft = editor ? applyEditor(draft, editor) : draft;
    const command: ScriptCommandConfig = {
      id: createId("script"),
      name: t.newScript,
      keyword: nextAvailableKeyword(nextDraft, "cmd"),
      description: "",
      aliases: [],
      enabled: true,
      runtime: "python",
      scriptPath: "",
      immediate: false,
      debounceMs: 50,
      timeoutMs: 3000,
    };
    setDraft({ ...nextDraft, scriptCommands: [...nextDraft.scriptCommands, command] });
    setCategory("scripts");
    setEditor({ kind: "script", id: command.id, original: null, value: cloneScript(command) });
  };

  const addWebSearch = () => {
    if (!draft) return;
    const nextDraft = editor ? applyEditor(draft, editor) : draft;
    const search: WebSearchConfig = {
      id: createId("web"),
      name: t.newWebSearch,
      keyword: nextAvailableKeyword(nextDraft, "web"),
      description: "",
      aliases: [],
      enabled: true,
      urlTemplate: "https://example.com/search?q={query}",
    };
    setDraft({ ...nextDraft, webSearches: [...nextDraft.webSearches, search] });
    setCategory("web");
    setEditor({ kind: "web", id: search.id, original: null, value: cloneWebSearch(search) });
  };

  const removeScript = (command: ScriptCommandConfig) => {
    if (!draft || !window.confirm(t.confirmRemove.replace("{name}", command.name))) return;
    setDraft({
      ...draft,
      scriptCommands: draft.scriptCommands.filter((item) => item.id !== command.id),
    });
    setEditor(null);
  };

  const removeWebSearch = (search: WebSearchConfig) => {
    if (!draft || !window.confirm(t.confirmRemove.replace("{name}", search.name))) return;
    setDraft({
      ...draft,
      webSearches: draft.webSearches.filter((item) => item.id !== search.id),
    });
    setEditor(null);
  };

  const updateAppearance = (appearance: AppConfig["appearance"]) => {
    applyAppearance(appearance);
    setDraft((current) => (current ? { ...current, appearance } : current));
  };

  const hotkey = /Mac/i.test(navigator.platform) ? "Command + Space" : "Alt + Space";

  return (
    <main className="settings-stage">
      <header className="settings-titlebar" data-tauri-drag-region>
        <div className="settings-brand" data-tauri-drag-region>
          <SuoIcon className="settings-brand-icon" />
          <strong data-tauri-drag-region>{zhCN.settingsTitle}</strong>
        </div>
        <button type="button" onClick={() => void close()} aria-label={zhCN.closeSettings}>×</button>
      </header>

      <div
        className={`settings-layout ${saving ? "saving" : ""}`}
        inert={saving}
        onInputCapture={(event) => {
          if (!(event.target as HTMLElement).closest("[data-independent-config]")) {
            draftRevisionRef.current += 1;
          }
        }}
      >
        <aside>
          {(Object.keys(sectionCopy) as Section[]).map((key) => (
            <button
              type="button"
              key={key}
              className={section === key ? "settings-nav-active" : ""}
              onClick={() => changeSection(key)}
            >
              {sectionCopy[key].title}
            </button>
          ))}
        </aside>

        <section className="settings-content" aria-busy={saving}>
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
                  <label className="setting-row">
                    <div><strong>{zhCN.compactWhenEmpty}</strong><small>{zhCN.compactWhenEmptyDescription}</small></div>
                    <input className="switch" type="checkbox" checked={draft.launcher.compactWhenEmpty} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, compactWhenEmpty: event.target.checked } })} />
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

              {section === "configuration" && (
                <div className="configuration-section">
                  <div className="configuration-toolbar">
                    <div className="configuration-tabs" role="tablist" aria-label={t.commandsAndServices}>
                      <button type="button" role="tab" aria-selected={category === "scripts"} className={category === "scripts" ? "active" : ""} onClick={() => changeCategory("scripts")}>{t.scriptsTab}</button>
                      <button type="button" role="tab" aria-selected={category === "web"} className={category === "web" ? "active" : ""} onClick={() => changeCategory("web")}>{t.webTab}</button>
                      <button type="button" role="tab" aria-selected={category === "services"} className={category === "services" ? "active" : ""} onClick={() => changeCategory("services")}>{t.servicesTab}</button>
                    </div>
                    {category === "scripts" && <button className="add-button" type="button" onClick={addScript}>＋ {t.addScript}</button>}
                    {category === "web" && <button className="add-button" type="button" onClick={addWebSearch}>＋ {t.addWeb}</button>}
                  </div>

                  <div className="configuration-list" role="tabpanel">
                    {category === "scripts" && (
                      <>
                        {draft.scriptCommands.length === 0 && <div className="configuration-empty">{t.emptyScripts}</div>}
                        {draft.scriptCommands.map((command) => {
                          const activeEditor = editor?.kind === "script" && editor.id === command.id ? editor : null;
                          const summary = activeEditor?.value ?? command;
                          return (
                            <ConfigurationItem
                              key={command.id}
                              panelId={`script-editor-${command.id}`}
                              open={Boolean(activeEditor)}
                              enabled={summary.enabled}
                              name={summary.name}
                              keyword={summary.keyword}
                              description={summary.description}
                              badges={[runtimeLabels[summary.runtime], summary.immediate ? `${summary.debounceMs} ms ${t.immediateBadge}` : t.enterBadge]}
                              onToggle={() => openScript(command)}
                            >
                              {activeEditor && (
                                <>
                                  <div className="configuration-editor-header">
                                    <label className="inline-toggle"><input type="checkbox" checked={activeEditor.value.enabled} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, enabled: event.target.checked } })} />{t.enabled}</label>
                                    <span>{t.pageDraftHint}</span>
                                  </div>
                                  <div className="form-grid">
                                    <Field label={t.name}><input value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <Field label={t.aliases}><AliasesInput key={`script-aliases-${activeEditor.id}`} value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                    <Field label={t.runtime}><select value={activeEditor.value.runtime} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, runtime: event.target.value as ScriptRuntime } })}><option value="python">Python</option><option value="powerShell">PowerShell</option><option value="bash">Bash</option><option value="executable">Executable</option></select></Field>
                                    <div className="form-field wide">
                                      <span id={`script-path-label-${activeEditor.id}`}>{t.scriptPath}</span>
                                      <div className="script-path-row">
                                        <input aria-labelledby={`script-path-label-${activeEditor.id}`} value={activeEditor.value.scriptPath} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, scriptPath: event.target.value } })} placeholder="D:\\scripts\\tool.py" />
                                        <button className="secondary-button reveal-script-button" type="button" disabled={!activeEditor.value.scriptPath.trim()} onClick={() => void revealScript(activeEditor.value.scriptPath)}>{t.revealScript}</button>
                                      </div>
                                    </div>
                                    <Field label={t.timeout}><input type="number" min={100} max={60000} value={activeEditor.value.timeoutMs} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, timeoutMs: Number(event.target.value) } })} /></Field>
                                    <Field label={t.executionMode}><select value={activeEditor.value.immediate ? "immediate" : "enter"} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, immediate: event.target.value === "immediate" } })}><option value="enter">{t.enterMode}</option><option value="immediate">{t.immediateMode}</option></select></Field>
                                    {activeEditor.value.immediate && <Field label={t.debounce}><input type="number" min={20} max={60000} value={activeEditor.value.debounceMs} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, debounceMs: Number(event.target.value) } })} /></Field>}
                                  </div>
                                  <EditorActions onRemove={() => removeScript(activeEditor.value)} onCancel={cancelEditor} onDone={commitEditor} />
                                </>
                              )}
                            </ConfigurationItem>
                          );
                        })}
                      </>
                    )}

                    {category === "web" && (
                      <>
                        {draft.webSearches.length === 0 && <div className="configuration-empty">{t.emptyWebSearches}</div>}
                        {draft.webSearches.map((search) => {
                          const activeEditor = editor?.kind === "web" && editor.id === search.id ? editor : null;
                          const summary = activeEditor?.value ?? search;
                          return (
                            <ConfigurationItem
                              key={search.id}
                              panelId={`web-editor-${search.id}`}
                              open={Boolean(activeEditor)}
                              enabled={summary.enabled}
                              name={summary.name}
                              keyword={summary.keyword}
                              description={summary.description}
                              badges={[t.browserBadge]}
                              onToggle={() => openWebSearch(search)}
                            >
                              {activeEditor && (
                                <>
                                  <div className="configuration-editor-header">
                                    <label className="inline-toggle"><input type="checkbox" checked={activeEditor.value.enabled} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, enabled: event.target.checked } })} />{t.enabled}</label>
                                    <span>{t.pageDraftHint}</span>
                                  </div>
                                  <div className="form-grid">
                                    <Field label={t.name}><input value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <Field label={t.aliases}><AliasesInput key={`web-aliases-${activeEditor.id}`} value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                    <Field label={t.urlTemplate} wide><input value={activeEditor.value.urlTemplate} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, urlTemplate: event.target.value } })} /></Field>
                                  </div>
                                  <EditorActions onRemove={() => removeWebSearch(activeEditor.value)} onCancel={cancelEditor} onDone={commitEditor} />
                                </>
                              )}
                            </ConfigurationItem>
                          );
                        })}
                      </>
                    )}

                    {category === "services" && (() => {
                      const activeEditor = editor?.kind === "translation" ? editor : null;
                      const summary = activeEditor?.value ?? draft.translation;
                      return (
                        <ConfigurationItem
                          panelId="translation-editor"
                          open={Boolean(activeEditor)}
                          enabled={summary.enabled}
                          name={t.translation}
                          keyword={summary.keyword}
                          description={summary.description}
                          badges={[t.microsoftProvider, view?.translationApiKeyConfigured ? t.apiKeyConfiguredBadge : t.apiKeyMissingBadge]}
                          onToggle={openTranslation}
                        >
                          {activeEditor && (
                            <>
                              <div className="configuration-editor-header">
                                <label className="inline-toggle"><input type="checkbox" checked={activeEditor.value.enabled} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, enabled: event.target.checked } })} />{t.enabled}</label>
                                <span>{t.pageDraftHint}</span>
                              </div>
                              <div className="form-grid">
                                <Field label={t.keyword}><input value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                <Field label={t.aliases}><AliasesInput key="translation-aliases" value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                <Field label={t.region}><input value={activeEditor.value.region} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, region: event.target.value } })} placeholder="eastasia" /></Field>
                                <Field label={t.defaultTarget}><input value={activeEditor.value.defaultTargetLanguage} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, defaultTargetLanguage: event.target.value } })} /></Field>
                                <Field label={t.chineseTarget}><input value={activeEditor.value.chineseTargetLanguage} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, chineseTargetLanguage: event.target.value } })} /></Field>
                                <Field label={t.apiKey} wide><div className="credential-row" data-independent-config><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={t.apiKeyPlaceholder} /><button className="secondary-button" type="button" disabled={!apiKey.trim()} onClick={() => void saveApiKey()}>{t.saveApiKey}</button><button className="danger-button" type="button" disabled={!view?.translationApiKeyConfigured} onClick={() => void clearApiKey()}>{t.clearApiKey}</button></div><small className={view?.translationApiKeyConfigured ? "credential-ok" : "credential-missing"}>{view?.translationApiKeyConfigured ? t.apiKeyConfigured : t.apiKeyMissing}</small></Field>
                              </div>
                              <EditorActions onCancel={cancelEditor} onDone={commitEditor} />
                            </>
                          )}
                        </ConfigurationItem>
                      );
                    })()}
                  </div>
                  <p className="configuration-hint">{t.configurationHint}</p>
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

function ConfigurationItem({
  panelId,
  open,
  enabled,
  name,
  keyword,
  description,
  badges,
  onToggle,
  children,
}: {
  panelId: string;
  open: boolean;
  enabled: boolean;
  name: string;
  keyword: string;
  description: string;
  badges: string[];
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <article className={`configuration-item ${open ? "open" : ""}`}>
      <button className="configuration-summary" type="button" aria-expanded={open} aria-controls={panelId} onClick={onToggle}>
        <span className={`configuration-status-dot ${enabled ? "" : "off"}`} aria-hidden="true" />
        <span className="configuration-summary-copy">
          <span className="configuration-title-line"><strong>{name || t.unnamed}</strong><code>{keyword || "—"}</code></span>
          <span className={`configuration-description ${description ? "" : "empty"}`}>{description || t.noDescription}</span>
        </span>
        <span className="configuration-badges">{badges.map((badge) => <span className="configuration-badge" key={badge}>{badge}</span>)}</span>
        <span className="configuration-chevron" aria-hidden="true">⌄</span>
      </button>
      {open && <div className="configuration-editor" id={panelId}>{children}</div>}
    </article>
  );
}

function EditorActions({ onRemove, onCancel, onDone }: { onRemove?: () => void; onCancel: () => void; onDone: () => void }) {
  return (
    <div className="editor-actions">
      <div>{onRemove && <button className="danger-button" type="button" onClick={onRemove}>{t.remove}</button>}</div>
      <div className="editor-actions-right"><button className="secondary-button" type="button" onClick={onCancel}>{t.cancel}</button><button className="primary-button" type="button" onClick={onDone}>{t.completeEdit}</button></div>
    </div>
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
