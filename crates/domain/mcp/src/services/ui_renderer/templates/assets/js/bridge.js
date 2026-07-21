// Host bridge per MCP Apps (SEP-1865). The app is the JSON-RPC client: it
// opens with a `ui/initialize` request, then issues requests such as
// `tools/call`, `resources/read`, and `ui/message` over postMessage.
const McpAppBridge = {
    parent: window.parent,
    origin: '*',
    requestId: 0,
    pendingRequests: new Map(),
    hostContext: null,

    init() {
        window.addEventListener('message', (event) => this.handleMessage(event));
        this.sendRequest('ui/initialize', {
            appInfo: { name: 'systemprompt-artifact', version: '1.0.0' },
            appCapabilities: {}
        }).then((result) => {
            this.hostContext = (result && result.hostContext) || null;
            this.sendNotification('ui/notifications/initialized', {});
        }).catch((err) => {
            console.error('ui/initialize failed:', err);
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

    // Sends a user turn to the host's chat interface.
    async sendMessage(text) {
        return this.sendRequest('ui/message', {
            role: 'user',
            content: { type: 'text', text }
        });
    },

    updateModelContext(data) {
        this.sendNotification('ui/update-model-context', { data });
    }
};

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => McpAppBridge.init());
} else {
    McpAppBridge.init();
}
