const mermaidScript = document.createElement("script");
mermaidScript.text = window.__MDVIEW_MERMAID_SOURCE;
document.head.appendChild(mermaidScript);

const mermaid = globalThis.mermaid;
mermaid.initialize({ startOnLoad: false, securityLevel: "loose" });

const config = window.__MDGLANCE_CONFIG;
const initialState = window.__MDGLANCE_INITIAL_STATE;

const content = document.getElementById("content");
const presentationRoot = document.getElementById("presentation-root");
const tocPanel = document.getElementById("toc-panel");
const tocNav = document.getElementById("toc-nav");
const tocEmpty = document.getElementById("toc-empty");
const searchBar = document.getElementById("search-bar");
const searchInput = document.getElementById("search-input");
const searchStatus = document.getElementById("search-status");
const helpOverlay = document.getElementById("help-overlay");
const helpList = document.getElementById("help-list");
const linkHintsLayer = document.createElement("div");
linkHintsLayer.className = "link-hints hidden";
document.body.append(linkHintsLayer);
const PRESENTATION_STAGE_WIDTH = 1600;
const PRESENTATION_STAGE_HEIGHT = 900;

const state = {
  documentKind: initialState.document_kind,
  focusMode: "document",
  tocVisible: config.toc.visible_on_start,
  tocItems: [],
  tocIndex: new Map(),
  headingNodes: [],
  activeHeadingId: null,
  activeTocId: null,
  tocSelectionId: null,
  searchQuery: "",
  searchHits: [],
  currentHit: -1,
  linkHintMode: false,
  linkHintQuery: "",
  linkHints: [],
  presentation: {
    enabled: Boolean(initialState.presentation?.enabled),
    mode: initialState.presentation?.default_mode ?? "markdown",
    header: initialState.presentation?.header ?? null,
    footer: initialState.presentation?.footer ?? null,
    pageNumbers: Boolean(initialState.presentation?.page_numbers),
    slides: initialState.presentation?.slides ?? [],
    currentSlide: 0,
  },
  svg: {
    viewport: null,
    stage: null,
    root: null,
    baseWidth: 0,
    baseHeight: 0,
    scale: 1,
    minScale: 0.1,
    maxScale: 24,
    offsetX: 0,
    offsetY: 0,
  },
};

const ACTIONS = [
  { id: "scroll_down", description: "Scroll down" },
  { id: "scroll_up", description: "Scroll up" },
  { id: "scroll_left", description: "Scroll left" },
  { id: "scroll_right", description: "Scroll right" },
  { id: "half_page_down", description: "Half page down" },
  { id: "half_page_up", description: "Half page up" },
  { id: "page_down", description: "Page down" },
  { id: "top", description: "Top" },
  { id: "bottom", description: "Bottom" },
  { id: "open_search", description: "Open search" },
  { id: "accept_search", description: "Accept search" },
  { id: "next_search_hit", description: "Next search hit" },
  { id: "previous_search_hit", description: "Previous search hit" },
  { id: "show_help", description: "Show help" },
  { id: "close_overlay", description: "Close search or help" },
  { id: "toggle_toc", description: "Show or hide table of contents" },
  { id: "toggle_focus", description: "Switch focus between document and table of contents" },
  { id: "toggle_presentation", description: "Toggle presentation mode" },
  { id: "back", description: "Back in history or previous slide" },
  { id: "forward", description: "Forward in history or next slide" },
  { id: "previous_file", description: "Previous file in viewer queue" },
  { id: "next_file", description: "Next file in viewer queue" },
  { id: "open_link_hints", description: "Open keyboard link hints" },
  { id: "toc_down", description: "TOC: next heading" },
  { id: "toc_up", description: "TOC: previous heading" },
  { id: "activate_selection", description: "TOC: jump to selected heading" },
  { id: "zoom_in", description: "SVG: zoom in" },
  { id: "zoom_out", description: "SVG: zoom out" },
  { id: "reset_view", description: "SVG: reset view" },
  { id: "quit", description: "Quit" },
];

const keybindings = new Map(config.keybindings.map((binding) => [binding.action, binding]));
const orderedBindings = config.keybindings.flatMap((binding) =>
  binding.shortcuts.map((shortcut) => ({ action: binding.action, shortcut })),
);

const GLOBAL_ACTIONS = new Set(["quit", "show_help", "toggle_toc", "close_overlay"]);
const SEARCH_ACTIONS = new Set(["accept_search", "close_overlay", "quit"]);
const DOCUMENT_ACTIONS = new Set([
  "scroll_down",
  "scroll_up",
  "half_page_down",
  "half_page_up",
  "page_down",
  "top",
  "bottom",
  "open_search",
  "next_search_hit",
  "previous_search_hit",
  "toggle_focus",
  "toggle_presentation",
  "back",
  "forward",
  "previous_file",
  "next_file",
  "open_link_hints",
]);
const PRESENTATION_ACTIONS = new Set([
  "scroll_down",
  "scroll_up",
  "half_page_down",
  "half_page_up",
  "page_down",
  "top",
  "bottom",
  "toggle_presentation",
  "back",
  "forward",
  "previous_file",
  "next_file",
]);
const TOC_ACTIONS = new Set([
  "toc_down",
  "toc_up",
  "activate_selection",
  "toggle_focus",
  "previous_file",
  "next_file",
]);
const HELP_ACTIONS = new Set([
  "show_help",
  "close_overlay",
  "quit",
  "scroll_down",
  "scroll_up",
  "half_page_down",
  "half_page_up",
  "page_down",
  "top",
  "bottom",
]);
const SVG_ACTIONS = new Set([
  "scroll_down",
  "scroll_up",
  "scroll_left",
  "scroll_right",
  "previous_file",
  "next_file",
  "zoom_in",
  "zoom_out",
  "reset_view",
]);

