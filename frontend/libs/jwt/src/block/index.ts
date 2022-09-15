import type { BlockMetadata } from './indexer';

export type BlockSearchItem = Partial<
    BlockMetadata & {
        readonly content: string;
    }
>;

export { AbstractBlock } from './abstract';
export type { ReadableContentExporter } from './abstract';
export type { BlockCapability } from './capability';
export { BlockIndexer } from './indexer';
