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
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let xml = read_zip_entry(&mut archive.by_name("word/document.xml").ok()?)?;
    clean(xml)
}

/// Extract text from a PPTX (PowerPoint) file by concatenating slide XML.
pub fn read_pptx(path: &Path) -> Option<String> {
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
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut workbook = calamine::open_workbook_auto(path).ok()?;
        let sheets = workbook.sheet_names().to_vec();
        if sheets.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for name in sheets {
            let range = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| workbook.worksheet_range(&name))) {
                Ok(value) => match value {
                    Ok(range) => range,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            for row in range.rows() {
                let line: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                if !line.is_empty() {
                    out.push(line.join(" "));
                }
            }
        }
        clean(out.join("\n"))
    }));

    match result {
        Ok(text) => text,
        Err(_) => None,
    }
}

/// Extract text from a PDF file.
pub fn read_pdf(path: &Path) -> Option<String> {
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
