/* eslint-disable @typescript-eslint/ban-ts-comment */
import { nanoid } from 'nanoid';
import { debounce } from 'ts-debounce';
import { Awareness } from 'y-protocols/awareness.js';
import { Array as YArray, Doc, Map as YMap, transact } from 'yjs';

import type { BlockItem } from '../types';
import type { BlockEventBus } from '../utils';
import { RemoteBinaries } from './binary';
import { YBlock } from './block';
import { GateKeeper } from './gatekeeper';
import { HistoryManager } from './history';
import type { YProviderFactory } from './provider';
import type { ChangedStateKeys, Connectivity } from './types';
import { assertExists } from './utils';

type YProvider = {
    awareness: Awareness;
    binaries: Doc;
    doc: Doc;
    gatekeeper: GateKeeper;
    userId: string;
    remoteToken: string | undefined; // remote storage token
    synced: Promise<void[]> | undefined;
};

const yProviders = new Map<string, YProvider>();

function initYProvider(
    workspace: string,
    options: {
        userId: string;
        eventBus: BlockEventBus;
        token?: string | undefined;
        providers?: Record<string, YProviderFactory> | undefined;
    }
): YProvider {
    const provider = yProviders.get(workspace);
    // TODO: temporarily handle this
    if (
        provider &&
        (provider.userId === options.userId || options.userId === 'default')
    ) {
        return provider;
    }

    const { userId, token } = options;

    const doc = new Doc({ autoLoad: true, shouldLoad: true });
    const binaries = new Doc({ autoLoad: true, shouldLoad: true });

    const awareness = new Awareness(doc);

    const gateKeeperData = doc.getMap<YMap<string>>('gatekeeper');

    const gatekeeper = new GateKeeper(
        userId,
        gateKeeperData.get('creators') ||
            gateKeeperData.set('creators', new YMap()),
        gateKeeperData.get('common') || gateKeeperData.set('common', new YMap())
    );

    let synced: Promise<void[]> | undefined = undefined;
    // TODO: eject async logic
    if (options.providers) {
        const emitState = (c: Connectivity) => {
            options.eventBus
                .type('system')
                .topic('connectivity')
                .emit(new Map([[workspace, c]]));
        };
        synced = Promise.all(
            Object.entries(options.providers).flatMap(([, p]) => [
                p({ awareness, doc, token, workspace, emitState }),
                // p({
                //     awareness,
                //     doc: binaries,
                //     token,
                //     workspace: `${workspace}_binaries`,
                //     emitState,
                // }),
            ])
        );
    }

    const newProvider: YProvider = {
        awareness,
        binaries,
        doc,
        gatekeeper,
        userId,
        remoteToken: token,
        synced,
    };

    yProviders.set(workspace, newProvider);

    return newProvider;
}

export type { YBlock } from './block';
export { HistoryManager } from './history';
export { getYProviders } from './provider';
export type { YProviderOptions, YProviderType } from './provider';

export type YInitOptions = {
    eventBus: BlockEventBus;
    userId?: string;
    token?: string;
    providers?: Record<string, YProviderFactory>;
};

export class YBlockManager {
    private readonly _provider: YProvider;
    private readonly _doc: Doc; // doc instance
    private readonly _awareness: Awareness; // lightweight state synchronization
    private readonly _gatekeeper: GateKeeper; // Simple access control
    private readonly _history!: HistoryManager;

    // Block Collection
    // key is a randomly generated global id
    private readonly _blocks!: YMap<YMap<unknown>>;
    private readonly _blockUpdated!: YMap<YArray<YArray<number | string>>>;
    private readonly _blockMap = new Map<string, YBlock>();

    private readonly _eventBus: BlockEventBus;

    private readonly _reload: () => void;
    private readonly _synced: Promise<void[]> | undefined;

