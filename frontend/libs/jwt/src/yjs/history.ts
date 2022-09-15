/* eslint-disable @typescript-eslint/no-explicit-any */
import type { Map as YMap } from 'yjs';
import { UndoManager } from 'yjs';

import type { TopicEventBus } from '../utils';

type StackItem = UndoManager['undoStack'][0];

type HistoryCallback<T = unknown> = (map: Map<string, T>) => void;

export class HistoryManager {
    private readonly _historyManager: UndoManager;
    private readonly _eventBus: TopicEventBus;

    constructor(scope: YMap<any>, eventBus: TopicEventBus, tracker?: any[]) {
        this._historyManager = new UndoManager(scope, {
            trackedOrigins: tracker ? new Set(tracker) : undefined,
        });
        this._eventBus = eventBus;
        // eslint-disable-next-line no-console
        console.log('HistoryManager', scope, eventBus, tracker);

        this._historyManager.on(
            'stack-item-added',
            (event: { stackItem: StackItem }) => {
                const meta = event.stackItem.meta;
                this._eventBus.topic('push').emit(meta);
            }
        );

        this._historyManager.on(
            'stack-item-popped',
            (event: { stackItem: StackItem }) => {
                const meta = event.stackItem.meta;
                this._eventBus.topic('pop').emit(new Map(meta));
            }
        );
    }

    on<T = unknown>(
        type: 'push' | 'pop',
        name: string,
        callback: HistoryCallback<T>
    ) {
        this._eventBus.topic<Map<string, T>>(type).on(name, callback);
    }

    off(type: 'push' | 'pop', name: string) {
        this._eventBus.topic(type).off(name);
    }

    undo<T = unknown>(): Map<string, T> | undefined {
        return this._historyManager.undo()?.meta;
    }

    redo<T = unknown>(): Map<string, T> | undefined {
        return this._historyManager.redo()?.meta;
    }

    clear(): void {
        return this._historyManager.clear();
    }
}
