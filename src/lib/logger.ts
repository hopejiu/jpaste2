/**
 * Frontend logger that mirrors the old jPaste approach:
 * - Logs to browser console (for dev inspector)
 * - Forwards all logs to Rust backend via Tauri event `frontend-log`
 *   so they appear in the terminal and log files
 */

import { emit } from '@tauri-apps/api/event';

type LogLevel = 'debug' | 'info' | 'warn' | 'error';

let component = 'app';

/** Set the current component name for log messages */
export function setComponent(name: string) {
  component = name;
}

/** Emit a log event to the Rust backend via Tauri event */
async function emitToBackend(level: LogLevel, msg: string) {
  try {
    await emit('frontend-log', {
      level,
      component,
      msg,
      ts: new Date().toISOString(),
    });
  } catch {
    // Tauri API not available (e.g., in vitest or browser dev)
  }
}

/** Format args like console.log does */
function formatArgs(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === 'object') {
        try {
          return JSON.stringify(a, null, 2);
        } catch {
          return String(a);
        }
      }
      return String(a);
    })
    .join(' ');
}

// ── Public API ────────────────────────────────────────────────────────

export function debug(...args: unknown[]) {
  const msg = formatArgs(args);
  console.debug(`[${component}]`, ...args);
  emitToBackend('debug', msg);
}

export function info(...args: unknown[]) {
  const msg = formatArgs(args);
  console.info(`[${component}]`, ...args);
  emitToBackend('info', msg);
}

export function warn(...args: unknown[]) {
  const msg = formatArgs(args);
  console.warn(`[${component}]`, ...args);
  emitToBackend('warn', msg);
}

export function error(...args: unknown[]) {
  const msg = formatArgs(args);
  console.error(`[${component}]`, ...args);
  emitToBackend('error', msg);
}

/** Create a child logger with a sub-component prefix */
export function child(subComponent: string) {
  const fullName = `${component}:${subComponent}`;
  return {
    debug: (...args: unknown[]) => {
      const prev = component;
      component = fullName;
      debug(...args);
      component = prev;
    },
    info: (...args: unknown[]) => {
      const prev = component;
      component = fullName;
      info(...args);
      component = prev;
    },
    warn: (...args: unknown[]) => {
      const prev = component;
      component = fullName;
      warn(...args);
      component = prev;
    },
    error: (...args: unknown[]) => {
      const prev = component;
      component = fullName;
      error(...args);
      component = prev;
    },
  };
}
