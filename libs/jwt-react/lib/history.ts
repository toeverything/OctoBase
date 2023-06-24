import { useCallback } from 'react'

import { useClient } from './client.js'

export const useHistory = (workspace?: string | undefined) => {
	const client = useClient(workspace)

	const undo = useCallback(() => {
		if (client) {
			client.history.undo()
		}
	}, [client])

	const redo = useCallback(() => {
		if (client) {
			client.history.redo()
		}
	}, [client])

	return { undo, redo }
}