function allowedActionsFor() {
  const allowed = new Set(GLOBAL_ACTIONS);
  const source =
    state.documentKind === "svg"
      ? SVG_ACTIONS
      : state.presentation.enabled && state.presentation.mode === "presentation"
        ? PRESENTATION_ACTIONS
      : state.focusMode === "toc"
        ? TOC_ACTIONS
        : DOCUMENT_ACTIONS;
  for (const action of source) {
    allowed.add(action);
  }
  return allowed;
}

async function renderMermaidIn(root) {
  if (!root) {
    return;
  }
  const nodes = root.querySelectorAll("pre.mermaid");
  if (nodes.length === 0) {
    return;
  }
  for (const node of nodes) {
    node.removeAttribute("data-processed");
  }
  await mermaid.run({ nodes });
}

window.__mdglanceUpdate = async function (payload) {
  const scrollRatio = currentScrollRatio();
  const previousSelection = state.tocSelectionId;
  const wasPresentationEnabled = state.presentation.enabled;
  const previousMode = state.presentation.mode;
  const previousSlide = state.presentation.currentSlide;

  state.documentKind = payload.document_kind;
  state.presentation.enabled = Boolean(payload.presentation?.enabled);
  state.presentation.header = payload.presentation?.header ?? null;
  state.presentation.footer = payload.presentation?.footer ?? null;
  state.presentation.pageNumbers = Boolean(payload.presentation?.page_numbers);
  state.presentation.slides = payload.presentation?.slides ?? [];
  state.presentation.mode =
    state.presentation.enabled && wasPresentationEnabled
      ? previousMode
      : state.presentation.enabled
        ? payload.presentation?.default_mode ?? "presentation"
        : "markdown";
  document.title = "mdglance - " + payload.title;
  content.innerHTML = payload.body;
  renderPresentationSlides();
  setTocItems(payload.toc);
  if (state.documentKind === "markdown") {
    if (state.presentation.enabled) {
      state.presentation.currentSlide = wasPresentationEnabled
        ? Math.min(previousSlide, Math.max(0, state.presentation.slides.length - 1))
        : 0;
      syncPresentationMode();
      if (state.presentation.mode === "presentation") {
        await preparePresentationSlide(state.presentation.currentSlide);
      } else {
        await renderMermaidIn(content);
      }
    } else {
      await renderMermaidIn(content);
      state.presentation.currentSlide = 0;
      state.presentation.mode = "markdown";
      syncPresentationMode();
    }
  } else {
    closeLinkHints();
    closeSearch(true);
    state.focusMode = "document";
    state.tocVisible = false;
    bindSvgViewer();
    state.presentation.enabled = false;
    state.presentation.mode = "markdown";
  }

  if (
    state.documentKind === "markdown" &&
    state.presentation.mode !== "presentation" &&
    state.searchQuery
  ) {
    highlightSearch(state.searchQuery);
  }

  if (state.documentKind === "markdown") {
    if (state.presentation.mode !== "presentation") {
      window.scrollTo(
        0,
        scrollRatio * Math.max(1, document.body.scrollHeight - window.innerHeight),
      );

      refreshHeadings();
      syncActiveHeading();

      if (state.focusMode === "toc" && previousSelection && state.tocIndex.has(previousSelection)) {
        state.tocSelectionId = previousSelection;
      } else if (state.focusMode === "toc") {
        alignTocSelectionToActive();
      }

      if (payload.anchor) {
        jumpToAnchor(payload.anchor);
      } else if (typeof payload.scroll_ratio === "number") {
        window.scrollTo(
          0,
          payload.scroll_ratio * Math.max(1, document.body.scrollHeight - window.innerHeight),
        );
      }
    } else {
      refreshHeadings();
      syncActiveHeading();
      showPresentationSlide(state.presentation.currentSlide);
    }
  } else {
    resetSvgView();
  }

  updateTocState();
};

window.__mdglanceShowError = function (message) {
  state.documentKind = "markdown";
  state.presentation.enabled = false;
  state.presentation.mode = "markdown";
  state.presentation.header = null;
  state.presentation.footer = null;
  state.presentation.pageNumbers = false;
  state.presentation.slides = [];
  presentationRoot.replaceChildren();
  content.innerHTML = `<pre class="error">${message}</pre>`;
  resetSvgBindings();
  syncPresentationMode();
};

window.__mdglanceJumpToAnchor = function (payload) {
  if (payload?.anchor) {
    jumpToAnchor(payload.anchor);
  }
};

function normalizeEventKey(event) {
  if (event.key.length === 1 && /[a-z]/i.test(event.key)) {
    return event.key.toLowerCase();
  }
  return event.key;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function matchesShortcut(shortcut, event) {
  if (shortcut.meta !== event.metaKey) {
    return false;
  }
  if (shortcut.ctrl !== event.ctrlKey) {
    return false;
  }
  if (shortcut.alt !== event.altKey) {
    return false;
  }
  if (shortcut.shift !== null && shortcut.shift !== event.shiftKey) {
    return false;
  }
  return shortcut.key === normalizeEventKey(event);
}

function findAction(event, allowedActions = null) {
  for (const binding of orderedBindings) {
    if (allowedActions && !allowedActions.has(binding.action)) {
      continue;
    }
    if (matchesShortcut(binding.shortcut, event)) {
      return binding.action;
    }
  }
  return null;
}

function renderHelp() {
  const rows = [];

  for (const action of ACTIONS) {
    const binding = keybindings.get(action.id);
    if (!binding || binding.keys.length === 0) {
      continue;
    }

    const term = document.createElement("dt");
    term.textContent = binding.keys.join(" / ");
    rows.push(term);

    const description = document.createElement("dd");
    description.textContent = action.description;
    rows.push(description);
  }

  helpList.replaceChildren(...rows);
}

function isTypingTarget(element) {
  return (
    element &&
    (element.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName))
  );
}

