export type ChangedStateKeys = 'add' | 'update' | 'delete';
export type ChangedStates<S = ChangedStateKeys> = Map<string, S>;

export type BlockListener<S = ChangedStateKeys, R = unknown> = (
    states: ChangedStates<S>
) => Promise<R> | R;

export type Connectivity = 'disconnect' | 'connecting' | 'connected' | 'retry';

type TextAttributes = Record<string, string>;

export type TextToken = {
    insert: string;
    attributes?: TextAttributes;
};

type BlockPosition = { pos?: number; before?: string; after?: string };

export type DataExporter = (binary: Uint8Array) => Promise<void>;

export type { BlockPosition };
