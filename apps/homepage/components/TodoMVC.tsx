import { useHistory } from '@toeverything/jwt-react'
import { useEffect, useState } from 'react'

import { SyncedTextBlock } from './SyncedTextBlock'

export function TodoMVC() {
	const [id, setId] = useState<string>()
	const { undo, redo } = useHistory()

	useEffect(() => {
		const loadId = async () => {
			const { nanoid } = await import('nanoid')
			setId(nanoid())
		}
		loadId()
		return () => setId(undefined)
	}, [])

	if (id) {
		return (
			<div style={{ display: 'flex', flexDirection: 'column' }}>
				<span>{id}</span>
				<div style={{ display: 'flex', flexDirection: 'row', margin: '0.5em' }}>
					<SyncedTextBlock key={1} name="1" id={id} />
					<SyncedTextBlock key={2} name="2" id={id} />
				</div>
				<div style={{ display: 'flex', flexDirection: 'row', margin: '0.5em' }}>
					<button onClick={undo}>undo</button>
					<button onClick={redo}>redo</button>
				</div>
			</div>
		)
	}
	return null
}
