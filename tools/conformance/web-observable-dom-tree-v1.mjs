// Independent implementation of docs/conformance/ag9-cross-engine-comparison-reporting.md.
// This is an inspector, NOT a capture mechanism. Reading a DOM proves no history.
export const algorithm = Object.freeze({ id: 'web-observable-dom-tree-v1-inspector', version: '1' });
export const MAX_BYTES = 8_388_608;
const HTML = 'http://www.w3.org/1999/xhtml';
const SVG = 'http://www.w3.org/2000/svg';
const MATH = 'http://www.w3.org/1998/Math/MathML';
const XML = 'http://www.w3.org/XML/1998/namespace';
const XMLNS = 'http://www.w3.org/2000/xmlns/';
const XLINK = 'http://www.w3.org/1999/xlink';
export class InspectionFailure extends Error {
  constructor(code) { super(code); this.code = code; }
}
function fail(code) { throw new InspectionFailure(code); }
function string(value) {
  if (typeof value !== 'string') fail('invalid-string');
  for (let i = 0; i < value.length; i++) {
    const n = value.charCodeAt(i);
    if (n >= 0xd800 && n <= 0xdbff) {
      const low = value.charCodeAt(++i);
      if (!(low >= 0xdc00 && low <= 0xdfff)) fail('malformed-unicode');
    } else if (n >= 0xdc00 && n <= 0xdfff) fail('malformed-unicode');
  }
  return value;
}
function utf8Length(value) {
  string(value);
  let n = 0;
  for (const ch of value) {
    const cp = ch.codePointAt(0);
    n += cp < 128 ? 1 : cp < 2048 ? 2 : cp < 65536 ? 3 : 4;
    if (n > MAX_BYTES) fail('too-large');
  }
  return n;
}
class Writer {
  constructor() { this.bytes = new Uint8Array(MAX_BYTES); this.length = 0; }
  raw(value) {
    const size = utf8Length(value);
    const next = this.length + size;
    if (!Number.isSafeInteger(next) || next > MAX_BYTES) fail('too-large');
    // encodeInto cannot retain an unbounded temporary UTF-8 artifact.
    const result = new TextEncoder().encodeInto(value, this.bytes.subarray(this.length, next));
    if (result.read !== value.length || result.written !== size) fail('encoding');
    this.length = next;
  }
  quoted(value) {
    string(value); this.raw('"');
    let start = 0;
    for (let i = 0; i < value.length; i++) {
      const n = value.charCodeAt(i);
      let escape;
      if (n === 34) escape = '\\"';
      else if (n === 92) escape = '\\\\';
      else if (n === 10) escape = '\\n';
      else if (n === 13) escape = '\\r';
      else if (n === 9) escape = '\\t';
      else if (n < 32 || n === 127) escape = `\\u00${n.toString(16).padStart(2, '0')}`;
      if (escape !== undefined) { this.raw(value.slice(start, i)); this.raw(escape); start = i + 1; }
    }
    this.raw(value.slice(start)); this.raw('"');
  }
  field(key, value) { this.raw(`${key} = `); this.quoted(value); this.raw('\n'); }
  optional(key, value) { if (value === null) this.raw(`${key} = null\n`); else this.field(key, value); }
  count(key, value) {
    if (!Number.isSafeInteger(value) || value < 0) fail('invalid-count');
    this.raw(`${key} = ${value}\n`);
  }
  finish() { return this.bytes.slice(0, this.length); }
}
function children(node) {
  const list = node.childNodes;
  if (!list || !Number.isSafeInteger(list.length) || list.length < 0) fail('invalid-children');
  // Every node occupies more than one encoded byte. Reject impossible counts
  // before constructing any count-sized workspace.
  if (list.length > MAX_BYTES) fail('too-large');
  return list;
}
function byteCompare(a, b) {
  if (a === b) return 0;
  if (a === null) return -1;
  if (b === null) return 1;
  // Validated Unicode scalar order equals lexicographic UTF-8 byte order;
  // encode explicitly here so this cannot become JS UTF-16 ordering.
  utf8Length(a); utf8Length(b);
  const encoder = new TextEncoder();
  const x = encoder.encode(a), y = encoder.encode(b);
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] - y[i];
  return x.length - y.length;
}
function keyCompare(a, b) {
  for (const key of ['namespaceURI', 'localName', 'prefix', 'name']) {
    const c = byteCompare(a[key], b[key]); if (c) return c;
  }
  return 0;
}
function attributes(node, w) {
  const source = node.attributes;
  if (!source || !Number.isSafeInteger(source.length) || source.length < 0) fail('invalid-attributes');
  if (source.length > MAX_BYTES / 100) fail('too-large');
  const sorted = [];
  for (let i = 0; i < source.length; i++) {
    const a = source[i];
    utf8Length(a.localName); utf8Length(a.name); utf8Length(a.value);
    if (a.prefix !== null) string(a.prefix);
    const ns = a.namespaceURI, p = a.prefix;
    if (!((ns === null && p === null) || (ns === XML && p === 'xml') ||
      (ns === XLINK && p === 'xlink') || (ns === XMLNS && (p === 'xmlns' || (p === null && a.localName === 'xmlns'))))) fail('invalid-attribute-namespace');
    if (a.name !== (p === null ? a.localName : `${p}:${a.localName}`)) fail('invalid-qualified-name');
    sorted.push(a);
  }
  sorted.sort(keyCompare);
  for (let i = 1; i < sorted.length; i++) if (keyCompare(sorted[i - 1], sorted[i]) === 0) fail('duplicate-attribute');
  w.count('attribute-count', sorted.length);
  for (const a of sorted) {
    w.raw('attribute-begin = true\n'); w.optional('namespace-uri', a.namespaceURI);
    w.optional('prefix', a.prefix); w.field('local-name', a.localName);
    w.field('qualified-name', a.name); w.field('value', a.value); w.raw('attribute-end = true\n');
  }
}
function inspect(document) {
  if (document?.nodeType !== 9) fail('document-required');
  const w = new Writer(), seen = new WeakSet();
  w.raw('format = "web-observable-dom-tree-v1"\nroot-count = 1\n');
  const stack = [{ node: document, root: true }];
  while (stack.length) {
    const frame = stack.pop();
    if (frame.end) { w.field('node-end', frame.end); continue; }
    if (frame.list) {
      if (frame.index < frame.list.length) {
        stack.push({ list: frame.list, index: frame.index + 1 });
        stack.push({ node: frame.list[frame.index], root: false });
      }
      continue;
    }
    if ('template' in frame) {
      w.field('template-contents', frame.template === null ? 'absent' : 'present');
      if (frame.template !== null) { const list = children(frame.template); w.count('template-child-count', list.length); stack.push({ list, index: 0 }); }
      continue;
    }
    const n = frame.node;
    if (!n || typeof n !== 'object' || seen.has(n)) fail('invalid-tree');
    seen.add(n);
    switch (n.nodeType) {
      case 9: {
        if (!frame.root) fail('nested-document');
        w.field('node-begin', 'document'); const list = children(n); w.count('child-count', list.length);
        stack.push({ end: 'document' }, { list, index: 0 }); break;
      }
      case 10:
        w.field('node-begin', 'document-type'); w.field('name', n.name); w.field('public-id', n.publicId); w.field('system-id', n.systemId); w.field('node-end', 'document-type'); break;
      case 3: case 8: case 7: {
        const kind = n.nodeType === 3 ? 'text' : n.nodeType === 8 ? 'comment' : 'processing-instruction';
        w.field('node-begin', kind); if (n.nodeType === 7) w.field('target', n.target);
        w.field('data', n.data); w.field('node-end', kind); break;
      }
      case 1: {
        // shadowRoot is an Element member; unrelated Node/HTML IDL members
        // (notably hyperlink .host) do not identify a shadow interface.
        if (n.shadowRoot != null) fail('shadow-state');
        if (![HTML, SVG, MATH].includes(n.namespaceURI)) fail('element-namespace');
        if (n.prefix !== null) fail('element-prefix');
        w.field('node-begin', 'element'); w.field('namespace-uri', n.namespaceURI); w.field('local-name', n.localName);
        attributes(n, w); const list = children(n); w.count('child-count', list.length);
        let content = null;
        if (n.namespaceURI === HTML && n.localName === 'template') {
          content = n.content;
          if (!content || content.nodeType !== 11 || seen.has(content)) fail('template-content');
          // A template DocumentFragment is its own shadow-including root.
          // A ShadowRoot instead resolves through its host, even if detached
          // or closed. This uses Node semantics without probing .host or
          // realm-specific instanceof checks. It cannot discover a hidden root
          // for which the inspector has no reference.
          if (typeof content.getRootNode !== 'function' || content.getRootNode({ composed: true }) !== content) fail('template-content');
          seen.add(content);
        }
        stack.push({ end: 'element' }, { template: content }, { list, index: 0 }); break;
      }
      default: fail('unsupported-node');
    }
  }
  return w.finish();
}
export function inspectWebObservableDomTreeV1(document) {
  try { return inspect(document); }
  catch (error) { if (error instanceof InspectionFailure) throw error; throw new InspectionFailure('allocation-or-infrastructure'); }
}
export function captureWebObservableDomTreeV1() {
  // No mechanism currently establishes the frozen historical input context.
  throw new InspectionFailure('unsupported-capture-mechanism');
}
