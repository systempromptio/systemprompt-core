const McpAppFrame = {
    parent: window.parent,
    origin: '*',
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
        return Math.ceil(Math.max(
            doc.scrollHeight,
            doc.offsetHeight,
            body ? body.scrollHeight : 0,
            body ? body.offsetHeight : 0
        ));
    },

    publish() {
        const height = this.measure();
        if (height === this.lastHeight || height === 0) {
            return;
        }
        this.lastHeight = height;

        this.parent.postMessage({
            jsonrpc: '2.0',
            method: 'ui/notifications/size-changed',
            params: { height }
        }, this.origin);

        this.parent.postMessage({
            type: 'ui-size-change',
            payload: { height }
        }, this.origin);
    }
};

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => McpAppFrame.init());
} else {
    McpAppFrame.init();
}
