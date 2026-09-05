import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { inspectWebObservableDomTreeV1 as inspect, captureWebObservableDomTreeV1 as capture, MAX_BYTES } from './web-observable-dom-tree-v1.mjs';
const H = 'http://www.w3.org/1999/xhtml', S = 'http://www.w3.org/2000/svg', M = 'http://www.w3.org/1998/Math/MathML';
const X = 'http://www.w3.org/XML/1998/namespace', N = 'http://www.w3.org/2000/xmlns/', L = 'http://www.w3.org/1999/xlink';
const doc = (...childNodes) => ({ nodeType: 9, childNodes });
const text = data => ({ nodeType: 3, data });
const comment = data => ({ nodeType: 8, data });
const element = (namespaceURI, localName, attributes = [], childNodes = []) => ({ nodeType: 1, namespaceURI, localName, prefix: null, attributes, childNodes });
const attr = (namespaceURI, prefix, localName, value) => ({ namespaceURI, prefix, localName, name: prefix === null ? localName : `${prefix}:${localName}`, value });
// Root relationships model Node.getRootNode, independently of the codec.
class DocumentFragmentDouble {
  constructor(childNodes = []) { this.nodeType = 11; this.childNodes = childNodes; }
  getRootNode() { return this; }
}
class ShadowRootDouble extends DocumentFragmentDouble {
  constructor(shadowIncludingRoot) { super(); this.shadowIncludingRoot = shadowIncludingRoot; }
  getRootNode({ composed = false } = {}) { return composed ? this.shadowIncludingRoot : this; }
}
const template = (childNodes, contents) => ({ ...element(H, 'template', [], childNodes), content: new DocumentFragmentDouble(contents) });
function golden(name, tree) {
  const expected = readFileSync(new URL(`../../tests/contract-vectors/web-observable-dom-tree-v1/${name}.txt`, import.meta.url));
  assert.deepEqual(Buffer.from(inspect(tree)), expected);
}
test('independent DOM setup against shared reviewed vectors', () => {
  golden('nodes', doc({ nodeType: 10, name: 'html', publicId: '', systemId: '' }, { nodeType: 10, name: 'html', publicId: 'public', systemId: 'system' }, text('text'), comment('comment'), { nodeType: 7, target: 'target', data: 'data' }));
  golden('namespaces-attributes', doc(element(H, 'div'), element(S, 'foreignObject', [attr(X, 'xml', 'space', 'preserve'), attr(N, null, 'xmlns', S), attr(X, 'xml', 'lang', 'en'), attr(null, null, 'id', 'plain'), attr(N, 'xmlns', 'xlink', L), attr(L, 'xlink', 'href', '#target')]), element(M, 'mi')));
  golden('templates', doc(template([text('ordinary')], [comment('content'), template([], [text('nested')])])));
  golden('escaping-utf8-ordering', doc(text('\0\u000b\u001f\u007f\r\n\t"\\é😀\u2028\u2029'), element(H, 'div', [attr(null, null, '\u{10000}', 'second'), attr(null, null, '\ue000', 'first')])));
  golden('static-document-different', doc());
  golden('static-document', doc({ nodeType: 10, name: 'html', publicId: '', systemId: '' }, element(H, 'html', [], [element(H, 'head'), element(H, 'body', [], [text('lead'), element(H, 'p', [], [text('ok')]), text('\n')])])));
});
test('unsupported and malformed states fail closed', () => {
  const bad = [text('no document'), doc(doc()), doc({ nodeType: 4 }), doc({ nodeType: 11, childNodes: [] }), doc(element('unknown', 'x')), doc({ ...element(H, 'p'), prefix: 'p' }), doc(text('\ud800')), doc(text('\udfff')), doc({ ...element(H, 'p'), shadowRoot: {} }), doc(element(H, 'template')), doc({ ...template([], []), content: text('wrong') })];
  for (const tree of bad) assert.throws(() => inspect(tree));
  for (const a of [attr(null, 'x', 'id', ''), attr(X, null, 'lang', ''), attr(L, 'xml', 'href', ''), attr(N, null, 'other', ''), attr('unknown', null, 'id', ''), { ...attr(null, null, 'id', ''), name: 'other' }]) assert.throws(() => inspect(doc(element(S, 'svg', [a]))));
  const a = attr(null, null, 'id', ''); assert.throws(() => inspect(doc(element(H, 'p', [a, a]))));
  const shared = template([], []); assert.throws(() => inspect(doc(shared, { ...template([], []), content: shared.content })));
  assert.throws(capture, /unsupported-capture-mechanism/);
});
test('exact encoded ceiling and escaping expansion', () => {
  const overhead = inspect(doc(text(''))).length;
  const count = MAX_BYTES - overhead;
  assert.equal(inspect(doc(text('a'.repeat(count)))).length, MAX_BYTES);
  assert.throws(() => inspect(doc(text('a'.repeat(count + 1)))), /too-large/);
  assert.throws(() => inspect(doc(text('\0'.repeat(Math.floor(count / 6) + 1)))), /too-large/);
  assert.throws(() => inspect({ nodeType: 9, get childNodes() { throw new RangeError('allocation'); } }), /allocation-or-infrastructure/);
});
test('only HTML template identity selects the content fragment', () => {
  for (const name of ['meta', 'div']) {
    const node = element(H, name);
    node.content = 'unrelated IDL value';
    const bytes = Buffer.from(inspect(doc(node))).toString('utf8');
    assert.ok(bytes.includes(`local-name = "${name}"\n`));
    assert.ok(bytes.includes('template-contents = "absent"\n'));
    Object.defineProperty(node, 'content', { get() { throw new Error('must not read'); } });
    assert.equal(Buffer.from(inspect(doc(node))).toString('utf8'), bytes);
  }
  for (const content of [undefined, null, 'string', text('wrong')]) {
    assert.throws(() => inspect(doc({ ...element(H, 'template'), content })));
  }
  golden('templates', doc(template([text('ordinary')], [comment('content'), template([], [text('nested')])])));
  const shared = template([], []);
  assert.throws(() => inspect(doc(shared, { ...template([], []), content: shared.content })));
});

