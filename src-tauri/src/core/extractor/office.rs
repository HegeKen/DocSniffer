//! Office / PDF content extraction.
//!
//! DOCX & PPTX are ZIP archives containing XML; we pull the document XML out,
//! strip markup and decode common entities. XLSX uses `calamine` for a robust
//! cell-text walk. PDF uses `pdf-extract`.

use super::MAX_EXTRACT_BYTES;
use calamine::Reader;
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::Read;
use std::path::Path;

/// Maximum *decompressed* bytes tolerated inside a single archive (DOCX /
/// PPTX / XLSX / …). A highly-compressed zip bomb may be tiny on disk yet
/// expand by orders of magnitude; the per-entry read cap does not cover the
/// sum of many entries, so we reject the archive up front using the size
/// totals in its central directory, before anything is decompressed.
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 100 * 1024 * 1024;

/// Tighter on-disk cap for formats whose parser materialises the whole
/// document and whose streams can expand far beyond their compressed size
/// with no streaming / bounding API: PDF (`pdf-extract`) and OLE CFB legacy
/// Office/WPS (`office_oxide`).
const MAX_LEGACY_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Result of a cheap archive probe performed before invoking a full parser.
enum ArchiveProbe {
    /// File is a valid zip; total declared uncompressed size included.
    Zip(u64),
    /// File is not a zip archive (e.g. legacy OLE `.xls`/`.et`).
    NotZip,
}

/// Open the file as a zip and sum the declared uncompressed size of every
/// entry. Returns `None` if the file cannot be opened at all.
fn probe_archive(path: &Path) -> Option<ArchiveProbe> {
    let file = std::fs::File::open(path).ok()?;
    match zip::ZipArchive::new(file) {
        Ok(mut archive) => {
            let mut total = 0u64;
            for i in 0..archive.len() {
                // Declared sizes can lie on a malicious archive; the per-entry
                // `take` cap during actual extraction remains the second layer.
                total = total.saturating_add(archive.by_index(i).ok()?.size());
            }
            Some(ArchiveProbe::Zip(total))
        }
        Err(_) => Some(ArchiveProbe::NotZip),
    }
}

/// Reject zip-based office files whose declared uncompressed total exceeds
/// the bomb cap. Non-zip files are allowed through for the parser to handle.
fn archive_within_limits(path: &Path) -> bool {
    match probe_archive(path) {
        Some(ArchiveProbe::Zip(total)) => total <= MAX_ARCHIVE_TOTAL_BYTES,
        Some(ArchiveProbe::NotZip) => true,
        None => false,
    }
}

/// Read one ZIP entry as UTF-8, bounding the *decompressed* size so a
/// high-compression zip bomb cannot exhaust memory even when the archive file
/// itself is tiny. Returns `None` when the cap is exceeded.
fn read_zip_entry<R: Read>(entry: &mut R) -> Option<String> {
    let mut xml = String::new();
    entry
        .take(MAX_EXTRACT_BYTES + 1)
        .read_to_string(&mut xml)
        .ok()?;
    if xml.len() as u64 > MAX_EXTRACT_BYTES {
        return None;
    }
    Some(xml)
}

