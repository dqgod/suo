import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  aliasesFromText,
  AppConfig,
  AppConfigView,
  applySettingsAppearance,
  loadAppConfig,
  resolveLauncherTheme,
  ScriptCommandConfig,
  ScriptRuntime,
  TranslationConfig,
  validateCommandIconImageDataUrl,
  WebSearchConfig,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import { SuoIcon } from "./SuoIcon";
import AppearanceEditor from "./AppearanceEditor";
import "./Settings.css";

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

const maximumQueryDebounceMs = 60_000;
const launcherWidthBounds = { minimum: 560, maximum: 1_200 } as const;
const launcherHeightBounds = { minimum: 320, maximum: 720 } as const;
const launcherHorizontalOffsetBounds = { minimum: -400, maximum: 400 } as const;
const launcherVerticalOffsetBounds = { minimum: -240, maximum: 240 } as const;
const shortcutModifierCodes = new Set([
  "AltLeft", "AltRight", "ControlLeft", "ControlRight", "MetaLeft", "MetaRight", "ShiftLeft", "ShiftRight",
]);
const shortcutNamedCodes = new Set([
  "Backquote", "Backslash", "BracketLeft", "BracketRight", "Pause", "Comma", "Equal", "Minus", "Period", "Quote", "Semicolon", "Slash",
  "Backspace", "CapsLock", "Enter", "Space", "Tab", "Delete", "End", "Home", "Insert", "PageDown", "PageUp", "PrintScreen", "ScrollLock",
  "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp", "NumLock", "NumpadAdd", "NumpadDecimal", "NumpadDivide", "NumpadEnter", "NumpadEqual",
  "NumpadMultiply", "NumpadSubtract", "AudioVolumeDown", "AudioVolumeUp", "AudioVolumeMute", "MediaPlay", "MediaPause", "MediaPlayPause",
  "MediaStop", "MediaTrackNext", "MediaTrackPrevious",
]);

function isSupportedShortcutCode(code: string) {
  return /^(?:Key[A-Z]|Digit[0-9]|Numpad[0-9]|F(?:[1-9]|1[0-9]|2[0-4]))$/.test(code)
    || shortcutNamedCodes.has(code);
}

function shortcutFromKeyboardEvent(event: {
  code: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}) {
  if (shortcutModifierCodes.has(event.code)) return { kind: "waiting" as const };
  if (!event.altKey && !event.ctrlKey && !event.metaKey) return { kind: "missing-modifier" as const };
  if (!isSupportedShortcutCode(event.code)) return { kind: "unsupported" as const };
  const parts: string[] = [];
  if (event.shiftKey) parts.push("shift");
  if (event.ctrlKey) parts.push("control");
  if (event.altKey) parts.push("alt");
  if (event.metaKey) parts.push("super");
  parts.push(event.code);
  return { kind: "shortcut" as const, value: parts.join("+") };
}

function displayShortcut(value: string, isMac: boolean) {
  const parts = value.split("+").map((part) => part.trim()).filter(Boolean);
  return parts.map((part) => {
    const token = part.toLowerCase();
    if (token === "control" || token === "ctrl") return "Ctrl";
    if (token === "alt" || token === "option") return isMac ? "Option" : "Alt";
    if (token === "shift") return "Shift";
    if (["super", "command", "cmd", "meta"].includes(token)) return isMac ? "Command" : "Win";
    if (/^key[a-z]$/i.test(part)) return part.slice(3).toUpperCase();
    if (/^digit[0-9]$/i.test(part)) return part.slice(5);
    return part;
  }).join(" + ");
}

function queryDebounceFromInput(value: string) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 0;
  return Math.min(maximumQueryDebounceMs, Math.max(0, Math.trunc(parsed)));
}

function boundedIntegerFromInput(value: string, current: number, minimum: number, maximum: number) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return current;
  return Math.min(maximum, Math.max(minimum, Math.trunc(parsed)));
}

type PixelRangeControlProps = {
  value: number;
  minimum: number;
  maximum: number;
  disabled: boolean;
  onChange: (value: number) => void;
};

