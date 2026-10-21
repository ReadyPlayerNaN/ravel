"use strict";

// Keep one index (and one tab order): open as a sidebar on wide viewports,
// otherwise reset to a native, keyboard-operable disclosure above the article.
const contents = document.querySelector(".contents-nav > .contents");
if (contents) {
    const wideContents = window.matchMedia("(min-width: 1000px)");
    function updateContents() {
        if (!wideContents.matches && contents.contains(document.activeElement)) {
            contents.querySelector("summary").focus({preventScroll: true});
        }
        contents.open = wideContents.matches;
    }
    wideContents.addEventListener("change", updateContents);
    updateContents();
}

// This is application code, never interpolated with document text. KaTeX's trust
// mode stays disabled: TeX cannot create links, images, HTML or load resources.
let mathBudget = 1024 * 1024;
for (const node of document.querySelectorAll(".math[data-tex]")) {
    const tex = node.dataset.tex;
    const bytes = new TextEncoder().encode(tex).length;
    mathBudget -= bytes;
    try {
        if (bytes > 32768 || mathBudget < 0) {
            throw new Error("Math typesetting budget exceeded");
        }
        katex.render(tex, node, {
            displayMode: node.classList.contains("math-display"),
            output: "htmlAndMathml",
            trust: false,
            throwOnError: true,
            strict: "ignore",
            maxExpand: 1000,
            maxSize: 20
        });
    } catch (error) {
        node.textContent = tex;
        node.classList.add("math-error");
        node.title = String(error);
        const diagnostic = document.createElement("span");
        diagnostic.className = "math-diagnostic";
        diagnostic.textContent = " [Math could not be typeset; hover for details]";
        node.append(diagnostic);
    }
}

// Native checkbox controls provide keyboard and accessibility behavior. State
// lives only in this DOM and is discarded on document replacement; no saving.
function updateTask(input) {
    const item = input.closest("li");
    if (item) item.classList.toggle("task-checked", input.checked);
}
for (const input of document.querySelectorAll("input.task")) {
    updateTask(input);
    input.addEventListener("change", () => updateTask(input));
}

for (const media of document.querySelectorAll(".media img, .media audio, .media video")) {
    function failed() {
        media.parentElement.querySelector(".media-error").hidden = false;
    }
    media.addEventListener("error", failed);
    if ((media instanceof HTMLImageElement && media.complete && !media.naturalWidth) || media.error) {
        failed();
    }
}

// Focus the destination as well as scrolling, so keyboard readers can follow
// headings and footnotes and continue from the destination rather than the link.
function focusAnchor() {
    try {
        const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
        if (target) {
            target.tabIndex = -1;
            target.focus({preventScroll: true});
            target.scrollIntoView();
        }
    } catch (_) { /* Invalid percent encoding is not a navigable anchor. */ }
}
window.addEventListener("hashchange", focusAnchor);
focusAnchor();
document.documentElement.dataset.ready = "true";
