import { Route, Routes } from 'react-router-dom';
import { HomePage } from './home';

export function App() {
    return (
        <Routes>
            <Route path="/" element={<HomePage />} />
        </Routes>
    );
}

export default App;
