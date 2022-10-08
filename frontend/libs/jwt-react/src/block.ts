import { nanoid } from 'nanoid';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { useClient } from './client';
import { BasicBlockData, Block } from './types';

export const usePage = (href?: string, workspace?: string | undefined) => {
    const client = useClient(workspace);

    const page = useMemo(() => {
        if (client) {
            const [page] = client.query({ href });
            if (page) {
                return client.get(page);
            } else {
                const pages = client.getBlockByFlavor('page');
                if (pages.length > 0) {
                    for (const page of pages) {
                        const block = client.get(page);
                        if ((block?.getContent() as any)?.['href'] === href) {
                            return block;
                        }
                    }
                }
            }
        }
        return undefined;
    }, [client, href]);

    return page;
};

export const usePages = (workspace?: string | undefined) => {
    const client = useClient(workspace);
    const [pages, setPages] = useState<Array<Block>>([]);

    useEffect(() => {
        if (client) {
            const uuid = nanoid(10);
            const root = client.getWorkspace();

            const refreshPages = () => {
                const pages = Array.from(client.getByFlavor('page').values());
                if (pages.length) {
                    setPages(pages);
                }
            };

            refreshPages();

            root.addChildrenListener(uuid, refreshPages);
            return () => root.removeChildrenListener(uuid);
        }
        return undefined;
    }, [client]);

    const addPage = useCallback(() => {
        if (client) {
            const page = client.dispatchOperation({
                type: 'InsertBlockOperation',
                content: { flavor: 'page' },
            }) as Block;

            return page;
        }
        return undefined;
    }, [client]);

    const removePage = useCallback(
        (id: string) => {
            if (client) {
                client.dispatchOperation({
                    type: 'DeleteBlockOperation',
                    id,
                });
            }
        },
        [client]
    );

    return { pages, addPage, removePage };
};

export const useBlock = (workspace?: string | undefined, blockId?: string) => {
    const client = useClient(workspace);
    const [block, setBlock] = useState<Block | undefined>();

    useEffect(() => {
        if (client) {
            if (blockId && client.has([blockId])) {
                setBlock(client.get(blockId));
            } else {
                setBlock(
                    client.dispatchOperation({
                        type: 'InsertBlockOperation',
                        content: { flavor: 'text' },
                    }) as Block
                );
            }
        }
    }, [blockId, client]);

    const onChange = useCallback(
        <T extends BasicBlockData>(
            name: string,
            key: string,
            callback: (data: T | undefined) => void
        ) => {
            if (block) {
                block.on('content', name, state => {
                    console.log(state, block.getContent(), key);
                    callback((block.getContent() as any)[key] as T | undefined);
                });
            }
        },
        [block]
    );

    const offChange = useCallback(
        (name: string) => {
            if (block) {
                block.off('content', name);
            }
        },
        [block]
    );

    const undo = useCallback(() => {
        if (block) {
            block.history.undo();
        }
    }, [block]);

    const redo = useCallback(() => {
        if (block) {
            block.history.redo();
        }
    }, [block]);

    return { block, onChange, offChange, undo, redo };
};