    constructor(workspace: string, options: YInitOptions) {
        const {
            eventBus,
            userId = 'default',
            token,
            providers: provider,
        } = options;
        const providers = initYProvider(workspace, {
            userId,
            eventBus,
            token,
            providers: provider,
        });

        this._provider = providers;
        this._doc = providers.doc;
        this._awareness = providers.awareness;
        this._gatekeeper = providers.gatekeeper;
        this._eventBus = eventBus;
        this._reload = () => {
            // @ts-ignore
            this._blocks = this._doc.getMap('blocks');
            // @ts-ignore
            this._blockUpdated = this._doc.getMap('updated');
            this._blockMap.clear();
            // @ts-ignore
            this._binaries = new RemoteBinaries(
                providers.binaries.getMap(),
                providers.remoteToken
            );
            // @ts-ignore
            this._history = new HistoryManager(
                this._blocks,
                this._eventBus.type('history')
            );

            this._blocks.observeDeep(events => {
                const now = Date.now();

                const keys = events.flatMap(e => {
                    // eslint-disable-next-line no-bitwise
                    if ((e.path?.length | 0) > 0) {
                        return [
                            [e.path[0], 'update'] as [string, ChangedStateKeys],
                        ];
                    }
                    return Array.from(e.changes.keys.entries()).map(
                        ([k, { action }]) =>
                            [k, action] as [string, ChangedStateKeys]
                    );
                });

                this._eventBus
                    .type('system')
                    .topic('updated')
                    .emit(new Map(keys));

                transact(this._doc, () => {
                    for (const [key, action] of keys) {
                        if (action === 'delete') {
                            this._blockUpdated.delete(key);
                        } else {
                            const updated = this._blockUpdated.get(key);

                            const content = new YArray<number | string>();
                            content.push([this._doc.clientID, now, action]);

                            if (updated) {
                                updated.push([content]);
                            } else {
                                const array = new YArray<
                                    YArray<number | string>
                                >();
                                array.push([content]);
                                this._blockUpdated.set(key, array);
                            }
                        }
                    }
                });
            });
        };
        this._reload();
        this._synced = providers.synced;

        const debouncedEditingNotifier = debounce(
            () => {
                const mapping = this._awareness.getStates();
                const editingMapping: Record<string, string[]> = {};
                for (const { userId, editing, updated } of mapping.values()) {
                    // Only return the status with refresh time within 10 seconds
                    if (
                        userId &&
                        editing &&
                        updated &&
                        typeof updated === 'number' &&
                        updated + 1000 * 10 > Date.now()
                    ) {
                        if (!editingMapping[editing]) {
                            editingMapping[editing] = [];
                        }
                        editingMapping[editing]?.push(userId);
                    }
                }
                this._eventBus
                    .type('system')
                    .topic('editing')
                    .emit(
                        new Map(
                            Object.entries(editingMapping).map(([k, v]) => [
                                k,
                                new Set(v),
                            ])
                        )
                    );
            },
            200,
            { maxWait: 1000 }
        );

        this._awareness.setLocalStateField('userId', providers.userId);

        this._awareness.on('update', debouncedEditingNotifier);
    }

    get synced() {
        return this._synced?.then(() => this._reload());
    }

    reload() {
        this._reload();
    }

    public count(): number {
        return this._blocks.size;
    }

    getUserId(): string {
        return this._provider.userId;
    }

    inspector() {
        const resolveBlock = (blocks: Record<string, any>, id: string) => {
            const block = blocks[id];
            if (block) {
                return {
                    ...block,
                    children: block.children.map((id: string) =>
                        resolveBlock(blocks, id)
                    ),
                };
            }
        };

        return {
            parse: () => this._doc.toJSON(),
            parsePage: (page_id: string) => {
                const blocks = this._blocks.toJSON();
                return resolveBlock(blocks, page_id);
            },
            parsePages: (resolve = false) => {
                const blocks = this._blocks.toJSON();
                return Object.fromEntries(
                    Object.entries(blocks)
                        .filter(([, block]) => block.flavor === 'page')
                        .map(([key, block]) => {
                            if (resolve) {
                                return resolveBlock(blocks, key);
                            }
                            return [key, block];
                        })
                );
            },
            clear: () => {
                this._blocks.clear();
                this._blockUpdated.clear();
                this._gatekeeper.clear();
                this._doc.getMap('blocks').clear();
                this._doc.getMap('gatekeeper').clear();
            },
        };
    }

