import { useEffect, useRef, useState } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { copyToClipboard } from '../../lib/clipboard';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { info as logInfo, error as logError } from '../../lib/logger';
import { listen } from '@tauri-apps/api/event';

interface Message {
  type: 'sent' | 'received' | 'system';
  text: string;
  ts: string;
  id: number;
}

let msgCounter = 0;

function isJson(str: string): boolean {
  try { JSON.parse(str); return true; } catch { return false; }
}

function formatJson(str: string): string {
  try { return JSON.stringify(JSON.parse(str), null, 2); } catch { return str; }
}

export function WsViewPage() {
  const { entryId, content, error } = useViewerEntry();
  const [url, setUrl] = useState('');
  const [connected, setConnected] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [expandedJson, setExpandedJson] = useState<Record<number, boolean>>({});
  const wsRef = useRef<WebSocket | null>(null);
  const logRef = useRef<HTMLDivElement>(null);

  // Close socket when the entry changes / component unmounts
  useEffect(() => () => { wsRef.current?.close(); }, []);

  useEffect(() => { if (content) setUrl(content); }, [content]);

  useEffect(() => { if (entryId > 0) logInfo('WsViewPage', { entryId }); }, [entryId]);

  // Window hide → auto disconnect
  useEffect(() => {
    const handler = () => {
      if (wsRef.current) {
        wsRef.current.close(1001, 'window hidden');
        wsRef.current = null;
        setConnected(false);
      }
    };
    window.addEventListener('tauri://close-requested', handler);
    // Also listen for the window-hiding event from the backend
    const unsubPromise = listen('window-hiding', handler).catch(() => undefined);
    return () => {
      window.removeEventListener('tauri://close-requested', handler);
      unsubPromise.then((fn) => fn?.());
    };
  }, []);

  const connect = () => {
    if (!url) return;
    logInfo('WsViewPage:connect', { url });
    const ws = new WebSocket(url);
    ws.onopen = () => {
      logInfo('WsViewPage:connected');
      setConnected(true);
      addMsg('system', '已连接');
    };
    ws.onmessage = (e) => {
      logInfo('WsViewPage:message', { len: e.data?.length });
      addMsg('received', e.data);
    };
    ws.onerror = () => { logError('WsViewPage:error'); addMsg('system', '连接错误'); };
    ws.onclose = () => {
      logInfo('WsViewPage:disconnected');
      setConnected(false);
      addMsg('system', '连接已关闭');
    };
    wsRef.current = ws;
  };

  const disconnect = () => {
    logInfo('WsViewPage:disconnect');
    wsRef.current?.close();
    wsRef.current = null;
  };

  const send = () => {
    if (!input || !wsRef.current) return;
    wsRef.current.send(input);
    addMsg('sent', input);
    setInput('');
  };

  const addMsg = (type: Message['type'], text: string) => {
    const id = msgCounter++;
    setMessages((prev) => [...prev, { type, text, ts: new Date().toLocaleTimeString(), id }]);
    setTimeout(() => logRef.current?.scrollTo(0, logRef.current.scrollHeight), 10);
  };

  const copyMessage = (text: string) => {
    copyToClipboard(text);
  };

  const toggleJson = (id: number) => {
    setExpandedJson((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const clearMessages = () => {
    setMessages([]);
    msgCounter = 0;
  };

  if (error) return <div class="viewer-error">{error}</div>;

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="wifi" size={20} />
        </div>
        <span class="viewer-title">WS 调试</span>
        {messages.length > 0 && (
          <button class="viewer-btn" onClick={clearMessages}>
            <FluentIcon name="delete" size={14} /> 清空
          </button>
        )}
      </div>
      <div class="viewer-content">
        <div class="viewer-section">
          <div class="viewer-section-title">连接</div>
          <div class="viewer-section-desc">输入 WebSocket 地址进行连接</div>
          <div class="ws-url-row">
            <input value={url} onInput={(e) => setUrl((e.target as HTMLInputElement).value)} placeholder="ws://..." />
            {connected
              ? <button class="viewer-btn danger" onClick={disconnect}>断开</button>
              : <button class="viewer-btn primary" onClick={connect}>连接</button>
            }
          </div>
        </div>
        {connected && (
          <div class="viewer-section">
            <div class="viewer-section-title">发送消息</div>
            <div class="viewer-section-desc">输入消息内容，按 Enter 发送</div>
            <div class="ws-input-row">
              <input value={input} onInput={(e) => setInput((e.target as HTMLInputElement).value)} onKeyDown={(e) => e.key === 'Enter' && send()} placeholder="输入消息，Enter 发送" />
              <button class="viewer-btn primary" onClick={send}>发送</button>
            </div>
          </div>
        )}
        <div class="viewer-section">
          <div class="viewer-section-title">消息日志</div>
          <div class="viewer-section-desc">{messages.length} 条消息</div>
          <div class="ws-log" ref={logRef}>
            {messages.length === 0 ? (
              <div class="ws-empty">暂无消息</div>
            ) : (
              messages.map((msg) => {
                const isJsonMsg = isJson(msg.text);
                const expanded = !!expandedJson[msg.id];
                return (
                  <div class={`ws-msg ${msg.type}`} key={msg.id}>
                    <div class="ws-msg-header">
                      <span class="ws-ts">[{msg.ts}]</span>
                      <span class="ws-msg-actions">
                        {isJsonMsg && (
                          <button class="ws-act-btn" onClick={() => toggleJson(msg.id)} title="JSON 展开/折叠">
                            <FluentIcon name={expanded ? 'chevronDown' : 'chevronRight'} size={12} /> JSON
                          </button>
                        )}
                        <button class="ws-act-btn" onClick={() => copyMessage(msg.text)} title="复制" aria-label="复制"><FluentIcon name="copy" size={16} /></button>
                      </span>
                    </div>
                    <span class="ws-text">
                      {isJsonMsg && expanded ? formatJson(msg.text) : msg.text}
                    </span>
                  </div>
                );
              })
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
