import { AbstractType as YAbstractType } from 'yjs';

// eslint-disable-next-line @typescript-eslint/naming-convention, @typescript-eslint/no-explicit-any
export const isYObject = (obj: any): obj is YAbstractType<any> =>
    obj && obj instanceof YAbstractType;

export function assertExists<T>(val: T | undefined): asserts val is T {
    if (!val) {
        throw new Error('val does not exist');
    }
}
