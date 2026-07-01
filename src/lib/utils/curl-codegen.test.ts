import { describe, it, expect } from 'vitest';
import { genCode, CODEGEN_LANGS } from './curl-codegen';

const hdrs = [
  { key: 'Content-Type', value: 'application/json' },
  { key: 'Authorization', value: 'Bearer tok' },
];
const body = '{"a":1}';

describe('genCode', () => {
  it('covers every language in CODEGEN_LANGS', () => {
    for (const lang of CODEGEN_LANGS) {
      const out = genCode(lang, 'POST', 'https://x.io/api', hdrs, body);
      expect(out.length).toBeGreaterThan(0);
    }
  });

  it('curl: includes method, url, headers and body', () => {
    const out = genCode('curl', 'POST', 'https://x.io/api', hdrs, body);
    expect(out).toContain("-X POST");
    expect(out).toContain("'https://x.io/api'");
    expect(out).toContain("-H 'Content-Type: application/json'");
    expect(out).toContain("-d '{\"a\":1}'");
  });

  it('curl: omits -X for GET and -d when no body', () => {
    const out = genCode('curl', 'GET', 'https://x.io/api', hdrs, '');
    expect(out).not.toContain('-X GET');
    expect(out).not.toContain('-d');
  });

  it('python: embeds json body and headers', () => {
    const out = genCode('python', 'POST', 'https://x.io/api', hdrs, body);
    expect(out).toContain('import requests');
    expect(out).toContain('"Content-Type": "application/json"');
    expect(out).toContain('data=');
  });

  it('javascript: inlines parsed JSON body when possible', () => {
    const out = genCode('javascript', 'POST', 'https://x.io/api', hdrs, body);
    expect(out).toContain('fetch(');
    expect(out).toContain('body: {"a":1}');
  });

  it('go: produces compilable-shaped snippet with body reader', () => {
    const out = genCode('go', 'POST', 'https://x.io/api', hdrs, body);
    expect(out).toContain('package main');
    expect(out).toContain('strings.NewReader(');
    expect(out).toContain('req.Header.Set(');
  });

  it('go: uses nil body when no body', () => {
    const out = genCode('go', 'GET', 'https://x.io/api', hdrs, '');
    expect(out).toContain('http.NewRequest(method, url, nil)');
  });

  it('java: well-formed with method and URI', () => {
    const out = genCode('java', 'POST', 'https://x.io/api', hdrs, body);
    expect(out).toContain('HttpRequest.newBuilder()');
    expect(out).toContain('.method("POST",');
    expect(out).toContain('BodyPublishers.ofString(');
  });
});
