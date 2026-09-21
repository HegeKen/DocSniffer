//! Index manager (Tantivy writer) and the custom CJK tokenizer.
//!
//! The tokenizer chops text into ASCII alphanumeric runs plus *each* CJK
//! character as an individual unigram token. This provides usable Chinese
//! search without pulling an external segmentation library.

use crate::core::indexer::schema::{build_schema, Fields, TOKENIZER_ZH};
use crate::core::scanner::FileEntry;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{Query, QueryParser, TermQuery};
use tantivy::schema::{IndexRecordOption, Schema, Value};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

/// A unigram, CJK-aware tokenizer (see module docs).
#[derive(Clone, Default)]
pub struct CjkTokenizer;

impl Tokenizer for CjkTokenizer {
    type TokenStream<'a> = CjkTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        CjkTokenStream {
            text,
            byte_idx: 0,
            position: 0,
            token: Token {
                offset_from: 0,
                offset_to: 0,
                position: 0,
                position_length: 1,
                text: String::new(),
            },
        }
    }
}

/// Streaming tokenizer state.
///
/// IMPORTANT: this must NOT materialise the whole token vector up front. An
/// earlier implementation built `Vec<Token>` with one heap-allocated `String`
/// per CJK character, so a 50MB Chinese document (~17M chars) needed roughly
/// 1.5GB just for tokens — a guaranteed OOM on big files. Here exactly one
/// `Token` exists at a time and its `text`
/// buffer is reused across advances.
pub struct CjkTokenStream<'a> {
    text: &'a str,
    byte_idx: usize,
    position: usize,
    token: Token,
}

impl TokenStream for CjkTokenStream<'_> {
    fn advance(&mut self) -> bool {
        let bytes = self.text.as_bytes();
        while self.byte_idx < bytes.len() {
            let ch = self.text[self.byte_idx..].chars().next().unwrap();
            let len = ch.len_utf8();

            if ch.is_ascii_alphanumeric() {
                let start = self.byte_idx;
                let mut j = self.byte_idx + len;
                while j < bytes.len() {
                    let c2 = self.text[j..].chars().next().unwrap();
                    if c2.is_ascii_alphanumeric() {
                        j += c2.len_utf8();
                    } else {
                        break;
                    }
                }
                self.emit(start, j, bytes[start..j].iter().map(|b| b.to_ascii_lowercase() as char));
                self.byte_idx = j;
                return true;
            } else if (ch as u32) >= 0x2e80 && !ch.is_whitespace() {
                let start = self.byte_idx;
                self.emit(start, start + len, ch.to_lowercase());
                self.byte_idx += len;
                return true;
            } else {
                self.byte_idx += len;
            }
        }
        false
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

impl CjkTokenStream<'_> {
    /// Refill the single reusable token for the byte range `[from, to)`.
    fn emit(&mut self, from: usize, to: usize, lowercased: impl Iterator<Item = char>) {
        self.token.offset_from = from;
        self.token.offset_to = to;
        self.token.position = self.position;
        self.token.position_length = 1;
        self.token.text.clear();
        self.token.text.extend(lowercased);
        self.position += 1;
    }
}

