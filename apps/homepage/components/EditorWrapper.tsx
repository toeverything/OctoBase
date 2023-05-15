import dynamic from "next/dynamic";
import styled from "@emotion/styled";

const Editor = dynamic(() => {
    return import('./Editor');
}, {ssr: false});


export default function EditorWrapper() {
    return <Editor editorNum={2}/>;

}