test('hyperlink interface host values never identify shadow state', () => {
  class HyperlinkElementDouble {
    constructor(name, href) {
      Object.assign(this, element(H, name, href === null ? [] : [attr(null, null, 'href', href)]));
      this.hrefValue = href;
    }
    // Like the hyperlink IDL getter, this is inherited, not an own property.
    get host() { return this.hrefValue === null ? '' : new URL(this.hrefValue).host; }
  }
  for (const name of ['a', 'area']) {
    for (const href of [null, 'https://example.com/']) {
      const node = new HyperlinkElementDouble(name, href);
      assert.equal(Object.hasOwn(node, 'host'), false);
      assert.equal(node.host, href === null ? '' : 'example.com');
      const output = Buffer.from(inspect(doc(node))).toString('utf8');
      assert.ok(output.includes(`local-name = "${name}"\n`));
      assert.ok(output.includes('template-contents = "absent"\n'));
      // Assert full byte equivalence to the same semantic tree without URL IDL.
      assert.deepEqual(inspect(doc(node)), inspect(doc(element(H, name, node.attributes))));
      Object.defineProperty(node, 'host', { get() { throw new Error('host must not be read'); } });
      assert.equal(Buffer.from(inspect(doc(node))).toString('utf8'), output);
    }
  }
  const ordinary = element(H, 'div');
  Object.defineProperty(ordinary, 'host', { get() { throw new Error('unrelated host'); } });
  assert.ok(Buffer.from(inspect(doc(ordinary))).toString('utf8').includes('template-contents = "absent"\n'));
});
test('shadow checks follow Element and fragment root relationships only', () => {
  const exposed = new ShadowRootDouble(doc());
  assert.throws(() => inspect(doc({ ...element(H, 'div'), shadowRoot: exposed })), /shadow-state/);
  for (const fragment of [new DocumentFragmentDouble(), exposed]) {
    assert.throws(() => inspect(fragment), /document-required/);
    assert.throws(() => inspect(doc(fragment)), /unsupported-node/);
  }
  // A referenced shadow root resolves outside itself, whether its host tree is
  // connected, detached, or exposes no root through Element.shadowRoot (closed).
  for (const root of [doc(), { ...element(H, 'div'), shadowRoot: null }]) {
    const shadow = new ShadowRootDouble(root);
    assert.equal(shadow.getRootNode(), shadow);
    assert.notEqual(shadow.getRootNode({ composed: true }), shadow);
    assert.throws(() => inspect(doc({ ...template([], []), content: shadow })), /template-content/);
  }
  for (const content of [{ nodeType: 11, childNodes: [] }, { nodeType: 11, childNodes: [], getRootNode: () => null }]) {
    assert.throws(() => inspect(doc({ ...template([], []), content })), /template-content/);
  }
  const t = template([], []);
  const document = doc(t, text('text'), comment('comment'));
  for (const node of [document, t.content, ...document.childNodes.slice(1)]) {
    for (const key of ['host', 'shadowRoot']) {
      Object.defineProperty(node, key, { get() { throw new Error(`unrelated ${key}`); } });
    }
  }
  inspect(document); // Never inspect Element-only properties on other node kinds.
  const shared = template([], []);
  assert.throws(() => inspect(doc(shared, { ...template([], []), content: shared.content })), /template-content/);
});
