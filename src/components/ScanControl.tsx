import { useState } from "react";
import { ScanProgress, scanDirectory } from "../lib/api";

interface Props {
  progress: ScanProgress | null;
  onDone: () => void;
}

export default function ScanControl({ progress, onDone }: Props) {
  const [path, setPath] = useState("");
  const [batchName, setBatchName] = useState("");
  const [includeContent, setIncludeContent] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  const run = async () => {
    if (!path.trim()) {
      setError("请输入要扫描的目录路径");
      return;
    }
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const count = await scanDirectory(path.trim(), includeContent, batchName.trim() || undefined);
      setMessage(`扫描完成，共索引 ${count} 个文件。`);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const pct =
    progress && progress.total > 0
      ? Math.round((progress.indexed / progress.total) * 100)
      : 0;

  return (
    <div className="panel">
      <h2>全盘 / 目录扫描</h2>
      <div className="form-row">
        <input
          className="search-input"
          placeholder="输入要扫描的目录，例如 /Users/me/Documents"
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
      </div>
      <div className="form-row">
        <input
          className="search-input"
          placeholder="批次名称（可选，留空则自动生成）"
          value={batchName}
          onChange={(e) => setBatchName(e.target.value)}
        />
      </div>
      <div className="form-row inline">
        <label>
          <input
            type="checkbox"
            checked={includeContent}
            onChange={(e) => setIncludeContent(e.target.checked)}
          />
          索引文件内容（否则仅索引文件名）
        </label>
        <button className="btn primary" onClick={run} disabled={busy}>
          {busy ? "扫描中…" : "开始扫描"}
        </button>
      </div>

      {error && <div className="error">{error}</div>}
      {message && <div className="ok">{message}</div>}

      {progress && (
        <div className="progress-wrap">
          <div className="progress-bar">
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <div className="progress-label">
            {progress.indexed} / {progress.total} · {progress.path}
          </div>
        </div>
      )}
    </div>
  );
}
