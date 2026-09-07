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
            <>
              已索引 <b>{status.documents.toLocaleString()}</b> 个文件 · {status.data_dir}
            </>
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
      </main>
    </div>
  );
}
