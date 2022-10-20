import type { RefMetadata } from './metadata';

// block flavor
// block has the same basic structure
// but different flavors provide different parsing of their content
// this enum just use internal, an block create by outside code can use any string as flavor
export enum BlockFlavors {
    workspace = 'workspace', // workspace
    page = 'page', // page
    group = 'group', // group
    tag = 'tag', // tag
    reference = 'reference', // reference,
    text = 'text', // text
    title = 'title', // title
    heading1 = 'heading1', // heading 1
    heading2 = 'heading2', // heading 2
    heading3 = 'heading3', // heading 3
    todo = 'todo',
}

export type BlockItem = {
    // block flavor
    // block has the same basic structure
    // But different flavors provide different parsing of their content
    flavor: string;
    children: string[];
    // creation time, UTC timestamp
    readonly created: number;
    // update time, UTC timestamp
    readonly updated?: number | undefined;
    // creator id
    readonly creator?: string | undefined;
    // Essentially what is stored here is either Uint8Array (binary resource) or YDoc (structured resource)
    content: Record<string, unknown>;
};

export type BlockPage = {
    title?: string;
    description?: string;
    // preview
    preview?: string;
    // Whether it is a draft, the draft will not be indexed
    draft?: string;
    // Expiration time, documents that exceed this time will be deleted
    ttl?: number;
    // Metadata, currently only bibtex definitions
    // When metadata exists, it will be treated as a reference and can be searched by tag reference
    metadata?: RefMetadata;
};
