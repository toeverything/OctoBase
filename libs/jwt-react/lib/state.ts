import { useCallback, useEffect, useState } from 'react'

import { useBlock } from './block.js'
import type { BasicBlockData } from './types.js'

type SyncStateOptions<T extends BasicBlockData> = {
	workspace: string | undefined
	blockId: string
	key: string
	defaultValue: T
}

export const useSyncedState = <T extends BasicBlockData>(name: string, options: SyncStateOptions<T>) => {
	const { workspace, blockId, key, defaultValue } = options

	const { block, setValue, onChange, offChange } = useBlock(workspace, blockId)
	const [current, setCurrent] = useState<T | undefined>(options.defaultValue)

	useEffect(() => {
		onChange<T>(name, key, (data) => setCurrent(data))
		return () => offChange(name)
	}, [key, name, offChange, onChange])

	useEffect(() => {
		if (block) {
			const data = (block.getContent() as any)[key]
			if (typeof data !== 'undefined') {
				setCurrent(data as T)
			}
		}
	}, [block, defaultValue, key])

	const setContent = useCallback(
		(value: T) => {
			setValue({ [key]: value })
		},
		[setValue, key]
	)

	return [current, setContent] as const
}
