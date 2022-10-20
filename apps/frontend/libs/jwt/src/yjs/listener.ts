/* eslint-disable @typescript-eslint/no-explicit-any */
import type { YEvent } from 'yjs';
import type { TopicEventBus } from '../utils';

import type { ChangedStateKeys } from './types';

export function ChildrenListenerHandler(
    eventBus: TopicEventBus,
    event: YEvent<any>,
    topic?: string
) {
    const keys = Array.from(event.keys.entries()).map(
        ([key, { action }]) => [key, action] as [string, ChangedStateKeys]
    );
    const added = Array.from(event.changes.added.values())
        .flatMap(val => val.content.getContent() as string[])
        .filter(v => v)
        .map(k => [k, 'add'] as [string, ChangedStateKeys]);
    const deleted = Array.from(event.changes.deleted.values())
        .flatMap(val => val.content.getContent() as string[])
        .filter(v => v)
        .map(k => [k, 'delete'] as [string, ChangedStateKeys]);
    const events = [...keys, ...added, ...deleted];

    if (events.length) {
        eventBus.topic(topic || 'children').emit(new Map(events));
    }
}

export function ContentListenerHandler(
    eventBus: TopicEventBus,
    events: YEvent<any>[],
    topic?: string
) {
    const keys = events
        .flatMap(e => {
            // eslint-disable-next-line no-bitwise
            if ((e.path?.length | 0) > 0) {
                return [[e.path[0], 'update'] as [string, 'update']];
            }
            return Array.from(e.changes.keys.entries()).map(
                ([k, { action }]) => [k, action] as [string, typeof action]
            );
        })
        .filter(([k]) => k.startsWith('prop:'));

    if (keys.length) {
        eventBus.topic(topic || 'content').emit(new Map(keys));
    }
}
