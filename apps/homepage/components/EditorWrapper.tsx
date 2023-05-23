import dynamic from "next/dynamic";
import {useState} from "react";
import styled from "@emotion/styled";
import {nanoid} from "nanoid";

const Editor = dynamic(() => {
    return import('./Editor');
}, {ssr: false});

const Button = styled.button`
  color: #333;
  border: none;
  border-radius: 2px;
  text-align: center;
  text-decoration: none;
  display: inline-block;
  font-size: 16px;
  transition: background-color 0.3s ease;
  cursor: pointer;
  padding: 0 10px;
  margin-right: 10px;

  &:hover {
    background: #eeeeee;
  }
`

export default function EditorWrapper() {
    const [workspaceId, _] = useState(nanoid());
    const [editorNum, setEditorNum] = useState(2);
    return (
        <>
            <div style={{margin: '10px 0'}}>
                <Button onClick={() => {setEditorNum(editorNum + 1)}}>Add Editor</Button>
                <Button onClick={() => {setEditorNum(editorNum - 1)}}>Remove Editor</Button>
            </div>
            <Editor editorNum={editorNum} workspaceId={workspaceId}/>
        </>
    );
}