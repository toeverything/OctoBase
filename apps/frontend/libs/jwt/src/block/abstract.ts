/* eslint-disable max-lines */
import type { BlockItem } from '../types';
import { BlockFlavors } from '../types';
import type { TopicEventBus } from '../utils';
import { getLogger, nanoid } from '../utils';
import type { HistoryManager, YBlock } from '../yjs';
import type {
    BlockListener,
    BlockPosition,
    ChangedStateKeys,
    ChangedStates,
} from '../yjs/types';
import type { BlockCapability } from './capability';

declare const JWT_DEV: boolean;
const logger = getLogger('BlockDB:block');
const loggerDebug = getLogger('debug:BlockDB:block');

const GET_BLOCK = Symbol('GET_BLOCK');
const SET_PARENT = Symbol('SET_PARENT');

type CallbackData = ChangedStates<ChangedStateKeys>;

export type ReadableContentExporter<R = string> = (content: unknown) => R;
type GetExporter<R> = (
    block: YBlock
) => Readonly<[string, ReadableContentExporter<R>]>[];

type Exporters = {
    content: GetExporter<string>;
    metadata: GetExporter<Array<[string, number | string | string[]]>>;
    tag: GetExporter<string[]>;
};

export type IndexMetadata = Readonly<{
    content: string | undefined;
    reference: string | undefined;
    tags: string[];
}>;
export type QueryMetadata = Readonly<
    {
        [key: string]: number | string | string[] | undefined;
    } & Omit<BlockItem, 'content'>
>;

export class AbstractBlock<T extends object = object> {
    private readonly _id: string;
    private readonly _block: YBlock;
    private readonly _history: HistoryManager;
    private readonly _root: AbstractBlock | undefined;
    protected readonly eventBus: TopicEventBus;
    private readonly _internalId: string = nanoid(10);

    private readonly _contentExportersGetter: () => Map<
        string,
        ReadableContentExporter<string>
    >;

    private readonly _metadataExportersGetter: () => Map<
        string,
        ReadableContentExporter<Array<[string, number | string | string[]]>>
    >;

    private readonly _tagExportersGetter: () => Map<
        string,
        ReadableContentExporter<string[]>
    >;

    private _parent: AbstractBlock | undefined;
    private _changeParent?: () => void;

    private _cachedChildren: string[] | undefined;
    private _cachedContent: unknown | undefined;

    constructor(
        block: YBlock,
        bus: TopicEventBus,
        root?: AbstractBlock,
        parent?: AbstractBlock,
        exporters?: Exporters
    ) {
        this._id = block.id;
        this._block = block;
        this._history = this._block.scopedHistory([this._id]);

        this._root = root;
        this.eventBus = bus;

        this._contentExportersGetter = () => new Map(exporters?.content(block));
        this._metadataExportersGetter = () =>
            new Map(exporters?.metadata(block));
        this._tagExportersGetter = () => new Map(exporters?.tag(block));

        this._block.on('content', 'internal' + this._internalId, () => {
            this._cachedContent = undefined;
        });

        JWT_DEV && loggerDebug(`init: exists ${this._id}`);
        if (parent) {
            this._refreshParent(parent);
        }
    }

    public get root() {
        return this._root;
    }

    protected get parentNode() {
        return this._parent;
    }

    get parent() {
        return this.parentNode;
    }

    protected _getParentPage(warning = true): string | undefined {
        if (this.flavor === 'page') {
            return this._block.id;
        }
        if (!this._parent) {
            if (warning && this.flavor !== 'workspace') {
                console.warn('parent not found');
            }
            return undefined;
        }
        return this._parent.parentPage;
    }

    public get parentPage(): string | undefined {
        return this._getParentPage();
    }

    public on(
        event: 'content' | 'children' | 'parent',
        name: string,
        callback: BlockListener
    ) {
        if (event === 'parent') {
            this.eventBus
                .topic<CallbackData>('parentChange')
                .on(name, callback);
        } else {
            this._block.on(event, name, callback);
        }
    }

    public off(event: 'content' | 'children' | 'parent', name: string) {
        if (event === 'parent') {
            this.eventBus.topic('parentChange').off(name);
        } else {
            this._block.off(event, name);
        }
    }

