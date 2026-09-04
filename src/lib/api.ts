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

export const scanDirectory = (path: string, includeContent: boolean): Promise<number> =>
  invoke<number>("scan_directory", { path, includeContent });

export const indexStatus = (): Promise<IndexStatus> => invoke<IndexStatus>("index_status");

export const searchFiles = (query: string, limit?: number): Promise<SearchResult[]> =>
  invoke<SearchResult[]>("search_files", { query, limit: limit ?? 200 });

export const getRules = (): Promise<Rule[]> => invoke<Rule[]>("get_rules");

export const saveRules = (rules: Rule[]): Promise<void> =>
  invoke<void>("save_rules", { rules });

export const sensitiveScan = (
  path: string,
  includeContent: boolean,
  activeRules?: Rule[],
): Promise<Hit[]> =>
  invoke<Hit[]>("sensitive_scan", { path, includeContent, activeRules });

// ---- Events ----

export const onScanProgress = (
  cb: (p: ScanProgress) => void,
): Promise<() => void> => listen<ScanProgress>("scan-progress", (e) => cb(e.payload));
