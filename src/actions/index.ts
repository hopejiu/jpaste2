// Import all action modules to trigger automatic registration
import './math';
import './json';
import './folder';
import './decoder';
import './open-url';
import './curl';
import './ws';
import './timestamp';
import './qrcode';

export { register, detect, getModules, get } from './registry';
export type { ActionModule } from './registry';
