declare module 'jsoneditor' {
  interface JSONEditorOptions {
    mode?: 'tree' | 'code' | 'text' | 'view' | 'form';
    modes?: string[];
    mainMenuBar?: boolean;
    navigationBar?: boolean;
    statusBar?: boolean;
    search?: boolean;
    history?: boolean;
    indentation?: number;
    sortObjectKeys?: boolean;
    limitDragging?: boolean;
    onChange?: () => void;
    onModeChange?: (mode: string) => void;
    onError?: (err: Error) => void;
  }

  class JSONEditor {
    constructor(container: HTMLElement, options?: JSONEditorOptions, json?: any);
    set(json: any): void;
    update(json: any): void;
    get(): any;
    getText(): string;
    setMode(mode: string): void;
    destroy(): void;
  }

  export default JSONEditor;
}
