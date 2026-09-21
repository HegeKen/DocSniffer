import { useState } from "react";
import { openPath, SearchResult } from "../lib/api";

interface Props {
  results: SearchResult[];
  query: string;
}

function fmtSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function fmtDate(secs: number): string {
  if (!secs) return "-";
  return new Date(secs * 1000).toLocaleString();
}

export default function ResultList({ results, query }: Props) {
  const [error, setError] = useState("");

  const openWithDefaultApp = async (result: SearchResult) => {
    try {
      await openPath(result.path);
      setError("");
    } catch (e) {
      setError(`打开失败：${e}`);
    }
  };

  if (!query) {
    return <div className="empty">输入关键词开始搜索。</div>;
  }
  if (results.length === 0) {
    return <div className="empty">没有匹配结果。</div>;
  }

  return (
    <div className="result-list">
      <div className="result-meta">共 {results.length} 条结果</div>
      {error && <div className="error">{error}</div>}
      {results.map((r) => (
        <div
          className="result-item clickable"
          key={r.id}
          role="button"
          title={`点击用默认程序打开：${r.path}`}
          onClick={() => openWithDefaultApp(r)}
        >
          <div className="result-title">
            <span className="result-name">{r.name}</span>
            <span className="result-ext">.{r.ext}</span>
          </div>
          <div className="result-path">{r.path}</div>
          {r.snippet && <div className="result-snippet">{r.snippet}</div>}
          <div className="result-meta-line">
            <span>{fmtSize(r.size)}</span>
            <span>{fmtDate(r.mtime)}</span>
            <span>得分 {r.score.toFixed(2)}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
