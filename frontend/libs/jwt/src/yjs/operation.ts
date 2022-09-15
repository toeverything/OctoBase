/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable max-lines */
import { nanoid } from 'nanoid';
import type { AbstractType as YAbstractType } from 'yjs';
import { Array as YArray, Map as YMap, Text as YText } from 'yjs';

import type { TopicEventBus } from '../utils';
import { ChildrenListenerHandler, ContentListenerHandler } from './listener';
import type { BlockListener, ChangedStates, TextToken } from './types';
import { isYObject } from './utils';

const INTO_INNER = Symbol('INTO_INNER');

export const DO_NOT_USE_THIS_OR_YOU_WILL_BE_FIRED_SYMBOL_INTO_INNER: typeof INTO_INNER =
    INTO_INNER;

// eslint-disable-next-line @typescript-eslint/ban-types
type BaseTypes = string | number | boolean | {};

export type ContentTypes = BaseTypes | YContentOperation;

type Operable<T, Base = YAbstractType<any>> = T extends Base
    ? YContentOperation
    : T;

export class YContentOperation {
    private readonly _content: YAbstractType<unknown>;
    protected readonly eventBus: TopicEventBus;

    constructor(eventBus: TopicEventBus, content: YAbstractType<any>) {
        this._content = content;
        this.eventBus = eventBus;
    }

    get length(): number {
        if (this._content instanceof YMap) {
            return this._content.size;
        }
        if (this._content instanceof YArray || this._content instanceof YText) {
            return this._content.length;
        }
        return 0;
    }

    createText(): YTextOperation {
        return new YTextOperation(this.eventBus, new YText());
    }

    createArray<
        T extends ContentTypes = YContentOperation
    >(): YArrayOperation<T> {
        return new YArrayOperation(this.eventBus, new YArray());
    }

    createMap<T extends ContentTypes = YContentOperation>(): YMapOperation<T> {
        return new YMapOperation(this.eventBus, new YMap());
    }

    asText(): YTextOperation | undefined {
        if (this._content instanceof YText) {
            return new YTextOperation(this.eventBus, this._content);
        }
        return undefined;
    }

    asArray<T extends ContentTypes = YContentOperation>():
        | YArrayOperation<T>
        | undefined {
        if (this._content instanceof YArray) {
            return new YArrayOperation(this.eventBus, this._content);
        }
        return undefined;
    }

    asMap<T extends ContentTypes = YContentOperation>():
        | YMapOperation<T>
        | undefined {
        if (this._content instanceof YMap) {
            return new YMapOperation(this.eventBus, this._content);
        }
        return undefined;
    }

    protected _intoInner<T>(content: Operable<T>): T {
        if (content instanceof YContentOperation) {
            return content[INTO_INNER]() as unknown as T;
        }
        return content as T;
    }

    protected _toOperable<T>(content: T): Operable<T> {
        if (isYObject(content)) {
            return new YContentOperation(
                this.eventBus,
                content
            ) as unknown as Operable<T>;
        }
        return content as Operable<T>;
    }

    [INTO_INNER](): YAbstractType<unknown> | undefined {
        if (isYObject(this._content)) {
            return this._content;
        }
        return undefined;
    }

    public getStructuredContent() {
        return this._content.toJSON();
    }
}

export class YTextOperation extends YContentOperation {
    private readonly _textContent: YText;

    constructor(eventBus: TopicEventBus, content: YText) {
        super(eventBus, content);
        this._textContent = content;
    }

    insert(
        index: number,
        content: string,
        format?: Record<string, string>
    ): void {
        this._textContent.insert(index, content, format);
    }

    format(
        index: number,
        length: number,
        format: Record<string, string>
    ): void {
        this._textContent.format(index, length, format);
    }

    delete(index: number, length: number): void {
        this._textContent.delete(index, length);
    }

    setAttribute(name: string, value: BaseTypes) {
        this._textContent.setAttribute(name, value);
    }

    getAttribute<T extends BaseTypes = string>(name: string): T | undefined {
        return this._textContent.getAttribute(name);
    }

    override toString(): TextToken[] {
        return this._textContent.toDelta();
    }
}

