/* eslint-disable max-lines */
import type { DocumentSearchOptions } from 'flexsearch';

import type { BlockSearchItem, ReadableContentExporter } from './block';
import { AbstractBlock, BlockIndexer } from './block';
import type { QueryIndexMetadata } from './block/indexer';
import type { AbstractCommand, IStoreClient } from './command';
import { StoreClient } from './command';
import type { BlockContent, Operation } from './command/operation';
import type { BlockItem, ExcludeFunction } from './types';
import { BlockFlavors, BucketBackend } from './types';
import type { BlockEventBus } from './utils';
import { genUuid, getLogger, JwtEventBus } from './utils';
import type { HistoryManager, YBlock, YProviderType } from './yjs';
import { getYProviders, YBlockManager } from './yjs';
import type {
    BlockListener,
    ChangedStates,
    Connectivity,
    DataExporter,
} from './yjs/types';
import { getDataExporter } from './yjs/types';
import { assertExists } from './yjs/utils';

declare const JWT_DEV: boolean;

const logger = getLogger('BlockDB:store');
// const logger_debug = getLogger('debug:BlockDB:store');

const namedUUID = Symbol('namedUUID');

type YBlockValues = ExcludeFunction<YBlock>;
export type BlockMatcher = Partial<YBlockValues>;

type BlockExporters<R> = Map<
    string,
    [BlockMatcher, ReadableContentExporter<R>]
>;
export type JwtOptions = {
    content?: BlockExporters<string>;
    metadata?: BlockExporters<Array<[string, number | string | string[]]>>;
    tagger?: BlockExporters<string[]>;
    enabled?: YProviderType[];
    token?: string;
};

export type IJwtStore = {
    addBlockListener: (tag: string, listener: BlockListener) => void;
    removeBlockListener: (tag: string) => void;
    createStoreClient: (command?: AbstractCommand) => IStoreClient;
};

export class JwtStore implements IJwtStore {
    private readonly _manager: YBlockManager;
    private readonly _workspace: string;

    private readonly _blockMap = new Map<string, AbstractBlock>();
    private readonly _blockIndexer: BlockIndexer;

    private readonly _exporters: {
        readonly content: BlockExporters<string>;
        readonly metadata: BlockExporters<
            Array<[string, number | string | string[]]>
        >;
        readonly tag: BlockExporters<string[]>;
    };

    private readonly _eventBus: BlockEventBus;

    private readonly _parentMapping: Map<string, string[]>;
    private readonly _pageMapping: Map<string, string>;

    private readonly _root: { node?: AbstractBlock | undefined };

    private readonly _installExporter: (
        initialData: Uint8Array,
        exporter: DataExporter
    ) => Promise<void>;

    constructor(workspace: string, options?: JwtOptions) {
        const { importData, exportData, hasExporter, installExporter } =
            getDataExporter();

        const manager = new YBlockManager(workspace, {
            eventBus: JwtEventBus.get(workspace),
            providers: getYProviders({
                enabled: [],
                backend: BucketBackend.YWebSocketAffine,
                importData,
                exportData,
                hasExporter,
                ...options,
            }),
            ...options,
        });

        this._manager = manager;
        this._workspace = workspace;

        this._exporters = {
            content: options?.content || new Map(),
            metadata: options?.metadata || new Map(),
            tag: options?.tagger || new Map(),
        };

        this._eventBus = JwtEventBus.get(workspace);

        this._blockIndexer = new BlockIndexer(
            this._manager,
            this._workspace,
            block => this._blockMap.get(block.id) || this._blockBuilder(block),
            this._eventBus.type('system')
        );

        this._parentMapping = new Map();
        this._pageMapping = new Map();

        this._eventBus
            .type('system')
            .topic<string[]>('reindex')
            .on('reindex', this._rebuildIndex.bind(this), {
                debounce: { wait: 1000, maxWait: 1000 },
            });

        this._root = {};
        this._installExporter = installExporter;
    }

