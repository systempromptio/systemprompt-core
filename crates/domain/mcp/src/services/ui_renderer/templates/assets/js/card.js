const CARD_CTA_BY_ID = new Map((window.CARD_CTAS || []).map((cta) => [cta.id, cta]));

for (const button of document.querySelectorAll('.card-cta')) {
    button.addEventListener('click', () => {
        const cta = CARD_CTA_BY_ID.get(button.dataset.ctaId);
        if (!cta) {
            return;
        }
        McpAppBridge.sendMessage(cta.message).catch((err) => {
            console.error('ui/message failed:', err);
        });
    });
}
