import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import "./App.css";
import Settings from "./Settings";
import { AppConfig, applyAppearance, loadAppConfig } from "./config";
import { zhCN } from "./i18n/zh-CN";

type ResultAction =
  | { type: "openPath"; path: string }
  | { type: "openUrl"; url: string }
  | { type: "copyText"; text: string }
  | { type: "runScript"; commandId: string; args: string[] }
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
  actionEpoch: number;
  results: SearchResult[];
};

type CancelStatus = {
  actionEpoch: number;
};

type IndexStatus = {
  indexing: boolean;
  indexedFileCount: number;
};

const initialResponse: SearchResponse = {
  query: "",
  provider: zhCN.loading,
  providerDetail: zhCN.preparingIndex,
  hotkeyStatus: zhCN.registeringHotkey,
  indexing: true,
  indexedFileCount: 0,
  actionEpoch: 0,
  results: [],
};

const kindIcons: Record<string, string> = {
  app: "◆",
  file: "▱",
  calculator: "=",
  script: ">_",
  web: "↗",
  translation: "译",
  settings: "⚙",
  hint: "?",
  error: "!",
};

function Launcher() {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState<SearchResponse>(initialResponse);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [message, setMessage] = useState("");
  const [composing, setComposing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const requestId = useRef(0);
  const queryRef = useRef("");
  const completedRequestIdRef = useRef(-1);
  const actionEpochRef = useRef(0);
  const activationReadyRef = useRef(false);
  const preserveCancellationRef = useRef<number | null>(null);
  const keepLastInputRef = useRef(false);

  const cancelPending = useCallback(() => {
    const generation = ++requestId.current;
    activationReadyRef.current = false;
    return {
      generation,
      promise: invoke<CancelStatus>("cancel_search", { generation }),
    };
  }, []);

  const updateQuery = useCallback((value: string) => {
    preserveCancellationRef.current = null;
    queryRef.current = value;
    setQuery(value);
    setResponse((current) => ({
      ...current,
      query: value.trim(),
      provider: zhCN.loading,
      providerDetail: zhCN.waitingForInput,
      results: [],
    }));
    const cancellation = cancelPending();
    void cancellation.promise.catch(() => undefined);
  }, [cancelPending]);

  const search = useCallback(async (value: string) => {
    const currentRequest = ++requestId.current;
    activationReadyRef.current = false;
    try {
      const next = await invoke<SearchResponse>("search_launcher", {
        query: value,
        generation: currentRequest,
      });
      if (currentRequest === requestId.current) {
        actionEpochRef.current = next.actionEpoch;
        activationReadyRef.current = true;
        completedRequestIdRef.current = currentRequest;
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
    void loadAppConfig()
      .then((view) => {
        keepLastInputRef.current = view.config.launcher.keepLastInput;
        applyAppearance(view.config.appearance);
        if (view.configLoadWarning) setMessage(view.configLoadWarning);
      })
      .catch((error) => setMessage(String(error)));
    const updated = listen<AppConfig>("app-config-updated", (event) => {
      keepLastInputRef.current = event.payload.launcher.keepLastInput;
      applyAppearance(event.payload.appearance);
    });
    const providersUpdated = listen("provider-config-updated", () => {
      updateQuery(queryRef.current);
    });
    return () => {
      void updated.then((unlisten) => unlisten());
      void providersUpdated.then((unlisten) => unlisten());
    };
  }, [updateQuery]);

  useEffect(() => {
    if (composing) return;
    const timer = window.setTimeout(() => void search(query), query ? 50 : 0);
    return () => window.clearTimeout(timer);
  }, [composing, query, search]);

  useEffect(() => {
    const shown = listen("launcher-shown", () => {
      window.setTimeout(() => inputRef.current?.focus(), 0);
    });
    const hidden = listen("launcher-hidden", () => {
      const queryCompleted = completedRequestIdRef.current === requestId.current;
      if (
        keepLastInputRef.current &&
        (queryCompleted || preserveCancellationRef.current !== null)
      ) {
        if (queryCompleted && preserveCancellationRef.current === null) {
          const cancellation = cancelPending();
          preserveCancellationRef.current = cancellation.generation;
          void cancellation.promise
            .then((status) => {
              if (
                requestId.current !== cancellation.generation ||
                preserveCancellationRef.current !== cancellation.generation
              ) return;
              actionEpochRef.current = status.actionEpoch;
              activationReadyRef.current = true;
              completedRequestIdRef.current = cancellation.generation;
              setResponse((current) => ({
                ...current,
                actionEpoch: status.actionEpoch,
              }));
            })
            .catch((error) => {
              if (
                requestId.current === cancellation.generation &&
                preserveCancellationRef.current === cancellation.generation
              ) {
                completedRequestIdRef.current = -1;
                setMessage(String(error));
              }
            })
            .finally(() => {
              if (preserveCancellationRef.current === cancellation.generation) {
                preserveCancellationRef.current = null;
              }
            });
        }
      } else {
        updateQuery("");
      }
      setSelectedIndex(0);
      setMessage("");
    });
    const indexStarted = listen("file-index-started", () => {
      setResponse((current) => ({ ...current, indexing: true }));
    });
    window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => {
      void shown.then((unlisten) => unlisten());
      void hidden.then((unlisten) => unlisten());
      void indexStarted.then((unlisten) => unlisten());
    };
  }, [cancelPending, updateQuery]);

  useEffect(() => {
    if (!response.indexing) return;
    const timer = window.setInterval(() => {
      const polledQuery = queryRef.current;
      const polledRequest = requestId.current;
      const polledProvider = response.provider;
      void invoke<IndexStatus>("get_index_status")
        .then((status) => {
          if (
            response.indexing &&
            !status.indexing &&
            polledProvider.includes("限定目录索引") &&
            polledRequest === requestId.current &&
            polledQuery === queryRef.current
          ) {
            void search(polledQuery);
            return;
          }
          setResponse((current) => ({
            ...current,
            indexing: status.indexing,
            indexedFileCount: status.indexedFileCount,
          }));
        })
        .catch((error) => setMessage(String(error)));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [response.indexing, response.provider, search]);

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
        if (result.action.type === "runScript" && !activationReadyRef.current) {
          setMessage("正在恢复命令状态，请稍候");
          return;
        }
        const output = await invoke<SearchResult | null>("activate_result", {
          action: result.action,
          keepOpen,
          actionEpoch: actionEpochRef.current,
        });
        if (output) {
          setResponse((current) => ({ ...current, results: [output] }));
          setSelectedIndex(0);
          return;
        }
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
    const status = await invoke<IndexStatus>("rebuild_file_index");
    setResponse((current) => ({
      ...current,
      indexing: status.indexing,
      indexedFileCount: status.indexedFileCount,
    }));
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
            onCompositionStart={(event) => {
              setComposing(true);
              updateQuery(event.currentTarget.value);
            }}
            onCompositionEnd={(event) => {
              setComposing(false);
              updateQuery(event.currentTarget.value);
            }}
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
