import { useCallback, useEffect, useState } from "react";
import { BatchItem, clearAllIndex, deleteBatch, listBatches, updateBatch } from "../lib/api";

export default function BatchManager({ onChanged }: { onChanged?: () => void }) {
  const [batches, setBatches] = useState<BatchItem[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [updatingId, setUpdatingId] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setBatches(await listBatches());
      setError("");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const updateOne = async (batch: BatchItem) => {
    if (!window.confirm(`重新扫描「${batch.name}」的目录并更新其索引？将重新索引当前目录下的全部文件。`)) return;
    setUpdatingId(batch.id);
    setMessage("");
    setError("");
    try {
      const n = await updateBatch(batch.id);
      setMessage(`已更新批次「${batch.name}」，当前索引 ${n} 个文档。`);
      await reload();
      onChanged?.();
    } catch (e) {
      setError(`更新失败：${e}`);
    } finally {
      setUpdatingId(null);
    }
  };

  const removeOne = async (batch: BatchItem) => {
    if (!window.confirm(`确认删除批次「${batch.name}」及其全部 ${batch.documents} 个文档？`)) return;
    setBusy(true);
    setMessage("");
    setError("");
    try {
      const n = await deleteBatch(batch.id);
      setMessage(`已删除批次「${batch.name}」，移除 ${n} 个文档。`);
      await reload();
      onChanged?.();
    } catch (e) {
      setError(`删除失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  const clearAll = async () => {
    if (!window.confirm("确认清除现有全部索引和所有批次？此操作不可撤销。")) return;
    setBusy(true);
    setMessage("");
    setError("");
    try {
      await clearAllIndex();
      setMessage("已清除全部索引和批次。");
      await reload();
      onChanged?.();
    } catch (e) {
      setError(`清除失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  const total = batches.reduce((s, b) => s + b.documents, 0);

  return (
    <div className="panel">
      <h2>索引管理</h2>
      <div className="toolbar">
        <button className="btn danger" onClick={clearAll} disabled={busy || batches.length === 0}>
          一键清除全部索引
        </button>
        <button className="btn" onClick={reload} disabled={busy}>
          刷新
        </button>
        <span className="hint">
          {loaded ? `共 ${batches.length} 个批次 · ${total.toLocaleString()} 个文档` : "加载中…"}
        </span>
        {message && <span className="ok inline">{message}</span>}
        {error && <span className="error">{error}</span>}
      </div>

      <table className="rules-table">
        <thead>
          <tr>
            <th>批次名称</th>
            <th>导入目录</th>
            <th>文档数</th>
            <th>创建时间</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {batches.map((b) => (
            <tr key={b.id}>
              <td>{b.name}</td>
              <td>
                <div className="hit-path">{b.path}</div>
              </td>
              <td>{b.documents.toLocaleString()}</td>
              <td>{new Date(b.created_at).toLocaleString()}</td>
              <td>
                <button
                  className="btn"
                  onClick={() => updateOne(b)}
                  disabled={busy || updatingId !== null}
                >
                  {updatingId === b.id ? "更新中…" : "更新"}
                </button>{" "}
                <button
                  className="btn danger"
                  onClick={() => removeOne(b)}
                  disabled={busy || updatingId !== null}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {loaded && batches.length === 0 && (
            <tr>
              <td colSpan={5} className="empty">
                暂无导入记录。可在“扫描”页导入一个目录，每次导入会生成一个批次。
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
