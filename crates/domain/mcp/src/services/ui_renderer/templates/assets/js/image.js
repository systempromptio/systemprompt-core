const ImageApp = {
    zoom: 1,
    minZoom: 0.5,
    maxZoom: 3,
    zoomStep: 0.25,

    init() {
        this.img = document.querySelector('.artifact-image');
        this.status = document.getElementById('zoom-status');
        this.inBtn = document.querySelector('.zoom-in');
        this.outBtn = document.querySelector('.zoom-out');
        this.wrapper = this.img.closest('.image-wrapper');
        this.setupControls();
        this.setupErrorHandling();
        this.setupLoading();
        this.applyZoom();
    },

    /* The wrapper holds a skeleton until the image is actually decodable, so
     * the frame does not render empty and then jump when the bytes land. */
    setupLoading() {
        const done = () => this.wrapper?.classList.remove('is-loading', 'skeleton');
        if (this.img.complete && this.img.naturalWidth > 0) {
            done();
            return;
        }
        this.img.addEventListener('load', done);
        this.img.addEventListener('error', done);
    },

    setupControls() {
        this.inBtn.addEventListener('click', () => this.setZoom(this.zoom + this.zoomStep));
        this.outBtn.addEventListener('click', () => this.setZoom(this.zoom - this.zoomStep));
        document.querySelector('.zoom-reset').addEventListener('click', () => this.setZoom(1));
    },

    /* A blocked or missing source used to leave the browser's broken-image
     * glyph as the only explanation. */
    setupErrorHandling() {
        this.img.addEventListener('error', () => {
            const figure = this.img.closest('.image-figure');
            if (!figure) return;
            const msg = document.createElement('p');
            msg.className = 'error-message';
            msg.setAttribute('role', 'alert');
            msg.textContent = 'This image could not be loaded.';
            figure.replaceChildren(msg);
        });
    },

    setZoom(value) {
        this.zoom = Math.min(this.maxZoom, Math.max(this.minZoom, value));
        this.applyZoom();
    },

    applyZoom() {
        this.img.style.transform = `scale(${this.zoom})`;
        /* Buttons used to stay enabled at the bounds, so clicking did nothing
         * with no indication why. */
        this.inBtn.disabled = this.zoom >= this.maxZoom;
        this.outBtn.disabled = this.zoom <= this.minZoom;
        const pct = `${Math.round(this.zoom * 100)}%`;
        if (this.status) this.status.textContent = `Zoom ${pct}`;
    }
};

document.addEventListener('DOMContentLoaded', () => ImageApp.init());