/// Extract text from a DOCX (Word) file.
pub fn read_docx(path: &Path) -> Option<String> {
    if !archive_within_limits(path) {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let xml = read_zip_entry(&mut archive.by_name("word/document.xml").ok()?)?;
    clean(xml)
}

/// Extract text from a PPTX (PowerPoint) file by concatenating slide XML.
pub fn read_pptx(path: &Path) -> Option<String> {
    if !archive_within_limits(path) {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    let mut total = 0u64;
    let mut chunks = Vec::new();
    for name in names {
        if !name.starts_with("ppt/slides/slide") || !name.ends_with(".xml") {
            continue;
        }
        if let Ok(mut f) = archive.by_name(&name) {
            if let Some(xml) = read_zip_entry(&mut f) {
                total += xml.len() as u64;
                if total > MAX_EXTRACT_BYTES {
                    return None;
                }
                chunks.push(xml);
            }
        }
    }
    clean(chunks.join("\n"))
}

/// Extract text from a legacy WPS Writer document (`.wps`).
///
/// WPS `.wps` files are OLE CFB compound documents laid out like classic Word
/// `.doc` (a `WordDocument` stream plus a CLX piece table), but carry the
/// `.wps` extension that `office_oxide`'s extension-based detection does not
/// recognise. We therefore open them with an explicit `DocumentFormat::Doc`.
pub fn read_wps(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > MAX_LEGACY_FILE_BYTES {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let doc =
        office_oxide::Document::from_reader(file, office_oxide::DocumentFormat::Doc).ok()?;
    let text = doc.plain_text();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract text from a legacy WPS Presentation document (`.dps`).
///
/// WPS `.dps` files are OLE CFB compound documents laid out like classic
/// PowerPoint `.ppt`; they are opened explicitly as `DocumentFormat::Ppt`
/// because the `.dps` extension is not in `office_oxide`'s extension table.
pub fn read_dps(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > MAX_LEGACY_FILE_BYTES {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let doc =
        office_oxide::Document::from_reader(file, office_oxide::DocumentFormat::Ppt).ok()?;
    let text = doc.plain_text();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract text from an Excel/WPS-spreadsheet workbook (XLSX/XLSM/ET) via
/// `calamine`. `open_workbook_auto` probes Xls/Xlsx/Xlsb/Ods readers in turn
/// when the `.et` extension is not in its known list.
pub fn read_xlsx(path: &Path) -> Option<String> {
    // Zip-based workbooks: reject zip-bombs from their declared totals up
    // front. Legacy OLE workbooks (`.xls`, some `.et`) are not zips and are
    // left to calamine (the generic 50MB metadata gate already applies).
    if !archive_within_limits(path) {
        return None;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut workbook = calamine::open_workbook_auto(path).ok()?;
        let sheets = workbook.sheet_names().to_vec();
        if sheets.is_empty() {
            return None;
        }
        let mut out = String::new();
        'sheets: for name in sheets {
            let range = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| workbook.worksheet_range(&name))) {
                Ok(value) => match value {
                    Ok(range) => range,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            for row in range.rows() {
                // Bound our own accumulator: calamine keeps the whole sheet
                // resident, and joining every cell of a huge workbook into a
                // second giant String doubles the peak.
                if out.len() as u64 >= MAX_EXTRACT_BYTES {
                    break 'sheets;
                }
                let mut first = true;
                for cell in row {
                    let s = cell.to_string();
                    if !first {
                        out.push(' ');
                    }
                    out.push_str(&s);
                    first = false;
                }
                out.push('\n');
            }
        }
        clean(out)
    }));

    match result {
        Ok(text) => text,
        Err(_) => None,
    }
}

/// Extract text from a PDF file.
pub fn read_pdf(path: &Path) -> Option<String> {
    // pdf-extract materialises decompressed page streams with no streaming
    // bound, so keep the on-disk gate tighter than the general cap.
    if std::fs::metadata(path).ok()?.len() > MAX_LEGACY_FILE_BYTES {
        return None;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match pdf_extract::extract_text(path) {
            Ok(text) if !text.trim().is_empty() => Some(text),
            _ => None,
        }
    }));

    match result {
        Ok(text) => text,
        Err(_) => None,
    }
}

/// Compiled once and shared by every document extraction (this used to be
/// recompiled on every call, which dominated scans of document-heavy folders).
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Strip XML markup, translate paragraph boundaries to newlines and decode the
/// handful of XML/HTML entities that appear in Office files.
fn clean(xml: String) -> Option<String> {
    if xml.is_empty() {
        return None;
    }
    let mut s = xml;
    s = s.replace("</w:p>", "\n").replace("</a:p>", "\n").replace("</w:tc>", " ");
    s = s.replace("</w:tr>", "\n").replace("</a:tr>", "\n");

    // Remove tags but keep their inner text.
    let s = TAG_RE.replace_all(&s, "");

    let s = s
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