function PixelRangeControl({ value, minimum, maximum, disabled, onChange }: PixelRangeControlProps) {
  const [numberDraft, setNumberDraft] = useState(String(value));
  useEffect(() => setNumberDraft(String(value)), [value]);
  const commitNumberDraft = () => {
    const next = boundedIntegerFromInput(numberDraft, value, minimum, maximum);
    setNumberDraft(String(next));
    if (next !== value) onChange(next);
  };
  return (
    <span className="pixel-range-control">
      <input
        type="range"
        min={minimum}
        max={maximum}
        step={1}
        disabled={disabled}
        value={value}
        onChange={(event) => {
          const next = Number(event.target.value);
          setNumberDraft(String(next));
          onChange(next);
        }}
      />
      <span className="pixel-number-input">
        <input
          type="number"
          min={minimum}
          max={maximum}
          step={1}
          disabled={disabled}
          value={numberDraft}
          onChange={(event) => setNumberDraft(event.target.value)}
          onBlur={commitNumberDraft}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
            if (event.key === "Escape") {
              setNumberDraft(String(value));
              event.currentTarget.blur();
            }
          }}
        />
        <span>{zhCN.pixels}</span>
      </span>
    </span>
  );
}

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

function settleEditorForNavigation(config: AppConfig, editor: EditorState): AppConfig {
  if (editor.original !== null) return applyEditor(config, editor);
  if (editor.kind === "script") {
    return {
      ...config,
      scriptCommands: config.scriptCommands.filter((command) => command.id !== editor.id),
    };
  }
  if (editor.kind === "web") {
    return {
      ...config,
      webSearches: config.webSearches.filter((search) => search.id !== editor.id),
    };
  }
  return config;
}

