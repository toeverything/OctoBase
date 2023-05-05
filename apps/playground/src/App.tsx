import '@blocksuite/editor/themes/affine.css';
import './App.css';

import {Editor} from "./components/Editor.tsx";
import {Iframe} from "./components/Iframe.tsx";

function App() {
    return (
        <div>
            {[...Array(2)].map((_, i) => i).map(id => {
                return <div key={id}>
                    <Iframe>
                        <Editor/>
                    </Iframe>
                </div>
            })}
        </div>
    );
}

export default App
