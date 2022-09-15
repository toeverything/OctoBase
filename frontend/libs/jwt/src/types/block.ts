import type { YContentOperation } from '../yjs';
import type { RefMetadata } from './metadata';

// base type of block
// y_block: tree structure expression with YAbstractType as root
// y_binary: arbitrary binary data
export const BlockTypes = {
    block: 'y_block' as const, // data block
    binary: 'y_binary' as const, // binary data
};

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

export type BlockTypeKeys = keyof typeof BlockTypes;

export type BlockItem = {
    readonly type: typeof BlockTypes[BlockTypeKeys];
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
    content: YContentOperation;
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
