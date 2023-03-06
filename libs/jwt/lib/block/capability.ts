import type { BlockSearchItem } from './index.js'

export class BlockCapability {
	// Accept a block instance, check its type, content data structure
	// Does it meet the structural requirements of the current capability
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	protected _checkBlock(block: BlockSearchItem): boolean {
		return true
	}

	// data structure upgrade
	protected _migration(): void {
		// TODO: need to override
	}
}
