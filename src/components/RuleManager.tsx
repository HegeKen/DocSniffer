import { useEffect, useState } from "react";
import { getRules, saveRules, sensitiveScan, Hit, Rule } from "../lib/api";

export default function RuleManager() {
  const [rules, setRules] = useState<Rule[]>([]);
  const [scanPath, setScanPath] = useState("");
  const [includeContent, setIncludeContent] = useState(true);
  const [hits, setHits] = useState<Hit[]>([]);
  const [busyScan, setBusyScan] = useState(false);
  const [message, setMessage] = useState("");
  const [dirty, setDirty] = useState(false);

  const reload = async () => {
    try {
      setRules(await getRules());
    } catch {
      setRules([]);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const update = (idx: number, patch: Partial<Rule>) => {
    setRules((prev) => prev.map((r, i) => (i === idx ? { ...r, ...patch } : r)));
    setDirty(true);
  };

  const addRule = () => {
    setRules((prev) => [
      ...prev,
      { id: `rule_${Date.now()}`, name: "新规则", pattern: "", type: "regex", risk_level: "medium", scope: ["content", "filename"] },
    ]);
    setDirty(true);
  };

  const removeRule = (idx: number) => {
    setRules((prev) => prev.filter((_, i) => i !== idx));
    setDirty(true);
  };

  const persist = async () => {
    try {
      await saveRules(rules);
      setDirty(false);
      setMessage("规则已保存。");
    } catch (e) {
      setMessage(`保存失败：${e}`);
    }
  };

  const runScan = async () => {
    if (!scanPath.trim()) return;
    setBusyScan(true);
    setMessage("");
    try {
      const res = await sensitiveScan(scanPath.trim(), includeContent, rules);
      setHits(res);
      setMessage(`检测完成，发现 ${res.length} 处敏感命中。`);
    } catch (e) {
      setMessage(`检测失败：${e}`);
    } finally {
      setBusyScan(false);
    }
  };

  return (
    <div className="panel">
      <h2>敏感规则配置</h2>
      <div className="toolbar">
        <button className="btn" onClick={addRule}>
          添加规则
        </button>
        <button className="btn primary" onClick={persist} disabled={!dirty}>
          保存规则
        </button>
        <span className="hint">{dirty ? "有未保存的修改" : ""}</span>
        {message && <span className="ok inline">{message}</span>}
      </div>

      <table className="rules-table">
        <thead>
          <tr>
            <th>名称</th>
            <th>类型</th>
            <th>风险</th>
            <th>作用范围</th>
            <th>正则 / 关键词</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {rules.map((r, i) => (
            <tr key={r.id}>
              <td>
                <input value={r.name} onChange={(e) => update(i, { name: e.target.value })} />
              </td>
              <td>
                <select value={r.type} onChange={(e) => update(i, { type: e.target.value as Rule["type"] })}>
                  <option value="regex">regex</option>
                  <option value="keyword">keyword</option>
                </select>
              </td>
              <td>
                <select value={r.risk_level} onChange={(e) => update(i, { risk_level: e.target.value })}>
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              </td>
              <td>
                <label>
                  <input type="checkbox" checked={r.scope.includes("filename")} onChange={(e) => update(i, { scope: toggleScope(r.scope, "filename", e.target.checked) })} />
                  文件名
                </label>
                <label>
                  <input type="checkbox" checked={r.scope.includes("content")} onChange={(e) => update(i, { scope: toggleScope(r.scope, "content", e.target.checked) })} />
                  内容
                </label>
              </td>
              <td>
                <input value={r.pattern} onChange={(e) => update(i, { pattern: e.target.value })} />
              </td>
              <td>
                <button className="btn danger" onClick={() => removeRule(i)}>
                  删除
                </button>
              </td>
            </tr>
          ))}
          {rules.length === 0 && (
            <tr>
              <td colSpan={6} className="empty">暂无规则，可点击“添加规则”。</td>
            </tr>
          )}
        </tbody>
      </table>

      <h2>执行检测</h2>
      <div className="form-row inline">
        <input
          className="search-input"
          placeholder="输入要检测的路径（文件或目录）"
          value={scanPath}
          onChange={(e) => setScanPath(e.target.value)}
        />
        <label>
          <input type="checkbox" checked={includeContent} onChange={(e) => setIncludeContent(e.target.checked)} />
          检测文件内容
        </label>
        <button className="btn primary" onClick={runScan} disabled={busyScan}>
          {busyScan ? "检测中…" : "开始检测"}
        </button>
      </div>

      {hits.length > 0 && (
        <div className="hits">
          {hits.map((h, i) => (
            <div className="hit-item" key={i}>
              <div className="hit-title">
                <span className={`risk risk-${h.risk_level}`}>{h.risk_level}</span>
                <b>{h.rule_name}</b>
                <span className="hit-kind">[{h.match_type}]</span>
              </div>
              <div className="hit-path">{h.path}</div>
              <div className="hit-matched">{h.matched}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function toggleScope(scope: string[], key: string, on: boolean): string[] {
  const set = new Set(scope);
  if (on) set.add(key);
  else set.delete(key);
  return Array.from(set);
}
