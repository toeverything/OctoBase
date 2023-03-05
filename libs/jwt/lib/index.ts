import type { AbstractBlock } from './block/index.js'
import type { QueryIndexMetadata } from './block/indexer.js'
import type { JwtOptions } from './instance.js'
import { JwtStore } from './instance.js'
import { sleep } from './utils/index.js'
import { assertExists } from './yjs/utils.js'

declare const JWT_DEV: boolean

const getJwtInitializer = () => {
	const workspaces: Record<string, JwtStore> = {}

	const _asyncInitLoading = new Set<string>()
	const _waitLoading = async (workspace: string) => {
		while (_asyncInitLoading.has(workspace)) {
			await sleep()
		}
	}

	const init = async (workspace: string, options?: JwtOptions) => {
		if (_asyncInitLoading.has(workspace)) {
			await _waitLoading(workspace)
		}

		if (!workspaces[workspace]) {
			_asyncInitLoading.add(workspace)
			workspaces[workspace] = new JwtStore(workspace, options)
			await workspaces[workspace]?.buildIndex()
			await workspaces[workspace]?.synced

			if (JWT_DEV) {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any
				;(window as any).store = workspaces[workspace]
			}
		}
		const result = workspaces[workspace]
		assertExists(result)

		_asyncInitLoading.delete(workspace)

		return result
	}
	return init
}

export const getWorkspace = getJwtInitializer()

export type { BlockSearchItem, ReadableContentExporter as BlockContentExporter } from './block/index.js'
export * from './command/index.js'
export { JwtStore } from './instance.js'
export { BucketBackend as BlockBackend } from './types/index.js'
export { isBlock } from './utils/index.js'
export type { ChangedStates, Connectivity } from './yjs/types.js'
export type { QueryIndexMetadata, AbstractBlock }
