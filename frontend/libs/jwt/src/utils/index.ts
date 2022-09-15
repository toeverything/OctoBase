/* eslint-disable @typescript-eslint/no-explicit-any */
import type { Buffer } from 'buffer';
import debug from 'debug';
import { enableAllPlugins } from 'immer';
import { SHAKE } from 'sha3';
import { v5 as UUIDv5 } from 'uuid';
import { AbstractBlock } from '../block/abstract';

declare const JWT_DEV: boolean;

enableAllPlugins();

const hash = new SHAKE(128);

// sha3-256(toeverything uuid) -> truncate 128 bits
// e66a34f77a3b09d2020eb20e1f77e3c56250c19788ed2c70993ad2c495e55de6
const UUID_NAMESPACE = Uint8Array.from([
    0xe6, 0x6a, 0x34, 0xf7, 0x7a, 0x3b, 0x09, 0xd2, 0x02, 0x0e, 0xb2, 0x0e,
    0x1f, 0x77, 0xe3, 0xc5,
]);

export function genUuid(workspace: string): string {
    return UUIDv5(workspace, UUID_NAMESPACE);
}

export function sha3(buffer: Buffer): string {
    hash.reset();
    hash.update(buffer);
    return hash
        .digest('base64')
        .replace(/=/g, '')
        .replace(/\+/g, '-')
        .replace(/\//g, '_');
}

export function getLogger(namespace: string) {
    if (JWT_DEV) {
        const logger = debug(namespace);
        // eslint-disable-next-line no-console
        logger.log = console.log.bind(console);
        if (JWT_DEV === ('testing' as any)) {
            logger.enabled = true;
        }
        return logger;
    }
    // eslint-disable-next-line @typescript-eslint/no-empty-function
    return () => {};
}

export function isBlock(obj: any) {
    return obj && obj instanceof AbstractBlock;
}

export function sleep() {
    return new Promise(resolve => {
        setTimeout(resolve, 100);
    });
}

export { JwtEventBus } from './event-bus';
export type { BlockEventBus, TopicEventBus } from './event-bus';
