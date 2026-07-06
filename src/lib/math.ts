/**
 * Safe math expression evaluator using Shunting-yard algorithm.
 * Extracted from calc-view.tsx for reuse by actions/math.tsx.
 */

/** Safely evaluate a math expression, returning null on invalid input */
export function safeEvaluate(expr: string): number | null {
  const s = expr.trim();
  if (!s) return null;
  const tokens: (string | number)[] = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === ' ' || c === '\t') { i++; continue; }
    if ((c >= '0' && c <= '9') || c === '.') {
      let num = '';
      while (i < s.length && ((s[i] >= '0' && s[i] <= '9') || s[i] === '.')) num += s[i++];
      const parsed = parseFloat(num);
      if (isNaN(parsed)) return null;
      tokens.push(parsed); continue;
    }
    if ('+-*/%()'.includes(c)) { tokens.push(c); i++; continue; }
    return null;
  }
  if (tokens.length === 0) return null;
  try { return evaluateTokens(tokens); } catch { return null; }
}

function evaluateTokens(tokens: (string | number)[]): number {
  const output: number[] = [];
  const ops: string[] = [];
  const precedence: Record<string, number> = { '+': 1, '-': 1, '*': 2, '/': 2, '%': 2 };
  const applyOp = () => {
    const op = ops.pop(); const b = output.pop(); const a = output.pop();
    if (!op || a === undefined || b === undefined) throw new Error('Invalid');
    switch (op) {
      case '+': output.push(a + b); break;
      case '-': output.push(a - b); break;
      case '*': output.push(a * b); break;
      case '/': if (b === 0) throw new Error('Div0'); output.push(a / b); break;
      case '%': if (b === 0) throw new Error('Div0'); output.push(a % b); break;
    }
  };
  for (const token of tokens) {
    if (typeof token === 'number') { output.push(token); }
    else if (token === '(') { ops.push(token); }
    else if (token === ')') { while (ops.length > 0 && ops[ops.length - 1] !== '(') applyOp(); ops.pop(); }
    else { while (ops.length > 0 && ops[ops.length - 1] !== '(' && (precedence[ops[ops.length - 1]] ?? 0) >= (precedence[token] ?? 0)) applyOp(); ops.push(token); }
  }
  while (ops.length > 0) applyOp();
  return output[0];
}