function clearSearchHighlights() {
  for (const mark of Array.from(content.querySelectorAll("mark.mdglance-search-hit"))) {
    mark.replaceWith(document.createTextNode(mark.textContent));
  }
  content.normalize();
  state.searchHits = [];
  state.currentHit = -1;
}

function setSearchStatus() {
  if (!state.searchQuery) {
    searchStatus.textContent = "";
  } else if (state.searchHits.length === 0) {
    searchStatus.textContent = "0";
  } else {
    searchStatus.textContent = `${state.currentHit + 1}/${state.searchHits.length}`;
  }
}

function setCurrentHit(index) {
  if (state.searchHits.length === 0) {
    state.currentHit = -1;
    setSearchStatus();
    return;
  }

  state.currentHit = (index + state.searchHits.length) % state.searchHits.length;
  for (const hit of state.searchHits) {
    hit.classList.remove("current");
  }
  state.searchHits[state.currentHit].classList.add("current");
  state.searchHits[state.currentHit].scrollIntoView({ block: "center", inline: "nearest" });
  setSearchStatus();
}

function highlightSearch(query) {
  clearSearchHighlights();
  state.searchQuery = query;

  if (!query) {
    setSearchStatus();
    return;
  }

  const needle = query.toLocaleLowerCase();
  const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent || parent.closest("script, style, mark.mdglance-search-hit")) {
        return NodeFilter.FILTER_REJECT;
      }
      return node.nodeValue.toLocaleLowerCase().includes(needle)
        ? NodeFilter.FILTER_ACCEPT
        : NodeFilter.FILTER_REJECT;
    },
  });

  const nodes = [];
  while (walker.nextNode()) {
    nodes.push(walker.currentNode);
  }

  for (const node of nodes) {
    const text = node.nodeValue;
    const lower = text.toLocaleLowerCase();
    const fragment = document.createDocumentFragment();
    let cursor = 0;
    let index = lower.indexOf(needle);

    while (index !== -1) {
      if (index > cursor) {
        fragment.append(document.createTextNode(text.slice(cursor, index)));
      }

      const mark = document.createElement("mark");
      mark.className = "mdglance-search-hit";
      mark.textContent = text.slice(index, index + query.length);
      fragment.append(mark);
      state.searchHits.push(mark);

      cursor = index + query.length;
      index = lower.indexOf(needle, cursor);
    }

    if (cursor < text.length) {
      fragment.append(document.createTextNode(text.slice(cursor)));
    }

    node.replaceWith(fragment);
  }

  setCurrentHit(0);
}

function openSearch() {
  searchBar.classList.remove("hidden");
  searchInput.value = state.searchQuery;
  searchInput.focus();
  searchInput.select();
  setSearchStatus();
}

function closeSearch(clear) {
  searchBar.classList.add("hidden");
  searchInput.blur();
  if (clear) {
    state.searchQuery = "";
    clearSearchHighlights();
    setSearchStatus();
  }
}

function toggleHelp(show) {
  helpOverlay.classList.toggle("hidden", !show);
}

function sendIpc(message) {
  window.ipc.postMessage(JSON.stringify(message));
}

function currentScrollRatio() {
  if (state.documentKind === "svg" || state.presentation.mode === "presentation") {
    return 0;
  }
  return window.scrollY / Math.max(1, document.body.scrollHeight - window.innerHeight);
}

function jumpToAnchor(anchor) {
  const target = document.getElementById(anchor);
  if (target) {
    target.scrollIntoView({ block: "start", behavior: "instant" });
  }
}

function setTocItems(items) {
  if (state.documentKind === "svg" || state.presentation.mode === "presentation") {
    state.tocItems = [];
    state.tocIndex = new Map();
    state.activeHeadingId = null;
    state.activeTocId = null;
    state.tocSelectionId = null;
    renderToc();
    return;
  }

  const normalized = items.map((item) => ({
    ...item,
    parentId: null,
    children: [],
  }));
  const stack = [];

  for (const item of normalized) {
    while (stack.length > 0 && stack.at(-1).level >= item.level) {
      stack.pop();
    }

    if (stack.length > 0) {
      const parent = stack.at(-1);
      item.parentId = parent.id;
      parent.children.push(item.id);
    }

    stack.push(item);
  }

  state.tocItems = normalized;
  state.tocIndex = new Map(normalized.map((item, index) => [item.id, { item, index }]));

  if (!state.tocIndex.has(state.activeHeadingId)) {
    state.activeHeadingId = normalized[0]?.id ?? null;
  }
  if (!state.tocIndex.has(state.activeTocId)) {
    state.activeTocId = normalized[0]?.id ?? null;
  }
  if (!state.tocIndex.has(state.tocSelectionId)) {
    state.tocSelectionId = state.activeTocId ?? normalized[0]?.id ?? null;
  }

  renderToc();
}

