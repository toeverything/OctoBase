import '@rich-data/viewer/theme/basic.css';
import '@rich-data/json-plugin/theme/basic.css';

import {createJsonPlugins} from '@rich-data/json-plugin';
import {createViewerHook} from '@rich-data/viewer';

export default function RichData(value: Record<string, unknown>) {
    const {useViewer, Provider} = createViewerHook({
        plugins: createJsonPlugins(),
    });
    const {Viewer} = useViewer();
    return (
        <Provider>
            <Viewer value={value}/>
        </Provider>
    );
}