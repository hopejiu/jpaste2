// Import all action modules to trigger automatic registration.
// Each feature lives under src/features/<name>/ and registers itself on import.
import '../features/json/action';
import '../features/curl/action';
import '../features/ws/action';
import '../features/decoder/action';
import '../features/timestamp/action';
import '../features/calc/action';
import '../features/qr/action';
import '../features/folder';
import '../features/open-url';

export { register, detect, getModules, get } from './registry';
export type { ActionModule } from './registry';
