const ImageApp = {
    zoom: 1,
    minZoom: 0.5,
    maxZoom: 3,
    zoomStep: 0.25,

    init() {
        this.img = document.querySelector('.artifact-image');
        this.setupControls();
    },

    setupControls() {
        document.querySelector('.zoom-in').addEventListener('click', () => this.zoomIn());
        document.querySelector('.zoom-out').addEventListener('click', () => this.zoomOut());
        document.querySelector('.zoom-reset').addEventListener('click', () => this.resetZoom());
    },

    zoomIn() {
        if (this.zoom < this.maxZoom) {
            this.zoom += this.zoomStep;
            this.applyZoom();
        }
    },

    zoomOut() {
        if (this.zoom > this.minZoom) {
            this.zoom -= this.zoomStep;
            this.applyZoom();
        }
    },

    resetZoom() {
        this.zoom = 1;
        this.applyZoom();
    },

    applyZoom() {
        this.img.style.transform = `scale(${this.zoom})`;
    }
};

document.addEventListener('DOMContentLoaded', () => ImageApp.init());
