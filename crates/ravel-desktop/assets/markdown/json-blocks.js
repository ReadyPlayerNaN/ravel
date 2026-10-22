"use strict";

// The native side owns parsed documents. This bridge sends only bounded slot
// geometry and focus IDs, never JSON text, filesystem paths or executable code.
window.ravelJson = (() => {
    const slots = new Map();
    const bridge = window.webkit?.messageHandlers?.ravelJson;
    let scheduled = false;
    function post(message) { bridge?.postMessage(JSON.stringify(message)); }
    function report() {
        scheduled = false;
        const layout = [];
        if (!document.fullscreenElement) {
            for (const [id, entry] of slots) {
                if (entry.slot.hidden) continue;
                const r = entry.slot.getBoundingClientRect();
                if (r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth) {
                    layout.push([id, r.x, r.y, r.width, r.height]);
                }
            }
        }
        post({layout});
    }
    function schedule() {
        if (!scheduled) { scheduled = true; requestAnimationFrame(report); }
    }
    const resize = new ResizeObserver(schedule);
    resize.observe(document.body);
    window.addEventListener("scroll", schedule, {passive: true, capture: true});
    window.addEventListener("resize", schedule);
    document.addEventListener("toggle", schedule, true);
    document.addEventListener("load", schedule, true);
    document.addEventListener("fullscreenchange", schedule);
    document.fonts.ready.then(schedule);

    function enable(heights) {
        if (!bridge || !Array.isArray(heights) || heights.length > 16) return;
        for (const block of document.querySelectorAll(".code-block[data-json-block]")) {
            const id = Number(block.dataset.jsonBlock);
            if (!Number.isInteger(id) || id < 0 || id >= heights.length || slots.has(id)) continue;
            const height = heights[id];
            if (!Number.isInteger(height) || height < 48 || height > 640) continue;
            const raw = block.querySelector("pre");
            const header = block.querySelector(".code-language");
            const slot = document.createElement("div");
            slot.className = "json-structured-slot";
            slot.style.height = `${height}px`;
            slot.tabIndex = 0;
            slot.setAttribute("role", "group");
            slot.setAttribute("aria-label", "Structured JSON. Use arrow keys to explore; Tab to leave.");
            const button = document.createElement("button");
            button.type = "button";
            button.className = "json-view-toggle";
            function update(structured) {
                raw.hidden = structured;
                slot.hidden = !structured;
                button.textContent = structured ? "Raw" : "Structured";
                button.setAttribute("aria-label", structured ? "Show raw JSON" : "Show structured JSON");
                report();
            }
            button.addEventListener("click", () => update(slot.hidden));
            slot.addEventListener("focus", () => { report(); post({focus: id}); });
            header.append(button);
            block.append(slot);
            slots.set(id, {block, raw, slot, button});
            resize.observe(slot);
            update(true);
        }
        schedule();
    }
    function setHeight(id, height) {
        const entry = slots.get(id);
        if (!entry || !Number.isInteger(height) || height < 48 || height > 640) return;
        const size = `${height}px`;
        if (entry.slot.style.height !== size) {
            entry.slot.style.height = size;
            report();
        }
    }
    function exit(id, backward) {
        const entry = slots.get(id);
        if (!entry) return;
        if (backward) { entry.button.focus(); return; }
        const stops = Array.from(document.querySelectorAll('a[href], button, input, summary, [tabindex="0"]'))
            .filter(node => node.getClientRects().length && !node.closest("[hidden]"));
        const index = stops.indexOf(entry.slot);
        const next = index >= 0 ? stops[index + 1] : null;
        if (next) { next.focus(); } else {
            const after = entry.block.nextElementSibling || document.body;
            after.tabIndex = -1;
            after.focus();
        }
    }
    return {enable, exit, setHeight};
})();
