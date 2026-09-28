import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import SettingsDialog from "./SettingsDialog";
import { settingsRedesign as ui } from "./i18n/settings-redesign";
import {
  aliasesFromText,
  AppConfig,
  AppConfigView,
  applySettingsAppearance,
  loadAppConfig,
  resolveLauncherTheme,
  resolveSettingsTheme,
  ScriptCommandConfig,
  ScriptResultAction,
  ScriptRuntime,
  TerminalCommandConfig,
  TranslationConfig,
  TranslationProvider,
  validateCommandIconImageDataUrl,
  WebSearchConfig,
  SettingsIconStyle,
  visualBounds,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import { SuoIcon } from "./SuoIcon";
import AppearanceEditor from "./AppearanceEditor";
import "./Settings.css";

const t = zhCN.settings;
function validateVisibleFields(form: HTMLFormElement | null) {
  if (!form) return false;
  for (const field of form.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea")) {
    if (field.getClientRects().length && !field.disabled && !field.reportValidity()) return false;
  }
  return true;
}
type Section = "general" | "search" | "configuration" | "appearance";
type ConfigurationCategory = "builtins" | "scripts" | "web" | "services";

type EditorState =
  | {
      kind: "terminal";
      id: "terminal";
      original: TerminalCommandConfig;
      value: TerminalCommandConfig;
    }
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

type PendingRemoval = {
  kind: "script" | "web";
  id: string;
  name: string;
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

const scriptResultActionLabels: Record<ScriptResultAction, string> = {
  copy: t.copyScriptResultBadge,
  executeShell: t.executeShellResultBadge,
};

const translationProviderLabels: Record<TranslationProvider, string> = {
  microsoft: t.microsoftProvider,
  google: t.googleProvider,
  youdao: t.youdaoProvider,
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

type PixelRangeControlProps = {
  value: number;
  minimum: number;
  maximum: number;
  legalMinimum: number;
  legalMaximum: number;
  label: string;
  disabled: boolean;
  onChange: (value: number) => void;
};

function PixelRangeControl({ value, minimum, maximum, legalMinimum, legalMaximum, label, disabled, onChange }: PixelRangeControlProps) {
  const [numberDraft, setNumberDraft] = useState(String(value));
  useEffect(() => { setNumberDraft(String(value)); numberRef.current?.setCustomValidity(""); }, [value]);
  const numberRef = useRef<HTMLInputElement>(null);
  const commitNumberDraft = () => {
    const next = Number(numberDraft);
    const valid = numberDraft.trim() !== "" && Number.isInteger(next) && next >= legalMinimum && next <= legalMaximum;
    numberRef.current?.setCustomValidity(valid ? "" : ui.numberRange.replace("{min}", String(legalMinimum)).replace("{max}", String(legalMaximum)));
    if (valid && next !== value) onChange(next);
  };
  return (
    <span className="pixel-range-control">
      <input
        type="range"
        aria-label={label}
        title={ui.comfortableRange}
        min={minimum}
        max={maximum}
        step={1}
        disabled={disabled}
        value={value}
        onChange={(event) => {
          const next = Number(event.target.value);
          setNumberDraft(String(next));
          numberRef.current?.setCustomValidity("");
          onChange(next);
        }}
      />
      <span className="pixel-number-input">
        <input
          ref={numberRef}
          aria-label={`${label} (${zhCN.pixels})`}
          type="number"
          min={legalMinimum}
          max={legalMaximum}
          step={1}
          disabled={disabled}
          value={numberDraft}
          onChange={(event) => { setNumberDraft(event.target.value); event.target.setCustomValidity(""); }}
          onBlur={commitNumberDraft}
          onKeyDown={(event) => {
            if (event.key === "Enter") { event.preventDefault(); event.currentTarget.blur(); }
            if (event.key === "Escape") {
              event.stopPropagation();
              setNumberDraft(String(value));
              event.currentTarget.setCustomValidity("");
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

function cloneTerminal(terminal: TerminalCommandConfig): TerminalCommandConfig {
  return { ...terminal };
}

function applyEditor(config: AppConfig, editor: EditorState): AppConfig {
  if (editor.kind === "terminal") {
    return {
      ...config,
      launcher: { ...config.launcher, terminal: cloneTerminal(editor.value) },
    };
  }
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
  const [category, setCategory] = useState<ConfigurationCategory>("builtins");
  const [view, setView] = useState<AppConfigView | null>(null);
  const [draft, setDraftState] = useState<AppConfig | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [youdaoAppKey, setYoudaoAppKey] = useState("");
  const [youdaoAppSecret, setYoudaoAppSecret] = useState("");
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [autoSaving, setAutoSaving] = useState(false);
  const [autoSaveNeedsRetry, setAutoSaveNeedsRetry] = useState(false);
  const [recordingHotkey, setRecordingHotkey] = useState(false);
  const [changingConfigLocation, setChangingConfigLocation] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<PendingRemoval | null>(null);
  const [appearanceDirty, setAppearanceDirty] = useState(false);
  const [appearanceResetToken, setAppearanceResetToken] = useState(0);
  const [detailTarget, setDetailTarget] = useState<HTMLDivElement | null>(null);
  const [commandFilter, setCommandFilter] = useState("");
  const [pendingAction, setPendingAction] = useState<{ kind: "switch" | "close" | "discard"; run: () => void } | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const hotkeyButtonRef = useRef<HTMLButtonElement | null>(null);
  const draftRevisionRef = useRef(0);
  const draftRef = useRef<AppConfig | null>(null);
  const persistedSignatureRef = useRef("");
  const autoSaveDesiredRef = useRef<{ config: AppConfig; signature: string; revision: number } | null>(null);
  const autoSaveRevisionRef = useRef(0);
  const autoSaveBlockedRef = useRef(false);
  const autoSaveRunningRef = useRef(false);
  const statusTimerRef = useRef<number | null>(null);
  const hotkeyOperationRef = useRef<Promise<void>>(Promise.resolve());
  const hotkeyCleanupErrorRef = useRef(false);
  const hotkeyCapturePendingRef = useRef(false);

  const editorDirty = Boolean(editor && (editor.original === null || JSON.stringify(editor.value) !== JSON.stringify(editor.original)));
  const configDirty = Boolean(draft && JSON.stringify(draft) !== persistedSignatureRef.current);
  const credentialsDirty = Boolean(apiKey || youdaoAppKey || youdaoAppSecret);
  const hasUnsaved = configDirty || editorDirty || appearanceDirty || credentialsDirty;

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

  const changeHotkeyRecordingBackend = useCallback((recording: boolean) => {
    const operation = hotkeyOperationRef.current
      .catch(() => undefined)
      .then(() => invoke<void>("set_hotkey_recording", { recording }));
    hotkeyOperationRef.current = operation.catch(() => undefined);
    return operation;
  }, []);

  const stopHotkeyRecording = useCallback(async () => {
    setRecordingHotkey(false);
    try {
      await changeHotkeyRecordingBackend(false);
      if (hotkeyCleanupErrorRef.current) {
        hotkeyCleanupErrorRef.current = false;
        setError("");
      }
      return true;
    } catch (recordingError) {
      hotkeyCleanupErrorRef.current = true;
      setRecordingHotkey(true);
      setError(String(recordingError));
      return false;
    }
  }, [changeHotkeyRecordingBackend]);

  const beginHotkeyRecording = useCallback(async () => {
    setError("");
    hotkeyCapturePendingRef.current = false;
    try {
      await changeHotkeyRecordingBackend(true);
      setRecordingHotkey(true);
    } catch (recordingError) {
      setError(String(recordingError));
    }
  }, [changeHotkeyRecordingBackend]);

  const finishHotkeyCapture = useCallback((value: string) => {
    if (hotkeyCapturePendingRef.current) return;
    hotkeyCapturePendingRef.current = true;
    setError("");
    void (async () => {
      try {
        if (!await stopHotkeyRecording()) return;
        setDraft((current) => current ? {
          ...current,
          launcher: { ...current.launcher, globalHotkey: value },
        } : current);
      } finally {
        hotkeyCapturePendingRef.current = false;
      }
    })();
  }, [setDraft, stopHotkeyRecording]);

  const hideWindow = useCallback(async () => {
    try {
      await invoke("hide_settings");
    } catch (closeError) {
      setError(String(closeError));
    }
  }, []);

  const discardDrafts = useCallback(() => {
    autoSaveDesiredRef.current = null;
    autoSaveBlockedRef.current = true;
    setAutoSaveNeedsRetry(false);
    if (view) { setDraft(view.config); applySettingsAppearance(view.config.settingsTheme); }
    setEditor(null);
    setApiKey(""); setYoudaoAppKey(""); setYoudaoAppSecret("");
    setAppearanceResetToken((token) => token + 1);
    setAppearanceDirty(false);
    setError("");
  }, [view, setDraft]);

  const close = useCallback(async () => {
    if (saving || autoSaving || changingConfigLocation) return;
    if (hasUnsaved) {
      setPendingAction({ kind: "close", run: () => { discardDrafts(); void hideWindow(); } });
    } else await hideWindow();
  }, [hasUnsaved, saving, autoSaving, changingConfigLocation, discardDrafts, hideWindow]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("settings-close-requested", () => void close()).then((off) => { if (disposed) off(); else unlisten = off; });
    return () => { disposed = true; unlisten?.(); };
  }, [close]);

  const guardEditorChange = (run: () => void) => {
    if (editorDirty || credentialsDirty) setPendingAction({ kind: "switch", run });
    else run();
  };

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
      setApiKey("");
      setYoudaoAppKey("");
      setYoudaoAppSecret("");
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
    void changeHotkeyRecordingBackend(false);
  }, [changeHotkeyRecordingBackend]);

  useEffect(() => {
    let active = true;
    let unlistenStopped: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenCaptured: (() => void) | undefined;
    void listen("hotkey-recording-stopped", () => {
      setRecordingHotkey(false);
      if (hotkeyCleanupErrorRef.current) {
        hotkeyCleanupErrorRef.current = false;
        setError("");
      }
    }).then((dispose) => {
      if (active) unlistenStopped = dispose;
      else dispose();
    });
    void listen<string>("hotkey-recording-error", (event) => {
      hotkeyCleanupErrorRef.current = true;
      setError(event.payload);
    }).then((dispose) => {
      if (active) unlistenError = dispose;
      else dispose();
    });
    void listen<string>("hotkey-recording-captured", (event) => {
      finishHotkeyCapture(event.payload);
    }).then((dispose) => {
      if (active) unlistenCaptured = dispose;
      else dispose();
    });
    return () => {
      active = false;
      unlistenStopped?.();
      unlistenError?.();
      unlistenCaptured?.();
    };
  }, [finishHotkeyCapture]);

  useEffect(() => {
    draftRevisionRef.current += 1;
  }, [draft, editor]);

  useEffect(() => {
    if (
      !draft
      || draft.saveSettingsManually
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
        void stopHotkeyRecording();
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
      finishHotkeyCapture(captured.value);
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!hotkeyButtonRef.current?.contains(event.target as Node)) {
        void stopHotkeyRecording();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [finishHotkeyCapture, recordingHotkey, stopHotkeyRecording]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || document.querySelector("dialog[open]")) return;
      if (saving || autoSaving) return;
      if (pendingRemoval) {
        event.preventDefault();
        setPendingRemoval(null);
        return;
      }
      if (recordingHotkey) {
        event.preventDefault();
        void stopHotkeyRecording();
        return;
      }
      if (editor) {
        event.preventDefault();
        guardEditorChange(cancelEditor);
      } else {
        void close();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close, editor, editorDirty, credentialsDirty, pendingRemoval, recordingHotkey, saving, autoSaving, stopHotkeyRecording]);

  const save = async () => {
    if (!draft || saving || autoSaving || view?.configReadOnly) return;
    if (credentialsDirty) { setError(ui.credentialsUnsaved); return; }
    if (!validateVisibleFields(formRef.current)) return;
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

  const saveTranslationCredentials = async (provider: TranslationProvider) => {
    if (saving || autoSaving || view?.configReadOnly) return;
    setSaving(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("set_translation_credentials", {
        provider,
        apiKey: provider === "youdao" ? null : apiKey,
        appKey: provider === "youdao" ? youdaoAppKey : null,
        appSecret: provider === "youdao" ? youdaoAppSecret : null,
      });
      setView(next);
      setApiKey("");
      setYoudaoAppKey("");
      setYoudaoAppSecret("");
      showStatus(t.credentialsSaved);
    } catch (saveError) {
      setError(String(saveError));
    } finally { setSaving(false); }
  };

  const clearTranslationCredentials = async (provider: TranslationProvider) => {
    if (saving || autoSaving || view?.configReadOnly) return;
    if (!window.confirm(t.confirmClearCredentials.replace("{provider}", translationProviderLabels[provider]))) return;
    setSaving(true);
    setError("");
    try {
      const next = await invoke<AppConfigView>("clear_translation_credentials", { provider });
      setView(next);
      setApiKey("");
      setYoudaoAppKey("");
      setYoudaoAppSecret("");
      showStatus(t.credentialsCleared);
    } catch (clearError) {
      setError(String(clearError));
    } finally { setSaving(false); }
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

  const resetTranslationCredentialDrafts = () => {
    setApiKey("");
    setYoudaoAppKey("");
    setYoudaoAppSecret("");
  };

  const commitEditor = async () => {
    if (!draft || !editor || saving || autoSaving || view?.configReadOnly) return;
    if (!validateVisibleFields(formRef.current)) return;
    if (credentialsDirty) { setError(ui.credentialsUnsaved); return; }
    const config = applyEditor(draft, editor);
    setSaving(true); setError("");
    try {
      await invoke("validate_app_config", { config });
      if (draft.saveSettingsManually) { setDraft(config); showStatus(ui.editorDraft); }
      else {
        const next = await invoke<AppConfigView>("save_app_config", { config });
        persistedSignatureRef.current = JSON.stringify(next.config);
        autoSaveDesiredRef.current = null; autoSaveBlockedRef.current = false;
        setAutoSaveNeedsRetry(false); setView(next); setDraft(next.config);
        applySettingsAppearance(next.config.settingsTheme); showStatus(ui.editorSaved);
      }
      setEditor(null);
    } catch (saveError) { setError(String(saveError)); }
    finally { setSaving(false); }
  };

  function cancelEditor() {
    resetTranslationCredentialDrafts();
    setEditor(null);
  }

  const changeSection = (next: Section) => { setSection(next); setCommandFilter(""); };
  const changeCategory = (next: ConfigurationCategory) => { setSection("configuration"); setCategory(next); setCommandFilter(""); };
  const openScript = (command: ScriptCommandConfig) => {
    if (editor?.kind === "script" && editor.id === command.id) return;
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setEditor({ kind: "script", id: command.id, original: cloneScript(command), value: cloneScript(command) }); });
  };
  const openWebSearch = (search: WebSearchConfig) => {
    if (editor?.kind === "web" && editor.id === search.id) return;
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setEditor({ kind: "web", id: search.id, original: cloneWebSearch(search), value: cloneWebSearch(search) }); });
  };
  const openTranslation = () => {
    if (!draft || editor?.kind === "translation") return;
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setEditor({ kind: "translation", id: "translation", original: cloneTranslation(draft.translation), value: cloneTranslation(draft.translation) }); });
  };
  const openTerminal = () => {
    if (!draft || editor?.kind === "terminal") return;
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setEditor({ kind: "terminal", id: "terminal", original: cloneTerminal(draft.launcher.terminal), value: cloneTerminal(draft.launcher.terminal) }); });
  };

  const addScript = () => {
    if (!draft) return;
    const nextDraft = draft;
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
      resultAction: "copy",
      immediate: false,
      debounceMs: 50,
      timeoutMs: 3000,
    };
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setCategory("scripts"); setCommandFilter(""); setEditor({ kind: "script", id: command.id, original: null, value: cloneScript(command) }); });
  };

  const addWebSearch = () => {
    if (!draft) return;
    const nextDraft = draft;
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
    guardEditorChange(() => { resetTranslationCredentialDrafts(); setCategory("web"); setCommandFilter(""); setEditor({ kind: "web", id: search.id, original: null, value: cloneWebSearch(search) }); });
  };

  const removeScript = (command: ScriptCommandConfig) => {
    setPendingRemoval({ kind: "script", id: command.id, name: command.name || t.unnamed });
  };

  const removeWebSearch = (search: WebSearchConfig) => {
    setPendingRemoval({ kind: "web", id: search.id, name: search.name || t.unnamed });
  };

  const confirmRemoval = () => {
    if (!draft || !pendingRemoval) return;
    const next = pendingRemoval.kind === "script"
      ? {
          ...draft,
          scriptCommands: draft.scriptCommands.filter((item) => item.id !== pendingRemoval.id),
        }
      : {
          ...draft,
          webSearches: draft.webSearches.filter((item) => item.id !== pendingRemoval.id),
        };
    setDraft(next);
    setEditor(null);
    setPendingRemoval(null);
  };

  const changeSaveMode = async (saveSettingsManually: boolean) => {
    if (!draft || saving || autoSaving || view?.configReadOnly) return;
    if (hasUnsaved) { setError(ui.saveModeDirty); return; }
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
    if (editor?.kind === "script" && editor.id === id) {
      setEditor({ ...editor, value: { ...editor.value, enabled } }); return;
    }
    setDraft((current) => current ? { ...current, scriptCommands: current.scriptCommands.map((item) => item.id === id ? { ...item, enabled } : item) } : current);
  };
  const setWebSearchEnabled = (id: string, enabled: boolean) => {
    if (editor?.kind === "web" && editor.id === id) {
      setEditor({ ...editor, value: { ...editor.value, enabled } }); return;
    }
    setDraft((current) => current ? { ...current, webSearches: current.webSearches.map((item) => item.id === id ? { ...item, enabled } : item) } : current);
  };
  const setTranslationEnabled = (enabled: boolean) => {
    if (editor?.kind === "translation") {
      setEditor({ ...editor, value: { ...editor.value, enabled } }); return;
    }
    setDraft((current) => current ? { ...current, translation: { ...current.translation, enabled } } : current);
  };
  const setTerminalEnabled = (enabled: boolean) => {
    if (editor?.kind === "terminal") {
      setEditor({ ...editor, value: { ...editor.value, enabled } }); return;
    }
    setDraft((current) => current ? { ...current, launcher: { ...current.launcher, terminal: { ...current.launcher.terminal, enabled } } } : current);
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

  const browserPlatform = `${navigator.platform} ${navigator.userAgent}`;
  const categoryCopy = {
    builtins: { title: t.builtinsTab, description: ui.builtinDescription },
    scripts: { title: t.scriptsTab, description: ui.scriptDescription },
    web: { title: t.webTab, description: ui.webDescription },
    services: { title: t.translation, description: ui.serviceDescription },
  };
  const matchesFilter = (item: { name: string; keyword: string; description: string; id: string }) => editor?.id === item.id || `${item.name} ${item.keyword} ${item.description}`.toLocaleLowerCase().includes(commandFilter.toLocaleLowerCase());
  const scriptItems = [...(draft?.scriptCommands ?? []), ...(editor?.kind === "script" && editor.original === null ? [editor.value] : [])].filter(matchesFilter);
  const webItems = [...(draft?.webSearches ?? []), ...(editor?.kind === "web" && editor.original === null ? [editor.value] : [])].filter(matchesFilter);
  const visibleEditor = editor && ((category === "scripts" && editor.kind === "script") || (category === "web" && editor.kind === "web") || (category === "services" && editor.kind === "translation") || (category === "builtins" && editor.kind === "terminal"));
  const isMac = /Mac/i.test(browserPlatform);
  const isWindows = /Win/i.test(browserPlatform);

  return (
    <main className="settings-stage" data-large-text={Boolean(view && resolveSettingsTheme(view.config.settingsTheme).baseFontSizePx > 20)}>
      <header className="settings-titlebar" data-tauri-drag-region>
        <div className="settings-brand" data-tauri-drag-region>
          <SuoIcon className="settings-brand-icon" iconStyle={draft?.settingsIconStyle} />
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
        <aside className="settings-sidebar">
          <div className="workspace-brand"><SuoIcon className="workspace-logo" iconStyle={draft?.settingsIconStyle} /><div><strong>Suo</strong><small>{ui.tagline}</small></div></div>
          <nav aria-label={zhCN.settingsTitle}>
            <span className="nav-group-label">{ui.preferences}</span>
            {(["general", "search"] as Section[]).map((key) => <button type="button" key={key} aria-current={section === key ? "page" : undefined} className={section === key ? "settings-nav-active" : ""} onClick={() => changeSection(key)}><NavIcon name={key} />{sectionCopy[key].title}</button>)}
            <span className="nav-group-label">{ui.capabilities}</span>
            {(["builtins", "scripts", "web", "services"] as ConfigurationCategory[]).map((key) => <button type="button" key={key} aria-current={section === "configuration" && category === key ? "page" : undefined} className={section === "configuration" && category === key ? "settings-nav-active" : ""} onClick={() => changeCategory(key)}><NavIcon name={key} />{categoryCopy[key].title}</button>)}
            <span className="nav-group-label">{ui.personalization}</span>
            <button type="button" aria-current={section === "appearance" ? "page" : undefined} className={section === "appearance" ? "settings-nav-active" : ""} onClick={() => changeSection("appearance")}><NavIcon name="appearance" />{zhCN.appearance}</button>
          </nav>
          <small className="sidebar-bottom">{ui.localFirst}</small>
        </aside>

        <form ref={formRef} className="settings-content" aria-busy={saving} onSubmit={(event) => event.preventDefault()}>
          <div className="settings-heading">
            <div>
              <h1>{(section === "configuration" ? categoryCopy[category] : sectionCopy[section]).title}</h1>
              <p>{(section === "configuration" ? categoryCopy[category] : sectionCopy[section]).description}</p>
            </div>

          </div>

          <div className={`settings-scroll ${section === "configuration" ? "command-page" : ""}`}>
          {!draft ? (
            <div className="settings-note">{t.loading}</div>
          ) : (
            <>
              {section === "general" && (
                <div className="settings-card">
                  <div className="setting-row icon-style-row">
                    <div><strong>{ui.iconStyle}</strong><small>{ui.iconStyleDescription}</small></div>
                    <div className="brand-style-options" role="group" aria-label={ui.iconStyle}>
                      {(["transparentColor", "monochrome", "original"] as SettingsIconStyle[]).map((style) => <button type="button" key={style} aria-pressed={draft.settingsIconStyle === style} disabled={Boolean(view?.configReadOnly)} onClick={() => setDraft({ ...draft, settingsIconStyle: style })}><SuoIcon iconStyle={style} /><span>{ui[style]}</span></button>)}
                    </div>
                  </div>
                  <div className="setting-row">
                    <div><strong>{zhCN.globalHotkey}</strong><small>{zhCN.globalHotkeyDescription}</small></div>
                    <button
                      ref={hotkeyButtonRef}
                      className={`hotkey-recorder ${recordingHotkey ? "recording" : ""}`}
                      type="button"
                      disabled={saving || Boolean(view?.configReadOnly)}
                      aria-label={zhCN.globalHotkey}
                      onClick={() => {
                        void beginHotkeyRecording();
                      }}
                    >
                      {recordingHotkey ? zhCN.hotkeyRecording : displayShortcut(draft.launcher.globalHotkey, isMac)}
                    </button>
                  </div>
                  <label className="setting-row">
                    <div><strong>{zhCN.startAtLogin}</strong><small>{zhCN.startAtLoginDescription}</small></div>
                    <input className="switch" type="checkbox" disabled={saving || Boolean(view?.configReadOnly)} checked={draft.launcher.startAtLogin} onChange={(event) => setDraft({ ...draft, launcher: { ...draft.launcher, startAtLogin: event.target.checked } })} />
                  </label>
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
                      label={zhCN.launcherWindowWidth}
                      legalMinimum={visualBounds.launcher.windowWidthPx.min}
                      legalMaximum={visualBounds.launcher.windowWidthPx.max}
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
                      label={zhCN.launcherWindowHeight}
                      legalMinimum={visualBounds.launcher.windowHeightPx.min}
                      legalMaximum={visualBounds.launcher.windowHeightPx.max}
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
                      label={zhCN.launcherHorizontalOffset}
                      legalMinimum={visualBounds.launcher.horizontalOffsetPx.min}
                      legalMaximum={visualBounds.launcher.horizontalOffsetPx.max}
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
                      label={zhCN.launcherVerticalOffset}
                      legalMinimum={visualBounds.launcher.verticalOffsetPx.min}
                      legalMaximum={visualBounds.launcher.verticalOffsetPx.max}
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
                            disabled={saving || autoSaving || changingConfigLocation || hasUnsaved || view.configReadOnly}
                            onClick={() => void chooseConfigDirectory()}
                          >
                            {changingConfigLocation ? t.changingConfigLocation : t.changeConfigLocation}
                          </button>
                          {(!view.usingDefaultConfigLocation || view.configLocationNeedsReset) && (
                            <button
                              className="secondary-button"
                              type="button"
                              disabled={saving || autoSaving || changingConfigLocation || hasUnsaved || view.configReadOnly}
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
                    {(category === "scripts" || category === "web") && <input type="search" className="command-filter" aria-label={ui.filterCommands} placeholder={ui.filterCommands} value={commandFilter} onChange={(event) => setCommandFilter(event.target.value)} />}
                    {category === "scripts" && <button className="add-button" type="button" disabled={Boolean(view?.configReadOnly)} onClick={addScript}>＋ {t.addScript}</button>}
                    {category === "web" && <button className="add-button" type="button" disabled={Boolean(view?.configReadOnly)} onClick={addWebSearch}>＋ {t.addWeb}</button>}
                  </div>

                  <div className="configuration-workspace">
                  <div className="configuration-list" aria-label={categoryCopy[category].title}>
                    {category === "builtins" && (() => {
                      const activeEditor = editor?.kind === "terminal" ? editor : null;
                      const summary = activeEditor?.value ?? draft.launcher.terminal;
                      const target = isWindows
                        ? summary.windowsShell === "powerShell" ? zhCN.windowsPowerShell : zhCN.windowsCommandPrompt
                        : isMac ? summary.macosTerminalApplication || zhCN.macosTerminalApplication : "Bash";
                      return (
                        <ConfigurationItem
                              detailTarget={detailTarget}
                          panelId="terminal-command-editor"
                          open={Boolean(activeEditor)}
                          enabled={summary.enabled}
                          name={zhCN.terminalCommand}
                          keyword=">"
                          description={zhCN.terminalCommandDescription}
                          badges={[t.builtinBadge, target]}
                          onToggle={openTerminal}
                          onEnabledChange={setTerminalEnabled}
                          readOnly={Boolean(view?.configReadOnly)}
                        >
                          {activeEditor && (
                            <>
                              <div className="configuration-editor-header">
                                <span>{draft.saveSettingsManually ? t.pageDraftHint : t.itemInstantHint}</span>
                              </div>
                              <div className="form-grid">
                                {isWindows && (
                                  <Field label={zhCN.windowsTerminalShell} wide>
                                    <select value={activeEditor.value.windowsShell} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, windowsShell: event.target.value as TerminalCommandConfig["windowsShell"] } })}>
                                      <option value="powerShell">{zhCN.windowsPowerShell}</option>
                                      <option value="commandPrompt">{zhCN.windowsCommandPrompt}</option>
                                    </select>
                                    <small className="form-help">{zhCN.windowsTerminalShellDescription}</small>
                                  </Field>
                                )}
                                {isMac && (
                                  <Field label={zhCN.macosTerminalApplication} wide>
                                    <input type="text" maxLength={160} value={activeEditor.value.macosTerminalApplication} placeholder={zhCN.macosTerminalApplicationPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, macosTerminalApplication: event.target.value } })} />
                                    <small className="form-help">{zhCN.macosTerminalApplicationDescription}</small>
                                  </Field>
                                )}
                                <Field label={t.executionSafety} wide><small className="form-help shell-result-warning">{t.terminalCommandWarning}</small></Field>
                              </div>
                              <EditorActions onCancel={cancelEditor} onDone={commitEditor} />
                            </>
                          )}
                        </ConfigurationItem>
                      );
                    })()}

                    {category === "scripts" && (
                      <>
                        {scriptItems.length === 0 && <div className="configuration-empty">{commandFilter ? ui.noMatches : t.emptyScripts}</div>}
                        {scriptItems.map((command) => {
                          const activeEditor = editor?.kind === "script" && editor.id === command.id ? editor : null;
                          const summary = activeEditor?.value ?? command;
                          return (
                            <ConfigurationItem
                              detailTarget={detailTarget}
                              key={command.id}
                              panelId={`script-editor-${command.id}`}
                              open={Boolean(activeEditor)}
                              enabled={summary.enabled}
                              name={summary.name}
                              keyword={summary.keyword}
                              description={summary.description}
                              badges={[runtimeLabels[summary.runtime], scriptResultActionLabels[summary.resultAction], summary.immediate ? `${summary.debounceMs} ms ${t.immediateBadge}` : t.enterBadge]}
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
                                    <Field label={t.name}><input required maxLength={80} value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input required value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <Field label={t.aliases}><AliasesInput key={`script-aliases-${activeEditor.id}`} value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                    <Field label={t.runtime}><select value={activeEditor.value.runtime} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, runtime: event.target.value as ScriptRuntime } })}><option value="python">Python</option><option value="powerShell">PowerShell</option><option value="bash">Bash</option><option value="executable">Executable</option></select></Field>
                                    <div className="form-field wide">
                                      <span id={`script-path-label-${activeEditor.id}`}>{t.scriptPath}</span>
                                      <div className="script-path-row">
                                        <input required aria-labelledby={`script-path-label-${activeEditor.id}`} value={activeEditor.value.scriptPath} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, scriptPath: event.target.value } })} placeholder={ui.scriptPathPlaceholder} />
                                        <button className="secondary-button reveal-script-button" type="button" disabled={!activeEditor.value.scriptPath.trim()} onClick={() => void revealScript(activeEditor.value.scriptPath)}>{t.revealScript}</button>
                                      </div>
                                    </div>
                                  </div>
                                  <details className="script-advanced" open key={activeEditor.id}><summary>{ui.advanced}</summary><div className="form-grid">
                                    <CommandIconField key={`script-icon-${activeEditor.id}`} value={activeEditor.value.iconDataUrl} disabled={Boolean(view?.configReadOnly)} onChange={(iconDataUrl) => setEditor({ ...activeEditor, value: { ...activeEditor.value, iconDataUrl } })} />
                                    <Field label={t.inputHint} wide><input maxLength={160} value={activeEditor.value.inputHint} placeholder={t.scriptInputHintPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, inputHint: event.target.value } })} /></Field>
                                    <Field label={t.scriptResultAction} wide>
                                      <select value={activeEditor.value.resultAction} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, resultAction: event.target.value as ScriptResultAction } })}>
                                        <option value="copy">{t.copyScriptResult}</option>
                                        <option value="executeShell">{t.executeShellResult}</option>
                                      </select>
                                      {activeEditor.value.resultAction === "executeShell" && <small className="form-help shell-result-warning">{t.executeShellResultWarning}</small>}
                                    </Field>
                                    <Field label={t.timeout}><input type="number" min={100} max={60000} value={activeEditor.value.timeoutMs} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, timeoutMs: Number(event.target.value) } })} /></Field>
                                    <Field label={t.executionMode}><select value={activeEditor.value.immediate ? "immediate" : "enter"} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, immediate: event.target.value === "immediate" } })}><option value="enter">{t.enterMode}</option><option value="immediate">{t.immediateMode}</option></select></Field>
                                    {activeEditor.value.immediate && <Field label={t.debounce}><input type="number" min={20} max={60000} value={activeEditor.value.debounceMs} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, debounceMs: Number(event.target.value) } })} /></Field>}
                                  </div></details>
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
                        {webItems.length === 0 && <div className="configuration-empty">{commandFilter ? ui.noMatches : t.emptyWebSearches}</div>}
                        {webItems.map((search) => {
                          const activeEditor = editor?.kind === "web" && editor.id === search.id ? editor : null;
                          const summary = activeEditor?.value ?? search;
                          return (
                            <ConfigurationItem
                              detailTarget={detailTarget}
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
                                    <Field label={t.name}><input required maxLength={80} value={activeEditor.value.name} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, name: event.target.value } })} /></Field>
                                    <Field label={t.keyword}><input required value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                    <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                    <CommandIconField key={`web-icon-${activeEditor.id}`} value={activeEditor.value.iconDataUrl} disabled={Boolean(view?.configReadOnly)} onChange={(iconDataUrl) => setEditor({ ...activeEditor, value: { ...activeEditor.value, iconDataUrl } })} />
                                    <Field label={t.inputHint} wide><input maxLength={160} value={activeEditor.value.inputHint} placeholder={t.webInputHintPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, inputHint: event.target.value } })} /></Field>
                                    <Field label={t.aliases}><AliasesInput key={`web-aliases-${activeEditor.id}`} value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                    <Field label={t.urlTemplate} wide><input required value={activeEditor.value.urlTemplate} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, urlTemplate: event.target.value } })} /></Field>
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
                      const providerLabel = translationProviderLabels[summary.provider];
                      const credentialConfigured = view?.translationCredentialStatus[summary.provider] ?? false;
                      return (
                        <ConfigurationItem
                              detailTarget={detailTarget}
                          panelId="translation-editor"
                          open={Boolean(activeEditor)}
                          enabled={summary.enabled}
                          name={t.translation}
                          keyword={summary.keyword}
                          description={summary.description}
                          badges={[providerLabel, credentialConfigured ? t.credentialsConfiguredBadge : t.credentialsMissingBadge]}
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
                                <Field label={t.keyword}><input required value={activeEditor.value.keyword} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, keyword: event.target.value } })} /></Field>
                                <Field label={t.translationProvider}><select value={activeEditor.value.provider} onChange={(event) => {
                                  const provider = event.target.value as TranslationProvider;
                                  resetTranslationCredentialDrafts();
                                  setEditor({ ...activeEditor, value: { ...activeEditor.value, provider } });
                                }}><option value="microsoft">{t.microsoftProvider}</option><option value="google">{t.googleProvider}</option><option value="youdao">{t.youdaoProvider}</option></select></Field>
                                <Field label={t.description} wide><textarea maxLength={200} value={activeEditor.value.description} placeholder={t.descriptionPlaceholder} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, description: event.target.value } })} /></Field>
                                <Field label={t.aliases}><AliasesInput key="translation-aliases" value={activeEditor.value.aliases} onChange={(aliases) => setEditor({ ...activeEditor, value: { ...activeEditor.value, aliases } })} /></Field>
                                {activeEditor.value.provider === "microsoft" && <Field label={t.region}><input value={activeEditor.value.region} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, region: event.target.value } })} placeholder="eastasia" /></Field>}
                                <Field label={t.defaultTarget}><input value={activeEditor.value.defaultTargetLanguage} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, defaultTargetLanguage: event.target.value } })} /></Field>
                                <Field label={t.chineseTarget}><input value={activeEditor.value.chineseTargetLanguage} onChange={(event) => setEditor({ ...activeEditor, value: { ...activeEditor.value, chineseTargetLanguage: event.target.value } })} /></Field>
                                {activeEditor.value.provider === "youdao" ? (
                                  <>
                                    <Field label={t.youdaoAppKey}><input type="password" data-independent-config disabled={Boolean(view?.configReadOnly)} value={youdaoAppKey} onChange={(event) => setYoudaoAppKey(event.target.value)} placeholder={t.youdaoAppKeyPlaceholder} /></Field>
                                    <Field label={t.youdaoAppSecret}><input type="password" data-independent-config disabled={Boolean(view?.configReadOnly)} value={youdaoAppSecret} onChange={(event) => setYoudaoAppSecret(event.target.value)} placeholder={t.youdaoAppSecretPlaceholder} /></Field>
                                    <Field label={t.translationCredentials} wide><div className="credential-status-row" data-independent-config><small className={credentialConfigured ? "credential-ok" : "credential-missing"}>{credentialConfigured ? t.credentialsConfigured : t.credentialsMissing}</small><div className="credential-actions"><button className="secondary-button" type="button" disabled={Boolean(view?.configReadOnly) || !youdaoAppKey.trim() || !youdaoAppSecret.trim()} onClick={() => void saveTranslationCredentials("youdao")}>{t.saveCredentials}</button><button className="danger-button" type="button" disabled={Boolean(view?.configReadOnly) || !credentialConfigured} onClick={() => void clearTranslationCredentials("youdao")}>{t.clearCredentials}</button></div></div></Field>
                                  </>
                                ) : (
                                  <Field label={activeEditor.value.provider === "microsoft" ? t.microsoftApiKey : t.googleApiKey} wide><div className="credential-row" data-independent-config><input type="password" disabled={Boolean(view?.configReadOnly)} value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={t.apiKeyPlaceholder} /><button className="secondary-button" type="button" disabled={Boolean(view?.configReadOnly) || !apiKey.trim()} onClick={() => void saveTranslationCredentials(activeEditor.value.provider)}>{t.saveCredentials}</button><button className="danger-button" type="button" disabled={Boolean(view?.configReadOnly) || !credentialConfigured} onClick={() => void clearTranslationCredentials(activeEditor.value.provider)}>{t.clearCredentials}</button></div><small className={credentialConfigured ? "credential-ok" : "credential-missing"}>{credentialConfigured ? t.credentialsConfigured : t.credentialsMissing}</small></Field>
                                )}
                                <Field label={t.providerLanguageHint} wide><small className="form-help">{activeEditor.value.provider === "youdao" ? t.youdaoLanguageHint : t.commonLanguageHint}</small></Field>
                              </div>
                              <EditorActions onCancel={cancelEditor} onDone={commitEditor} />
                            </>
                          )}
                        </ConfigurationItem>
                      );
                    })()}
                  </div>
                  <div className="configuration-detail" ref={setDetailTarget}>
                    {!visibleEditor && <div className="editor-placeholder"><NavIcon name={category} /><p>{ui.selectCommand}</p></div>}
                  </div>
                  </div>
                  <p className="configuration-hint">{draft.saveSettingsManually ? t.configurationHint : t.configurationHintInstant}</p>
                </div>
              )}

              <div hidden={section !== "appearance"}>
                <AppearanceEditor
                  onDirtyChange={setAppearanceDirty}
                  resetToken={appearanceResetToken}
                  launcherTheme={draft.launcherTheme}
                  settingsTheme={draft.settingsTheme}
                  onChange={updateAppearanceThemes}
                  saveSettingsManually={draft.saveSettingsManually}
                  readOnly={Boolean(view?.configReadOnly)}
                  saving={saving || autoSaving}
                />
              </div>
            </>
          )}

          </div>
          <div className="settings-notices" aria-live="polite">
          {view?.credentialStoreError && <div className="settings-error">{t.credentialWarning}：{view.credentialStoreError}</div>}
          {error && <div role="alert" className="settings-error">{error}</div>}
          </div>
          <footer className="settings-footer">
            {appearanceDirty && section !== "appearance" && <button type="button" className="draft-return" onClick={() => changeSection("appearance")}>{ui.returnToAppearance}</button>}
            {editorDirty && editor && (section !== "configuration" || !visibleEditor) && <button type="button" className="draft-return" onClick={() => changeCategory(editor.kind === "script" ? "scripts" : editor.kind === "web" ? "web" : editor.kind === "terminal" ? "builtins" : "services")}>{ui.returnToEditor.replace("{name}", editor.kind === "script" || editor.kind === "web" ? editor.value.name || t.unnamed : editor.kind === "terminal" ? zhCN.terminalCommand : t.translation)}</button>}
            <span className="settings-save-state" role="status">{view?.configReadOnly ? ui.readOnly : appearanceDirty ? ui.appearanceUnsaved : credentialsDirty ? ui.credentialsUnsaved : editorDirty ? ui.editorUnsaved : configDirty ? ui.unsaved : ui.saved}</span>
            <div className="settings-actions">
              {(status || autoSaving) && <span className="saved-indicator visible">{autoSaving ? t.savingAutomatically : status}</span>}
              {draft && (
                <label className="settings-save-mode">
                  <span><strong>{t.unifiedSave}</strong><small>{draft.saveSettingsManually ? t.unifiedSaveManual : t.unifiedSaveInstant}</small></span>
                  <input
                    className="switch"
                    type="checkbox"
                    checked={draft.saveSettingsManually}
                    disabled={saving || autoSaving || hasUnsaved || view?.configReadOnly}
                    title={hasUnsaved ? ui.saveModeDirty : undefined}
                    aria-label={t.unifiedSave}
                    onChange={(event) => void changeSaveMode(event.target.checked)}
                  />
                </label>
              )}
              {draft?.saveSettingsManually && (
                <button className="primary-button" type="button" disabled={saving || autoSaving || view?.configReadOnly || !configDirty && !editorDirty} onClick={() => void save()}>
                  {saving ? t.saving : t.save}
                </button>
              )}
              {draft && !draft.saveSettingsManually && autoSaveNeedsRetry && (
                <button className="secondary-button" type="button" disabled={saving || autoSaving || view?.configReadOnly} onClick={() => queueAutoSave(draft)}>
                  {t.retrySave}
                </button>
              )}
            </div>
            {hasUnsaved && <button type="button" className="secondary-button" disabled={saving || autoSaving} onClick={() => setPendingAction({ kind: "discard", run: discardDrafts })}>{ui.discardAll}</button>}
          </footer>
        </form>
      </div>
      <SettingsDialog open={Boolean(pendingRemoval)} title={t.confirmRemoveTitle} onClose={() => setPendingRemoval(null)} footer={<><button className="secondary-button" type="button" onClick={() => setPendingRemoval(null)}>{t.cancel}</button><button className="danger-button" type="button" onClick={confirmRemoval}>{t.confirmDelete}</button></>}>
        <p>{(draft?.saveSettingsManually ? t.confirmRemove : t.confirmRemoveInstant).replace("{name}", pendingRemoval?.name ?? "")}</p>
      </SettingsDialog>
      <SettingsDialog open={Boolean(pendingAction)} title={pendingAction?.kind === "discard" ? ui.discardTitle : ui.unsavedTitle} onClose={() => setPendingAction(null)} footer={<><button className="secondary-button" type="button" onClick={() => setPendingAction(null)}>{ui.keepEditing}</button><button className="danger-button" type="button" onClick={() => { const action = pendingAction; setPendingAction(null); action?.run(); }}>{pendingAction?.kind === "close" ? ui.closeAndDiscard : ui.discardAndContinue}</button></>}>
        <p>{pendingAction?.kind === "switch" ? ui.switchDescription : pendingAction?.kind === "discard" ? ui.discardDescription : ui.unsavedDescription}</p>
      </SettingsDialog>
    </main>
  );
}

function NavIcon({ name }: { name: string }) {
  const paths: Record<string, string> = {
    general: "M4 7h16M4 17h16M8 4v6M16 14v6",
    search: "M21 21l-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    scripts: "m8 5-6 7 6 7m8-14 6 7-6 7m-2-16-4 18",
    web: "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0M3 12h18M12 3c5 5 5 13 0 18-5-5-5-13 0-18",
    services: "M3 5h12M9 2v3M5 8c2 5 5 8 9 9M13 5c-1 6-4 10-10 13M14 21l4-11 4 11m-6-4h4",
    builtins: "m5 7 5 5-5 5m8 0h6",
    appearance: "M12 3a9 9 0 1 0 9 9c0-3-3-2-5-2s-4 0-4-3V3Z",
  };
  return <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={paths[name] ?? paths.general} /></svg>;
}

function ConfigurationItem({
  detailTarget,
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
  detailTarget: HTMLDivElement | null;
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
      {open && detailTarget && createPortal(<fieldset disabled={readOnly} className="configuration-editor" id={panelId}>{children}</fieldset>, detailTarget)}
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
