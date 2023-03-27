import { NoSsr } from '@mui/material'
import { useHistory } from '@toeverything/jwt-react'
import { useState } from 'react'

import { SyncedTextBlock } from './SyncedTextBlock'
import { nanoid } from './utils'

export function TodoMVC() {
	const [id] = useState(nanoid())
	const { undo, redo } = useHistory()

	if (id) {
		return (
			<NoSsr>
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
			</NoSsr>
		)
	}
	return null
}
