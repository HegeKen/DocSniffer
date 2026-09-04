//! Field names (also used as storage keys).
pub const FIELD_ID: &str = "id";
pub const FIELD_PATH: &str = "path";
pub const FIELD_NAME: &str = "name";
pub const FIELD_CONTENT: &str = "content";
pub const FIELD_EXT: &str = "ext";
pub const FIELD_SIZE: &str = "size";
pub const FIELD_MTIME: &str = "mtime";

/// Name of the custom CJK-aware tokenizer registered on the index.
pub const TOKENIZER_ZH: &str = "zh";

/// The set of `Field` handles, used to build queries and read stored values.
#[derive(Clone)]
pub struct Fields {
    pub id: Field,
    pub path: Field,
    pub name: Field,
    pub content: Field,
    pub ext: Field,
    pub size: Field,
    pub mtime: Field,
}

use tantivy::schema::{
    Field, IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions,
};

/// Build the Tantivy schema described in the README ("五、数据库与索引设计").
///
/// `content` is indexed with the CJK tokenizer but deliberately *not* stored so
/// the on-disk store stays lean (snippets are produced by re-reading the file).
pub fn build_schema() -> (Schema, Fields) {
    let mut b = Schema::builder();

    // `id` is a raw (un-tokenized) stored field: it is used as a stable key for
    // delete_term, so it must index as a single token.
    let id = b.add_text_field(
        FIELD_ID,
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored(),
    );
    let path = b.add_text_field(
        FIELD_PATH,
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );
    let name = b.add_text_field(
        FIELD_NAME,
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );

    let content_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(TOKENIZER_ZH)
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    let content = b.add_text_field(FIELD_CONTENT, content_options);

    let ext = b.add_text_field(
        FIELD_EXT,
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored(),
    );
    let size = b.add_u64_field(
        FIELD_SIZE,
        NumericOptions::default().set_indexed().set_stored().set_fast(),
    );
    let mtime = b.add_u64_field(
        FIELD_MTIME,
        NumericOptions::default().set_indexed().set_stored().set_fast(),
    );

    let schema = b.build();
    (
        schema,
        Fields {
            id,
            path,
            name,
            content,
            ext,
            size,
            mtime,
        },
    )
}
