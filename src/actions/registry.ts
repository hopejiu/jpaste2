export interface ActionModule {
  id: string;
  label: string;
  priority: number;
  detect(content: string, tagMask?: number): boolean;
  handler?(content: string, entryId: number): void;
}

const modules = new Map<string, ActionModule>();

export function register(mod: ActionModule) {
  modules.set(mod.id, mod);
}

export function getModules(): ActionModule[] {
  return Array.from(modules.values());
}

export function detect(content: string, tagMask?: number): ActionModule[] {
  return getModules()
    .filter((m) => m.detect(content, tagMask))
    .sort((a, b) => b.priority - a.priority)
    .slice(0, 3);
}

export function get(id: string): ActionModule | undefined {
  return modules.get(id);
}
