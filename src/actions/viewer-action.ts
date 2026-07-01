import { register } from './registry';
import { api } from '../lib/invoke';
import { error as logError } from '../lib/logger';

/**
 * Register a viewer-launching action.
 *
 * Replaces the previous per-viewer duplication (json/curl/ws/base64/timestamp/
 * math each calling its own `open_*_viewer` Tauri command with a near-identical
 * `.catch` fallback). All viewers now go through the single `open_viewer`
 * command driven by the `VIEWERS` registry on the Rust side.
 *
 * `fallback` mirrors the original behavior: json/curl/ws navigate the main
 * window to the route on failure (dev fallback), while base64/timestamp/math
 * only log the error.
 */
export function registerViewerAction(
  id: string,
  label: string,
  priority: number,
  route: string,
  detect: (content: string) => boolean,
  fallback: boolean = false,
): void {
  register({
    id,
    label,
    priority,
    detect,
    handler: (_content: string, entryId: number) => {
      api
        .openViewer(route, entryId)
        .catch((e) => {
          logError(`action:${id} openViewer FAILED`, e);
          if (fallback) window.location.hash = `${route}?id=${entryId}`;
        });
    },
  });
}
