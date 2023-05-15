import dynamic from "next/dynamic";
import styled from "@emotion/styled";

const Editor = dynamic(() => {
    return import('./Editor');
}, {ssr: false});


const StyledEditor = styled.div`
  display: flex;
  justify-content: center;
`

export default function EditorWrapper() {
    return (
        <StyledEditor>
            <Editor editorNum={2}/>
        </StyledEditor>
    )
}