function renderToc() {
  if (state.tocItems.length === 0) {
    tocNav.replaceChildren();
    tocEmpty.classList.remove("hidden");
    updateTocState();
    return;
  }

  tocEmpty.classList.add("hidden");
  const rows = state.tocItems.map((item) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "toc-item";
    button.dataset.id = item.id;
    button.dataset.level = String(item.level);
    button.style.setProperty("--toc-level", item.level);
    button.title = item.title;

    const level = document.createElement("span");
    level.className = "toc-level";
    level.textContent = `H${item.level}`;

    const label = document.createElement("span");
    label.className = "toc-label";
    label.textContent = item.title;

    button.append(level, label);
    button.addEventListener("click", () => {
      state.tocSelectionId = item.id;
      jumpToHeading(item.id, true);
    });
    return button;
  });

  tocNav.replaceChildren(...rows);
  updateTocState();
}

function updateTocState() {
  const tocVisible =
    state.documentKind === "markdown" &&
    state.presentation.mode !== "presentation" &&
    state.tocVisible;
  document.body.classList.toggle("toc-visible", tocVisible);
  document.body.classList.toggle("toc-focus", state.focusMode === "toc");
  document.body.classList.toggle("document-focus", state.focusMode === "document");
  document.body.classList.toggle("link-hint-mode", state.linkHintMode);
  document.body.classList.toggle("svg-mode", state.documentKind === "svg");
  document.body.classList.toggle("presentation-mode", state.presentation.mode === "presentation");

  for (const row of tocNav.querySelectorAll(".toc-item")) {
    const id = row.dataset.id;
    const isActive = id === state.activeTocId;
    const isSelected = id === state.tocSelectionId;
    row.classList.toggle("is-active", isActive);
    row.classList.toggle("is-selected", isSelected);
    row.setAttribute("aria-current", isActive ? "location" : "false");
    row.setAttribute("aria-selected", isSelected ? "true" : "false");
  }
}

function refreshHeadings() {
  if (state.documentKind !== "markdown" || state.presentation.mode === "presentation") {
    state.headingNodes = [];
    return;
  }

  state.headingNodes = Array.from(content.querySelectorAll("[data-mdglance-heading]")).map(
    (node) => ({
      id: node.id,
      level: Number.parseInt(node.dataset.level ?? node.tagName.slice(1), 10),
      node,
    }),
  );
}

function syncActiveHeading() {
  if (state.documentKind !== "markdown" || state.presentation.mode === "presentation") {
    state.activeHeadingId = null;
    state.activeTocId = null;
    updateTocState();
    return;
  }

  if (state.headingNodes.length === 0) {
    state.activeHeadingId = null;
    state.activeTocId = null;
    updateTocState();
    return;
  }

  const threshold = Math.max(72, Math.round(window.innerHeight * 0.18));
  let active = state.headingNodes[0];

  for (const heading of state.headingNodes) {
    if (heading.node.getBoundingClientRect().top - threshold <= 0) {
      active = heading;
    } else {
      break;
    }
  }

  state.activeHeadingId = active.id;
  state.activeTocId = resolveActiveTocId(active.id);

  if (state.focusMode === "document") {
    state.tocSelectionId = state.activeTocId;
  }

  updateTocState();
}

function resolveActiveTocId(activeHeadingId) {
  for (let index = state.headingNodes.length - 1; index >= 0; index -= 1) {
    const heading = state.headingNodes[index];
    if (heading.id === activeHeadingId) {
      for (let candidateIndex = index; candidateIndex >= 0; candidateIndex -= 1) {
        const candidate = state.headingNodes[candidateIndex];
        if (state.tocIndex.has(candidate.id)) {
          return candidate.id;
        }
      }
      break;
    }
  }
  return state.tocItems[0]?.id ?? null;
}

function alignTocSelectionToActive() {
  if (state.tocIndex.has(state.activeTocId)) {
    state.tocSelectionId = state.activeTocId;
  } else {
    state.tocSelectionId = state.tocItems[0]?.id ?? null;
  }
}

