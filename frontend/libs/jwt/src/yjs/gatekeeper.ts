import type { Map as YMap } from 'yjs';

export class GateKeeper {
    private readonly _userId: string;
    private readonly _creators: YMap<string>;
    private readonly _common: YMap<string>;

    constructor(userId: string, creators: YMap<string>, common: YMap<string>) {
        this._userId = userId;
        this._creators = creators;
        this._common = common;
    }

    getCreator(blockId: string): string | undefined {
        return this._creators.get(blockId) || this._common.get(blockId);
    }

    setCreator(blockId: string) {
        if (!this._creators.get(blockId)) {
            this._creators.set(blockId, this._userId);
        }
    }

    setCommon(blockId: string) {
        if (!this._creators.get(blockId) && !this._common.get(blockId)) {
            this._common.set(blockId, this._userId);
        }
    }

    private _checkDelete(blockId: string): boolean {
        const creator = this._creators.get(blockId);
        return creator === this._userId || !!this._common.get(blockId);
    }

    checkDeleteLists(blockIds: string[]): [string[], string[]] {
        const success = [];
        const fail = [];
        for (const blockId of blockIds) {
            if (this._checkDelete(blockId)) {
                success.push(blockId);
            } else {
                fail.push(blockId);
            }
        }
        return [success, fail];
    }

    clear() {
        this._creators.clear();
        this._common.clear();
    }
}
