import { useEffect, useState } from 'react';

import { useBlock } from './block';
import { BasicBlockData } from './types';

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