function ensureSelectedRowVisible() {
  const selected = tocNav.querySelector(".toc-item.is-selected");
  if (selected) {
    selected.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
}

function switchFocus(nextMode) {
  if (state.documentKind !== "markdown" || state.presentation.mode === "presentation") {
    return false;
  }

  if (nextMode === "toc") {
    if (state.tocItems.length === 0) {
      return false;
    }
    state.tocVisible = true;
    alignTocSelectionToActive();
    state.focusMode = "toc";
    tocPanel.focus();
    updateTocState();
    ensureSelectedRowVisible();
    return true;
  }

  state.focusMode = "document";
  content.focus();
  updateTocState();
  return true;
}

function toggleTocVisibility() {
  if (state.documentKind !== "markdown" || state.presentation.mode === "presentation") {
    return false;
  }

  if (state.tocVisible) {
    state.tocVisible = false;
    if (state.focusMode === "toc") {
      switchFocus("document");
    } else {
      updateTocState();
    }
  } else {
    state.tocVisible = true;
    updateTocState();
  }
  return true;
}

function moveTocSelection(offset) {
  if (!state.tocIndex.has(state.tocSelectionId)) {
    alignTocSelectionToActive();
  }

  const current = state.tocIndex.get(state.tocSelectionId);
  if (!current) {
    return false;
  }

  const next = state.tocItems[current.index + offset];
  if (!next) {
    return false;
  }

  state.tocSelectionId = next.id;
  updateTocState();
  ensureSelectedRowVisible();
  return true;
}

function jumpToHeading(id, switchBackToDocument) {
  if (state.documentKind !== "markdown" || state.presentation.mode === "presentation") {
    return false;
  }

  const target = document.getElementById(id);
  if (!target) {
    return false;
  }

  target.scrollIntoView({ block: "start", behavior: "instant" });
  state.activeHeadingId = id;

  if (switchBackToDocument) {
    switchFocus("document");
  } else {
    updateTocState();
  }

  return true;
}

function renderPresentationSlides() {
  if (!state.presentation.enabled) {
    presentationRoot.replaceChildren();
    return;
  }

  const totalSlides = state.presentation.slides.length;
  const header = state.presentation.header ? escapeHtml(state.presentation.header) : "";
  const footer = state.presentation.footer ? escapeHtml(state.presentation.footer) : "";
  const slides = state.presentation.slides.map((slide) => {
    const section = document.createElement("section");
    section.className = "presentation-slide";
    section.dataset.slideIndex = String(slide.index);
    section.innerHTML = `
      <div class="presentation-shell">
        <div class="presentation-stage-frame">
          <article class="presentation-stage">
            <header class="presentation-chrome presentation-header${state.presentation.header ? "" : " hidden"}">
              ${header}
            </header>
            <div class="presentation-body-viewport">
              <div class="presentation-body-scale">
                <div class="presentation-slide-body">${slide.body}</div>
              </div>
            </div>
            <footer class="presentation-chrome presentation-footer${
              state.presentation.footer || state.presentation.pageNumbers ? "" : " hidden"
            }">
              <span class="presentation-footer-text${state.presentation.footer ? "" : " hidden"}">${footer}</span>
              <span class="presentation-page-number${state.presentation.pageNumbers ? "" : " hidden"}">${slide.index + 1} / ${totalSlides}</span>
            </footer>
          </article>
        </div>
      </div>`;
    return section;
  });

  presentationRoot.replaceChildren(...slides);
}

function syncPresentationMode() {
  const isPresentation = state.presentation.enabled && state.presentation.mode === "presentation";
  content.classList.toggle("hidden", isPresentation);
  presentationRoot.classList.toggle("hidden", !isPresentation);
  if (isPresentation) {
    state.focusMode = "document";
    state.tocVisible = false;
    showPresentationSlide(state.presentation.currentSlide);
    presentationRoot.focus();
  } else {
    presentationRoot.classList.add("hidden");
    content.classList.remove("hidden");
    content.focus();
  }
  updateTocState();
}

function showPresentationSlide(index) {
  if (!state.presentation.enabled) {
    return false;
  }

  const maxIndex = Math.max(0, state.presentation.slides.length - 1);
  state.presentation.currentSlide = Math.max(0, Math.min(index, maxIndex));

  for (const slide of presentationRoot.querySelectorAll(".presentation-slide")) {
    slide.classList.toggle(
      "is-active",
      Number.parseInt(slide.dataset.slideIndex ?? "-1", 10) === state.presentation.currentSlide,
    );
  }

  void preparePresentationSlide(state.presentation.currentSlide);
  return true;
}

function fitPresentationStage(slide) {
  const shell = slide.querySelector(".presentation-shell");
  const frame = slide.querySelector(".presentation-stage-frame");
  const stage = slide.querySelector(".presentation-stage");
  const viewport = slide.querySelector(".presentation-body-viewport");
  const scaleBox = slide.querySelector(".presentation-body-scale");
  const body = slide.querySelector(".presentation-slide-body");
  if (!shell || !frame || !stage || !viewport || !scaleBox || !body) {
    return;
  }

  const availableWidth = Math.max(1, shell.clientWidth);
  const availableHeight = Math.max(1, shell.clientHeight);
  const stageScale = Math.min(
    availableWidth / PRESENTATION_STAGE_WIDTH,
    availableHeight / PRESENTATION_STAGE_HEIGHT,
  );
  frame.style.width = `${PRESENTATION_STAGE_WIDTH * stageScale}px`;
  frame.style.height = `${PRESENTATION_STAGE_HEIGHT * stageScale}px`;
  stage.style.setProperty("--presentation-stage-scale", String(stageScale));

  body.style.width = `${viewport.clientWidth}px`;
  body.style.transform = "scale(1)";
  scaleBox.style.width = `${viewport.clientWidth}px`;
  scaleBox.style.height = "auto";

  const naturalWidth = Math.max(viewport.clientWidth, body.scrollWidth);
  const naturalHeight = Math.max(viewport.clientHeight, body.scrollHeight);
  const contentScale = Math.min(1, viewport.clientWidth / naturalWidth, viewport.clientHeight / naturalHeight);

  body.style.transform = `scale(${contentScale})`;
  scaleBox.style.width = `${naturalWidth * contentScale}px`;
  scaleBox.style.height = `${naturalHeight * contentScale}px`;
}

async function preparePresentationSlide(index) {
  const slide = presentationRoot.querySelector(
    `.presentation-slide[data-slide-index="${index}"]`,
  );
  if (!slide) {
    return;
  }

  for (const image of slide.querySelectorAll("img")) {
    if (image.dataset.mdglanceFitBound === "true") {
      continue;
    }
    image.dataset.mdglanceFitBound = "true";
    image.addEventListener("load", () => fitPresentationStage(slide), { once: true });
    image.addEventListener("error", () => fitPresentationStage(slide), { once: true });
  }

  if (slide.dataset.mdglanceMermaidReady !== "true") {
    await renderMermaidIn(slide);
    slide.dataset.mdglanceMermaidReady = "true";
  }
  fitPresentationStage(slide);
}

function activePresentationSlideFromDocument() {
  const activeHeading = state.activeHeadingId
    ? document.getElementById(state.activeHeadingId)
    : null;
  const fromHeading = activeHeading?.closest("[data-presentation-slide]");
  if (fromHeading) {
    return Number.parseInt(fromHeading.dataset.presentationSlide ?? "0", 10);
  }

  const slides = Array.from(content.querySelectorAll("[data-presentation-slide]"));
  if (slides.length === 0) {
    return 0;
  }

  const threshold = Math.max(72, Math.round(window.innerHeight * 0.18));
  let current = slides[0];
  for (const slide of slides) {
    if (slide.getBoundingClientRect().top - threshold <= 0) {
      current = slide;
    } else {
      break;
    }
  }

  return Number.parseInt(current.dataset.presentationSlide ?? "0", 10);
}

function jumpDocumentToPresentationSlide(index) {
  const target = content.querySelector(`[data-presentation-slide="${index}"]`);
  if (!target) {
    return false;
  }

  target.scrollIntoView({ block: "start", behavior: "instant" });
  refreshHeadings();
  syncActiveHeading();
  return true;
}

function togglePresentationMode() {
  if (state.documentKind !== "markdown" || !state.presentation.enabled) {
    return false;
  }

  closeLinkHints();
  closeSearch(true);

  if (state.presentation.mode === "presentation") {
    state.presentation.mode = "markdown";
    syncPresentationMode();
    void renderMermaidIn(content);
    jumpDocumentToPresentationSlide(state.presentation.currentSlide);
    return true;
  }

  refreshHeadings();
  syncActiveHeading();
  state.presentation.currentSlide = activePresentationSlideFromDocument();
  state.presentation.mode = "presentation";
  syncPresentationMode();
  return true;
}

function isPreviewableHref(href) {
  return href === "#" || href.endsWith(".md") || href.includes(".md#");
}

function isExternalHref(href) {
  return href.startsWith("http://") || href.startsWith("https://");
}

function activateLinkElement(link) {
  const href = link.getAttribute("href");
  if (!href) {
    return false;
  }

  if (href.startsWith("#")) {
    jumpToAnchor(href.slice(1));
    return true;
  }

  if (isPreviewableHref(href)) {
    sendIpc({ type: "open_markdown", href, scroll_ratio: currentScrollRatio() });
    return true;
  }

  if (isExternalHref(href)) {
    sendIpc({ type: "open_external", href });
    return true;
  }

  return false;
}

function hintLabel(index) {
  const alphabet = "asdfghjklqwertyuiopzxcvbnm";
  let value = index;
  let label = "";
  do {
    label = alphabet[value % alphabet.length] + label;
    value = Math.floor(value / alphabet.length) - 1;
  } while (value >= 0);
  return label;
}

function visibleDocumentLinks() {
  return Array.from(content.querySelectorAll("a[href]")).filter((link) => {
    const rect = link.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0 && rect.bottom >= 0 && rect.top <= window.innerHeight;
  });
}

function closeLinkHints() {
  state.linkHintMode = false;
  state.linkHintQuery = "";
  state.linkHints = [];
  linkHintsLayer.replaceChildren();
  linkHintsLayer.classList.add("hidden");
  updateTocState();
}

function openLinkHints() {
  const links = visibleDocumentLinks();
  if (links.length === 0) {
    return false;
  }

  state.linkHintMode = true;
  state.linkHintQuery = "";
  state.linkHints = links.map((link, index) => {
    const rect = link.getBoundingClientRect();
    const hint = document.createElement("button");
    hint.type = "button";
    hint.className = "link-hint";
    hint.textContent = hintLabel(index);
    hint.style.left = `${Math.max(8, rect.left + window.scrollX)}px`;
    hint.style.top = `${Math.max(8, rect.top + window.scrollY)}px`;
    hint.addEventListener("click", () => {
      activateLinkElement(link);
      closeLinkHints();
    });
    return {
      label: hint.textContent,
      link,
      hint,
    };
  });

  linkHintsLayer.replaceChildren(...state.linkHints.map((entry) => entry.hint));
  linkHintsLayer.classList.remove("hidden");
  updateTocState();
  return true;
}

function handleLinkHintKey(event) {
  if (event.key === "Escape") {
    closeLinkHints();
    return true;
  }

  if (event.key === "Backspace") {
    state.linkHintQuery = state.linkHintQuery.slice(0, -1);
  } else if (event.key.length === 1 && /[a-z]/i.test(event.key) && !event.metaKey && !event.ctrlKey && !event.altKey) {
    state.linkHintQuery += event.key.toLowerCase();
  } else {
    return false;
  }

  let exactMatch = null;
  let visibleCount = 0;
  for (const entry of state.linkHints) {
    const matches = entry.label.startsWith(state.linkHintQuery);
    entry.hint.classList.toggle("hidden", !matches);
    if (matches) {
      visibleCount += 1;
    }
    if (entry.label === state.linkHintQuery) {
      exactMatch = entry;
    }
  }

  if (exactMatch) {
    activateLinkElement(exactMatch.link);
    closeLinkHints();
  } else if (visibleCount === 0) {
    closeLinkHints();
  }

  return true;
}

function performAction(action) {
  const line = Math.max(48, Math.round(window.innerHeight * 0.08));
  const page = Math.max(120, Math.round(window.innerHeight * 0.82));
  const svgStep = svgPanStep();
  const helpOpen = !helpOverlay.classList.contains("hidden");
  const presentationOpen = state.presentation.mode === "presentation";
  const helpScroll = (delta) => {
    const panel = helpOverlay.querySelector(".help-panel");
    if (!panel) {
      return false;
    }
    panel.scrollBy({ top: delta, behavior: "instant" });
    return true;
  };

  switch (action) {
    case "scroll_down":
      if (helpOpen) {
        return helpScroll(line);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide + 1);
      }
      if (state.documentKind === "svg") {
        return panSvgBy(0, -svgStep);
      }
      window.scrollBy({ top: line, behavior: "instant" });
      return true;
    case "scroll_up":
      if (helpOpen) {
        return helpScroll(-line);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide - 1);
      }
      if (state.documentKind === "svg") {
        return panSvgBy(0, svgStep);
      }
      window.scrollBy({ top: -line, behavior: "instant" });
      return true;
    case "scroll_left":
      return state.documentKind === "svg" ? panSvgBy(svgStep, 0) : false;
    case "scroll_right":
      return state.documentKind === "svg" ? panSvgBy(-svgStep, 0) : false;
    case "zoom_in":
      return zoomSvgBy(1.2);
    case "zoom_out":
      return zoomSvgBy(1 / 1.2);
    case "reset_view":
      return resetSvgView();
    case "half_page_down":
      if (helpOpen) {
        return helpScroll(page / 2);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide + 1);
      }
      if (state.documentKind === "svg") {
        return false;
      }
      window.scrollBy({ top: page / 2, behavior: "instant" });
      return true;
    case "half_page_up":
      if (helpOpen) {
        return helpScroll(-page / 2);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide - 1);
      }
      if (state.documentKind === "svg") {
        return false;
      }
      window.scrollBy({ top: -page / 2, behavior: "instant" });
      return true;
    case "page_down":
      if (helpOpen) {
        return helpScroll(page);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide + 1);
      }
      if (state.documentKind === "svg") {
        return false;
      }
      window.scrollBy({ top: page, behavior: "instant" });
      return true;
    case "top":
      if (helpOpen) {
        return helpScroll(-Number.MAX_SAFE_INTEGER);
      }
      if (presentationOpen) {
        return showPresentationSlide(0);
      }
      if (state.documentKind === "svg") {
        return false;
      }
      window.scrollTo({ top: 0, behavior: "instant" });
      return true;
    case "bottom":
      if (helpOpen) {
        return helpScroll(Number.MAX_SAFE_INTEGER);
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.slides.length - 1);
      }
      if (state.documentKind === "svg") {
        return false;
      }
      window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
      return true;
    case "open_search":
      if (state.documentKind !== "markdown" || presentationOpen) {
        return false;
      }
      openSearch();
      return true;
    case "accept_search":
      closeSearch(false);
      return true;
    case "next_search_hit":
      if (!state.searchQuery) {
        return false;
      }
      setCurrentHit(state.currentHit + 1);
      return true;
    case "previous_search_hit":
      if (!state.searchQuery) {
        return false;
      }
      setCurrentHit(state.currentHit - 1);
      return true;
    case "show_help":
      toggleHelp(helpOverlay.classList.contains("hidden"));
      return true;
    case "close_overlay":
      closeLinkHints();
      closeSearch(true);
      toggleHelp(false);
      return true;
    case "toggle_toc":
      return toggleTocVisibility();
    case "toggle_focus":
      return state.documentKind === "markdown"
        ? switchFocus(state.focusMode === "document" ? "toc" : "document")
        : false;
    case "toggle_presentation":
      return togglePresentationMode();
    case "back":
      if (state.documentKind !== "markdown") {
        return false;
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide - 1);
      }
      sendIpc({ type: "back", scroll_ratio: currentScrollRatio() });
      return true;
    case "forward":
      if (state.documentKind !== "markdown") {
        return false;
      }
      if (presentationOpen) {
        return showPresentationSlide(state.presentation.currentSlide + 1);
      }
      sendIpc({ type: "forward", scroll_ratio: currentScrollRatio() });
      return true;
    case "open_link_hints":
      if (state.documentKind !== "markdown" || presentationOpen) {
        return false;
      }
      return openLinkHints();
    case "previous_file":
      sendIpc({ type: "previous_file", scroll_ratio: currentScrollRatio() });
      return true;
    case "next_file":
      sendIpc({ type: "next_file", scroll_ratio: currentScrollRatio() });
      return true;
    case "toc_down":
      return moveTocSelection(1);
    case "toc_up":
      return moveTocSelection(-1);
    case "activate_selection":
      return jumpToHeading(state.tocSelectionId, false);
    case "quit":
      sendIpc({ type: "close" });
      return true;
    default:
      return false;
  }
}

