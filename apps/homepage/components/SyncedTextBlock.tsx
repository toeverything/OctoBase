import { useSyncedState } from '@toeverything/jwt-react'

const blockOptions = {
	workspace: 'test',
	key: 'data',
	defaultValue: 'default',
}

export const SyncedTextBlock = (props: { name: string; id: string }) => {
	const [text, setText] = useSyncedState<string>(props.name, {
		...blockOptions,
		blockId: props.id,
	})

	return <input value={text} onChange={(v) => setText(v.currentTarget.value)} />
}
