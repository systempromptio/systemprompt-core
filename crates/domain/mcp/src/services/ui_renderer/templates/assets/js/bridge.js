// Host bridge per MCP Apps (SEP-1865). The app is the JSON-RPC client.
// Method names come from the injected MCP_UI constants, generated from the
// Rust UiMethod enum — never spell them out here.
const McpAppBridge = {
    parent: window.parent,
    origin: '*',
    requestId: 0,
    pendingRequests: new Map(),
    hostContext: null,

    init() {
        window.addEventListener('message', (event) => this.handleMessage(event));
        this.sendRequest(MCP_UI.INITIALIZE, {
            appInfo: { name: 'systemprompt-artifact', version: '1.0.0' },
            appCapabilities: {},
            protocolVersion: MCP_UI.PROTOCOL_VERSION
        }).then((result) => {
            this.hostContext = (result && result.hostContext) || null;
            this.sendNotification(MCP_UI.INITIALIZED, {});
        }).catch((err) => {
            console.error('MCP Apps initialize failed:', err);
        });
    },

    handleMessage(event) {
        const { id, result, error } = event.data || {};
        if (id && this.pendingRequests.has(id)) {
            const { resolve, reject } = this.pendingRequests.get(id);
            this.pendingRequests.delete(id);
            if (error) {
                reject(new Error(error.message || 'Unknown error'));
            } else {
                resolve(result);
            }
        }
    },

    sendRequest(method, params) {
        return new Promise((resolve, reject) => {
            const id = ++this.requestId;
            this.pendingRequests.set(id, { resolve, reject });
            this.parent.postMessage({
                jsonrpc: '2.0',
                id,
                method,
                params
            }, this.origin);
        });
    },

    sendNotification(method, params) {
        this.parent.postMessage({
            jsonrpc: '2.0',
            method,
            params
        }, this.origin);
    },

    async callTool(name, args) {
        return this.sendRequest('tools/call', { name, arguments: args });
    },

    async readResource(uri) {
        return this.sendRequest('resources/read', { uri });
    },

    // `content` is an array of content blocks, not a bare block.
    async sendMessage(text) {
        return this.sendRequest(MCP_UI.MESSAGE, {
            role: 'user',
            content: [{ type: 'text', text }]
        });
    },

    updateModelContext(data) {
        this.sendNotification(MCP_UI.UPDATE_MODEL_CONTEXT, { data });
    }
};

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => McpAppBridge.init());
} else {
    McpAppBridge.init();
}
