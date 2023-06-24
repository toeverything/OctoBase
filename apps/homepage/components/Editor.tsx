import {assertExists, Text, Workspace} from "@blocksuite/store";
import {AffineSchemas} from "@blocksuite/blocks/models";
import {EditorContainer} from "@blocksuite/editor";
// @ts-ignore
import {WebsocketProvider} from "y-websocket";
import styled from "@emotion/styled";
import {useEffect, useRef, useState} from "react";
import {ContentParser} from "@blocksuite/blocks/content-parser";
import RichData from "./RichData";
import {websocketPrefixUrl} from "./config";

export default function Editor({editorNum, workspaceId}: { editorNum: number, workspaceId: string }) {
    return (
        <>
            {Array.from({length: editorNum}, (_, i) => i).map(id => {
                const workspace = new Workspace({
                    id: `test-workspace-${workspaceId}`
                });
                workspace.register(AffineSchemas);
                return <div key={id}>
                    <EditorLoader workspace={workspace}
                                  editor={new EditorContainer()}
                                  provider={new WebsocketProvider(`${websocketPrefixUrl}/collaboration`, workspace.id, workspace.doc)}/>
                </div>
            })}
        </>
    );
}

const StyledEditor = styled.div`
  display: flex;
  position: relative;
  width: 100%;
  height: 500px;
  margin-bottom: 20px;
`

const presetMarkdown = `
This is a collaborating playground. Data are persisted and collaborated through octobase.
`;

function EditorLoader({workspace, editor, provider}: {
    workspace: Workspace,
    provider: WebsocketProvider,
    editor: EditorContainer
}) {
    const [ready, setReady] = useState(false);
    const [json, setJson] = useState<Record<string, unknown>>({});
    useEffect(() => {
        provider.on('sync', () => {
            const page0 = workspace.getPage('page0');
            if (workspace.isEmpty || !page0) {
                const page = workspace.createPage({id: 'page0'});
                const contentParser = new ContentParser(page);
                // Add page block and surface block at root level
                const pageBlockId = page.addBlock('affine:page', {
                    title: new Text(),
                });

                page.addBlock('affine:surface', {}, null);

                // Add frame block inside page block
                const frameId = page.addBlock('affine:frame', {}, pageBlockId);

                contentParser.importMarkdown(presetMarkdown, frameId).then(() => {
                    page.resetHistory();

                    editor.page = page;
                    setReady(true);
                });

            } else {
                assertExists(page0);
                editor.page = page0;
                setReady(true);
            }
        });

        provider.on('connection-close', () => {
            setReady(false);
        });

        provider.on('connection-error', () => {
            setReady(false);
        });

        setJson(workspace.doc.toJSON());

        let timeoutId;
        workspace.doc.on('update', (update, origin, doc) => {
            if (timeoutId) {
                clearTimeout(timeoutId);
            }
            timeoutId = setTimeout(() => {
                setJson(doc.toJSON());
            }, 200);
        });

        // prevent editor from overflowing vertically
        document.styleSheets[0].insertRule(`
            .affine-default-viewport {
                height: 500px !important;
            }
        `, 0);
    }, []);

    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (!ref.current || !ready) return;
        ref.current.appendChild(editor);
    }, [ref, ready]);

    return (
        <StyledEditor>
            <div style={{width: '55%'}}>
                {ready ? <div ref={ref} id="editor-container"/> : <div className="tip">
                    <div>1. To collaborate with octobase, please use <code>pnpn dev:collaboration</code>.</div>
                    <div>2. BlockSuite Editor will mount automatically after keck server is connected.</div>
                </div>}
            </div>
            <div
                style={{
                    width: '45%',
                    height: '90%',
                    overflow: 'auto',
                }}>
                <RichData value={json}/>
            </div>
        </StyledEditor>
    )
}
