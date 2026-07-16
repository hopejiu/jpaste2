import { registerViewerAction } from '../../actions/viewer-action';

registerViewerAction(
  'decoder',
  '解码',
  50,
  '/viewer/decoder',
  (content: string) => {
    const s = content.trim();
    if (!s) return false;
    // 匹配 URL 百分号编码（%XX）
    if (/%[0-9a-fA-F]{2}/.test(s)) return true;
    // 匹配 Unicode 转义（\uXXXX）
    if (/\\u[0-9a-fA-F]{4}/.test(s)) return true;
    // 匹配转义字符串（至少两个转义序列，如 \" \n \\ 等）
    if ((s.match(/\\(["'ntrb\/\\])/g) || []).length >= 2) return true;
    // 匹配 Base64（长度>4、4的倍数、仅含合法字符）—— 最易误触发，放最后兜底
    if (s.length > 4 && s.length % 4 === 0 && /^[A-Za-z0-9+/=]+$/.test(s)) return true;
    return false;
  },
  false,
);
