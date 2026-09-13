import { useEffect, useState } from "react";
import { BatchItem, listBatches } from "../lib/api";

interface Props {
  onSearch: (query: string, batchId: string) => void;
  busy: boolean;
}

export default function SearchBar({ onSearch, busy }: Props) {
  const [value, setValue] = useState("");
  const [batchId, setBatchId] = useState("");
  const [batches, setBatches] = useState<BatchItem[]>([]);

  useEffect(() => {
    listBatches()
      .then(setBatches)
      .catch(() => setBatches([]));
  }, []);

  const submit = () => {
    if (value.trim()) onSearch(value.trim(), batchId);
  };

  return (
    <div className="search-bar">
      <select
        className="search-input batch-select"
        value={batchId}
        onChange={(e) => setBatchId(e.target.value)}
        title="在此索引（导入批次）范围内搜索"
      >
        <option value="">全部索引</option>
        {(batches ?? []).map((b) => (
          <option key={b.id} value={b.id}>
            {b.name}（{b.documents.toLocaleString()} 个文档）
          </option>
        ))}
      </select>
      <input
        className="search-input"
        placeholder='搜索文件名 / 内容，支持高级语法如 path:reports ext:pdf size:>10mb -draft'
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
      />
      <button className="btn primary" onClick={submit} disabled={busy}>
        {busy ? "搜索中…" : "搜索"}
      </button>
    </div>
  );
}