    public addChildrenListener(name: string, listener: BlockListener) {
        this._block.on('children', name, listener);
    }

    public removeChildrenListener(name: string) {
        this._block.off('children', name);
    }

    public addContentListener(name: string, listener: BlockListener) {
        this._block.on('content', name, listener);
    }

    public removeContentListener(name: string) {
        this._block.off('content', name);
    }

    public getContent(): T {
        if (!this._cachedContent) {
            this._cachedContent = this._block.content;
        }
        return this._cachedContent as T;
    }

    public setContent(value: Partial<T>) {
        Object.entries(value).forEach(([key, value]) => {
            this._block.set(key, value);
        });
    }

    private _getDateText(timestamp?: number): string | undefined {
        try {
            if (timestamp) {
                return new Date(timestamp)
                    .toISOString()
                    .split('T')[0]
                    ?.replace(/-/g, '');
            }
            // eslint-disable-next-line no-empty
        } catch (e) {}
        return undefined;
    }

    // Last update UTC time
    public get lastUpdated(): number {
        return this._block.updated || this._block.created;
    }

    private get _lastUpdatedDate(): string | undefined {
        return this._getDateText(this.lastUpdated);
    }

    // create UTC time
    public get created(): number {
        return this._block.created;
    }

    private get _createdDate(): string | undefined {
        return this._getDateText(this.created);
    }

    // creator id
    public get creator(): string | undefined {
        return this._block.creator;
    }

    [GET_BLOCK]() {
        return this._block;
    }

    private _emitParent(
        parentId: string,
        type: 'update' | 'delete' = 'update'
    ) {
        const states: Map<string, 'update' | 'delete'> = new Map([
            [parentId, type],
        ]);
        this.eventBus.topic<CallbackData>('parentChange').emit(states);
    }

    private _refreshParent(parent: AbstractBlock) {
        this._changeParent?.();
        parent.addChildrenListener(this._id + this._internalId, states => {
            if (states.get(this._id) === 'delete') {
                this._emitParent(parent._id, 'delete');
            }
        });

        this._parent = parent;
        this._changeParent = () => parent.removeChildrenListener(this._id);
    }

    [SET_PARENT](parent: AbstractBlock) {
        this._refreshParent(parent);
        this._emitParent(parent.id);
    }

    /**
     * Get document index tags
     */
    public getTags(): string[] {
        const created = this._createdDate;
        const updated = this._lastUpdatedDate;

        return [
            `id:${this._id}`,
            `type:${this.flavor}`,
            this.flavor === BlockFlavors.page && 'type:doc', // normal documentation
            this.flavor === BlockFlavors.tag && 'type:card', // tag document
            // this.type === ??? && `type:theorem`, // global marked math formula
            created && `created:${created}`,
            updated && `updated:${updated}`,
        ].filter((v): v is string => !!v);
    }

    /**
     * current document instance id
     */
    public get id(): string {
        return this._id;
    }

    /**
     * current block flavor
     */
    public get flavor(): string {
        return this._block.flavor;
    }

    public get children(): string[] {
        if (this._cachedChildren) {
            return this._cachedChildren;
        }
        this._cachedChildren = this._block.children;
        return this._cachedChildren;
    }

    /**
     * Insert children block
     * @param block Block instance
     * @param position Insertion position, if it is empty, it will be inserted at the end. If the block already exists, the position will be moved
     * @returns
     */
    public async insertChildren(
        block: AbstractBlock,
        position?: BlockPosition
    ) {
        JWT_DEV && logger('insertChildren: start');

        if (block.id === this._id) {
            // avoid self-reference
            return;
        }

        this._block.insertChildren(block[GET_BLOCK](), position);
        block[SET_PARENT](this);
    }

    public hasChildren(id: string): boolean {
        return this._block.hasChildren(id);
    }

    /**
     * Get an instance of the child Block
     * @param blockId block id
     * @returns
     */
    protected _getChildren(blockId?: string): YBlock[] {
        JWT_DEV && logger(`get children: ${blockId || 'unknown'}`);
        return this._block.getChildren([blockId]);
    }

    public removeChildren(blockId?: string) {
        this._block.removeChildren([blockId]);
    }

