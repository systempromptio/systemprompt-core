const McpAppBridge = {
    parent: window.parent,
    origin: '*',
    requestId: 0,
    pendingRequests: new Map(),

    init() {
        window.addEventListener('message', (event) => this.handleMessage(event));
        this.sendNotification('ui/ready', {});
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

    updateContext(data) {
        this.sendNotification('ui/context', { data });
    }
};

document.addEventListener('DOMContentLoaded', () => McpAppBridge.init());
