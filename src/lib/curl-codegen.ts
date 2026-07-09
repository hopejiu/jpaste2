export const CODEGEN_LANGS = ['python', 'javascript', 'go', 'java', 'curl'] as const;
export type CodegenLang = typeof CODEGEN_LANGS[number];

// ponytail: built with plain concatenation (no nested template interpolations) — keeps the
// generated snippets readable and avoids TSX template-literal parsing pitfalls.
export function genCode(lang: CodegenLang, method: string, url: string, hdrs: { key: string; value: string }[], body: string): string {
  const NL = '\n';
  const headerObj: Record<string, string> = {};
  for (const h of hdrs) if (h.key.trim()) headerObj[h.key.trim()] = h.value;
  const entries = Object.entries(headerObj);

  if (lang === 'curl') {
    const parts = ['curl'];
    if (method !== 'GET') parts.push('-X ' + method);
    parts.push("'" + url + "'");
    for (const [k, v] of entries) parts.push("-H '" + k + ': ' + v + "'");
    if (body) parts.push("-d '" + body + "'");
    return parts.join(' \\\n  ');
  }
  if (lang === 'python') {
    const hl = entries.map(([k, v]) => '    "' + k + '": ' + JSON.stringify(v) + ',').join(NL);
    const dataArg = body ? ',' + NL + '    data=' + JSON.stringify(body) : '';
    return 'import requests' + NL + NL + 'url = ' + JSON.stringify(url) + NL + NL +
      'headers = {' + NL + hl + NL + '}' + NL + NL +
      'response = requests.request(' + JSON.stringify(method) + ', url, headers=headers' + dataArg + ')' + NL + NL +
      'print(response.status_code)' + NL + 'print(response.text)';
  }
  if (lang === 'javascript') {
    const hl = entries.map(([k, v]) => '    ' + JSON.stringify(k) + ': ' + JSON.stringify(v) + ',').join(NL);
    let parsed: unknown;
    try { parsed = body ? JSON.parse(body) : undefined; } catch { parsed = undefined; }
    const bodyArg = body ? ',' + NL + '    body: ' + (parsed !== undefined ? JSON.stringify(parsed) : JSON.stringify(body)) : '';
    return 'fetch(' + JSON.stringify(url) + ', {' + NL + '  method: ' + JSON.stringify(method) + ',' + NL +
      '  headers: {' + NL + hl + NL + '}' + bodyArg + NL + '})' + NL + '  .then(r => r.text())' + NL + '  .then(console.log);';
  }
  if (lang === 'go') {
    const hl = entries.map(([k, v]) => '    req.Header.Set(' + JSON.stringify(k) + ', ' + JSON.stringify(v) + ')').join(NL);
    const hasBody = !!body;
    return 'package main' + NL + NL + 'import (' + NL + '\t"fmt"' + NL + '\t"io"' + NL + '\t"net/http"' + NL + '\t"strings"' + NL + ')' + NL + NL +
      'func main() {' + NL + '\turl := ' + JSON.stringify(url) + NL + '\tmethod := ' + JSON.stringify(method) + NL +
      '\tbody := strings.NewReader(' + JSON.stringify(body || '') + ')' + NL + NL +
      '\treq, _ := http.NewRequest(method, url, ' + (hasBody ? 'body' : 'nil') + ')' + NL +
      (hl ? hl + NL : '') + NL +
      '\tclient := &http.Client{}' + NL + '\tresp, err := client.Do(req)' + NL + '\tif err != nil {' + NL + '\t\tpanic(err)' + NL + '\t}' + NL +
      '\tdefer resp.Body.Close()' + NL + '\tb, _ := io.ReadAll(resp.Body)' + NL + '\tfmt.Println(string(b))' + NL + '}';
  }
  // java
  const hl = entries.map(([k, v]) => '      .header(' + JSON.stringify(k) + ', ' + JSON.stringify(v) + ')').join(NL);
  return 'import java.net.http.*;' + NL + 'import java.net.URI;' + NL + NL +
    'public class Main {' + NL + '  public static void main(String[] args) throws Exception {' + NL +
    '    var request = HttpRequest.newBuilder()' + NL + '      .uri(URI.create(' + JSON.stringify(url) + '))' + NL +
    '      .method(' + JSON.stringify(method) + ', HttpRequest.BodyPublishers.ofString(' + JSON.stringify(body || '') + '))' + NL +
    hl + NL + '      .build();' + NL +
    '    var response = HttpClient.newHttpClient().send(request, HttpResponse.BodyHandlers.ofString());' + NL +
    '    System.out.println(response.statusCode());' + NL + '    System.out.println(response.body());' + NL + '  }' + NL + '}';
}
