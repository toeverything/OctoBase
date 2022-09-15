import type { JwtStore } from '..';
import type { Operation } from './operation';

export type AnyFunc<T = unknown> = (...args: unknown[]) => T;

export type HandleSuccessFunc = (stopPropagation: () => void) => void;

export type HandleCancelFunc = (stopPropagation: () => void) => void;

export type HandleErrorFunc = (
    errorCode: string,
    errorMsg: string,
    cancelApply: AnyFunc,
    stopPropagation: () => void
) => void;

export type HandleError = (errorCode: string, errorMsg: string) => boolean;

export abstract class AbstractCommand {
    public abstract validate(
        store: JwtStore,
        handleError: HandleError,
        cancelApply: () => void
    ): boolean;

    public abstract apply(
        store: JwtStore,
        dispatchOperation: (op: Operation) => unknown,
        handleError: HandleError,
        cancelApply: () => void
    ): boolean;
}

// eslint-disable-next-line @typescript-eslint/consistent-type-definitions
export interface IStoreClient {
    next(
        command: AbstractCommand | AbstractCommand[] | (() => AbstractCommand[])
    ): IStoreClient;

    success(successCb: AnyFunc): IStoreClient;

    error(errorCb: HandleErrorFunc): IStoreClient;

    cancel(cancelCb: AnyFunc): IStoreClient;

    apply(): void;
}
