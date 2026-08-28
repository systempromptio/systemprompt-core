const CARD_CTA_BY_ID = new Map((window.CARD_CTAS || []).map((cta) => [cta.id, cta]));

/* A CTA used to post its message and return, changing nothing: no pending
 * state, no confirmation, and a failure that went only to the console. The
 * viewer could not tell a working button from a broken one, so every click
 * now resolves visibly and is announced. */
const status = document.getElementById('card-cta-status');

function announce(text, tone) {
    if (!status) return;
    status.textContent = text;
    status.dataset.tone = tone;
}

for (const button of document.querySelectorAll('.card-cta')) {
    button.addEventListener('click', async () => {
        const cta = CARD_CTA_BY_ID.get(button.dataset.ctaId);
        if (!cta || button.disabled) {
            return;
        }

        const label = button.textContent;
        button.disabled = true;
        button.setAttribute('aria-busy', 'true');
        button.textContent = 'Sending…';
        announce('Sending…', 'pending');

        try {
            await McpAppBridge.sendMessage(cta.message);
            button.textContent = 'Sent';
            announce(`${label} sent.`, 'success');
        } catch (err) {
            console.error('Sending message to host failed:', err);
            button.textContent = label;
            button.disabled = false;
            announce(`${label} could not be sent. Try again.`, 'error');
        } finally {
            button.removeAttribute('aria-busy');
        }
    });
}
