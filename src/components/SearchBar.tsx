import { useState } from "react";

interface Props {
  onSearch: (query: string) => void;
  busy: boolean;
}

export default function SearchBar({ onSearch, busy }: Props) {
  const [value, setValue] = useState("");

  const submit = () => {
    if (value.trim()) onSearch(value.trim());
  };

  return (
    <div className="search-bar">
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
