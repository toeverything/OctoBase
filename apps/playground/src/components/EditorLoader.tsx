import {assertExists, Text, Workspace} from "@blocksuite/store";
import {EditorContainer} from "@blocksuite/editor";
import {useEffect, useRef, useState} from "react";
// @ts-ignore
import {WebsocketProvider} from "y-websocket";

export default function EditorLoader({workspace, editor, provider}: {workspace: Workspace, provider: WebsocketProvider, editor: EditorContainer}) {
    const [ready, setReady] = useState(false);
    useEffect(() => {
        provider.on('sync', () => {
            if (workspace.isEmpty) {
                const page = workspace.createPage({id: 'page0'});
                // Add page block and surface block at root level
                const pageBlockId = page.addBlock('affine:page', {
                    title: new Text(),
                });

                page.addBlock('affine:surface', {}, null);

                // Add frame block inside page block
                const frameId = page.addBlock('affine:frame', {}, pageBlockId);
                // Add paragraph block inside frame block
                page.addBlock('affine:paragraph', {}, frameId);
                page.resetHistory();

                editor.page = page;
                setReady(true);
            } else {
                let page = workspace.getPage('page0');
                assertExists(page);
                editor.page = page;
                setReady(true);
            }
        });

        provider.on('connection-close', () => {
            setReady(false);
        });

        provider.on('connection-error', () => {
            setReady(false);
        });

    }, []);

    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (!ref.current || !ready) return;
        ref.current.appendChild(editor);
    }, [ref, ready]);

    return (
        <>
            <div ref={ref} id="editor-container"/>
        </>
    )

}