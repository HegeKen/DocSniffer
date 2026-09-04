//! Index engine built on Tantivy.
//!
//! Provides the `IndexManager` (writer + reader + query parser) and the custom
//! CJK-aware tokenizer described in the README. The `schema` submodule defines
//! the index fields; `writer` exposes the manager and tokenizer.

pub mod schema;
pub mod writer;

pub use schema::{build_schema, Fields, FIELD_CONTENT, FIELD_EXT, FIELD_ID, FIELD_MTIME, FIELD_NAME, FIELD_PATH, FIELD_SIZE};
pub use writer::{id_of, IndexManager};
