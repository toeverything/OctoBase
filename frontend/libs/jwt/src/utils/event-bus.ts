/* eslint-disable @typescript-eslint/no-explicit-any */
import { debounce } from 'ts-debounce';
import { assertExists } from '../yjs/utils';

import { getLogger } from './index';

declare const JWT_DEV: boolean;

const logger = getLogger('BlockDB:event_bus');

class GlobalBlockEventBus {
    private _busMaps: Map<string, BlockEventBus>;
    private _eventTarget: EventTarget;

    constructor() {
        this._busMaps = new Map();
        this._eventTarget = new EventTarget();
    }

    get(workspace: string) {
        if (!this._busMaps.has(workspace)) {
            const eventBus = new BlockEventBus(workspace, this._eventTarget);
            this._busMaps.set(eventBus.workspace, eventBus);
        }
        // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
        return this._busMaps.get(workspace)!;
    }
}

type EventListener = (e: Event) => void;

type EventBusCache = {
    callback: Map<string, EventListener>;
    scope: Map<string, TopicEventBus<any>>;
};

class BlockEventBus {
    private readonly _eventBus: EventTarget;
    private readonly _cache: EventBusCache;
    private readonly _workspace: string;

    constructor(
        workspace: string,
        eventBus: EventTarget,
        cache?: EventBusCache
    ) {
        this._eventBus = eventBus;
        this._cache = cache || { callback: new Map(), scope: new Map() };
        this._workspace = workspace;
    }

    public get workspace(): string {
        return this._workspace;
    }

    protected _on(topic: string, name: string, listener: EventListener) {
        const topicName = `${this._workspace}/${topic}`;
        const handlerName = `${topicName}/${name}`;
        // eslint-disable-next-line no-console
        console.log('on', topicName, handlerName);
        if (!this._cache.callback.has(handlerName)) {
            this._eventBus.addEventListener(topicName, listener);
            this._cache.callback.set(handlerName, listener);
        } else {
            console.warn(`event handler ${handlerName} is existing`);
            JWT_DEV && logger(`event handler ${handlerName} is existing`);
        }
    }

    protected _off(topic: string, name: string) {
        const topicName = `${this._workspace}/${topic}`;
        const handlerName = `${topicName}/${name}`;
        const listener = this._cache.callback.get(handlerName);
        if (listener) {
            this._eventBus.removeEventListener(topicName, listener);
            this._cache.callback.delete(handlerName);
        } else {
            JWT_DEV && logger(`event handler ${handlerName} is not existing`);
        }
    }

    protected _has(topic: string, name: string) {
        return this._cache.callback.has(`${this._workspace}/${topic}/${name}`);
    }

    protected _emit<T>(topic: string, detail?: T) {
        const topicName = `${this._workspace}/${topic}`;
        this._eventBus.dispatchEvent(new CustomEvent(topicName, { detail }));
    }

    public type<T = unknown>(
        topic: string,
        // eslint-disable-next-line @typescript-eslint/no-unused-vars
        root?: boolean
    ): TopicEventBus<T> {
        if (!this._cache.scope.has(topic)) {
            const bus = new TopicEventBus<T>(
                this._workspace,
                topic,
                this._eventBus,
                this._cache
            );
            this._cache.scope.set(topic, bus);
        }
        const result = this._cache.scope.get(topic);
        assertExists(result);
        return result;
    }
}

type DebounceOptions = {
    wait: number;
    maxWait?: number;
};

type ListenerOptions = {
    debounce?: DebounceOptions;
};

class TopicEventBus<T = unknown> extends BlockEventBus {
    private readonly _topic: string;

    constructor(
        workspace: string,
        topic: string,
        eventBus: EventTarget,
        cache: EventBusCache
    ) {
        super(workspace, eventBus, cache);
        this._topic = topic;
    }

    on(
        name: string,
        listener: ((e: T) => Promise<void>) | ((e: T) => void),
        options?: ListenerOptions
    ) {
        if (options?.debounce) {
            const { wait, maxWait = 500 } = options.debounce;
            const debounced = debounce(listener, wait, { maxWait });
            this._on(this._topic, name, e => {
                debounced((e as CustomEvent)?.detail);
            });
        } else {
            this._on(this._topic, name, e => {
                // eslint-disable-next-line no-console
                console.log(this._topic, name, (e as CustomEvent)?.detail);
                listener((e as CustomEvent)?.detail);
            });
        }
    }

    off(name: string) {
        this._off(this._topic, name);
    }

    has(name: string) {
        return this._has(this._topic, name);
    }

    emit(detail?: T) {
        this._emit(this._topic, detail);
    }

    topic<T = unknown>(topic: string, root?: boolean): TopicEventBus<T> {
        return this.type(root ? topic : `${this._topic}/${topic}`);
    }
}

const JwtEventBus = new GlobalBlockEventBus();

Object.freeze(JwtEventBus);

export { JwtEventBus };
export type { BlockEventBus, TopicEventBus };
