import {Workspace} from "@blocksuite/store";
import {AffineSchemas} from "@blocksuite/blocks/models";
import EditorLoader from "./EditorLoader";
import {EditorContainer} from "@blocksuite/editor";
// @ts-ignore
import {WebsocketProvider} from "y-websocket";

export default function Editor({editorNum}: { editorNum: number }) {
    return (
        <>
            {Array.from({length: editorNum}, (_, i) => i).map(id => {
                const workspace = new Workspace({
                    id: `test-workspace121212`
                });
                workspace.register(AffineSchemas);
                return <div key={id}>
                    <EditorLoader workspace={workspace}
                                  editor={new EditorContainer()}
                                  provider={new WebsocketProvider('ws://localhost:3001/collaboration', workspace.id, workspace.doc)}/>
                </div>
            })}
        </>
    );
}