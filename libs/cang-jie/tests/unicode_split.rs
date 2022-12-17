use cang_jie::{CangJieTokenizer, TokenizerOption, CANG_JIE};
use flexi_logger::{Logger, Record};
use jieba_rs::Jieba;
use std::{collections::HashSet, io, iter::FromIterator, sync::Arc};
use tantivy::{collector::TopDocs, doc, query::QueryParser, schema::*, Index};
use time::format_description::well_known::Rfc3339;

#[test]
fn full_test_unicode_split() -> tantivy::Result<()> {
    Logger::try_with_env_or_str("cang_jie=trace,error")
        .expect("failed to init logger")
        .format(logger_format)
        .start()
        .unwrap_or_else(|e| panic!("Logger initialization failed with {}", e));

    let mut schema_builder = SchemaBuilder::default();

    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer(CANG_JIE) // Set custom tokenizer
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();

    let title = schema_builder.add_text_field("title", text_options);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema);
    index.tokenizers().register(CANG_JIE, tokenizer()); // Build cang-jie Tokenizer

    let mut index_writer = index.writer(50 * 1024 * 1024)?;
    index_writer.add_document(doc! { title => "南京长江大桥" });
    index_writer.add_document(doc! { title => "这个是长江" });
    index_writer.add_document(doc! { title => "这个是南京长" });
    index_writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let query = QueryParser::for_index(&index, vec![title]).parse_query("京长")?;
    let top_docs = searcher.search(query.as_ref(), &TopDocs::with_limit(10000))?;

    let actual = top_docs
        .into_iter()
        .map(|x| {
            searcher
                .doc(x.1)
                .unwrap()
                .get_first(title)
                .unwrap()
                .as_text()
                .unwrap()
                .to_string()
        })
        .collect::<HashSet<_>>();

    let expect = HashSet::from_iter(vec!["这个是南京长".to_string(), "南京长江大桥".to_string()]);

    assert_eq!(actual, expect);

    Ok(())
}

fn tokenizer() -> CangJieTokenizer {
    CangJieTokenizer {
        worker: Arc::new(Jieba::empty()), // empty dictionary
        option: TokenizerOption::Unicode,
    }
}

pub fn logger_format(
    w: &mut dyn io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &Record,
) -> Result<(), io::Error> {
    write!(
        w,
        "[{}] {} [{}:{}] {}",
        now.now().format(&Rfc3339).unwrap(),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        &record.args()
    )
}
