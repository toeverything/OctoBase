import {assertExists, Text, Workspace} from "@blocksuite/store";
import {EditorContainer} from "@blocksuite/editor";
import {AffineSchemas} from "@blocksuite/blocks/models";
import {atom, useAtomValue, getDefaultStore} from 'jotai';
import {useEffect, useRef} from "react";
// @ts-ignore
import {WebsocketProvider} from 'y-websocket';

const workspace = new Workspace({
    id: 'test-workspace'
});
workspace.register(AffineSchemas);
let editor: EditorContainer | null = null;

const provider = new WebsocketProvider('ws://localhost:3000/collaboration', 'workspace_0', workspace.doc);
provider.connect();

const editorAtom = atom<EditorContainer | null>(editor);
const store = getDefaultStore();

provider.on('sync', () => {
    editor = new EditorContainer();
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
    } else {
        let page = workspace.getPage('page0');
        assertExists(page);
        editor.page = page;
    }
    store.set(editorAtom, editor);
});

provider.on('connection-close', () => {
    store.set(editorAtom, null);
});

provider.on('connection-error', () => {
    store.set(editorAtom, null);
});

export function Editor() {
    const ref = useRef<HTMLDivElement>(null);
    const editor = useAtomValue(editorAtom);

    useEffect(() => {
        if (!editor) return;
        if (ref.current) {
            const div = ref.current;
            div.appendChild(editor);
            return () => {
                div.removeChild(editor)
            };
        }
    }, [editor]);

    return (
        <>
            {!editor && <div className="container">
                <div className="tip">
                    <div>1. Please first start keck server with <code>cargo run -p keck</code>.</div>
                    <div>2. BlockSuite Editor will mount automatically after keck server is connected.</div>
                    <div>3. Try open more tabs / windows, all contents will be synchronized.</div>
                </div>
            </div>}
            <div ref={ref} id='editor-container'/>
        </>
    )
}