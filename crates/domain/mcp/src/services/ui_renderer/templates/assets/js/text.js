/* Clipboard access can be refused outright inside a sandboxed, opaque-origin
 * iframe, which is exactly where this runs. The failure used to go only to the
 * console, so the viewer clicked Copy and nothing happened and nothing said
 * why. Both outcomes are now announced. */
const copyBtn = document.getElementById('copy-btn');
const copyStatus = document.getElementById('copy-status');

copyBtn.addEventListener('click', async () => {
    const content = document.getElementById('text-content').innerText;
    const icon = copyBtn.querySelector('.copy-icon');

    try {
        await navigator.clipboard.writeText(content);
        if (icon) icon.textContent = '✓';
        copyStatus.textContent = 'Copied to clipboard.';
        copyStatus.dataset.tone = 'success';
        setTimeout(() => {
            if (icon) icon.textContent = '📋';
            copyStatus.textContent = '';
            delete copyStatus.dataset.tone;
        }, 2000);
    } catch (err) {
        console.error('Failed to copy:', err);
        copyStatus.textContent = 'Copy was blocked. Select the text and copy manually.';
        copyStatus.dataset.tone = 'error';
    }
});
