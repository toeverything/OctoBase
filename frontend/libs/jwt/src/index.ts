import type { AbstractBlock } from './block';
import type { QueryIndexMetadata } from './block/indexer';
import type { JwtOptions } from './instance';
import { JwtStore } from './instance';
import { sleep } from './utils';
import { assertExists } from './yjs/utils';

declare const JWT_DEV: boolean;

const getJwtInitializer = () => {
    const workspaces: Record<string, JwtStore> = {};

    const _asyncInitLoading = new Set<string>();
    const _waitLoading = async (workspace: string) => {
        while (_asyncInitLoading.has(workspace)) {
            await sleep();
        }
    };

    const init = async (workspace: string, options?: JwtOptions) => {
        if (_asyncInitLoading.has(workspace)) {
            await _waitLoading(workspace);
        }

        if (!workspaces[workspace]) {
            _asyncInitLoading.add(workspace);
            workspaces[workspace] = new JwtStore(workspace, options);
            workspaces[workspace]?.buildIndex();
            await workspaces[workspace]?.synced;

            if (JWT_DEV) {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                (window as any).store = workspaces[workspace];
            }
        }
        const result = workspaces[workspace];
        assertExists(result);

        _asyncInitLoading.delete(workspace);

        return result;
    };
    return init;
};

export const getWorkspace = getJwtInitializer();

export type {
    BlockSearchItem,
    ReadableContentExporter as BlockContentExporter,
} from './block';
export * from './command';
export { JwtStore } from './instance';
export { BlockTypes, BucketBackend as BlockBackend } from './types';
export type { BlockTypeKeys } from './types';
export { isBlock } from './utils';
export type { ChangedStates, Connectivity } from './yjs/types';
export type { QueryIndexMetadata, AbstractBlock };
