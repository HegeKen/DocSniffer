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

/** Result of a finished scan (mirrors Rust `ScanReport`). */
export interface ScanReport {
  indexed: number;
  warnings: string[];
}

export interface IndexStatus {
  documents: number;
  index_dir: string;
}

/** Result of moving the index to a custom directory (mirrors Rust `IndexMoveReport`). */
export interface IndexMoveReport {
  index_dir: string;
  documents: number;
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

// ---- Commands ----

export const scanDirectory = (
  path: string,
  includeContent: boolean,
  batchName?: string,
): Promise<ScanReport> => invoke<ScanReport>("scan_directory", { path, includeContent, batchName });

export const indexStatus = (): Promise<IndexStatus> => invoke<IndexStatus>("index_status");

export const searchFiles = (
  query: string,
  limit?: number,
  batchId?: string,
): Promise<SearchResult[]> =>
  invoke<SearchResult[]>("search_files", { query, limit: limit ?? 200, batchId });

export const getRules = (): Promise<Rule[]> => invoke<Rule[]>("get_rules");

export const saveRules = (rules: Rule[]): Promise<void> => invoke<void>("save_rules", { rules });

export const sensitiveScan = (
  path: string,
  includeContent: boolean,
  activeRules?: Rule[],
): Promise<Hit[]> =>
  invoke<Hit[]>("sensitive_scan", { path, includeContent, activeRules });

export const listBatches = (): Promise<BatchItem[]> => invoke<BatchItem[]>("list_batches");

export const deleteBatch = (batchId: string): Promise<number> =>
  invoke<number>("delete_batch", { batchId });

export const clearAllIndex = (): Promise<void> => invoke<void>("clear_all_index");

export const updateBatch = (batchId: string): Promise<number> =>
  invoke<number>("update_batch", { batchId });

/** Reveal the active index directory in the system file manager. */
export const openIndexDir = (): Promise<string> => invoke<string>("open_index_dir");

/** Move the index to `dir`, migrating the existing index data. */
export const setIndexDir = (dir: string): Promise<IndexMoveReport> =>
  invoke<IndexMoveReport>("set_index_dir", { dir });

/** Open `path` with the OS default application (folders open in the file manager). */
export const openPath = (path: string): Promise<void> => invoke<void>("open_path", { path });

// ---- Events ----

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<() => void> =>
  listen<ScanProgress>("scan-progress", (e) => cb(e.payload));
