/// <reference types="wicg-file-system-access" />
import { KeckProvider } from '@toeverything/jwt-rpc';
import { fromEvent } from 'file-selector';
import { useEffect } from 'react';
import * as Y from 'yjs';

export async function sleep(delay = 100) {
    return new Promise(res => {
        window.setTimeout(() => {
            res(true);
        }, delay);
    });
}

const load = async (doc: Y.Doc) => {
    try {
        const handles = await window.showOpenFilePicker({
            types: [
                {
                    description: 'AFFiNE Package',
                    accept: {
                        // eslint-disable-next-line @typescript-eslint/naming-convention
                        'application/affine': ['.affine'],
                    },
                },
            ],
        });
        const [file] = (await fromEvent(handles)) as File[];
        const binary = await file?.arrayBuffer();

        let updated = 0;
        let isUpdated = false;
        doc.on('update', () => {
            isUpdated = true;
            updated += 1;
        });
        setInterval(() => {
            if (updated > 0) {
                updated -= 1;
            }
        }, 500);

        const update_check = new Promise<void>(resolve => {
            const check = async () => {
                while (!isUpdated || updated > 0) {
                    await sleep();
                }
                resolve();
            };
            check();
        });

        new KeckProvider(
            'AFFiNE',
            'ws://localhost:3000/collaboration/',
            'AFFiNE',
            doc
        );

        if (binary) {
            Y.applyUpdate(doc, new Uint8Array(binary));
            await update_check;
        }

        return true;
    } catch (err) {
        console.log(err);
        return false;
    }
};

export function App() {
    useEffect(() => {
        const w = window as any;
        w.Y = Y;
        console.log(Y);

        w.doc = new Y.Doc();

        w.load = () => load(w.doc);
    }, []);

    return <div />;
}

export default App;