searchInput.addEventListener("input", () => {
  highlightSearch(searchInput.value);
});

searchInput.addEventListener("keydown", (event) => {
  const action = findAction(event, SEARCH_ACTIONS);
  if (!action) {
    return;
  }

  if (performAction(action)) {
    event.preventDefault();
  }
});

window.addEventListener("keydown", (event) => {
  if (state.linkHintMode) {
    if (handleLinkHintKey(event)) {
      event.preventDefault();
    }
    return;
  }

  if (!helpOverlay.classList.contains("hidden")) {
    const action = findAction(event, HELP_ACTIONS);
    if (action && performAction(action)) {
      event.preventDefault();
    }
    return;
  }

  if (!searchBar.classList.contains("hidden") || isTypingTarget(event.target)) {
    const action = findAction(event, SEARCH_ACTIONS);
    if (action && performAction(action)) {
      event.preventDefault();
    }
    return;
  }

  const action = findAction(event, allowedActionsFor());
  if (!action) {
    return;
  }

  if (performAction(action)) {
    event.preventDefault();
  }
});

content.addEventListener("click", (event) => {
  const link = event.target.closest("a[href]");
  if (!link) {
    return;
  }

  if (activateLinkElement(link)) {
    event.preventDefault();
  }
});

presentationRoot.addEventListener("click", (event) => {
  const link = event.target.closest("a[href]");
  if (!link) {
    return;
  }

  if (activateLinkElement(link)) {
    event.preventDefault();
  }
});

