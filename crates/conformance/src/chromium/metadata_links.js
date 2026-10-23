(() => {
    // Fixed read-only inspection. Read parsed attributes, never URL properties.
    const result = [];
    let strings = 0;
    function text(value) {
        strings += new TextEncoder().encode(value).length;
        if (strings > 8192) throw new Error("metadata link string limit");
        return value;
    }
    const nodes = document.querySelectorAll("link");
    if (nodes.length > 16) throw new Error("metadata link count limit");
    for (const element of nodes) {
        if (element.attributes.length > 32) throw new Error("metadata link attribute limit");
        const attributes = [];
        for (const a of element.attributes) attributes.push({
            namespace: text(a.namespaceURI || ""), prefix: text(a.prefix || ""),
            local_name: text(a.localName), value: text(a.value)
        });
        attributes.sort((a,b) => {
            for (const key of ["namespace","prefix","local_name"]) {
                if (a[key] < b[key]) return -1;
                if (a[key] > b[key]) return 1;
            }
            return 0;
        });
        result.push({namespace: text(element.namespaceURI || ""), local_name: text(element.localName), attributes});
    }
    return result;
})()
