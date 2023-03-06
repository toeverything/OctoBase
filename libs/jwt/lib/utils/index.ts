import debug from 'debug'
import { v5 as UUIDv5 } from 'uuid'

import { AbstractBlock } from '../block/abstract.js'

declare const JWT_DEV: boolean

// sha3-256(toeverything uuid) -> truncate 128 bits
// e66a34f77a3b09d2020eb20e1f77e3c56250c19788ed2c70993ad2c495e55de6
const UUID_NAMESPACE = Uint8Array.from([0xe6, 0x6a, 0x34, 0xf7, 0x7a, 0x3b, 0x09, 0xd2, 0x02, 0x0e, 0xb2, 0x0e, 0x1f, 0x77, 0xe3, 0xc5])

export function genUuid(workspace: string): string {
	return UUIDv5(workspace, UUID_NAMESPACE)
}

export function getLogger(namespace: string) {
	if (JWT_DEV) {
		const logger = debug(namespace)
		// eslint-disable-next-line no-console
		logger.log = console.log.bind(console)
		if (JWT_DEV === ('testing' as any)) {
			logger.enabled = true
		}
		return logger
	}
	// eslint-disable-next-line @typescript-eslint/no-empty-function
	return () => {}
}

export function isBlock(obj: any) {
	return obj && obj instanceof AbstractBlock
}

export function sleep() {
	return new Promise((resolve) => {
		setTimeout(resolve, 100)
	})
}

export const nanoid = (size = 21) =>
	crypto.getRandomValues(new Uint8Array(size)).reduce((id, byte) => {
		byte &= 63
		if (byte < 36) {
			id += byte.toString(36)
		} else if (byte < 62) {
			id += (byte - 26).toString(36).toUpperCase()
		} else if (byte > 62) {
			id += '-'
		} else {
			id += '_'
		}
		return id
	}, '')

export { JwtEventBus } from './event-bus.js'
export type { BlockEventBus, TopicEventBus } from './event-bus.js'
