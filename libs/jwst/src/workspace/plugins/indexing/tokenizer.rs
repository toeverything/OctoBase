use cang_jie::{CangJieTokenizer, TokenizerOption};
use tantivy::tokenizer::TokenizerManager;

pub use cang_jie::CANG_JIE as LANG_CN;

pub fn tokenizers_register(tokenizers: &TokenizerManager) {
    tokenizers.register(
        LANG_CN,
        CangJieTokenizer {
            option: TokenizerOption::ForSearch { hmm: true },
            ..Default::default()
        },
    );
}
