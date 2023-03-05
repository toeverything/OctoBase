import type { BlockMetadata } from './indexer.js'

export type BlockSearchItem = Partial<
	BlockMetadata & {
		readonly content: string
	}
>

export { AbstractBlock } from './abstract.js'
export type { ReadableContentExporter } from './abstract.js'
export type { BlockCapability } from './capability.js'
export { BlockIndexer } from './indexer.js'