const arrayListeners: Map<YArray<any>, string> = new Map();

export class YArrayOperation<T extends ContentTypes> extends YContentOperation {
    private readonly _arrayContent: YArray<T>;
    private readonly _hash: string;

    constructor(eventBus: TopicEventBus, content: YArray<T>) {
        super(eventBus, content);
        this._arrayContent = content;

        this._hash = arrayListeners.get(content) || nanoid(16);
        if (!arrayListeners.has(content)) {
            content.observe(event =>
                ChildrenListenerHandler(eventBus, event, this._hash)
            );
            arrayListeners.set(content, this._hash);
        }
    }

    on(name: string, listener: BlockListener) {
        this.eventBus.topic<ChangedStates>(this._hash).on(name, listener);
    }

    off(name: string) {
        this.eventBus.topic<ChangedStates>(this._hash).off(name);
    }

    insert(index: number, content: Array<Operable<T>>): void {
        this._arrayContent.insert(
            index,
            content.map(v => this._intoInner(v))
        );
    }

    delete(index: number, length: number): void {
        this._arrayContent.delete(index, length);
    }

    push(content: Array<Operable<T>>): void {
        this._arrayContent.push(content.map(v => this._intoInner(v)));
    }

    unshift(content: Array<Operable<T>>): void {
        this._arrayContent.unshift(content.map(v => this._intoInner(v)));
    }

    get(index: number): Operable<T> | undefined {
        const content = this._arrayContent.get(index);
        if (content) {
            return this._toOperable(content);
        }
        return undefined;
    }

    slice(start?: number, end?: number): Operable<T>[] {
        return this._arrayContent
            .slice(start, end)
            .map(v => this._toOperable(v));
    }

    map<R = unknown>(callback: (value: T, index: number) => R): R[] {
        return this._arrayContent.map((value, index) => callback(value, index));
    }

    // Traverse, if callback returns false, stop traversing
    forEach(callback: (value: T, index: number) => boolean) {
        for (let i = 0; i < this._arrayContent.length; i++) {
            const ret = callback(this._arrayContent.get(i), i);
            if (ret === false) {
                break;
            }
        }
    }

    find<R = unknown>(
        callback: (value: T, index: number) => boolean
    ): R | undefined {
        let result: R | undefined = undefined;
        this.forEach((value, i) => {
            const found = callback(value, i);
            if (found) {
                result = value as unknown as R;
                return false;
            }
            return true;
        });
        return result;
    }

    findIndex(callback: (value: T, index: number) => boolean): number {
        let position = -1;
        this.forEach((value, i) => {
            const found = callback(value, i);
            if (found) {
                position = i;
                return false;
            }
            return true;
        });
        return position;
    }
}

const mapListeners: Map<YMap<any>, string> = new Map();

export class YMapOperation<T extends ContentTypes> extends YContentOperation {
    private readonly _mapContent: YMap<T>;
    private readonly _hash: string;

    constructor(eventBus: TopicEventBus, content: YMap<T>) {
        super(eventBus, content);
        this._mapContent = content;
        this._hash = mapListeners.get(content) || nanoid(16);
        if (!mapListeners.has(content)) {
            content.observeDeep(events =>
                ContentListenerHandler(this.eventBus, events, this._hash)
            );
            mapListeners.set(content, this._hash);
        }
    }

    on(name: string, listener: BlockListener) {
        this.eventBus.topic<ChangedStates>(this._hash).on(name, listener);
    }

    off(name: string) {
        this.eventBus.topic(this._hash).off(name);
    }

    set(key: string, value: Operable<T>): void {
        if (value instanceof YContentOperation) {
            const content = value[INTO_INNER]();
            if (content) {
                this._mapContent.set(key, content as unknown as T);
            }
        } else {
            this._mapContent.set(key, value as T);
        }
    }

    get(key: string): Operable<T> | undefined {
        const content = this._mapContent.get(key);
        if (content) {
            return this._toOperable(content);
        }
        return undefined;
    }

    delete(key: string): void {
        this._mapContent.delete(key);
    }

    has(key: string): boolean {
        return this._mapContent.has(key);
    }
}
