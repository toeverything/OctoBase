use tantivy::tokenizer::{NgramTokenizer, TokenizerManager};

pub const GRAM_TOKENIZER: &str = "gram";

pub fn tokenizers_register(tokenizers: &TokenizerManager) {
    tokenizers.register(GRAM_TOKENIZER, NgramTokenizer::new(1, 10, false));
}