window.addEventListener("resize", () => {
  if (state.presentation.mode === "presentation") {
    void preparePresentationSlide(state.presentation.currentSlide);
  }
});

window.addEventListener(
  "scroll",
  () => {
    syncActiveHeading();
  },
  { passive: true },
);

window.addEventListener("resize", () => {
  if (state.documentKind === "svg") {
    resetSvgView();
  }
});

setTocItems(initialState.toc);
renderPresentationSlides();
renderHelp();
refreshHeadings();
syncActiveHeading();
updateTocState();
if (state.documentKind === "svg") {
  bindSvgViewer();
  resetSvgView();
} else {
  syncPresentationMode();
  if (state.presentation.mode === "presentation") {
    void preparePresentationSlide(state.presentation.currentSlide);
  } else {
    void renderMermaidIn(content);
  }
}

function resetSvgBindings() {
  state.svg.viewport = null;
  state.svg.stage = null;
  state.svg.root = null;
  state.svg.baseWidth = 0;
  state.svg.baseHeight = 0;
  state.svg.scale = 1;
  state.svg.minScale = 0.1;
  state.svg.maxScale = 24;
  state.svg.offsetX = 0;
  state.svg.offsetY = 0;
}

function bindSvgViewer() {
  resetSvgBindings();
  state.svg.viewport = document.getElementById("svg-viewport");
  state.svg.stage = document.getElementById("svg-stage");
  state.svg.root = state.svg.stage?.querySelector("svg") ?? null;

  if (!state.svg.viewport || !state.svg.stage || !state.svg.root) {
    return false;
  }

  const size = measureSvgRoot(state.svg.root);
  state.svg.baseWidth = size.width;
  state.svg.baseHeight = size.height;
  state.svg.stage.style.width = `${size.width}px`;
  state.svg.stage.style.height = `${size.height}px`;
  return true;
}