    public addBlockListener(tag: string, listener: BlockListener) {
        const bus = this._eventBus
            .type('system')
            .topic<ChangedStates>('updated');
        if (tag !== 'index' || !bus.has(tag)) {
            bus.on(tag, listener);
        } else {
            console.error(`block listener ${tag} is reserved or exists`);
        }
    }

    get synced() {
        return this._manager.synced;
    }

    public removeBlockListener(tag: string) {
        this._eventBus.type('system').topic('updated').off(tag);
    }

    public addEditingListener(
        tag: string,
        listener: BlockListener<Set<string>>
    ) {
        const bus = this._eventBus
            .type('system')
            .topic<ChangedStates<Set<string>>>('editing');
        if (tag !== 'index' || !bus.has(tag)) {
            bus.on(tag, listener);
        } else {
            console.error(`editing listener ${tag} is reserved or exists`);
        }
    }

    public removeEditingListener(tag: string) {
        this._eventBus.type('system').topic('editing').off(tag);
    }

    public addConnectivityListener(
        tag: string,
        listener: BlockListener<Connectivity>
    ) {
        const bus = this._eventBus
            .type('system')
            .topic<ChangedStates<Connectivity>>('connectivity');
        if (tag !== 'index' || !bus.has(tag)) {
            bus.on(tag, listener);
        } else {
            console.error(`connectivity listener ${tag} is reserved or exists`);
        }
    }

    public removeConnectivityListener(tag: string) {
        this._eventBus.type('system').topic('connectivity').off(tag);
    }

    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-ignore
    private _inspector() {
        return {
            ...this._manager.inspector(),
            indexed: () => this._blockIndexer.inspectIndex(),
        };
    }

    private _rebuildIndex(existsIds?: string[]) {
        JWT_DEV && logger('rebuild index');
        const blocks = this._manager.getAllBlock();
        const excluded = existsIds || [];

        blocks
            .filter(id => !excluded.includes(id))
            .map(id => this._blockIndexer.refreshIndex(id, 'add'));
    }

    public async buildIndex() {
        JWT_DEV && logger('buildIndex: start');
        // Skip the block index that exists in the metadata, assuming that the index of the block existing in the metadata is the latest, and modify this part if there is a problem
        // Although there may be cases where the index is refreshed but the metadata is not refreshed, re-indexing will be automatically triggered after the block is changed
        const existsIds = await this._blockIndexer.loadIndex();
        this._rebuildIndex(existsIds);
        this.addBlockListener('index', states => {
            Array.from(states.entries()).map(([id, state]) => {
                if (state === 'delete') {
                    this._blockMap.delete(id);
                }
                return this._blockIndexer.refreshIndex(id, state);
            });
        });
    }

    /**
     * Get a specific flavor of block, currently only the article type is supported
     * @param flavor block flavor
     * @returns
     */
    public getByFlavor(flavor: string): Map<string, AbstractBlock> {
        JWT_DEV && logger(`getByType: ${flavor}`);
        const ids = this.getBlockByFlavor(flavor);

        const docs = ids.map(id => [id, this.get(id)] as const);

        return new Map(
            docs.filter(
                (doc): doc is [string, AbstractBlock] =>
                    !!doc[1]?.children.length
            )
        );
    }

    /**
     * Full text search
     * @param partOfTitleOrContent Title or content keyword, support Chinese
     * @param partOfTitleOrContent.index search range, optional values: title, ttl, content, reference
     * @param partOfTitleOrContent.tag tag, string or array of strings, supports multiple tags
     * @param partOfTitleOrContent.query keyword, support Chinese
     * @param partOfTitleOrContent.limit The limit of the number of search results, the default is 100
     * @param partOfTitleOrContent.offset search result offset, used for page turning, default is 0
     * @param partOfTitleOrContent.suggest Fuzzy matching, after enabling the content including some keywords can also be searched, the default is false
     * @returns array of search results, each array is a list of attributed block ids
     */
    public search(
        partOfTitleOrContent: string | Partial<DocumentSearchOptions<boolean>>
    ) {
        return this._blockIndexer.search(partOfTitleOrContent);
    }

