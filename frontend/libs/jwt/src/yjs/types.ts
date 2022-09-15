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

export const getDataExporter = () => {
    let exporter: DataExporter | undefined = undefined;
    let importer: (() => Uint8Array | undefined) | undefined = undefined;

    const importData = () => importer?.();
    const exportData = (binary: Uint8Array) => exporter?.(binary);
    const hasExporter = () => !!exporter;

    const installExporter = (
        initialData: Uint8Array | undefined,
        cb: DataExporter
    ) => {
        return new Promise<void>(resolve => {
            importer = () => initialData;
            exporter = async (data: Uint8Array) => {
                exporter = cb;
                await cb(data);
                resolve();
            };
        });
    };

    return { importData, exportData, hasExporter, installExporter };
};

export type { BlockPosition };
