import { getWorkspace, JwtStore } from '@toeverything/jwt';
import { atom, useAtom } from 'jotai';
import { useCallback, useEffect, useState } from 'react';

type Block = Awaited<ReturnType<JwtStore['get']>>;
type BasicBlockData = string | number | boolean;

const blockClientAtom = atom<JwtStore | undefined>(undefined);

export const useClient = (workspace?: string | undefined) => {
    const [client, setClient] = useAtom(blockClientAtom);

    useEffect(() => {
        if (!client && workspace) {
            getWorkspace(workspace, {
                enabled: ['keck'],
                token: 'AFFiNE',
            }).then(client => {
                setClient(client);
            });
        }
    }, [client, setClient, workspace]);

    return client;
};

export const useHistory = (workspace?: string | undefined) => {
    const client = useClient(workspace);

    const undo = useCallback(() => {
        if (client) {
            client.history.undo();
        }
    }, [client]);

    const redo = useCallback(() => {
        if (client) {
            client.history.redo();
        }
    }, [client]);

    return { undo, redo };
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

type SyncStateOptions<T extends BasicBlockData> = {
    workspace: string | undefined;
    blockId: string;
    key: string;
    defaultValue: T;
};

export const useSyncedState = <T extends BasicBlockData>(
    name: string,
    options: SyncStateOptions<T>
) => {
    const { workspace, blockId, key, defaultValue } = options;

    const { block, onChange, offChange } = useBlock(workspace, blockId);
    const [current, setCurrent] = useState<T | undefined>(options.defaultValue);

    useEffect(() => {
        onChange<T>(name, key, data => setCurrent(data));
        return () => offChange(name);
    }, [key, name, offChange, onChange]);

    useEffect(() => {
        if (block) {
            const data = (block.getContent() as any)[key];
            if (typeof data !== 'undefined') {
                setCurrent(data as T);
            }
        }
    }, [block, defaultValue, key]);

    return current;
};
