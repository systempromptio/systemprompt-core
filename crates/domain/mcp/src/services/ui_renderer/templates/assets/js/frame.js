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
        window.addEventListener('message', (event) => this.onMessage(event));
        this.schedule();
    },

    // The shell relays the host's theme down (SEP-1865 `hostContext`). Every
    // --mcpui-* token is a light-dark() pair, and light-dark() resolves off the
    // computed color-scheme — so stamping it here is what makes the artifact
    // follow the HOST rather than the viewer's OS. Absent this message the
    // stylesheet's `color-scheme: light dark` still applies, which is the
    // correct fallback for a host that says nothing.
    onMessage(event) {
        if (event.source !== this.parent) {
            return;
        }
        const data = event.data || {};
        if (data.method !== MCP_UI.HOST_CONTEXT_CHANGED) {
            return;
        }
        const context = (data.params && data.params.hostContext) || data.params || {};
        if (context.theme === 'light' || context.theme === 'dark') {
            document.documentElement.style.colorScheme = context.theme;
        }
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
