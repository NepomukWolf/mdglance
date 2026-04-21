const mermaidScript = document.createElement("script");
mermaidScript.text = window.__MDVIEW_MERMAID_SOURCE;
document.head.appendChild(mermaidScript);

const mermaid = globalThis.mermaid;
mermaid.initialize({ startOnLoad: false, securityLevel: "loose" });

const config = window.__MDGLANCE_CONFIG;
const initialState = window.__MDGLANCE_INITIAL_STATE;

const content = document.getElementById("content");
const tocPanel = document.getElementById("toc-panel");
const tocNav = document.getElementById("toc-nav");
const tocEmpty = document.getElementById("toc-empty");
const tocMode = document.getElementById("toc-mode");
const searchBar = document.getElementById("search-bar");
const searchInput = document.getElementById("search-input");
const searchStatus = document.getElementById("search-status");
const helpOverlay = document.getElementById("help-overlay");
const helpList = document.getElementById("help-list");

applyTheme();

const state = {
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
  { id: "toc_down", description: "TOC: next heading" },
  { id: "toc_up", description: "TOC: previous heading" },
  { id: "toc_parent", description: "TOC: parent heading" },
  { id: "toc_child", description: "TOC: first child heading" },
  { id: "activate_selection", description: "TOC: jump to selected heading" },
  { id: "quit", description: "Quit" },
];

const keybindings = new Map(config.keybindings.map((binding) => [binding.action, binding]));
const orderedBindings = config.keybindings.flatMap((binding) =>
  binding.shortcuts.map((shortcut) => ({ action: binding.action, shortcut })),
);

const GLOBAL_ACTIONS = new Set(["quit", "show_help", "toggle_toc"]);
const HELP_ACTIONS = new Set(["show_help", "close_overlay", "quit"]);
const SEARCH_ACTIONS = new Set(["accept_search", "close_overlay", "quit"]);
const DOCUMENT_ACTIONS = new Set([
  "scroll_down",
  "scroll_up",
  "scroll_left",
  "scroll_right",
  "half_page_down",
  "half_page_up",
  "page_down",
  "top",
  "bottom",
  "open_search",
  "next_search_hit",
  "previous_search_hit",
  "toggle_focus",
]);
const TOC_ACTIONS = new Set([
  "toc_down",
  "toc_up",
  "toc_parent",
  "toc_child",
  "activate_selection",
  "toggle_focus",
]);

function allowedActionsFor(mode) {
  const allowed = new Set(GLOBAL_ACTIONS);
  const source = mode === "toc" ? TOC_ACTIONS : DOCUMENT_ACTIONS;
  for (const action of source) {
    allowed.add(action);
  }
  return allowed;
}

async function renderMermaid() {
  const nodes = document.querySelectorAll("pre.mermaid");
  for (const node of nodes) {
    node.removeAttribute("data-processed");
  }
  await mermaid.run({ nodes });
}

window.__mdglanceUpdate = async function (payload) {
  const scrollRatio =
    window.scrollY / Math.max(1, document.body.scrollHeight - window.innerHeight);
  const previousSelection = state.tocSelectionId;

  document.title = "mdglance - " + payload.title;
  content.innerHTML = payload.body;
  setTocItems(payload.toc);
  await renderMermaid();

  if (state.searchQuery) {
    highlightSearch(state.searchQuery);
  }

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

  updateTocState();
};

window.__mdglanceShowError = function (message) {
  content.innerHTML = `<pre class="error">${message}</pre>`;
};

function normalizeEventKey(event) {
  if (event.key.length === 1 && /[a-z]/i.test(event.key)) {
    return event.key.toLowerCase();
  }
  return event.key;
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

function setTocItems(items) {
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
  document.body.classList.toggle("toc-visible", state.tocVisible);
  document.body.classList.toggle("toc-focus", state.focusMode === "toc");
  document.body.classList.toggle("document-focus", state.focusMode === "document");
  tocMode.textContent = state.focusMode === "toc" ? "TOC focus" : "Document focus";

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

function applyTheme() {
  const root = document.documentElement;
  root.style.setProperty("--toc-active-background", config.theme.toc_active_background);
  root.style.setProperty("--toc-active-color", config.theme.toc_active_color);
  root.style.setProperty("--toc-selected-background", config.theme.toc_selected_background);
  root.style.setProperty("--toc-selected-color", config.theme.toc_selected_color);
  root.style.setProperty("--pane-background-focused", config.theme.pane_background_focused);
  root.style.setProperty("--pane-background-unfocused", config.theme.pane_background_unfocused);
}

function refreshHeadings() {
  state.headingNodes = Array.from(content.querySelectorAll("[data-mdglance-heading]")).map(
    (node) => ({
      id: node.id,
      level: Number.parseInt(node.dataset.level ?? node.tagName.slice(1), 10),
      node,
    }),
  );
}

function syncActiveHeading() {
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

function moveToParentHeading() {
  const current = state.tocIndex.get(state.tocSelectionId)?.item;
  if (!current || !current.parentId) {
    return false;
  }

  state.tocSelectionId = current.parentId;
  updateTocState();
  ensureSelectedRowVisible();
  return true;
}

function moveToChildHeading() {
  const current = state.tocIndex.get(state.tocSelectionId)?.item;
  if (!current || current.children.length === 0) {
    return false;
  }

  state.tocSelectionId = current.children[0];
  updateTocState();
  ensureSelectedRowVisible();
  return true;
}

function jumpToHeading(id, switchBackToDocument) {
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

function performAction(action) {
  const line = Math.max(48, Math.round(window.innerHeight * 0.08));
  const page = Math.max(120, Math.round(window.innerHeight * 0.82));

  switch (action) {
    case "scroll_down":
      window.scrollBy({ top: line, behavior: "instant" });
      return true;
    case "scroll_up":
      window.scrollBy({ top: -line, behavior: "instant" });
      return true;
    case "scroll_left":
      window.scrollBy({ left: -line, behavior: "instant" });
      return true;
    case "scroll_right":
      window.scrollBy({ left: line, behavior: "instant" });
      return true;
    case "half_page_down":
      window.scrollBy({ top: page / 2, behavior: "instant" });
      return true;
    case "half_page_up":
      window.scrollBy({ top: -page / 2, behavior: "instant" });
      return true;
    case "page_down":
      window.scrollBy({ top: page, behavior: "instant" });
      return true;
    case "top":
      window.scrollTo({ top: 0, behavior: "instant" });
      return true;
    case "bottom":
      window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
      return true;
    case "open_search":
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
      closeSearch(true);
      toggleHelp(false);
      return true;
    case "toggle_toc":
      return toggleTocVisibility();
    case "toggle_focus":
      return switchFocus(state.focusMode === "document" ? "toc" : "document");
    case "toc_down":
      return moveTocSelection(1);
    case "toc_up":
      return moveTocSelection(-1);
    case "toc_parent":
      return moveToParentHeading();
    case "toc_child":
      return moveToChildHeading();
    case "activate_selection":
      return jumpToHeading(state.tocSelectionId, true);
    case "quit":
      window.ipc.postMessage("close");
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

  const action = findAction(event, allowedActionsFor(state.focusMode));
  if (!action) {
    return;
  }

  if (performAction(action)) {
    event.preventDefault();
  }
});

window.addEventListener(
  "scroll",
  () => {
    syncActiveHeading();
  },
  { passive: true },
);

setTocItems(initialState.toc);
renderHelp();
refreshHeadings();
syncActiveHeading();
updateTocState();
renderMermaid();
