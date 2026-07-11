import { Navigate, Route, Routes } from "react-router-dom";
import ConsolePage from "./pages/ConsolePage";
import NotFound from "./pages/NotFound";

const App = () => (
  <Routes>
    <Route path="/" element={<Navigate to="/console" replace />} />
    <Route path="/console" element={<ConsolePage />} />
    <Route path="/console/:stepId/*" element={<ConsolePage />} />
    <Route path="*" element={<NotFound />} />
  </Routes>
);

export default App;