    /**
     * Full text search, the returned results are grouped by page dimension
     * @param partOfTitleOrContent Title or content keyword, support Chinese
     * @param partOfTitleOrContent.index search range, optional values: title, ttl, content, reference
     * @param partOfTitleOrContent.tag tag, string or array of strings, supports multiple tags
     * @param partOfTitleOrContent.query keyword, support Chinese
     * @param partOfTitleOrContent.limit The limit of the number of search results, the default is 100
     * @param partOfTitleOrContent.offset search result offset, used for page turning, default is 0
     * @param partOfTitleOrContent.suggest Fuzzy matching, after enabling the content including some keywords can also be searched, the default is false
     * @returns array of search results, each array is a page
     */
    public searchPages(
        partOfTitleOrContent: string | Partial<DocumentSearchOptions<boolean>>
    ): BlockSearchItem[] {
        let pages = [];
        if (partOfTitleOrContent) {
            const result = this.search(partOfTitleOrContent).flatMap(
                ({ result }) =>
                    result.map(id => {
                        const page = this._pageMapping.get(id as string);
                        if (page) {
                            return page;
                        }
                        const block = this.get(String(id));
                        return this._setPage(block);
                    })
            );

            pages = [...new Set(result.filter((v): v is string => !!v))];
        } else {
            pages = this.getBlockByFlavor('page');
        }
        return this._blockIndexer.getMetadata(pages).map(page => ({
            content:
                this._getDecodedContent(this._manager.getBlock(page.id)) || '',
            ...page,
        }));
    }

    /**
     * Inquire
     * @returns array of search results
     */
    public query(query: QueryIndexMetadata): string[] {
        return this._blockIndexer.query(query);
    }

    public queryBlocks(query: QueryIndexMetadata): BlockSearchItem[] {
        const ids = this.query(query);
        return this._blockIndexer.getMetadata(ids).map(page => ({
            content:
                this._getDecodedContent(this._manager.getBlock(page.id)) || '',
            ...page,
        }));
    }

    /**
     * Get a fixed name, which has the same UUID in each workspace, and is automatically created when it does not exist
     * Generally used to store workspace-level global configuration
     * @param name block name
     * @returns block instance
     */
    private _getNamedBlock(
        name: string,
        options?: { workspace?: boolean }
    ): AbstractBlock | undefined {
        const uuid = genUuid(name);
        if (this.has([uuid])) {
            return this.get(uuid, {
                [namedUUID]: true,
            });
        }
        return this._create(uuid, {
            flavor: options?.workspace
                ? BlockFlavors.workspace
                : BlockFlavors.page,
            [namedUUID]: true,
        });
    }

    /**
     * Get the workspace block of the current instance
     * @returns block instance
     */
    public getWorkspace() {
        if (!this._root.node) {
            this._root.node = this._getNamedBlock(this._workspace, {
                workspace: true,
            });
        }

        assertExists(this._root.node);
        return this._root.node;
    }

    private _getParent(id: string) {
        const parents = this._parentMapping.get(id);
        if (parents && parents[0]) {
            const parentBlockId = parents[0];
            if (!this._blockMap.has(parentBlockId)) {
                const block = this.get(parentBlockId);
                if (block) {
                    this._blockMap.set(parentBlockId, block);
                }
            }
            return this._blockMap.get(parentBlockId);
        }
        return undefined;
    }

    private _setParent(parent: string, child: string) {
        const parents = this._parentMapping.get(child);
        if (parents?.length) {
            if (!parents.includes(parent)) {
                console.error('parent already exists', child, parents);
                this._parentMapping.set(child, [...parents, parent]);
            }
        } else {
            this._parentMapping.set(child, [parent]);
        }
    }