/// A stable document id derived from the file path (used for dedup / deletes).
pub fn id_of(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Batch id assigned to documents that were never part of an explicit import
/// batch (e.g. legacy docs re-indexed by the file watcher).
const DEFAULT_BATCH: &str = "default";

/// Maximum number of characters of file content sent to the indexer for one
/// document. The extractor may return up to 50MB of text (~17M Chinese chars);
/// tokenising and building position postings for all of it can transiently
/// need hundreds of MB per *single* document. Content beyond this cap is
/// skipped (the file is still fully indexed by metadata), which bounds peak
/// indexing memory regardless of input file size. 2M chars ≈ 6MB of CJK text.
const MAX_INDEX_CHARS: usize = 2_000_000;

/// Tantivy writer indexing heap. Segments are flushed when this arena fills,
/// so it also bounds the in-memory index size.
const WRITER_HEAP_BYTES: usize = 64_000_000;

/// Truncate `text` to [`MAX_INDEX_CHARS`] on a UTF-8 boundary.
fn truncate_indexed(text: &str) -> &str {
    match text.char_indices().nth(MAX_INDEX_CHARS) {
        Some((byte_idx, _)) => &text[..byte_idx],
        None => text,
    }
}

/// Owns the Tantivy index, its writer and the schema/field handles.
pub struct IndexManager {
    pub index: Index,
    pub writer: Mutex<IndexWriter>,
    pub fields: Fields,
    pub schema: Schema,
    index_dir: PathBuf,
}

impl IndexManager {
    /// Open (or create) an index living in `index_dir` and register the custom
    /// tokenizer. Tokenizers are not persisted, so they must be re-registered
    /// on every open.
    pub fn open(index_dir: &Path) -> tantivy::Result<Self> {
        std::fs::create_dir_all(index_dir)?;
        let (schema, fields) = build_schema();

        let index = if index_dir.join("meta.json").exists() {
            Index::open_in_dir(index_dir)?
        } else {
            Index::create_in_dir(index_dir, schema.clone())?
        };
        index.tokenizers().register(TOKENIZER_ZH, CjkTokenizer);

        let writer = index.writer(WRITER_HEAP_BYTES)?;
        Ok(Self {
            index,
            writer: Mutex::new(writer),
            fields,
            schema,
            index_dir: index_dir.to_path_buf(),
        })
    }

    /// The directory this index lives in (authoritative, even after the user
    /// moves the index to a custom location).
    pub fn dir(&self) -> &Path {
        &self.index_dir
    }

    /// Re-create the index directory if it has been deleted while the app was
    /// running (e.g. the user removed `DocSniffer/index` by hand). Tantivy
    /// needs the directory to exist before it can write temp files or commit.
    fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.index_dir)
    }

    pub fn reader(&self) -> tantivy::Result<IndexReader> {
        self.index.reader()
    }

    /// Add one file to the index, with optional extracted text content.
    /// `content: None` simply means the file has no searchable text.
    /// `batch_id` tags the document so it can be managed (counted / deleted)
    /// per import batch.
    pub fn add_file(
        &self,
        fe: &FileEntry,
        content: Option<String>,
        batch_id: &str,
    ) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let id = id_of(&fe.path);
        let mut doc = TantivyDocument::new();
        doc.add_text(self.fields.id, id);
        doc.add_text(self.fields.path, fe.path.clone());
        doc.add_text(self.fields.name, fe.name.clone());
        if let Some(text) = content {
            doc.add_text(self.fields.content, truncate_indexed(&text));
        }
        doc.add_text(self.fields.ext, fe.ext.clone());
        doc.add_text(self.fields.batch_id, batch_id.to_string());
        doc.add_u64(self.fields.size, fe.size);
        doc.add_u64(self.fields.mtime, fe.mtime);

        let w = self.writer.lock().unwrap();
        w.add_document(doc).map(|_| ())
    }

    /// Delete a previously indexed file by `id` (path hash) and re-add it,
    /// preserving its original import batch (used by the file watcher).
    pub fn upsert_file(&self, fe: &FileEntry, content: Option<String>) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let id = id_of(&fe.path);
        let batch_id = self.batch_of(&id).unwrap_or_else(|| DEFAULT_BATCH.to_string());
        {
            let w = self.writer.lock().unwrap();
            let term = tantivy::Term::from_field_text(self.fields.id, &id);
            w.delete_term(term);
        }
        self.add_file(fe, content, &batch_id)
    }

    pub fn delete_by_path(&self, path: &str) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let id = id_of(path);
        let term = tantivy::Term::from_field_text(self.fields.id, &id);
        let w = self.writer.lock().unwrap();
        w.delete_term(term);
        Ok(())
    }

    /// Look up the batch a document currently belongs to (based on its stored
    /// `batch_id` field). Returns `None` for documents with no batch set.
    fn batch_of(&self, id: &str) -> Option<String> {
        let reader = self.index.reader().ok()?;
        let searcher = reader.searcher();
        let term = Term::from_field_text(self.fields.id, id);
        let query = TermQuery::new(term, IndexRecordOption::Basic);
        let top = searcher.search(&query, &TopDocs::with_limit(1)).ok()?;
        let (_, addr) = top.into_iter().next()?;
        let doc: TantivyDocument = searcher.doc(addr).ok()?;
        doc.get_first(self.fields.batch_id)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Delete every document tagged with `batch_id`.
    pub fn delete_by_batch(&self, batch_id: &str) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let term = Term::from_field_text(self.fields.batch_id, batch_id);
        let w = self.writer.lock().unwrap();
        w.delete_term(term);
        Ok(())
    }

    /// Count the number of documents currently tagged with `batch_id`.
    pub fn count_by_batch(&self, batch_id: &str) -> tantivy::Result<usize> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let term = Term::from_field_text(self.fields.batch_id, batch_id);
        let query = TermQuery::new(term, IndexRecordOption::Basic);
        searcher.search(&query, &Count)
    }

    pub fn commit(&self) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let mut w = self.writer.lock().unwrap();
        w.commit().map(|_| ())
    }

    /// Remove every document and compact the index.
    pub fn clear(&self) -> tantivy::Result<()> {
        self.ensure_dir()?;
        let mut w = self.writer.lock().unwrap();
        w.delete_all_documents()?;
        w.commit()?;
        Ok(())
    }

    /// Build a `QueryParser` over the text fields.
    pub fn parse_query(&self, q: &str) -> tantivy::Result<Box<dyn Query>> {
        let parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.name, self.fields.content, self.fields.path],
        );
        parser.parse_query(q).map_err(Into::into)
    }
}
