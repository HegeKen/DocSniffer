//! Content extraction module.
//!
//! Turns binary / text documents into plain text suitable for full-text
//! indexing. Supported families:
//!   - plain text / source code (encoding auto-detected)
//!   - Office: DOCX, XLSX, PPTX
//!   - PDF

pub mod office;
pub mod text;

use std::path::Path;

/// Maximum decoded text size extracted from a single file. Files larger than
/// this are still indexed by metadata, but their content is skipped — a single
/// multi-gigabyte log (or a zip bomb) must never be read fully into memory.
pub const MAX_EXTRACT_BYTES: u64 = 50 * 1024 * 1024;

/// Extract searchable plain text from `path` based on its extension.
///
/// Returns `None` when the format is unsupported, extraction fails, or the
/// file exceeds [`MAX_EXTRACT_BYTES`]; the caller simply indexes the file
/// without a content field in that case.
pub fn extract_text(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();

    // Cheap metadata gate before any parser touches the file.
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_EXTRACT_BYTES {
        return None;
    }

    // Extraction crates may panic on malformed / legacy / encoding-heavy docs
    // (e.g. PDFs with a `UniGB-UCS2-H` CMap or damaged XLS files). The panic
    // must be contained at the extraction boundary so it is treated as a
    // non-indexable file instead of crashing the process.
    fn safe_extract<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok().flatten()
    }

    safe_extract(|| match ext.as_str() {
        // Text & source-code families
        "txt" | "md" | "markdown" | "csv" | "log" | "json" | "xml" | "html" | "htm"
        | "shtml" | "yml" | "yaml" | "toml" | "ini" | "conf" | "cfg" | "properties"
        | "env" | "py" | "rs" | "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" | "c"
        | "cpp" | "h" | "hpp" | "cs" | "java" | "go" | "sh" | "bash" | "zsh" | "sql"
        | "r" | "rb" | "php" | "swift" | "kt" | "scala" | "vue" | "css" | "scss"
        | "less" | "graphql" => text::read_text(path),

        // Office documents
        "docx" | "word" => office::read_docx(path),
        "wps" => office::read_wps(path),
        "dps" => office::read_dps(path),
        "xlsx" | "xlsm" | "et" => office::read_xlsx(path),
        "pptx" => office::read_pptx(path),

        // PDF
        "pdf" => office::read_pdf(path),

        _ => None,
    })
}