    private _setPage(block: AbstractBlock | undefined) {
        if (block) {
            const page = this._pageMapping.get(block.id);
            if (page) {
                return page;
            }
            const parentPage = block.parentPage;
            if (parentPage) {
                this._pageMapping.set(block.id, parentPage);
                return parentPage;
            }
        }
        return undefined;
    }

    registerContentExporter(
        name: string,
        matcher: BlockMatcher,
        exporter: ReadableContentExporter<string>
    ) {
        this._exporters.content.set(name, [matcher, exporter]);
        this._eventBus.type('system').topic('reindex').emit(); // // rebuild the index every time the content exporter is registered
    }

    unregisterContentExporter(name: string) {
        this._exporters.content.delete(name);
        this._eventBus.type('system').topic('reindex').emit(); // Rebuild indexes every time content exporter logs out
    }

    registerMetadataExporter(
        name: string,
        matcher: BlockMatcher,
        exporter: ReadableContentExporter<
            Array<[string, number | string | string[]]>
        >
    ) {
        this._exporters.metadata.set(name, [matcher, exporter]);
        this._eventBus.type('system').topic('reindex').emit(); // // rebuild the index every time the content exporter is registered
    }

    unregisterMetadataExporter(name: string) {
        this._exporters.metadata.delete(name);
        this._eventBus.type('system').topic('reindex').emit(); // Rebuild indexes every time content exporter logs out
    }

    registerTagExporter(
        name: string,
        matcher: BlockMatcher,
        exporter: ReadableContentExporter<string[]>
    ) {
        this._exporters.tag.set(name, [matcher, exporter]);
        this._eventBus.type('system').topic('reindex').emit(); // Reindex every tag exporter registration
    }

    unregisterTagExporter(name: string) {
        this._exporters.tag.delete(name);
        this._eventBus.type('system').topic('reindex').emit(); // Reindex every time tag exporter logs out
    }

    private _getExporters<R>(
        exporterMap: BlockExporters<R>,
        block: YBlock
    ): Readonly<[string, ReadableContentExporter<R>]>[] {
        const exporters = [];
        for (const [name, [cond, exporter]] of exporterMap) {
            const conditions = Object.entries(cond);
            let matched = 0;
            for (const [key, value] of conditions) {
                if (block[key as keyof YBlockValues] === value) {
                    matched += 1;
                }
            }
            if (matched === conditions.length) {
                exporters.push([name, exporter] as const);
            }
        }

        return exporters;
    }

    private _getDecodedContent(block?: YBlock) {
        if (block) {
            const [exporter] = this._getExporters(
                this._exporters.content,
                block
            );
            if (exporter) {
                const op = block.content.asMap();
                if (op) {
                    return exporter[1](op);
                }
            }
        }
        return undefined;
    }

    private _blockBuilder(block: YBlock, root?: AbstractBlock) {
        return new AbstractBlock(
            block,
            this._eventBus.type('block').topic(block.id),
            root,
            this._getParent(block.id) || root,
            {
                content: block =>
                    this._getExporters(this._exporters.content, block),
                metadata: block =>
                    this._getExporters(this._exporters.metadata, block),
                tag: block => this._getExporters(this._exporters.tag, block),
            }
        );
    }

    private _initialBlock(block: YBlock, isNamedUuid?: boolean) {
        const root = isNamedUuid ? undefined : this.getWorkspace();

        for (const child of block.children) {
            this._setParent(block.id, child);
        }

        const abstractBlock = this._blockBuilder(block, root);
        this._setPage(abstractBlock);
        abstractBlock.on('parent', 'client_hook', state => {
            const [parent] = state.keys();
            if (parent) {
                this._setParent(parent, abstractBlock.id);
            }
            this._setPage(abstractBlock);
        });
        this._blockMap.set(abstractBlock.id, abstractBlock);

        if (root && abstractBlock.flavor === BlockFlavors.page) {
            root.insertChildren(abstractBlock);
        }
        return abstractBlock;
    }