    createBlock(
        options: Pick<BlockItem, 'flavor'> & {
            uuid: string | undefined;
            // TODO: how save binary?
            // binary: ArrayBufferLike | undefined;
        }
    ): YBlock {
        const uuid = options.uuid || `affine${nanoid(16)}`;

        const block = {
            flavor: options.flavor,
            children: [] as string[],
            created: Date.now(),
            content: {},
        };
        this._setBlock(uuid, block);
        const result = this.getBlock(uuid);
        assertExists(result);
        return result;
    }

    private _getUpdated(id: string) {
        const updated = this._blockUpdated.get(id);
        return (updated?.get(updated.length - 1)?.get(1) as number) || 0;
    }

    private _getCreator(id: string) {
        return this._gatekeeper.getCreator(id);
    }

    private _getBlockSync(id: string): YBlock | undefined {
        const cached = this._blockMap.get(id);
        if (cached) {
            return cached;
        }

        const block = this._blocks.get(id);

        // TODO: Synchronous read cannot read binary
        if (block) {
            const instance = new YBlock({
                id,
                block,
                eventBus: this._eventBus.type('block').topic(id),
                setBlock: this._setBlock.bind(this),
                getUpdated: this._getUpdated.bind(this),
                getCreator: this._getCreator.bind(this),
                getYBlock: this._getBlockSync.bind(this),
            });
            this._blockMap.set(id, instance);
            return instance;
        }

        return undefined;
    }

    getBlock(id: string): YBlock | undefined {
        const blockInstance = this._getBlockSync(id);
        if (blockInstance) {
            return blockInstance;
        }

        return undefined;
    }

    hasBlock(id: string): boolean {
        return this._blockMap.has(id) || this._blocks.has(id);
    }

    getBlockByFlavor(flavor: BlockItem['flavor']): string[] {
        const keys: string[] = [];
        this._blocks.forEach((doc, key) => {
            if (doc.get('sys:flavor') === flavor) {
                keys.push(key);
            }
        });

        return keys;
    }

    getAllBlock(): string[] {
        return Array.from(this._blocks.keys());
    }

    private _setBlock(key: string, item: BlockItem & { hash?: string }): void {
        const block = this._blocks.get(key) || new YMap();
        transact(this._doc, () => {
            // Insert only if the block doesn't exist yet
            // Other modification operations are done in the block instance

            if (!block.size) {
                const children = new YArray();
                children.push(item.children);

                block.set('sys:flavor', item.flavor);
                block.set('sys:children', children);
                block.set('sys:created', item.created);

                for (const [k, v] of Object.entries(item.content)) {
                    block.set('prop:' + k, v);
                }

                this._blocks.set(key, block);
            }

            if (item.flavor === 'page') {
                this._awareness.setLocalStateField('editing', key);
                this._awareness.setLocalStateField('updated', Date.now());
            }
            // References do not add delete restrictions
            if (item.flavor === 'reference') {
                this._gatekeeper.setCommon(key);
            } else {
                this._gatekeeper.setCreator(key);
            }
        });
    }

    public checkBlocks(keys: string[]): boolean {
        return (
            keys.filter(key => !!this._blocks.get(key)).length === keys.length
        );
    }

    public deleteBlocks(keys: string[]): string[] {
        const [success, fail] = this._gatekeeper.checkDeleteLists(keys);
        transact(this._doc, () => {
            for (const key of success) {
                this._blocks.delete(key);
            }
        });
        return fail;
    }

    public withTransact(cb: () => void) {
        transact(this._doc, () => {
            cb();
        });
    }

    public history(): HistoryManager {
        return this._history;
    }
}
