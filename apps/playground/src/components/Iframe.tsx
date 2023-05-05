import {ReactElement, useEffect, useRef} from "react";
import ReactDOM from 'react-dom/client'

export function Iframe({children}: { children: ReactElement }) {
    const iframeRef = useRef<HTMLIFrameElement>(null);

    useEffect(() => {
        if (!iframeRef.current) return;
        const iframe = iframeRef.current;
        const iframeDoc = iframe.contentDocument;
        if (!iframeDoc) return;
        const rootDiv = iframeDoc.createElement('div');
        iframeDoc.body.appendChild(rootDiv);

        const root = ReactDOM.createRoot(rootDiv);
        setTimeout(() => root.render(children));

        return () => {
            setTimeout(() => root.unmount());
        };
    }, [children]);

    return <iframe ref={iframeRef} title="Embedded React Component"/>;
}