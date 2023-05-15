import {assertExists, Text, Workspace} from "@blocksuite/store";
import {EditorContainer} from "@blocksuite/editor";
import {useEffect, useRef, useState} from "react";
// @ts-ignore
import {WebsocketProvider} from "y-websocket";
import {ContentParser} from "@blocksuite/blocks/content-parser";
import styled from "@emotion/styled";
import RichData from "./RichData";

const StyledWrapper = styled.div`
  display: flex;
  position: relative;
  width: 100%;
  height: 400px;
  margin-bottom: 20px;
`

const presetMarkdown = `
This is a collaborating playground. Data are persisted and collaborated through octobase.
`;

export default function EditorLoader({workspace, editor, provider}: {
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

        let timeoutId;
        workspace.doc.on('update', (update, origin, doc) => {
            if (timeoutId) {
                clearTimeout(timeoutId);
            }
            timeoutId = setTimeout(() => {
                setJson(doc.toJSON());
            }, 1000);
        });

    }, []);

    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (!ref.current || !ready) return;
        ref.current.appendChild(editor);

        return () => {
            ref.current.removeChild(editor);
        }
    }, [ref, ready]);

    return (
        <StyledWrapper>
            <div style={{width: '60%'}}>
                {ready ? <div ref={ref} id="editor-container"/> : <div className="tip">
                    <div>1. To collaborate with octobase, please use <code>pnpn dev:collaboration</code>.</div>
                    <div>2. BlockSuite Editor will mount automatically after keck server is connected.</div>
                </div>}
            </div>
            <div
                style={{
                    width: '40%',
                    height: '80%',
                    overflow: 'auto',
                }}>
                <RichData value={json}/>
            </div>
        </StyledWrapper>
    )

}