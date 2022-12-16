use serde::{Deserialize, Serialize};

/// Part of [SearchResult]
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchBlockItem {
    /// Dev information. In future perhaps a way to only create this in some kind of "debug mode".
    pub debug_string: Option<String>,
    pub block_id: String,
    // Additional info not be necessary for v0, since the front-end
    // only needs to know the search specific info, which could just
    // be the Block IDs since the front-end will end up rendering the
    // blocks anyways.
    // // WIP: other tantivy info like score etc?
    // pub search_meta: HashMap<String, lib0::any::Any>,
}

/// Returned from [Workspace::search]
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchBlockList {
    /// Dev information. In future perhaps a way to only create this in some kind of "debug mode".
    pub debug_string: Option<String>,
    pub items: Vec<SearchBlockItem>,
    // Additional info not be necessary for v0, since the front-end
    // only needs to know the search specific info, which could just
    // be the Block IDs since the front-end will end up rendering the
    // blocks anyways.
    // // WIP: other tantivy info like score etc?
    // pub search_meta: HashMap<String, lib0::any::Any>,
}

/// Options passed into [Workspace::search]
#[derive(Debug)]
pub struct SearchQueryOptions {
    /// Primary search text string.
    pub query: String,
}
