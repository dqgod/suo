import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import "./App.css";
import Settings from "./Settings";
import { zhCN } from "./i18n/zh-CN";
import { loadLauncherPreferences } from "./preferences";

type ResultAction =
  | { type: "openPath"; path: string }
  | { type: "openUrl"; url: string }
  | { type: "copyText"; text: string }
  | { type: "openSettings" }
  | { type: "none" };

type SearchResult = {
  id: string;
  title: string;
  subtitle: string;
  kind: string;
  badge: string;
  score: number;
  action: ResultAction;
};

type SearchResponse = {
  query: string;
  provider: string;
  providerDetail: string;
  hotkeyStatus: string;
  indexing: boolean;
  indexedFileCount: number;
  results: SearchResult[];
};

const initialResponse: SearchResponse = {
  query: "",
  provider: zhCN.loading,
  providerDetail: zhCN.preparingIndex,
  hotkeyStatus: zhCN.registeringHotkey,
  indexing: true,
  indexedFileCount: 0,
  results: [],
};

const kindIcons: Record<string, string> = {
  app: "◆",
  file: "▱",
  calculator: "=",
  script: ">_",
  web: "↗",
  settings: "⚙",
  hint: "?",
  error: "!",
};

function Launcher() {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState<SearchResponse>(initialResponse);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [message, setMessage] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const requestId = useRef(0);
  const queryRef = useRef("");

  const updateQuery = useCallback((value: string) => {
    const generation = ++requestId.current;
    queryRef.current = value;
    setQuery(value);
    setResponse((current) => ({
      ...current,
      query: value.trim(),
      results: [],
    }));
    void invoke("cancel_search", { generation });
  }, []);

  const search = useCallback(async (value: string) => {
    const currentRequest = ++requestId.current;
    try {
      const next = await invoke<SearchResponse>("search_launcher", {
        query: value,
        generation: currentRequest,
      });
      if (currentRequest === requestId.current) {
        setResponse(next);
        setSelectedIndex(0);
        setMessage("");
      }
    } catch (error) {
      if (currentRequest === requestId.current) {
        setMessage(String(error));
      }
    }
  }, []);

  useEffect(() => {
    const preferences = loadLauncherPreferences();
    void invoke("update_launcher_preferences", {
      closeOnBlur: preferences.closeOnBlur,
      keepLastInput: preferences.keepLastInput,
    });
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void search(query), query ? 90 : 0);
    return () => window.clearTimeout(timer);
  }, [query, search]);

  useEffect(() => {
    const shown = listen("launcher-shown", () => {
      window.setTimeout(() => inputRef.current?.focus(), 0);
      void search(queryRef.current);
    });
    const hidden = listen("launcher-hidden", () => {
      if (!loadLauncherPreferences().keepLastInput) updateQuery("");
      setSelectedIndex(0);
      setMessage("");
    });
    window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => {
      void shown.then((unlisten) => unlisten());
      void hidden.then((unlisten) => unlisten());
    };
  }, [search, updateQuery]);

  useEffect(() => {
    if (!response.indexing) return;
    const timer = window.setInterval(() => void search(queryRef.current), 1000);
    return () => window.clearInterval(timer);
  }, [response.indexing, search]);

  const hide = useCallback(async () => {
    await invoke("hide_launcher");
  }, []);

  const openSettings = useCallback(async () => {
    try {
      await invoke("open_settings");
      await hide();
    } catch (error) {
      setMessage(String(error));
    }
  }, [hide]);

  const activate = useCallback(
    async (result: SearchResult, keepOpen = false) => {
      try {
        if (result.action.type === "copyText") {
          await navigator.clipboard.writeText(result.action.text);
          setMessage(zhCN.copied);
          return;
        }
        if (result.action.type === "none") return;
        await invoke("activate_result", { action: result.action, keepOpen });
        if (!keepOpen) await hide();
      } catch (error) {
        setMessage(String(error));
      }
    },
    [hide],
  );

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      void hide();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((index) => {
        if (!response.results.length) return 0;
        return Math.min(index + 1, response.results.length - 1);
      });
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((index) => Math.max(index - 1, 0));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      if (response.query !== query.trim()) return;
      const result = response.results[selectedIndex];
      if (result) void activate(result, event.shiftKey);
    }
  };

  const rebuildIndex = async () => {
    await invoke("rebuild_file_index");
    await search(queryRef.current);
  };

  return (
    <main className="window-stage">
      <section className="launcher" aria-label={zhCN.productName}>
        <div className="search-box" data-tauri-drag-region>
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="10.8" cy="10.8" r="6.8" />
            <path d="m16 16 4.2 4.2" />
          </svg>
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => updateQuery(event.target.value)}
            onKeyDown={onKeyDown}
            placeholder={zhCN.searchPlaceholder}
            spellCheck={false}
            autoComplete="off"
            aria-label={zhCN.searchPlaceholder}
          />
          {query && (
            <button className="clear-button" type="button" onClick={() => updateQuery("")}>
              ×
            </button>
          )}
          <button
            className="brand-button"
            type="button"
            title={zhCN.openSettings}
            aria-label={zhCN.openSettings}
            onClick={() => void openSettings()}
          >
            <span aria-hidden="true">◇</span>
          </button>
        </div>

        <div className="provider-row">
          <div className="provider-copy">
            <span className={`status-dot ${response.indexing ? "busy" : ""}`} />
            <strong>{response.provider}</strong>
            <span>{response.providerDetail}</span>
          </div>
          <button className="index-button" type="button" onClick={() => void rebuildIndex()}>
            {zhCN.rebuildIndex}
          </button>
        </div>

        <div className="results" role="listbox" aria-label={zhCN.results}>
          {response.results.map((result, index) => (
            <button
              className={`result ${index === selectedIndex ? "selected" : ""}`}
              type="button"
              key={result.id}
              role="option"
              aria-selected={index === selectedIndex}
              onMouseEnter={() => setSelectedIndex(index)}
              onClick={() => void activate(result)}
            >
              <span className={`result-icon ${result.kind}`} aria-hidden="true">
                {kindIcons[result.kind] ?? "·"}
              </span>
              <span className="result-copy">
                <strong>{result.title}</strong>
                <small>{result.subtitle}</small>
              </span>
              <span className="result-badge">{result.badge}</span>
              {index === selectedIndex && <kbd>↵</kbd>}
            </button>
          ))}
          {!response.results.length && (
            <div className="empty-state">
              <span>⌕</span>
              <strong>{response.indexing ? zhCN.indexing : zhCN.noResults}</strong>
              <small>{zhCN.tryCommands}</small>
            </div>
          )}
        </div>

        <footer>
          <div className="key-help">
            <span><kbd>↑↓</kbd>{zhCN.select}</span>
            <span><kbd>↵</kbd>{zhCN.open}</span>
            <span><kbd>Shift ↵</kbd>{zhCN.keepOpen}</span>
          </div>
          <div className={response.hotkeyStatus.includes("失败") ? "hotkey failed" : "hotkey"}>
            <span className="status-dot" />
            {response.hotkeyStatus}
          </div>
        </footer>

        {message && <div className="toast">{message}</div>}
      </section>
    </main>
  );
}

function App() {
  return getCurrentWindow().label === "settings" ? <Settings /> : <Launcher />;
}

export default App;
