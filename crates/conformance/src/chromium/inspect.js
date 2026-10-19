(request => {
    "use strict";
    // Fixed trusted inspection only. Page scripts remain disabled. All values
    // below are native DOM/CSSOM facts; this collector implements no CSS rules.
    let count = 0, stringBytes = 0;
    const encoder = new TextEncoder();
    function text(value) {
        stringBytes += encoder.encode(value).length;
        if (stringBytes > 8192) throw new Error("inspection string limit");
        return value;
    }
    function node(n, depth) {
        if (++count > 128 || depth > 32) throw new Error("inspection tree limit");
        const result = {
            node_type: n.nodeType, name: "", namespace: "", value: "",
            public_id: "", system_id: "", attributes: [], children: [], template: null
        };
        switch (n.nodeType) {
        case 9: case 11: break;
        case 10:
            result.name = text(n.name);
            result.public_id = text(n.publicId);
            result.system_id = text(n.systemId);
            break;
        case 1:
            if (n.shadowRoot || n.localName === "iframe" || n.localName === "frame")
                throw new Error("inspection unsupported tree");
            result.name = text(n.localName);
            result.namespace = text(n.namespaceURI || "");
            if (n.attributes.length > 32) throw new Error("inspection attribute limit");
            for (const a of n.attributes) result.attributes.push({
                namespace: text(a.namespaceURI || ""), prefix: text(a.prefix || ""),
                name: text(a.localName), value: text(a.value)
            });
            if (n instanceof HTMLTemplateElement) result.template = node(n.content, depth + 1);
            break;
        case 3: case 8: result.value = text(n.data); break;
        default: throw new Error("inspection unsupported node kind");
        }
        for (const child of n.childNodes) result.children.push(node(child, depth + 1));
        return result;
    }
    const targets = new Set([...request.style_targets, ...request.layout_targets]);
    if (targets.size > 8) throw new Error("inspection target limit");
    // Bounded traversal also proves target lookup complete. Inert template
    // contents belong to the DOM projection, not to rendered target lookup.
    const tree = node(document, 0);
    const elements = new Map();
    for (const e of document.querySelectorAll("[id]")) {
        if (!targets.has(e.id)) continue;
        if (elements.has(e.id)) throw new Error("duplicate target id");
        if (e.namespaceURI !== "http://www.w3.org/1999/xhtml")
            throw new Error("unsupported target namespace");
        elements.set(e.id, e);
    }
    const styles = request.style_targets.map(target => {
        const e = elements.get(target);
        if (!e) return {target, values: null};
        const s = getComputedStyle(e);
        return {target, values: [s.color, s.backgroundColor, s.fontSize, s.display].map(text)};
    });
    const layout = request.layout_targets.map(target => {
        const e = elements.get(target);
        if (!e) return {target, boxes: null};
        return {target, boxes: Array.from(e.getClientRects(), r => ({
            left: r.left, top: r.top, right: r.right, bottom: r.bottom
        }))};
    });
    if (scrollX !== 0 || scrollY !== 0) throw new Error("unexpected inspection scroll");
    return {dom: request.dom ? tree : null, styles, layout, layout_width: innerWidth};
})
