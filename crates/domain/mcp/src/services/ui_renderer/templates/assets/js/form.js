const FormApp = {
    fields: window.FORM_FIELDS,
    submitTool: window.FORM_SUBMIT_TOOL,

    init() {
        document.getElementById('mcp-form').addEventListener('submit', (e) => this.handleSubmit(e));
    },

    async handleSubmit(e) {
        e.preventDefault();

        const form = e.target;
        const formData = new FormData(form);
        const data = {};

        this.fields.forEach(field => {
            if (field.type === 'checkbox') {
                data[field.name] = form.elements[field.name].checked;
            } else if (field.type === 'number') {
                const val = formData.get(field.name);
                data[field.name] = val ? Number(val) : null;
            } else {
                data[field.name] = formData.get(field.name);
            }
        });

        const messageEl = document.getElementById('form-message');

        if (this.submitTool) {
            try {
                messageEl.textContent = 'Submitting...';
                messageEl.className = 'form-message info';
                messageEl.style.display = 'block';

                const result = await McpAppBridge.callTool(this.submitTool, data);

                messageEl.textContent = 'Form submitted successfully!';
                messageEl.className = 'form-message success';
            } catch (err) {
                messageEl.textContent = 'Error: ' + err.message;
                messageEl.className = 'form-message error';
            }
        } else {
            McpAppBridge.updateContext(data);
            messageEl.textContent = 'Form data captured.';
            messageEl.className = 'form-message success';
            messageEl.style.display = 'block';
        }
    }
};

document.addEventListener('DOMContentLoaded', () => FormApp.init());