function Settings() {
  const [section, setSection] = useState<Section>("general");
  const [category, setCategory] = useState<ConfigurationCategory>("scripts");
  const [view, setView] = useState<AppConfigView | null>(null);
  const [draft, setDraftState] = useState<AppConfig | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [autoSaving, setAutoSaving] = useState(false);
  const [autoSaveNeedsRetry, setAutoSaveNeedsRetry] = useState(false);
  const [recordingHotkey, setRecordingHotkey] = useState(false);
  const [changingConfigLocation, setChangingConfigLocation] = useState(false);
  const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
  const draftRevisionRef = useRef(0);
  const draftRef = useRef<AppConfig | null>(null);
  const persistedSignatureRef = useRef("");
  const autoSaveDesiredRef = useRef<{ config: AppConfig; signature: string; revision: number } | null>(null);
  const autoSaveRevisionRef = useRef(0);
  const autoSaveBlockedRef = useRef(false);
  const autoSaveRunningRef = useRef(false);
  const statusTimerRef = useRef<number | null>(null);

  const setDraft = useCallback((update: AppConfig | null | ((current: AppConfig | null) => AppConfig | null)) => {
    const next = typeof update === "function" ? update(draftRef.current) : update;
    draftRef.current = next;
    setDraftState(next);
  }, []);

  const showStatus = useCallback((message: string) => {
    if (statusTimerRef.current !== null) window.clearTimeout(statusTimerRef.current);
    setStatus(message);
    statusTimerRef.current = window.setTimeout(() => {
      setStatus("");
      statusTimerRef.current = null;
    }, 1600);
  }, []);

  const close = useCallback(async () => {
    try {
      await invoke("hide_settings");
    } catch (closeError) {
      setError(String(closeError));
    }
  }, []);

  const queueAutoSave = useCallback((config: AppConfig) => {
    const request = {
      config,
      signature: JSON.stringify(config),
      revision: ++autoSaveRevisionRef.current,
    };
    autoSaveDesiredRef.current = request;
    autoSaveBlockedRef.current = false;
    setAutoSaveNeedsRetry(false);
    if (autoSaveRunningRef.current) return;

    const flush = async () => {
      autoSaveRunningRef.current = true;
      setAutoSaving(true);
      try {
        while (autoSaveDesiredRef.current && !autoSaveBlockedRef.current) {
          const currentRequest = autoSaveDesiredRef.current;
          if (currentRequest.signature === persistedSignatureRef.current) {
            if (autoSaveDesiredRef.current?.revision === currentRequest.revision) {
              autoSaveDesiredRef.current = null;
            }
            continue;
          }
          setError("");
          try {
            const next = await invoke<AppConfigView>("save_app_config", { config: currentRequest.config });
            const normalizedSignature = JSON.stringify(next.config);
            persistedSignatureRef.current = normalizedSignature;
            setView(next);
            const currentDraft = draftRef.current;
            if (currentDraft && JSON.stringify(currentDraft) === currentRequest.signature) {
              draftRef.current = next.config;
              setDraft(next.config);
              applySettingsAppearance(next.config.settingsTheme);
              showStatus(t.savedAutomatically);
            }
            const latest = autoSaveDesiredRef.current;
            if (
              latest?.revision === currentRequest.revision
              || latest?.signature === currentRequest.signature
              || latest?.signature === normalizedSignature
            ) {
              autoSaveDesiredRef.current = null;
            }
          } catch (saveError) {
            const latest = autoSaveDesiredRef.current;
            if (!latest || latest.revision === currentRequest.revision) {
              autoSaveBlockedRef.current = true;
              setAutoSaveNeedsRetry(true);
              setError(`${t.autoSaveFailed}：${String(saveError)}`);
            }
          }
        }
      } finally {
        autoSaveRunningRef.current = false;
        setAutoSaving(false);
      }
    };
    void flush();
  }, [showStatus]);

  const refresh = useCallback(async () => {
    try {
      const next = await loadAppConfig();
      persistedSignatureRef.current = JSON.stringify(next.config);
      autoSaveDesiredRef.current = null;
      autoSaveBlockedRef.current = false;
      setAutoSaveNeedsRetry(false);
      draftRef.current = next.config;
      setView(next);
      setDraft(next.config);
      setEditor(null);
      applySettingsAppearance(next.config.settingsTheme);
      if (next.configLoadWarning) setError(next.configLoadWarning);
    } catch (loadError) {
      setError(`${t.loadFailed}：${String(loadError)}`);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => () => {
    if (statusTimerRef.current !== null) window.clearTimeout(statusTimerRef.current);
  }, []);

  useEffect(() => {
    draftRevisionRef.current += 1;
  }, [draft, editor]);

  useEffect(() => {
    if (
      !draft
      || draft.saveSettingsManually
      || editor
      || saving
      || view?.configReadOnly
      || JSON.stringify(draft) === persistedSignatureRef.current
    ) return;
    const timer = window.setTimeout(() => queueAutoSave(draft), 180);
    return () => window.clearTimeout(timer);
  }, [draft, editor, queueAutoSave, saving, view?.configReadOnly]);

  useEffect(() => {
    if (!recordingHotkey) return;
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.key === "Escape" && !event.altKey && !event.ctrlKey && !event.metaKey) {
        setRecordingHotkey(false);
        return;
      }
      const captured = shortcutFromKeyboardEvent(event);
      if (captured.kind === "waiting") return;
      if (captured.kind === "missing-modifier") {
        setError(zhCN.hotkeyRequiresModifier);
        return;
      }
      if (captured.kind === "unsupported") {
        setError(zhCN.hotkeyUnsupportedKey);
        return;
      }
      setError("");
      setRecordingHotkey(false);
      setDraft((current) => current ? {
        ...current,
        launcher: { ...current.launcher, globalHotkey: captured.value },
      } : current);
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!hotkeyButtonRef.current?.contains(event.target as Node)) {
        setRecordingHotkey(false);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [recordingHotkey, setDraft]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (saving) return;
      if (recordingHotkey) {
        event.preventDefault();
        setRecordingHotkey(false);
        return;
      }
      if (editor) {
        event.preventDefault();
        cancelEditor();
      } else {
        void close();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close, editor, recordingHotkey, saving]);

  const save = async () => {
    if (!draft) return;
    const config = editor ? applyEditor(draft, editor) : draft;
    const startedAtRevision = draftRevisionRef.current;
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    setSaving(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("save_app_config", { config });
      persistedSignatureRef.current = JSON.stringify(next.config);
      autoSaveDesiredRef.current = null;
      autoSaveBlockedRef.current = false;
      setAutoSaveNeedsRetry(false);
      setView(next);
      if (draftRevisionRef.current === startedAtRevision) {
        draftRef.current = next.config;
        setDraft(next.config);
        setEditor(null);
        applySettingsAppearance(next.config.settingsTheme);
        showStatus(zhCN.saved);
      } else {
        showStatus(t.savedWithPendingChanges);
      }
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

  const openConfigDirectory = async () => {
    setError("");
    try {
      await invoke("open_config_directory");
    } catch (openError) {
      setError(String(openError));
    }
  };

  const relocateConfig = async (directory: string) => {
    setChangingConfigLocation(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("change_config_directory", { directory });
      persistedSignatureRef.current = JSON.stringify(next.config);
      setView(next);
      showStatus(t.configLocationChanged);
    } catch (locationError) {
      setError(String(locationError));
    } finally {
      setChangingConfigLocation(false);
    }
  };

  const chooseConfigDirectory = async () => {
    if (!view) return;
    setError("");
    try {
      const selected = await openDialog({
        title: t.chooseConfigDirectory,
        directory: true,
        multiple: false,
        defaultPath: view.configDirectory,
      });
      if (typeof selected === "string") await relocateConfig(selected);
    } catch (dialogError) {
      setError(String(dialogError));
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
    if (draft && editor) setDraft(settleEditorForNavigation(draft, editor));
    setEditor(null);
    setSection(next);
  };

  const changeCategory = (next: ConfigurationCategory) => {
    if (draft && editor) setDraft(settleEditorForNavigation(draft, editor));
    setEditor(null);
    setCategory(next);
  };

  const openScript = (command: ScriptCommandConfig) => {
    if (!draft) return;
    const nextDraft = editor ? settleEditorForNavigation(draft, editor) : draft;
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
    const nextDraft = editor ? settleEditorForNavigation(draft, editor) : draft;
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
    const nextDraft = editor ? settleEditorForNavigation(draft, editor) : draft;
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
    const nextDraft = editor ? settleEditorForNavigation(draft, editor) : draft;
    const command: ScriptCommandConfig = {
      id: createId("script"),
      name: t.newScript,
      keyword: nextAvailableKeyword(nextDraft, "cmd"),
      description: "",
      iconDataUrl: "",
      inputHint: "",
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
    const nextDraft = editor ? settleEditorForNavigation(draft, editor) : draft;
    const search: WebSearchConfig = {
      id: createId("web"),
      name: t.newWebSearch,
      keyword: nextAvailableKeyword(nextDraft, "web"),
      description: "",
      iconDataUrl: "",
      inputHint: "",
      aliases: [],
      enabled: true,
      urlTemplate: "https://example.com/search?q={query}",
    };
    setDraft({ ...nextDraft, webSearches: [...nextDraft.webSearches, search] });
    setCategory("web");
    setEditor({ kind: "web", id: search.id, original: null, value: cloneWebSearch(search) });
  };

  const removeScript = (command: ScriptCommandConfig) => {
    if (!draft || !window.confirm((draft.saveSettingsManually ? t.confirmRemove : t.confirmRemoveInstant).replace("{name}", command.name))) return;
    setDraft({
      ...draft,
      scriptCommands: draft.scriptCommands.filter((item) => item.id !== command.id),
    });
    setEditor(null);
  };

  const removeWebSearch = (search: WebSearchConfig) => {
    if (!draft || !window.confirm((draft.saveSettingsManually ? t.confirmRemove : t.confirmRemoveInstant).replace("{name}", search.name))) return;
    setDraft({
      ...draft,
      webSearches: draft.webSearches.filter((item) => item.id !== search.id),
    });
    setEditor(null);
  };

  const changeSaveMode = async (saveSettingsManually: boolean) => {
    if (!draft || editor || saving || autoSaving || view?.configReadOnly) return;
    const previous = draft;
    const next = { ...draft, saveSettingsManually };
    draftRef.current = next;
    setDraft(next);
    setSaving(true);
    setError("");
    try {
      const saved = await invoke<AppConfigView>("save_app_config", { config: next });
      persistedSignatureRef.current = JSON.stringify(saved.config);
      autoSaveDesiredRef.current = null;
      autoSaveBlockedRef.current = false;
      setAutoSaveNeedsRetry(false);
      draftRef.current = saved.config;
      setView(saved);
      setDraft(saved.config);
      applySettingsAppearance(saved.config.settingsTheme);
      showStatus(saveSettingsManually ? t.manualSaveEnabled : t.instantSaveEnabled);
    } catch (saveError) {
      draftRef.current = previous;
      setDraft(previous);
      setError(String(saveError));
    } finally {
      setSaving(false);
    }
  };

  const setScriptEnabled = (id: string, enabled: boolean) => {
    setDraft((current) => current ? {
      ...current,
      scriptCommands: current.scriptCommands.map((command) => (
        command.id === id ? { ...command, enabled } : command
      )),
    } : current);
    setEditor((current) => current?.kind === "script" && current.id === id
      ? { ...current, value: { ...current.value, enabled } }
      : current);
  };

  const setWebSearchEnabled = (id: string, enabled: boolean) => {
    setDraft((current) => current ? {
      ...current,
      webSearches: current.webSearches.map((search) => (
        search.id === id ? { ...search, enabled } : search
      )),
    } : current);
    setEditor((current) => current?.kind === "web" && current.id === id
      ? { ...current, value: { ...current.value, enabled } }
      : current);
  };

  const setTranslationEnabled = (enabled: boolean) => {
    setDraft((current) => current ? {
      ...current,
      translation: { ...current.translation, enabled },
    } : current);
    setEditor((current) => current?.kind === "translation"
      ? { ...current, value: { ...current.value, enabled } }
      : current);
  };

  const updateAppearanceThemes = async (themes: Pick<AppConfig, "launcherTheme" | "settingsTheme">) => {
    const current = draftRef.current;
    if (!current || view?.configReadOnly) return false;
    const nextConfig = { ...current, ...themes };
    if (current.saveSettingsManually) {
      setDraft(nextConfig);
      return true;
    }
    if (saving || autoSaving) return false;

    // Skin saves are an explicit product boundary even in instant-save mode.
    // Persist them directly and lock the editor until the normalized response
    // returns, so a delayed generic autosave cannot overwrite a second edit.
    setSaving(true);
    setError("");
    try {
      const saved = await invoke<AppConfigView>("save_app_config", { config: nextConfig });
      persistedSignatureRef.current = JSON.stringify(saved.config);
      autoSaveDesiredRef.current = null;
      autoSaveBlockedRef.current = false;
      setAutoSaveNeedsRetry(false);
      setView(saved);
      setDraft(saved.config);
      applySettingsAppearance(saved.config.settingsTheme);
      showStatus(t.savedAutomatically);
      return true;
    } catch (saveError) {
      setError(String(saveError));
      return false;
    } finally {
      setSaving(false);
    }
  };

  const isMac = /Mac/i.test(navigator.platform);

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
              {(status || autoSaving) && <span className="saved-indicator visible">{autoSaving ? t.savingAutomatically : status}</span>}
              {draft && (
                <label className="settings-save-mode">
                  <span><strong>{t.unifiedSave}</strong><small>{draft.saveSettingsManually ? t.unifiedSaveManual : t.unifiedSaveInstant}</small></span>
                  <input
                    className="switch"
                    type="checkbox"
                    checked={draft.saveSettingsManually}
                    disabled={saving || autoSaving || Boolean(editor) || view?.configReadOnly}
                    aria-label={t.unifiedSave}
                    onChange={(event) => void changeSaveMode(event.target.checked)}
                  />
                </label>
              )}
              {draft?.saveSettingsManually && (
                <button className="primary-button" type="button" disabled={saving || autoSaving || view?.configReadOnly} onClick={() => void save()}>
                  {saving ? t.saving : t.save}
                </button>
              )}
              {draft && !draft.saveSettingsManually && autoSaveNeedsRetry && (
                <button className="secondary-button" type="button" disabled={saving || autoSaving || view?.configReadOnly} onClick={() => queueAutoSave(draft)}>
                  {t.retrySave}
                </button>
              )}
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
                    <button
                      ref={hotkeyButtonRef}
                      className={`hotkey-recorder ${recordingHotkey ? "recording" : ""}`}
                      type="button"
                      disabled={saving || Boolean(view?.configReadOnly)}
                      aria-label={zhCN.globalHotkey}
                      onClick={() => {
                        setError("");
                        setRecordingHotkey(true);
                      }}
                    >
                      {recordingHotkey ? zhCN.hotkeyRecording : displayShortcut(draft.launcher.globalHotkey, isMac)}
                    </button>
                  </div>
                  <label className="setting-row">
                    <div><strong>{zhCN.closeOnBlur}</strong><small>{zhCN.closeOnBlurDescription}</small></div>
                    <input className="switch" type="checkbox" disabled={saving || Boolean(view?.configReadOnly)} checked={draft.launcher.closeOnBlur} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, closeOnBlur: event.target.checked } })} />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.keepLastInputSetting}</strong><small>{zhCN.keepLastInputDescription}</small></div>
                    <input className="switch" type="checkbox" disabled={saving || Boolean(view?.configReadOnly)} checked={draft.launcher.keepLastInput} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, keepLastInput: event.target.checked } })} />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.compactWhenEmpty}</strong><small>{zhCN.compactWhenEmptyDescription}</small></div>
                    <input className="switch" type="checkbox" disabled={saving || Boolean(view?.configReadOnly)} checked={draft.launcher.compactWhenEmpty} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, compactWhenEmpty: event.target.checked } })} />
                  </label>
                  {isMac && (
                    <label className="setting-row">
                      <div><strong>{zhCN.showDockIcon}</strong><small>{zhCN.showDockIconDescription}</small></div>
                      <input className="switch" type="checkbox" disabled={saving || Boolean(view?.configReadOnly)} checked={draft.launcher.showDockIcon} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, showDockIcon: event.target.checked } })} />
                    </label>
                  )}
                  <label className="setting-row">
                    <div><strong>{zhCN.launcherWindowWidth}</strong><small>{zhCN.launcherWindowWidthDescription}</small></div>
                    <PixelRangeControl
                      value={draft.launcher.windowWidthPx ?? resolveLauncherTheme(draft.launcherTheme).windowWidthPx}
                      minimum={launcherWidthBounds.minimum}
                      maximum={launcherWidthBounds.maximum}
                      disabled={saving || Boolean(view?.configReadOnly)}
                      onChange={(windowWidthPx) => setDraft({ ...draft, launcher: { ...draft.launcher, windowWidthPx } })}
                    />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.launcherWindowHeight}</strong><small>{zhCN.launcherWindowHeightDescription}</small></div>
                    <PixelRangeControl
                      value={draft.launcher.windowHeightPx}
                      minimum={launcherHeightBounds.minimum}
                      maximum={launcherHeightBounds.maximum}
                      disabled={saving || Boolean(view?.configReadOnly)}
                      onChange={(windowHeightPx) => setDraft({ ...draft, launcher: { ...draft.launcher, windowHeightPx } })}
                    />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.launcherHorizontalOffset}</strong><small>{zhCN.launcherHorizontalOffsetDescription}</small></div>
                    <PixelRangeControl
                      value={draft.launcher.horizontalOffsetPx}
                      minimum={launcherHorizontalOffsetBounds.minimum}
                      maximum={launcherHorizontalOffsetBounds.maximum}
                      disabled={saving || Boolean(view?.configReadOnly)}
                      onChange={(horizontalOffsetPx) => setDraft({ ...draft, launcher: { ...draft.launcher, horizontalOffsetPx } })}
                    />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.launcherVerticalOffset}</strong><small>{zhCN.launcherVerticalOffsetDescription}</small></div>
                    <PixelRangeControl
                      value={draft.launcher.verticalOffsetPx}
                      minimum={launcherVerticalOffsetBounds.minimum}
                      maximum={launcherVerticalOffsetBounds.maximum}
                      disabled={saving || Boolean(view?.configReadOnly)}
                      onChange={(verticalOffsetPx) => setDraft({ ...draft, launcher: { ...draft.launcher, verticalOffsetPx } })}
                    />
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.emptyQueryDebounce}</strong><small>{zhCN.emptyQueryDebounceDescription}</small></div>
                    <span className="millisecond-input">
                      <input
                        type="number"
                        min={0}
                        max={maximumQueryDebounceMs}
                        step={1}
                        disabled={saving || Boolean(view?.configReadOnly)}
                        value={draft.launcher.emptyQueryDebounceMs}
                        onChange={(event) => setDraft({
                          ...draft,
                          launcher: {
                            ...draft.launcher,
                            emptyQueryDebounceMs: queryDebounceFromInput(event.target.value),
                          },
                        })}
                      />
                      <span>{zhCN.milliseconds}</span>
                    </span>
                  </label>
                  <label className="setting-row">
                    <div><strong>{zhCN.nonEmptyQueryDebounce}</strong><small>{zhCN.nonEmptyQueryDebounceDescription}</small></div>
                    <span className="millisecond-input">
                      <input
                        type="number"
                        min={0}
                        max={maximumQueryDebounceMs}
                        step={1}
                        disabled={saving || Boolean(view?.configReadOnly)}
                        value={draft.launcher.nonEmptyQueryDebounceMs}
                        onChange={(event) => setDraft({
                          ...draft,
                          launcher: {
                            ...draft.launcher,
                            nonEmptyQueryDebounceMs: queryDebounceFromInput(event.target.value),
                          },
                        })}
                      />
                      <span>{zhCN.milliseconds}</span>
                    </span>
                  </label>
                  {view && (
                    <div className="setting-row config-location-row">
                      <div>
                        <strong>{t.configFileLocation}</strong>
                        <small>{t.configFileLocationDescription}</small>
                        {!view.usingDefaultConfigLocation && (
                          <small className="config-default-path">{t.defaultConfigLocation}：{view.defaultConfigFilePath}</small>
                        )}
                      </div>
                      <div className="config-location-control">
                        <code title={view.configFilePath}>{view.configFilePath}</code>
                        <span className="config-location-actions">
                          <button className="secondary-button" type="button" onClick={() => void openConfigDirectory()}>{t.openConfigDirectory}</button>
                          <button
                            className="secondary-button"
                            type="button"
                            disabled={saving || autoSaving || changingConfigLocation || Boolean(editor) || view.configReadOnly}
                            onClick={() => void chooseConfigDirectory()}
                          >
                            {changingConfigLocation ? t.changingConfigLocation : t.changeConfigLocation}
                          </button>
                          {(!view.usingDefaultConfigLocation || view.configLocationNeedsReset) && (
                            <button
                              className="secondary-button"
                              type="button"
                              disabled={saving || autoSaving || changingConfigLocation || Boolean(editor) || view.configReadOnly}
                              onClick={() => void relocateConfig(view.defaultConfigDirectory)}
                            >
                              {t.restoreDefaultConfigLocation}
                            </button>
                          )}
                        </span>
                      </div>
                    </div>
                  )}
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
                              onEnabledChange={(enabled) => setScriptEnabled(command.id, enabled)}
                              readOnly={Boolean(view?.configReadOnly)}
                            >
                              {activeEditor && (
                                <>
                                  <div className="configuration-editor-header">
                                    <span>{draft.saveSettingsManually ? t.pageDraftHint : t.itemInstantHint}</span>
                                  </div>
                                  <div className="form-grid">
                                    <Field label={t.name}><input value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <CommandIconField key={`script-icon-${activeEditor.id}`} value={activeEditor.value.iconDataUrl} disabled={Boolean(view?.configReadOnly)} onChange={(iconDataUrl) => setEditor({ ...activeEditor, value: { ...activeEditor.value, iconDataUrl } })} />
                                    <Field label={t.inputHint} wide><input maxLength={160} value={activeEditor.value.inputHint} placeholder={t.scriptInputHintPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, inputHint: event.target.value } })} /></Field>
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
                              onEnabledChange={(enabled) => setWebSearchEnabled(search.id, enabled)}
                              readOnly={Boolean(view?.configReadOnly)}
                            >
                              {activeEditor && (
                                <>
                                  <div className="configuration-editor-header">
                                    <span>{draft.saveSettingsManually ? t.pageDraftHint : t.itemInstantHint}</span>
                                  </div>
                                  <div className="form-grid">
                                    <Field label={t.name}><input value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <CommandIconField key={`web-icon-${activeEditor.id}`} value={activeEditor.value.iconDataUrl} disabled={Boolean(view?.configReadOnly)} onChange={(iconDataUrl) => setEditor({ ...activeEditor, value: { ...activeEditor.value, iconDataUrl } })} />
                                    <Field label={t.inputHint} wide><input maxLength={160} value={activeEditor.value.inputHint} placeholder={t.webInputHintPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, inputHint: event.target.value } })} /></Field>
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
                          onEnabledChange={setTranslationEnabled}
                          readOnly={Boolean(view?.configReadOnly)}
                        >
                          {activeEditor && (
                            <>
                              <div className="configuration-editor-header">
                                <span>{draft.saveSettingsManually ? t.pageDraftHint : t.itemInstantHint}</span>
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
                  <p className="configuration-hint">{draft.saveSettingsManually ? t.configurationHint : t.configurationHintInstant}</p>
                </div>
              )}

              {section === "appearance" && (
                <AppearanceEditor
                  launcherTheme={draft.launcherTheme}
                  settingsTheme={draft.settingsTheme}
                  onChange={updateAppearanceThemes}
                  saveSettingsManually={draft.saveSettingsManually}
                  readOnly={Boolean(view?.configReadOnly)}
                  saving={saving || autoSaving}
                />
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
  onEnabledChange,
  readOnly,
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
  onEnabledChange: (enabled: boolean) => void;
  readOnly: boolean;
  children: React.ReactNode;
}) {
  return (
    <article className={`configuration-item ${open ? "open" : ""} ${enabled ? "" : "disabled"}`}>
      <div className="configuration-summary">
        <button className="configuration-summary-main" type="button" aria-expanded={open} aria-controls={panelId} onClick={onToggle}>
          <span className={`configuration-status-dot ${enabled ? "" : "off"}`} aria-hidden="true" />
          <span className="configuration-summary-copy">
            <span className="configuration-title-line"><strong>{name || t.unnamed}</strong><code>{keyword || "—"}</code></span>
            <span className={`configuration-description ${description ? "" : "empty"}`}>{description || t.noDescription}</span>
          </span>
          <span className="configuration-badges">{badges.map((badge) => <span className="configuration-badge" key={badge}>{badge}</span>)}</span>
        </button>
        <label className="configuration-enable-switch">
          <input
            type="checkbox"
            checked={enabled}
            disabled={readOnly}
            aria-label={(enabled ? t.disableItem : t.enableItem).replace("{name}", name || t.unnamed)}
            onChange={(event) => onEnabledChange(event.target.checked)}
          />
          <span aria-hidden="true" />
        </label>
        <button className="configuration-chevron-button" type="button" aria-expanded={open} aria-controls={panelId} aria-label={open ? t.collapseItem : t.expandItem} onClick={onToggle}>
          <span className="configuration-chevron" aria-hidden="true">⌄</span>
        </button>
      </div>
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

function CommandIconField({
  value,
  disabled,
  onChange,
}: {
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const requestRef = useRef(0);
  const [notice, setNotice] = useState("");

  useEffect(() => () => {
    requestRef.current += 1;
  }, []);

  const loadIcon = (file: File) => {
    const request = ++requestRef.current;
    setNotice("");
    if (!/^image\/(?:png|jpeg|webp)$/.test(file.type) || file.size > 256 * 1024) {
      setNotice(t.commandIconInvalid);
      return;
    }
    const reader = new FileReader();
    reader.onload = async () => {
      if (requestRef.current !== request) return;
      const dataUrl = String(reader.result);
      try {
        await validateCommandIconImageDataUrl(dataUrl);
      } catch {
        if (requestRef.current === request) setNotice(t.commandIconInvalid);
        return;
      }
      if (requestRef.current !== request) return;
      onChange(dataUrl);
      setNotice("");
    };
    reader.onerror = () => {
      if (requestRef.current === request) setNotice(t.commandIconReadFailed);
    };
    reader.readAsDataURL(file);
  };

  return (
    <div className="form-field wide command-icon-field">
      <span>{t.commandIcon}</span>
      <div className="command-icon-control">
        <span className={`command-icon-preview ${value ? "loaded" : ""}`} aria-hidden="true">
          {value ? <img src={value} alt="" draggable={false} /> : "?"}
        </span>
        <span className="command-icon-copy">
          <strong>{value ? t.commandIconLoaded : t.commandIconEmpty}</strong>
          <small>{notice || t.commandIconRequirements}</small>
        </span>
        <span className="command-icon-actions">
          <input
            ref={inputRef}
            className="command-icon-file-input"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            disabled={disabled}
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) loadIcon(file);
              event.currentTarget.value = "";
            }}
          />
          <button className="secondary-button" type="button" disabled={disabled} onClick={() => inputRef.current?.click()}>{t.chooseCommandIcon}</button>
          {value && <button className="secondary-button" type="button" disabled={disabled} onClick={() => { requestRef.current += 1; setNotice(""); onChange(""); }}>{t.removeCommandIcon}</button>}
        </span>
      </div>
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
