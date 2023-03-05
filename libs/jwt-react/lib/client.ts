import { atom, useAtom } from 'jotai';
import { useEffect } from 'react';

import { getWorkspace, JwtStore } from '@toeverything/jwt';

const blockClientAtom = atom<JwtStore | undefined>(undefined);

export const useClient = (workspace?: string | undefined) => {
    const [client, setClient] = useAtom(blockClientAtom);

    useEffect(() => {
        if (!client && workspace) {
            getWorkspace(workspace, {
                enabled: ['keck'],
            }).then(client => {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore
                window['client'] = client;
                client.registerMetadataExporter(
                    'page',
                    { flavor: 'page' },
                    content => [['href', (content as any)['href']]]
                );
                setClient(client);
            });
        }
    }, [client, setClient, workspace]);

    return client;
};
