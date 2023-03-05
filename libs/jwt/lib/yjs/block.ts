import type { Array as YArray, Map as YMap } from 'yjs'
import { transact } from 'yjs'

import type { BlockItem } from '../types/index.js'
import type { TopicEventBus } from '../utils/index.js'
import { HistoryManager } from './history.js'
import { ChildrenListenerHandler, ContentListenerHandler } from './listener.js'
import type { BlockListener, ChangedStates } from './types.js'

const GET_BLOCK_ITEM = Symbol('GET_BLOCK_ITEM')

// eslint-disable-next-line @typescript-eslint/naming-convention
const getMapFromYArray = (array: YArray<string>) => new Map(array.map((child, index) => [child, index]))

type YBlockProps = {
	id: string
	block: YMap<unknown>
	eventBus: TopicEventBus
	setBlock: (id: string, block: BlockItem) => void
	getUpdated: (id: string) => number | undefined
	getCreator: (id: string) => string | undefined
	getYBlock: (id: string) => YBlock | undefined
}

export class YBlock {
	private readonly _id: string
	private readonly _block: YMap<unknown>
	private readonly _children: YArray<string>
	private readonly _setBlock: (id: string, block: BlockItem) => void
	private readonly _getUpdated: (id: string) => number | undefined
	private readonly _getCreator: (id: string) => string | undefined
	private readonly _getYBlock: (id: string) => YBlock | undefined
	private readonly _eventBus: TopicEventBus

	private _childrenMap: Map<string, number>

	constructor(props: YBlockProps) {
		this._id = props.id
		this._block = props.block
		this._eventBus = props.eventBus

		this._children = props.block.get('sys:children') as YArray<string>
		this._childrenMap = getMapFromYArray(this._children)
		this._setBlock = props.setBlock
		this._getUpdated = props.getUpdated
		this._getCreator = props.getCreator
		this._getYBlock = props.getYBlock

		this._children.observe((event) => ChildrenListenerHandler(this._eventBus, event))
		this._block.observeDeep((events) => ContentListenerHandler(this._eventBus, events))
	}

	on(key: 'children' | 'content', name: string, listener: BlockListener): void {
		this._eventBus.topic<ChangedStates>(key).on(name, listener)
	}

	off(key: 'children' | 'content', name: string): void {
		this._eventBus.topic<ChangedStates>(key).off(name)
	}

	get id() {
		return this._id
	}

	get content(): Record<string, unknown> {
		const content: Record<string, unknown> = {}
		this._block.forEach((value, key) => {
			if (key.startsWith('prop:')) {
				content[key.slice(5)] = value
			}
		})
		return content
	}

	get flavor(): BlockItem['flavor'] {
		return this._block.get('sys:flavor') as BlockItem['flavor']
	}

	get created(): BlockItem['created'] {
		return this._block.get('sys:created') as BlockItem['created']
	}

	get updated(): number {
		return this._getUpdated(this._id) || this.created
	}

	get creator(): string | undefined {
		return this._getCreator(this._id)
	}

	get children(): string[] {
		return this._children.toArray()
	}

	get<T = unknown>(key: string): T | undefined {
		return this._block.get('prop:' + key) as T
	}

	set<T = unknown>(key: string, value: T) {
		const prop = 'prop:' + key
		if (this._block.get(prop) !== value) {
			this._block.set(prop, value)
		}
	}

	getChildren(ids?: (string | undefined)[]): YBlock[] {
		const queryIds = ids?.filter((id): id is string => !!id) || []
		const existsIds = this._children.map((id) => id)
		const filterIds = queryIds.length ? queryIds : existsIds
		return existsIds
			.filter((id) => filterIds.includes(id))
			.map((id) => this._getYBlock(id))
			.filter((v): v is YBlock => !!v)
	}

	hasChildren(id: string): boolean {
		if (this.children.includes(id)) {
			return true
		}
		return this.getChildren().some((block) => block.hasChildren(id))
	}

	private _positionCalculator(maxPos: number, position?: { pos?: number; before?: string; after?: string }) {
		const { pos, before, after } = position || {}
		if (typeof pos === 'number' && Number.isInteger(pos)) {
			if (pos >= 0 && pos < maxPos) {
				return pos
			}
		} else if (before) {
			const currentPos = this._childrenMap.get(before)
			if (typeof currentPos === 'number' && Number.isInteger(currentPos)) {
				const prevPos = currentPos
				if (prevPos >= 0 && prevPos < maxPos) {
					return prevPos
				}
			}
		} else if (after) {
			const currentPos = this._childrenMap.get(after)
			if (typeof currentPos === 'number' && Number.isInteger(currentPos)) {
				const nextPos = currentPos + 1
				if (nextPos >= 0 && nextPos < maxPos) {
					return nextPos
				}
			}
		}
		return undefined
	}

	insertChildren(block: YBlock, pos?: { pos?: number; before?: string; after?: string }): void {
		const content = block[GET_BLOCK_ITEM]()
		if (content) {
			const lastIndex = this._childrenMap.get(block.id)
			if (typeof lastIndex === 'number') {
				this._children.delete(lastIndex)
				this._childrenMap = getMapFromYArray(this._children)
			}

			const position = this._positionCalculator(this._childrenMap.size, pos)
			if (typeof position === 'number') {
				this._children.insert(position, [block.id])
			} else {
				this._children.push([block.id])
			}
			this._setBlock(block.id, content)
			this._childrenMap = getMapFromYArray(this._children)
		}
	}

	removeChildren(ids: (string | undefined)[]): string[] {
		if (this._children.doc) {
			const failed: string[] = []
			transact(this._children.doc, () => {
				for (const id of ids) {
					let idx = -1
					for (const blockId of this._children) {
						idx += 1
						if (blockId === id) {
							this._children.delete(idx)
							break
						}
					}
					if (id) {
						failed.push(id)
					}
				}

				this._childrenMap = getMapFromYArray(this._children)
			})
			return failed
		}
		return ids.filter((id): id is string => !!id)
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	public scopedHistory(scope: any[]): HistoryManager {
		return new HistoryManager(this._block, this._eventBus.topic('history'), scope)
	}

	[GET_BLOCK_ITEM]() {
		// check null & undefined
		if (this.content != null) {
			return {
				flavor: this.flavor,
				children: this._children.slice(),
				created: this.created,
				content: this.content,
			}
		}
		return undefined
	}
}