function measureSvgRoot(root) {
  const viewBox = root.viewBox?.baseVal;
  if (viewBox && viewBox.width > 0 && viewBox.height > 0) {
    return { width: viewBox.width, height: viewBox.height };
  }

  const width = root.width?.baseVal?.value;
  const height = root.height?.baseVal?.value;
  if (width > 0 && height > 0) {
    return { width, height };
  }

  const bbox = root.getBBox?.();
  if (bbox && bbox.width > 0 && bbox.height > 0) {
    return { width: bbox.width, height: bbox.height };
  }

  const rect = root.getBoundingClientRect();
  return {
    width: Math.max(rect.width, 1),
    height: Math.max(rect.height, 1),
  };
}

function applySvgTransform() {
  if (!state.svg.stage) {
    return false;
  }

  state.svg.stage.style.transform = `translate(${state.svg.offsetX}px, ${state.svg.offsetY}px) scale(${state.svg.scale})`;
  return true;
}

function resetSvgView() {
  if (!state.svg.viewport || !state.svg.stage || state.svg.baseWidth <= 0 || state.svg.baseHeight <= 0) {
    return false;
  }

  const viewportWidth = Math.max(state.svg.viewport.clientWidth, 1);
  const viewportHeight = Math.max(state.svg.viewport.clientHeight, 1);
  const fitScale = Math.min(
    viewportWidth / state.svg.baseWidth,
    viewportHeight / state.svg.baseHeight,
  );

  state.svg.scale = Number.isFinite(fitScale) && fitScale > 0 ? fitScale : 1;
  state.svg.minScale = Math.max(Math.min(state.svg.scale * 0.25, state.svg.scale), 0.05);
  state.svg.maxScale = Math.max(state.svg.scale * 12, 8);
  state.svg.offsetX = (viewportWidth - state.svg.baseWidth * state.svg.scale) / 2;
  state.svg.offsetY = (viewportHeight - state.svg.baseHeight * state.svg.scale) / 2;
  return applySvgTransform();
}

function svgPanStep() {
  return Math.max(48, 96 / Math.max(state.svg.scale, 0.1));
}

function panSvgBy(deltaX, deltaY) {
  if (!state.svg.stage) {
    return false;
  }

  state.svg.offsetX += deltaX;
  state.svg.offsetY += deltaY;
  return applySvgTransform();
}

function zoomSvgBy(factor) {
  if (!state.svg.viewport || !state.svg.stage) {
    return false;
  }

  const nextScale = Math.min(
    state.svg.maxScale,
    Math.max(state.svg.minScale, state.svg.scale * factor),
  );
  if (nextScale === state.svg.scale) {
    return false;
  }

  const centerX = state.svg.viewport.clientWidth / 2;
  const centerY = state.svg.viewport.clientHeight / 2;
  const contentX = (centerX - state.svg.offsetX) / state.svg.scale;
  const contentY = (centerY - state.svg.offsetY) / state.svg.scale;

  state.svg.scale = nextScale;
  state.svg.offsetX = centerX - contentX * state.svg.scale;
  state.svg.offsetY = centerY - contentY * state.svg.scale;
  return applySvgTransform();
}
