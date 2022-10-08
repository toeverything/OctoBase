import { useEffect, useState } from 'react';

import { useBlock, useHistory, useSyncedState } from '@toeverything/jwt-react';

const blockOptions = {
    workspace: 'test',
    key: 'data',
    defaultValue: 'default',
};

const SyncedTextBlock = (props: { name: string; id: string }) => {
    const text = useSyncedState<string>(props.name, {
        ...blockOptions,
        blockId: props.id,
    });

    return <input value={text} />;
};

export function App() {
    const [id, setId] = useState<string | undefined>();
    const { undo, redo } = useHistory();
    const { block } = useBlock('test');

    useEffect(() => {
        if (block) {
            setId(block.id);
        }
    }, [block]);

    if (id) {
        return (
            <div style={{ display: 'flex', flexDirection: 'column' }}>
                <span>{id}</span>
                <SyncedTextBlock key={1} name="1" id={id} />
                <SyncedTextBlock key={2} name="2" id={id} />
                <button onClick={undo}>undo</button>
                <button onClick={redo}>redo</button>
            </div>
        );
    }
    return null;
}
