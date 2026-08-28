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
        const submitBtn = document.querySelector('#mcp-form .submit-btn');

        /* Every branch sets display itself, because the success branch used to
         * rely on the info branch having already done it. */
        const say = (text, tone) => {
            messageEl.textContent = text;
            messageEl.className = `form-message ${tone}`;
            messageEl.style.display = 'block';
        };

        if (this.submitTool) {
            /* Nothing disabled the button while the call was in flight, so a
             * second click fired the tool again. */
            if (submitBtn) {
                submitBtn.disabled = true;
                submitBtn.setAttribute('aria-busy', 'true');
            }
            try {
                say('Submitting…', 'info');
                await McpAppBridge.callTool(this.submitTool, data);
                say('Form submitted successfully.', 'success');
            } catch (err) {
                say(`Error: ${err.message}`, 'error');
            } finally {
                if (submitBtn) {
                    submitBtn.disabled = false;
                    submitBtn.removeAttribute('aria-busy');
                }
            }
        } else {
            McpAppBridge.updateModelContext(data);
            say('Form data captured.', 'success');
        }
    }
};

document.addEventListener('DOMContentLoaded', () => FormApp.init());
