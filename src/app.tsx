// Import all action modules to auto-register
import './actions';

import { Router, Route, Switch } from 'wouter';
import { useHashPathLocation } from './hooks/use-hash-path-location';
import { MainPage } from './routes/main';
import { SettingsPage } from './routes/settings';
import { JsonViewPage } from './routes/viewer/json-view';
import { ImageViewPage } from './routes/viewer/image-view';
import { CurlViewPage } from './routes/viewer/curl-view';
import { WsViewPage } from './routes/viewer/ws-view';
import { CalcViewPage } from './routes/viewer/calc-view';
import { DecoderViewPage } from './routes/viewer/decoder-view';
import { TimestampViewPage } from './routes/viewer/timestamp-view';
import { ToastPage } from './routes/toast';
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
        <Route path="/settings" component={SettingsPage} />
        <Route path="/toast" component={ToastPage} />
        <Route path="/viewer/json" component={JsonViewPage} />
        <Route path="/viewer/image" component={ImageViewPage} />
        <Route path="/viewer/curl" component={CurlViewPage} />
        <Route path="/viewer/ws" component={WsViewPage} />
        <Route path="/viewer/calc" component={CalcViewPage} />
        <Route path="/viewer/decoder" component={DecoderViewPage} />
        <Route path="/viewer/timestamp" component={TimestampViewPage} />
        <Route>404: Not Found</Route>
      </Switch>
    </Router>
  );
}
