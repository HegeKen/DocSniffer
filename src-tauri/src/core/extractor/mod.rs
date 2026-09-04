//! Content extraction module.
//!
//! Turns binary / text documents into plain text suitable for full-text
//! indexing. Supported families:
//!   - plain text / source code (encoding auto-detected)
//!   - Office: DOCX, XLSX, PPTX
//!   - PDF

pub mod office;
pub mod text;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

/// Extract searchable plain text from `path` based on its extension.
///
/// Returns `None` when the format is unsupported or extraction fails; the
/// caller simply indexes the file without a content field in that case.
pub fn extract_text(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    // Extraction crates (pdf-extract, calamine, zip/quick-xml) may panic on
    // malformed or unsupported documents — e.g. PDFs with a CJK
    // `UniGB-UCS2-H` CMap. A panic escaping into an indexer/scanner worker
    // thread would abort the whole process, so contain it here and treat the
    // file as non-indexable instead.
    match catch_unwind(AssertUnwindSafe(|| match ext.as_str() {
        // Text & source-code families
        "txt" | "md" | "markdown" | "csv" | "log" | "json" | "xml" | "html" | "htm"
        | "shtml" | "yml" | "yaml" | "toml" | "ini" | "conf" | "cfg" | "properties"
        | "env" | "py" | "rs" | "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" | "c"
        | "cpp" | "h" | "hpp" | "cs" | "java" | "go" | "sh" | "bash" | "zsh" | "sql"
        | "r" | "rb" | "php" | "swift" | "kt" | "scala" | "vue" | "css" | "scss"
        | "less" | "graphql" => text::read_text(path),

        // Office documents
        "docx" | "word" => office::read_docx(path),
        "xlsx" | "xlsm" => office::read_xlsx(path),
        "pptx" => office::read_pptx(path),

        // PDF
        "pdf" => office::read_pdf(path),

        _ => None,
    })) {
        Ok(out) => out,
        Err(_) => None,
    }
}
