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
  const [warnings, setWarnings] = useState<string[]>([]);

  const run = async () => {
    if (!path.trim()) {
      setError("请输入要扫描的目录路径");
      return;
    }
    setBusy(true);
    setError("");
    setMessage("");
    setWarnings([]);
    try {
      const report = await scanDirectory(
        path.trim(),
        includeContent,
        batchName.trim() || undefined,
      );
      setMessage(`扫描完成，共索引 ${report.indexed} 个文件。`);
      setWarnings(report.warnings ?? []);
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
      {warnings.length > 0 && (
        <div className="warning">
          <div>{warnings.length} 个条目被跳过或索引失败：</div>
          <ul className="warning-list">
            {warnings.slice(0, 10).map((w, i) => (
              <li key={i}>{w}</li>
            ))}
            {warnings.length > 10 && <li>…共 {warnings.length} 条，仅显示前 10 条</li>}
          </ul>
        </div>
      )}

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
