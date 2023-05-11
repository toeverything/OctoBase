import '@blocksuite/editor/themes/affine.css';
import './App.css';

// @ts-ignore
import {WebsocketProvider} from "y-websocket";
import Editor from "./components/Editor.tsx";

function App() {
    return (
        <Editor editorNum={2} />
    )
}

export default App
