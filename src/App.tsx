import { useCallback, useEffect, useState } from "react";
import {
  IndexStatus,
  indexStatus,
  onScanProgress,
  ScanProgress,
} from "./lib/api";
import SearchBar from "./components/SearchBar";
import ResultList from "./components/ResultList";
import ScanControl from "./components/ScanControl";
import RuleManager from "./components/RuleManager";
import BatchManager from "./components/BatchManager";
import ErrorBoundary from "./ErrorBoundary";
import { SearchResult } from "./lib/api";
import { searchFiles } from "./lib/api";

type Tab = "search" | "scan" | "rules" | "index";

export default function App() {
  const [tab, setTab] = useState<Tab>("search");
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [lastQuery, setLastQuery] = useState("");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await indexStatus());
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    const unlisten = onScanProgress((p) => setProgress(p));
    return () => {
      unlisten.then((un) => un());
    };
  }, [refreshStatus]);

  const runSearch = async (query: string, batchId: string) => {
    setBusy(true);
    setLastQuery(query);
    try {
      const r = await searchFiles(query, 200, batchId || undefined);
      setResults(r);
    } catch (e) {
      console.error("search failed", e);
      setResults([]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <h1>DocSniffer</h1>
          <span className="subtitle">文档嗅探器</span>
        </div>
        <div className="status-chip">
          {status ? (
            <div className="status-chip-inner">
              <div className="status-chip-count">
                已索引 <b>{status.documents.toLocaleString()}</b> 个文件
              </div>
              <div className="status-chip-path" title={status.data_dir}>
                {status.data_dir}
              </div>
            </div>
          ) : (
            "索引未就绪"
          )}
        </div>
      </header>

      <nav className="tabs">
        {(
          [
            ["search", "搜索"],
            ["scan", "扫描"],
            ["rules", "敏感检测"],
            ["index", "索引管理"],
          ] as [Tab, string][]
        ).map(([k, label]) => (
          <button
            key={k}
            className={`tab ${tab === k ? "active" : ""}`}
            onClick={() => setTab(k)}
          >
            {label}
          </button>
        ))}
      </nav>

      <main className="content">
        {/* keyed by tab so a render crash in one panel is cleared the moment
            the user switches to another tab, instead of wedging the whole app */}
        <ErrorBoundary key={tab}>
          {tab === "search" && (
            <>
              <SearchBar onSearch={runSearch} busy={busy} />
              <ResultList results={results} query={lastQuery} />
            </>
          )}
          {tab === "scan" && (
            <ScanControl progress={progress} onDone={refreshStatus} />
          )}
          {tab === "rules" && <RuleManager />}
          {tab === "index" && <BatchManager onChanged={refreshStatus} />}
        </ErrorBoundary>
      </main>
    </div>
  );
}
