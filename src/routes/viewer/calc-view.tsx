import { useState, useEffect } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { useViewerEntry } from '../../hooks/use-viewer-entry';
import { safeEvaluate } from '../../lib/math';

export function CalcViewPage() {
  const { content } = useViewerEntry();
  const [expr, setExpr] = useState('');
  const [result, setResult] = useState<string | null>(null);
  const [history, setHistory] = useState<Array<{ expr: string; result: string }>>([]);

  useEffect(() => {
    if (!content) return; // blank viewer for toolbox
    setExpr(content);
    const r = safeEvaluate(content);
    if (r !== null) {
      setResult(String(r));
      setHistory([{ expr: content.trim(), result: String(r) }]);
    }
  }, [content]);

  const handleEval = () => {
    const r = safeEvaluate(expr);
    if (r !== null) {
      const resultStr = String(r);
      setResult(resultStr);
      setHistory((prev) => [{ expr: expr.trim(), result: resultStr }, ...prev].slice(0, 20));
    } else {
      setResult('错误');
    }
  };

  useEffect(() => {
    const calcKeys = new Set(['0','1','2','3','4','5','6','7','8','9','+','-','*','/','%','(',')','.','Backspace','Delete','Escape']);
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key === 'Enter') { e.preventDefault(); handleEval(); }
      else if (e.key === 'Escape') { setExpr(''); setResult(null); }
      else if (calcKeys.has(e.key)) { e.preventDefault(); setExpr((prev) => prev + e.key); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [expr]);

  return (
    <div class="viewer-page">
      <div class="viewer-toolbar" data-tauri-drag-region>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="calculator" size={20} />
        </div>
        <span class="viewer-title">计算器</span>
      </div>
      <div class="viewer-content">
        <div class="viewer-section">
          <div class="viewer-section-title">表达式</div>
          <div class="viewer-section-desc">输入数学表达式进行计算</div>
          <div class="calc-display">
            <div class="calc-expr">{expr || '输入表达式'}</div>
            <div class="calc-result">{result ?? '0'}</div>
          </div>
          <div class="calc-input-row">
            <input
              class="calc-input"
              value={expr}
              onInput={(e) => setExpr((e.target as HTMLInputElement).value)}
              placeholder="例如: 1+2*3"
            />
            <button class="viewer-btn primary" onClick={handleEval}>=</button>
          </div>
        </div>
        <div class="viewer-section">
          <div class="viewer-section-title">键盘</div>
          <div class="viewer-section-desc">点击按键或键盘输入</div>
          <div class="calc-keypad">
            {['7','8','9','/','*','4','5','6','-','+','1','2','3','(',')','0','.','%','C','='].map((key) => (
              <button
                key={key}
                class={`calc-key ${['/','*','-','+','%','=',')','('].includes(key) ? 'op' : ''} ${key === 'C' ? 'clear' : ''}`}
                onClick={() => {
                  if (key === 'C') { setExpr(''); setResult(null); }
                  else { setExpr((prev) => prev + key); }
                }}
              >{key}</button>
            ))}
          </div>
        </div>
        {history.length > 0 && (
          <div class="viewer-section">
            <div class="viewer-section-title">历史</div>
            <div class="viewer-section-desc">最近 {history.length} 条计算记录</div>
            <div class="calc-history">
              {history.map((h, i) => (
                <div key={i} class="calc-history-item" onClick={() => { setExpr(h.expr); setResult(h.result); }}>
                  <span class="calc-history-expr">{h.expr}</span>
                  <span class="calc-history-res">= {h.result}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
