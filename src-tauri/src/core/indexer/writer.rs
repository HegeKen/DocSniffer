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
use tantivy::tokenizer::{BoxTokenStream, Token, TokenStream, Tokenizer};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

/// A unigram, CJK-aware tokenizer (see module docs).
#[derive(Clone, Default)]
pub struct CjkTokenizer;

impl Tokenizer for CjkTokenizer {
    type TokenStream<'a> = BoxTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        BoxTokenStream::new(CjkTokenStream {
            tokens: tokenize_cjk(text),
            idx: 0,
        })
    }
}

struct CjkTokenStream {
    tokens: Vec<Token>,
    idx: usize,
}

impl TokenStream for CjkTokenStream {
    fn advance(&mut self) -> bool {
        if self.idx < self.tokens.len() {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.idx - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.idx - 1]
    }
}

/// Produce a token vector: runs of ASCII alphanumerics, and each non-ASCII
/// ideographic character as a separate token.
fn tokenize_cjk(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut position = 0usize;
    let mut i = 0usize;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        let ch = text[i..].chars().next().unwrap();
        let len = ch.len_utf8();

        if ch.is_ascii_alphanumeric() {
            let start = i;
            let mut j = i + len;
            while j < bytes.len() {
                let c2 = text[j..].chars().next().unwrap();
                if c2.is_ascii_alphanumeric() {
                    j += c2.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                offset_from: start,
                offset_to: j,
                position,
                position_length: 1,
                text: text[start..j].to_lowercase(),
            });
            position += 1;
            i = j;
        } else if (ch as u32) >= 0x2e80 && !ch.is_whitespace() {
            tokens.push(Token {
                offset_from: i,
                offset_to: i + len,
                position,
                position_length: 1,
                text: ch.to_lowercase().to_string(),
            });
            position += 1;
            i += len;
        } else {
            i += len;
        }
    }
    tokens
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

        let writer = index.writer(150_000_000)?;
        Ok(Self {
            index,
            writer: Mutex::new(writer),
            fields,
            schema,
            index_dir: index_dir.to_path_buf(),
        })
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
            doc.add_text(self.fields.content, text);
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
