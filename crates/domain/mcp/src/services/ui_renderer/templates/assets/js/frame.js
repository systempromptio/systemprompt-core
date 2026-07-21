// Size negotiation per MCP Apps (SEP-1865). Method names come from the
// injected MCP_UI constants, which are generated from the Rust `UiMethod`
// enum — never spell them out here.
const McpAppFrame = {
    parent: window.parent,
    origin: '*',
    lastWidth: 0,
    lastHeight: 0,
    pending: false,

    init() {
        const observer = new ResizeObserver(() => this.schedule());
        observer.observe(document.documentElement);
        window.addEventListener('load', () => this.schedule());
        this.schedule();
    },

    schedule() {
        if (this.pending) {
            return;
        }
        this.pending = true;
        requestAnimationFrame(() => {
            this.pending = false;
            this.publish();
        });
    },

    measure() {
        const doc = document.documentElement;
        const body = document.body;
        return {
            width: Math.ceil(Math.max(doc.scrollWidth, body ? body.scrollWidth : 0)),
            height: Math.ceil(Math.max(
                doc.scrollHeight,
                doc.offsetHeight,
                body ? body.scrollHeight : 0,
                body ? body.offsetHeight : 0
            ))
        };
    },

    publish() {
        const { width, height } = this.measure();
        if (height === 0 || (width === this.lastWidth && height === this.lastHeight)) {
            return;
        }
        this.lastWidth = width;
        this.lastHeight = height;

        this.parent.postMessage({
            jsonrpc: '2.0',
            method: MCP_UI.SIZE_CHANGED,
            params: { width, height }
        }, this.origin);
    }
};

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => McpAppFrame.init());
} else {
    McpAppFrame.init();
}
