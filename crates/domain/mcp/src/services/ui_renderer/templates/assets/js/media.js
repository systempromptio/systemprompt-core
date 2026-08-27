/* Autoplay was passed straight through from the payload. A video that starts
 * by itself is motion the viewer did not ask for, and CSS cannot stop it — a
 * media query can pause an animation, not a <video>. */
(() => {
    if (!window.matchMedia || !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        return;
    }
    for (const el of document.querySelectorAll('video.media-element[autoplay], audio.media-element[autoplay]')) {
        el.autoplay = false;
        el.removeAttribute('autoplay');
        el.pause();
        /* Without controls the viewer would now have no way to start it. */
        el.controls = true;
    }
})();
