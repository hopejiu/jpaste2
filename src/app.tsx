// Import all action modules to auto-register
import './actions';

import { Router, Route, Switch } from 'wouter';
import { useHashPathLocation } from './hooks/use-hash-path-location';
import { MainPage } from './routes/main';
import { SettingsPage } from './routes/settings';
import { ToolboxPage } from './routes/toolbox';
import { QuickLaunchPage } from './routes/quicklaunch';
import { JsonViewPage } from './features/json';
import { ImageViewPage } from './features/image';
import { CurlViewPage } from './features/curl';
import { WsViewPage } from './features/ws';
import { CalcViewPage } from './features/calc';
import { DecoderViewPage } from './features/decoder';
import { TimestampViewPage } from './features/timestamp';
import { QrViewPage } from './features/qr';
import { SvgViewPage } from './features/svg';
import { KanbanPage } from './features/kanban';
import { ToastPage } from './routes/toast';
import { SharePage } from './routes/share';
import { setComponent } from './lib/logger';
import { useTauriEvent } from './hooks/use-events';
import { NAVIGATE } from './lib/events';

setComponent('app');

export function App() {
  // Listen for backend-driven navigation (e.g. return to main page on window show)
  useTauriEvent<string>(NAVIGATE, (path) => {
    window.location.hash = path;
  });

  return (
    <Router hook={useHashPathLocation}>
      <Switch>
        <Route path="/" component={MainPage} />
        <Route path="/toolbox" component={ToolboxPage} />
        <Route path="/quicklaunch" component={QuickLaunchPage} />
        <Route path="/settings" component={SettingsPage} />
        <Route path="/toast" component={ToastPage} />
        <Route path="/viewer/json" component={JsonViewPage} />
        <Route path="/viewer/image" component={ImageViewPage} />
        <Route path="/viewer/curl" component={CurlViewPage} />
        <Route path="/viewer/ws" component={WsViewPage} />
        <Route path="/viewer/calc" component={CalcViewPage} />
        <Route path="/viewer/decoder" component={DecoderViewPage} />
        <Route path="/viewer/timestamp" component={TimestampViewPage} />
        <Route path="/viewer/qr" component={QrViewPage} />
        <Route path="/viewer/svg" component={SvgViewPage} />
        <Route path="/viewer/kanban" component={KanbanPage} />
        <Route path="/share" component={SharePage} />
        <Route>404: Not Found</Route>
      </Switch>
    </Router>
  );
}