    public remove() {
        JWT_DEV && logger(`remove: ${this.id}`);
        if (this.flavor !== BlockFlavors.workspace) {
            // Pages other than workspace have parents
            this.parentNode?.removeChildren(this.id);
        }
    }

    private _insertBlocks(
        parentNode: AbstractBlock | undefined,
        blocks: AbstractBlock[],
        placement: 'before' | 'after',
        referenceNode?: AbstractBlock
    ) {
        if (!blocks || blocks.length === 0 || !parentNode) {
            return;
        }
        // TODO: array equal
        if (
            !referenceNode &&
            parentNode.children.join('') ===
                blocks.map(node => node.id).join('')
        ) {
            return;
        }
        blocks.forEach(block => {
            if (block.parentNode) {
                block.remove();
            }

            const placementInfo = {
                [placement]:
                    referenceNode?.id ||
                    (parentNode.hasChildNodes() &&
                        parentNode.children[
                            placement === 'before'
                                ? 0
                                : parentNode.children.length - 1
                        ]),
            };

            parentNode.insertChildren(
                block,
                placementInfo[placement] ? placementInfo : undefined
            );
        });
    }

    prepend(...blocks: AbstractBlock[]) {
        this._insertBlocks(this, blocks.reverse(), 'before');
    }

    append(...blocks: AbstractBlock[]) {
        this._insertBlocks(this, blocks, 'after');
    }

    before(...blocks: AbstractBlock[]) {
        this._insertBlocks(this.parentNode, blocks, 'before', this);
    }

    after(...blocks: AbstractBlock[]) {
        this._insertBlocks(this.parentNode, blocks.reverse(), 'after', this);
    }

    hasChildNodes() {
        return this.children.length > 0;
    }

    hasParent(blockId?: string) {
        let parent = this.parentNode;
        while (parent) {
            if (parent.id === blockId) {
                return true;
            }
            parent = parent.parentNode;
        }
        return false;
    }

    /**
     * TODO: scoped history
     */
    public get history(): HistoryManager {
        return this._history;
    }

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    getCapability<T extends BlockCapability>(key: string): T | undefined {
        // TODO: Capability api design
        return undefined;
    }

    get group(): AbstractBlock | undefined {
        if (this.flavor === 'group') {
            return this;
        }
        return this.parent?.group;
    }

    private _getIndexableMetadata() {
        const metadata: Record<string, number | string | string[]> = {};
        for (const [name, exporter] of this._metadataExportersGetter()) {
            try {
                for (const [key, val] of exporter(this.getContent())) {
                    metadata[key] = val;
                }
            } catch (err) {
                logger(`Failed to export metadata: ${name}`, err);
            }
        }
        try {
            const parentPage = this._getParentPage(false);
            if (parentPage) {
                metadata['page'] = parentPage;
            }
            if (this.group) {
                metadata['group'] = this.group.id;
            }
            if (this.parent) {
                metadata['parent'] = this.parent.id;
            }
        } catch (e) {
            logger('Failed to export default metadata', e);
        }

        return metadata;
    }

    public getQueryMetadata(): QueryMetadata {
        return {
            flavor: this.flavor,
            creator: this.creator,
            children: this.children,
            created: this.created,
            updated: this.lastUpdated,
            ...this._getIndexableMetadata(),
        };
    }

    private _getIndexableContent(): string | undefined {
        const contents = [];
        for (const [name, exporter] of this._contentExportersGetter()) {
            try {
                const content = exporter(this.getContent());
                if (content) {
                    contents.push(content);
                }
            } catch (err) {
                logger(`Failed to export content: ${name}`, err);
            }
        }
        if (!contents.length) {
            try {
                const content = this.getContent();
                return JSON.stringify(content);
                // eslint-disable-next-line no-empty
            } catch (e) {}
        }
        return contents.join('\n');
    }

    private _getIndexableTags(): string[] {
        const tags: string[] = [];
        for (const [name, exporter] of this._tagExportersGetter()) {
            try {
                tags.push(...exporter(this.getContent()));
            } catch (err) {
                logger(`Failed to export tags: ${name}`, err);
            }
        }

        return tags;
    }

    public getIndexMetadata(): IndexMetadata {
        return {
            content: this._getIndexableContent(),
            reference: '', // TODO: bibliography
            tags: [...this.getTags(), ...this._getIndexableTags()],
        };
    }
}
