import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ---- Shared types (mirror the Rust `Serialize` structs) ----

export interface SearchResult {
  id: string;
  path: string;
  name: string;
  ext: string;
  size: number;
  mtime: number;
  score: number;
  snippet: string;
}

export interface ScanProgress {
  indexed: number;
  total: number;
  path: string;
}

export interface IndexStatus {
  documents: number;
  data_dir: string;
}

export interface BatchItem {
  id: string;
  name: string;
  path: string;
  created_at: number;
  documents: number;
}

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface Rule {
  id: string;
  name: string;
  pattern: string;
  type: "regex" | "keyword";
  risk_level: string;
  scope: string[];
}

export interface Hit {
  path: string;
  file_name: string;
  rule_id: string;
  rule_name: string;
  rule_type: string;
  risk_level: string;
  match_type: string;
  matched: string;
  file_size: number;
  mtime: number;
}

// ---- Transport ----
//
// The same web UI runs in two environments:
// - Tauri desktop app: commands go over Tauri IPC (Windows 10+ / macOS / Linux).
// - `docsniffer-server` (headless mode, e.g. Windows 7): commands go over the
//   local HTTP API (see src-tauri/src/server/), with scan progress polled from
//   /api/scan_status instead of pushed as an event.

const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function httpJson<T>(method: "GET" | "POST", url: string, body?: unknown): Promise<T> {
  const res = await fetch(url, {
    method,
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  let payload: unknown = null;
  try {
    payload = await res.json();
  } catch {
    // non-JSON error page
  }
  if (!res.ok) {
    const msg =
      payload && typeof payload === "object" && "error" in payload
        ? String((payload as { error: unknown }).error)
        : `请求失败 (HTTP ${res.status})`;
    throw new Error(msg);
  }
  return payload as T;
}

const httpGet = <T,>(url: string) => httpJson<T>("GET", url);
const httpPost = <T,>(url: string, body?: unknown) => httpJson<T>("POST", url, body);

// ---- Commands ----

export const scanDirectory = (
  path: string,
  includeContent: boolean,
  batchName?: string,
): Promise<number> =>
  inTauri()
    ? invoke<number>("scan_directory", { path, includeContent, batchName })
    : startScanHttp(path, includeContent, batchName);

export const indexStatus = (): Promise<IndexStatus> =>
  inTauri()
    ? invoke<IndexStatus>("index_status")
    : httpGet<IndexStatus>("/api/index_status");

export const searchFiles = (
  query: string,
  limit?: number,
  batchId?: string,
): Promise<SearchResult[]> =>
  inTauri()
    ? invoke<SearchResult[]>("search_files", { query, limit: limit ?? 200, batchId })
    : httpPost<SearchResult[]>("/api/search_files", {
        query,
        limit: limit ?? 200,
        batch_id: batchId,
      });

export const getRules = (): Promise<Rule[]> =>
  inTauri() ? invoke<Rule[]>("get_rules") : httpGet<Rule[]>("/api/rules");

export const saveRules = (rules: Rule[]): Promise<void> =>
  inTauri()
    ? invoke<void>("save_rules", { rules })
    : httpPost<{ saved: boolean }>("/api/rules", { rules }).then(() => undefined);

export const sensitiveScan = (
  path: string,
  includeContent: boolean,
  activeRules?: Rule[],
): Promise<Hit[]> =>
  inTauri()
    ? invoke<Hit[]>("sensitive_scan", { path, includeContent, activeRules })
    : httpPost<Hit[]>("/api/sensitive_scan", {
        path,
        include_content: includeContent,
        active_rules: activeRules,
      });

export const listBatches = (): Promise<BatchItem[]> =>
  inTauri() ? invoke<BatchItem[]>("list_batches") : httpGet<BatchItem[]>("/api/batches");

export const deleteBatch = (batchId: string): Promise<number> =>
  inTauri()
    ? invoke<number>("delete_batch", { batchId })
    : httpPost<number>("/api/delete_batch", { batch_id: batchId });

export const clearAllIndex = (): Promise<void> =>
  inTauri()
    ? invoke<void>("clear_all_index")
    : httpPost<{ cleared: boolean }>("/api/clear_all_index").then(() => undefined);

export const updateBatch = (batchId: string): Promise<number> =>
  inTauri()
    ? invoke<number>("update_batch", { batchId })
    : httpPost<number>("/api/update_batch", { batch_id: batchId });

// ---- Events ----

type ProgressCb = (p: ScanProgress) => void;
const progressSubs = new Set<ProgressCb>();

export const onScanProgress = (cb: ProgressCb): Promise<() => void> => {
  if (inTauri()) {
    return listen<ScanProgress>("scan-progress", (e) => cb(e.payload));
  }
  progressSubs.add(cb);
  return Promise.resolve(() => {
    progressSubs.delete(cb);
  });
};

// ---- HTTP-mode scan plumbing ----

/** Shape of GET /api/scan_status (see src-tauri/src/server/api.rs). */
interface ScanStatusHttp {
  running: boolean;
  indexed: number;
  total: number;
  path: string;
  outcome: { ok?: number; err?: string } | null;
}

const SCAN_POLL_MS = 300;

async function startScanHttp(
  path: string,
  includeContent: boolean,
  batchName?: string,
): Promise<number> {
  await httpPost("/api/scan_directory", {
    path,
    include_content: includeContent,
    batch_name: batchName,
  });

  // Poll the job until it finishes, forwarding progress to subscribers (the
  // same callbacks that Tauri feeds with "scan-progress" events).
  let sawRunning = false;
  let idleTicks = 0;
  for (;;) {
    await new Promise((r) => setTimeout(r, SCAN_POLL_MS));
    const s = await httpGet<ScanStatusHttp>("/api/scan_status");
    if (s.running) sawRunning = true;
    const progress: ScanProgress = { indexed: s.indexed, total: s.total, path: s.path };
    progressSubs.forEach((cb) => cb(progress));
    if (!s.running && s.outcome) {
      if (s.outcome.err !== undefined) throw new Error(s.outcome.err);
      return s.outcome.ok ?? 0;
    }
    if (!s.running && !s.outcome) {
      // Outcome not published yet; give up eventually in case the server was
      // restarted mid-scan.
      if (sawRunning || ++idleTicks > 100) {
        throw new Error("扫描任务状态未知（服务可能已重启）");
      }
    }
  }
}
