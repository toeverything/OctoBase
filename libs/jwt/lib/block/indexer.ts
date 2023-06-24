import { createNewSortInstance } from 'fast-sort'
import type { DocumentSearchOptions } from 'flexsearch'
import { Document as DocumentIndexer } from 'flexsearch'
import type { Query } from 'sift'
import sift from 'sift'

import { BlockFlavors } from '../types/index.js'
import type { BlockEventBus } from '../utils/index.js'
import { getLogger } from '../utils/index.js'
import type { YBlock, YBlockManager } from '../yjs/index.js'
import type { ChangedStates } from '../yjs/types.js'
import { assertExists } from '../yjs/utils.js'
import type { AbstractBlock, IndexMetadata, QueryMetadata } from './abstract.js'

declare const JWT_DEV: boolean

const logger = getLogger('BlockDB:indexing')
const loggerDebug = getLogger('debug:BlockDB:indexing')

const naturalSort = createNewSortInstance({
	comparer: new Intl.Collator(undefined, {
		numeric: true,
		sensitivity: 'base',
	}).compare,
})

type ChangedState = ChangedStates extends Map<unknown, infer R> ? R : never

export type BlockMetadata = QueryMetadata & { readonly id: string }

function tokenizeZh(text: string) {
	// eslint-disable-next-line @typescript-eslint/ban-ts-comment
	// @ts-ignore
	const tokenizer = Intl?.v8BreakIterator
	if (tokenizer) {
		const it = tokenizer(['zh-CN'], { type: 'word' })
		it.adoptText(text)
		const words = []

		let cur = 0,
			prev = 0

		while (cur < text.length) {
			prev = cur
			cur = it.next()
			words.push(text.substring(prev, cur))
		}

		return words
	}
	// eslint-disable-next-line no-control-regex
	return text.replace(/[\x00-\x7F]/g, '').split('')
}

type BlockIndexedContent = {
	index: IndexMetadata
	query: QueryMetadata
}

export type QueryIndexMetadata = Query<QueryMetadata> & {
	$sort?: string
	$desc?: boolean
	$limit?: number
}

export class BlockIndexer {
	private readonly _manager: YBlockManager

	private readonly _blockIndexer: DocumentIndexer<IndexMetadata>
	private readonly _blockMetadataMap = new Map<string, QueryMetadata>()
	private readonly _eventBus: BlockEventBus

	private readonly _blockBuilder: (block: YBlock) => AbstractBlock

	private readonly _delayIndex: { documents: Map<string, AbstractBlock> }

	constructor(manager: YBlockManager, workspace: string, blockBuilder: (block: YBlock) => AbstractBlock, eventBus: BlockEventBus) {
		this._manager = manager

		this._blockIndexer = new DocumentIndexer({
			document: {
				id: 'id',
				index: ['content', 'reference'],
				tag: 'tags',
			},
			encode: tokenizeZh,
			tokenize: 'forward',
			context: true,
		})

		this._blockBuilder = blockBuilder
		this._eventBus = eventBus

		this._delayIndex = { documents: new Map() }

		this._eventBus.type('reindex').on('reindex', this._contentReindex.bind(this), {
			debounce: { wait: 1000, maxWait: 1000 * 10 },
		})
	}

	private _contentReindex() {
		const paddings: Record<string, BlockIndexedContent> = {}

		for (const [k, block] of this._delayIndex.documents) {
			paddings[k] = {
				index: block.getIndexMetadata(),
				query: block.getQueryMetadata(),
			}
			this._delayIndex.documents.delete(k)
		}

		for (const [key, { index, query }] of Object.entries(paddings)) {
			if (index.content) {
				this._blockIndexer.add(key, index)
				this._blockMetadataMap.set(key, query)
			}
		}
	}

	private _refreshIndex(block: AbstractBlock) {
		const filter: string[] = [
			BlockFlavors.page,
			BlockFlavors.title,
			BlockFlavors.heading1,
			BlockFlavors.heading2,
			BlockFlavors.heading3,
			BlockFlavors.text,
			BlockFlavors.todo,
			BlockFlavors.reference,
		]
		if (filter.includes(block.flavor)) {
			this._delayIndex.documents.set(block.id, block)
			this._eventBus.type('reindex').emit()
			return true
		}
		loggerDebug(`skip index ${block.flavor}: ${block.id}`)
		return false
	}

	refreshIndex(id: string, state: ChangedState) {
		JWT_DEV && logger(`refreshArticleIndex: ${id}`)
		if (state === 'delete') {
			this._blockIndexer.remove(id)
			this._blockMetadataMap.delete(id)
			this._delayIndex.documents.delete(id)

			return
		}
		const block = this._manager.getBlock(id)
		if (block?.id === id) {
			if (this._refreshIndex(this._blockBuilder(block))) {
				JWT_DEV && logger(state ? `refresh index: ${id}, ${state}` : `indexing: ${id}`)
			} else {
				JWT_DEV && logger(`skip index: ${id}, ${block.flavor}`)
			}
		} else {
			JWT_DEV && logger(`refreshArticleIndex: ${id} not exists`)
		}
	}

	public async inspectIndex() {
		const index: Record<string | number, any> = {}
		await this._blockIndexer.export((key, data) => {
			index[key] = data
		})
		return index
	}

	public search(partOfTitleOrContent: string | Partial<DocumentSearchOptions<boolean>>) {
		return this._blockIndexer.search(partOfTitleOrContent as string)
	}

	private _testMetaKey(key: string) {
		try {
			const metadata = this._blockMetadataMap.values().next().value
			if (!metadata || typeof metadata !== 'object') {
				return false
			}
			return !!(key in metadata)
		} catch (e) {
			return false
		}
	}

	private _getSortedMetadata(sort: string, desc?: boolean) {
		const sorter = naturalSort(Array.from(this._blockMetadataMap.entries()))
		if (desc) {
			return sorter.desc(([, m]) => m[sort])
		}
		return sorter.asc(([, m]) => m[sort])
	}

	public query(query: QueryIndexMetadata) {
		const matches: string[] = []
		const { $sort, $desc, $limit, ...condition } = query
		// @ts-ignore
		const filter = sift<QueryMetadata>(condition)
		const limit = $limit || this._blockMetadataMap.size

		if ($sort && this._testMetaKey($sort)) {
			const metadata = this._getSortedMetadata($sort, $desc)
			metadata.forEach(([key, value]) => {
				if (matches.length > limit) {
					return
				}
				if (filter(value)) {
					matches.push(key)
				}
			})

			return matches
		}
		this._blockMetadataMap.forEach((value, key) => {
			if (matches.length > limit) {
				return
			}
			if (filter(value)) {
				matches.push(key)
			}
		})

		return matches
	}

	public getMetadata(ids: string[]): Array<BlockMetadata> {
		return ids
			.filter((id) => this._blockMetadataMap.has(id))
			.map((id) => {
				const meta = this._blockMetadataMap.get(id)
				assertExists(meta)
				return { ...meta, id }
			})
	}
}
