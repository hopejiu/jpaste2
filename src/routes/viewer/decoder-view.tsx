import { useState, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useEntryId } from '../../hooks/use-entry-id';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { error as logError } from '../../lib/logger';

type DecodeMode = 'base64' | 'url' | 'unicode';

export function DecoderViewPage() {
  const entryId = useEntryId();
  const [input, setInput] = useState('');
  const [output, setOutput] = useState('');
  const [mode, setMode] = useState<DecodeMode>('base64');
  const [error, setError] = useState('');
  const [encodeMode, setEncodeMode] = useState(false);

  useEffect(() => {
    if (!entryId) return;
    api.getEntryContent(entryId).then((content) => {
      setInput(content);
      const detectedMode = detectMode(content);
      setMode(detectedMode);
      processDecode(content, detectedMode, encodeMode);
    }).catch((e) => logError('DecoderViewPage', e));
  }, [entryId]);

  const detectMode = (text: string): DecodeMode => {
    const s = text.trim();
    // URL 百分号编码
    if (/%[0-9a-fA-F]{2}/.test(s)) return 'url';
    // Base64（满足长度和字符集）
    if (s.length > 4 && s.length % 4 === 0 && /^[A-Za-z0-9+/=]+$/.test(s)) return 'base64';
    // Unicode 转义
    if (/\\u[0-9a-fA-F]{4}/.test(s)) return 'unicode';
    return 'base64';
  };

  const processDecode = (text: string, m: DecodeMode, enc: boolean) => {
    setError('');
    try {
      let result = '';
      if (m === 'base64') {
        result = enc ? btoa(text) : atob(text);
      } else if (m === 'url') {
        result = enc ? encodeURIComponent(text) : decodeURIComponent(text);
      } else if (m === 'unicode') {
        if (enc) {
          result = text.split('').map((c) => {
            const code = c.charCodeAt(0);
            return code > 127 ? `\\u${code.toString(16).padStart(4, '0')}` : c;
          }).join('');
        } else {
          result = text.replace(/\\u([0-9a-fA-F]{4})/g, (_, hex) =>
            String.fromCharCode(parseInt(hex, 16))
          );
        }
      }
      setOutput(result);
    } catch (e: any) {
      setError(e.message || '解码失败');
      setOutput('');
    }
  };

  const handleInputChange = (val: string) => {
    setInput(val);
    processDecode(val, mode, encodeMode);
  };

  const handleModeChange = (m: DecodeMode) => {
    setMode(m);
    processDecode(input, m, encodeMode);
  };

  const handleToggleEncode = () => {
    const newEnc = !encodeMode;
    setEncodeMode(newEnc);
    processDecode(input, mode, newEnc);
  };

  const handleCopy = () => {
    copyToClipboard(output);
  };

  const handleReverse = () => {
    setInput(output);
    const newEnc = !encodeMode;
    setEncodeMode(newEnc);
    processDecode(output, mode, newEnc);
  };

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="code" size={20} />
        </div>
        <span class="viewer-title">解码工具</span>
        <button class="viewer-btn" onClick={handleToggleEncode}>
          {encodeMode ? '编码' : '解码'}
        </button>
      </div>
      <div class="viewer-content">
        <div class="viewer-section">
          <div class="decoder-section-header">
            <div>
              <div class="viewer-section-title">编解码</div>
              <div class="viewer-section-desc">选择模式并输入内容</div>
            </div>
            <div class="decoder-tabs">
              <button class={`decoder-tab ${mode === 'base64' ? 'active' : ''}`} onClick={() => handleModeChange('base64')}>Base64</button>
              <button class={`decoder-tab ${mode === 'url' ? 'active' : ''}`} onClick={() => handleModeChange('url')}>URL</button>
              <button class={`decoder-tab ${mode === 'unicode' ? 'active' : ''}`} onClick={() => handleModeChange('unicode')}>Unicode</button>
            </div>
          </div>
          <div class="decoder-body">
            <div class="decoder-section">
              <label class="decoder-label">输入</label>
              <textarea
                class="decoder-textarea"
                value={input}
                onInput={(e) => handleInputChange((e.target as HTMLTextAreaElement).value)}
                placeholder="在此输入内容..."
                rows={4}
              />
            </div>
            <div class="decoder-section">
              <div class="decoder-label-row">
                <label class="decoder-label">输出</label>
                <div class="decoder-actions">
                  <button class="viewer-btn" onClick={handleReverse} disabled={!output}>
                    <FluentIcon name="arrowLeft" size={14} /> 反转
                  </button>
                  <button class="viewer-btn" onClick={handleCopy} disabled={!output}>
                    <FluentIcon name="copy" size={14} /> 复制
                  </button>
                </div>
              </div>
              {error ? (
                <div class="decoder-error">{error}</div>
              ) : (
                <textarea
                  class="decoder-textarea"
                  value={output}
                  readOnly
                  placeholder="输出将显示在这里..."
                  rows={4}
                />
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
