import { useCallback, useEffect, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import {
  BatchItem,
  clearAllIndex,
  deleteBatch,
  indexStatus,
  listBatches,
  openIndexDir,
  setIndexDir,
  updateBatch,
} from "../lib/api";

export default function BatchManager({ onChanged }: { onChanged?: () => void }) {
  const [indexDir, setIndexDirPath] = useState("");
  const [batches, setBatches] = useState<BatchItem[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [updatingId, setUpdatingId] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const [status, list] = await Promise.all([indexStatus(), listBatches()]);
      setIndexDirPath(status.index_dir);
      setBatches(list);
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

  const revealDir = async () => {
    try {
      await openIndexDir();
    } catch (e) {
      setError(`打开目录失败：${e}`);
    }
  };

  const changeDir = async () => {
    setMessage("");
    setError("");
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择索引存储位置",
      defaultPath: indexDir,
    });
    if (!picked || picked === indexDir) return;
    const ok = await ask(
      `将把索引从\n${indexDir}\n迁移到\n${picked}\n并切换到新位置，原目录下的索引会被清理。是否继续？`,
      { title: "迁移索引", kind: "warning" },
    );
    if (!ok) return;

    setBusy(true);
    try {
      const report = await setIndexDir(picked);
      setMessage(`索引已迁移到 ${report.index_dir}（${report.documents.toLocaleString()} 个文档）。`);
      await reload();
      onChanged?.();
    } catch (e) {
      setError(`迁移失败：${e}`);
    } finally {
      setBusy(false);
    }
  };

  const updateOne = async (batch: BatchItem) => {
    const ok = await ask(
      `重新扫描「${batch.name}」的目录并更新其索引？将重新索引当前目录下的全部文件。`,
      { title: "更新索引", kind: "warning" },
    );
    if (!ok) return;
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
    const ok = await ask(
      `确认删除批次「${batch.name}」及其全部 ${batch.documents} 个文档？`,
      { title: "删除批次", kind: "warning" },
    );
    if (!ok) return;
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
    const ok = await ask("确认清除现有全部索引和所有批次？此操作不可撤销。", {
      title: "清除全部索引",
      kind: "warning",
    });
    if (!ok) return;
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
      <h2>设置</h2>

      <section className="settings-block">
        <h3>索引存储位置</h3>
        <div className="settings-path" title={indexDir}>
          {indexDir || "加载中…"}
        </div>
        <div className="toolbar">
          <button className="btn" onClick={revealDir} disabled={!indexDir}>
            打开目录
          </button>
          <button className="btn" onClick={changeDir} disabled={busy || !indexDir}>
            更改位置…
          </button>
          <span className="hint">
            更改位置会把现有索引迁移到所选目录，此后索引均从该目录读写。
          </span>
        </div>
      </section>

      <section className="settings-block">
        <h3>索引批次</h3>
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
        </div>

        {message && <div className="ok">{message}</div>}
        {error && <div className="error">{error}</div>}

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
      </section>
    </div>
  );
}