    /**
     * Get a Block, which is automatically created if it does not exist
     * @param blockFlavor block type, create a new text block when BlockTypes/BlockFlavors are not provided. If BlockTypes/BlockFlavors are provided, create a block of the corresponding type
     * @param options.type The type of block created when block does not exist, the default is block
     * @param options.flavor The flavor of the block created when the block does not exist, the default is text
     * @param options.binary content of binary block, must be provided when type or block_id_or_type is binary
     * @returns block instance
     */
    private _create(
        blockFlavor?: string,
        options?: {
            flavor: BlockItem['flavor'];
            [namedUUID]?: boolean;
        }
    ) {
        JWT_DEV && logger(`create: ${blockFlavor}`);
        const { [namedUUID]: isNamedUuid, flavor } = options || {};

        const block = this._manager.createBlock({
            uuid: isNamedUuid ? blockFlavor : undefined,
            flavor: flavor || blockFlavor || 'text',
        });
        return this._initialBlock(block, isNamedUuid);
    }

    /**
     * Get a Block, which is automatically created if it does not exist
     * @param blockId block id, return block instance if block is created
     * @returns block instance
     */
    public get<T extends object = object>(
        blockId: string,
        options?: {
            [namedUUID]?: boolean;
        }
    ): AbstractBlock<T> | undefined {
        JWT_DEV && logger(`get: ${blockId}`);
        const { [namedUUID]: isNamedUuid } = options || {};
        if (this._blockMap.has(blockId)) {
            return this._blockMap.get(blockId) as AbstractBlock<T>;
        }
        const block = this._manager.getBlock(blockId);
        if (block) {
            return this._initialBlock(block, isNamedUuid) as AbstractBlock<T>;
        }

        return undefined;
    }

    public getBlockByFlavor(flavor: BlockItem['flavor']): string[] {
        return this._manager.getBlockByFlavor(flavor);
    }

    public has(blockIds: string[]): boolean {
        return this._manager.checkBlocks(blockIds);
    }

    public get count(): number {
        return this._manager.count();
    }

    public get history(): HistoryManager {
        return this._manager.history();
    }

    public async setupDataExporter(initialData: Uint8Array, cb: DataExporter) {
        await this._installExporter(initialData, cb);
        this._manager.reload();
    }

    public createStoreClient(command?: AbstractCommand): IStoreClient {
        return new StoreClient(this, this.dispatchOperation, command);
    }

    public dispatchOperation(op: Operation): unknown {
        switch (op.type) {
            case 'InsertBlockOperation': {
                const block = this._create(op.content.flavor);
                this._updateBlock(block.id, op.content);
                return block;
            }
            case 'UpdateBlockOperation': {
                return this._updateBlock(op.id, op.content);
            }
            case 'DeleteBlockOperation': {
                return this._deleteBlock(op.id);
            }
            default: {
                logger('unknown op: ', op);
                return undefined;
            }
        }
    }

    public withTransact(cb: () => void) {
        this._manager.withTransact(cb);
    }

    private _updateBlock(
        id: string,
        content: Partial<Omit<BlockContent, 'flavor'>>
    ): boolean {
        const dbBlock = this.get(id);
        if (!dbBlock) {
            return false;
        }

        if (content.parentId && content.parentId !== dbBlock.parent?.id) {
            dbBlock.remove();
            const parent = this.get(content.parentId);
            if (!parent) {
                return false;
            }
            parent.append(dbBlock);
        }

        if (content.properties != null) {
            dbBlock.setContent(content.properties);
        }

        return true;
    }

    private _deleteBlock(id: string): boolean {
        const dbBlock = this.get(id);
        if (!dbBlock) {
            return false;
        }
        dbBlock.remove();
        return true;
    }
}
