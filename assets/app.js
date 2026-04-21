const mermaidScript = document.createElement("script");
mermaidScript.text = window.__MDVIEW_MERMAID_SOURCE;
document.head.appendChild(mermaidScript);

const mermaid = globalThis.mermaid;
mermaid.initialize({ startOnLoad: false, securityLevel: "loose" });

const config = window.__MDGLANCE_CONFIG;
const content = document.getElementById("content");
const searchBar = document.getElementById("search-bar");
const searchInput = document.getElementById("search-input");
const searchStatus = document.getElementById("search-status");
const helpOverlay = document.getElementById("help-overlay");
const helpList = document.getElementById("help-list");

let searchQuery = "";
let searchHits = [];
let currentHit = -1;

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
  { id: "quit", description: "Quit" },
];

const keybindings = new Map(config.keybindings.map((binding) => [binding.action, binding]));
const orderedBindings = config.keybindings.flatMap((binding) =>
  binding.shortcuts.map((shortcut) => ({ action: binding.action, shortcut })),
);

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
  document.title = "mdglance - " + payload.title;
  content.innerHTML = payload.body;
  await renderMermaid();
  if (searchQuery) {
    highlightSearch(searchQuery);
  }
  window.scrollTo(
    0,
    scrollRatio * Math.max(1, document.body.scrollHeight - window.innerHeight),
  );
};

window.__mdglanceShowError = function (message) {
  content.innerHTML = `<pre class="error">${message}</pre>`;
};

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
  searchHits = [];
  currentHit = -1;
}

function setSearchStatus() {
  if (!searchQuery) {
    searchStatus.textContent = "";
  } else if (searchHits.length === 0) {
    searchStatus.textContent = "0";
  } else {
    searchStatus.textContent = `${currentHit + 1}/${searchHits.length}`;
  }
}

function setCurrentHit(index) {
  if (searchHits.length === 0) {
    currentHit = -1;
    setSearchStatus();
    return;
  }

  currentHit = (index + searchHits.length) % searchHits.length;
  for (const hit of searchHits) {
    hit.classList.remove("current");
  }
  searchHits[currentHit].classList.add("current");
  searchHits[currentHit].scrollIntoView({ block: "center", inline: "nearest" });
  setSearchStatus();
}

function highlightSearch(query) {
  clearSearchHighlights();
  searchQuery = query;

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
      searchHits.push(mark);

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
  searchInput.value = searchQuery;
  searchInput.focus();
  searchInput.select();
  setSearchStatus();
}

function closeSearch(clear) {
  searchBar.classList.add("hidden");
  searchInput.blur();
  if (clear) {
    searchQuery = "";
    clearSearchHighlights();
    setSearchStatus();
  }
}

function toggleHelp(show) {
  helpOverlay.classList.toggle("hidden", !show);
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
      if (!searchQuery) {
        return false;
      }
      setCurrentHit(currentHit + 1);
      return true;
    case "previous_search_hit":
      if (!searchQuery) {
        return false;
      }
      setCurrentHit(currentHit - 1);
      return true;
    case "show_help":
      toggleHelp(helpOverlay.classList.contains("hidden"));
      return true;
    case "close_overlay":
      closeSearch(true);
      toggleHelp(false);
      return true;
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
  const action = findAction(event, new Set(["accept_search", "close_overlay"]));
  if (!action) {
    return;
  }

  if (performAction(action)) {
    event.preventDefault();
  }
});

window.addEventListener("keydown", (event) => {
  if (!helpOverlay.classList.contains("hidden")) {
    const action = findAction(event, new Set(["show_help", "close_overlay"]));
    if (action && performAction(action)) {
      event.preventDefault();
    }
    return;
  }

  if (!searchBar.classList.contains("hidden") || isTypingTarget(event.target)) {
    const action = findAction(event, new Set(["quit"]));
    if (action && performAction(action)) {
      event.preventDefault();
    }
    return;
  }

  const action = findAction(event);
  if (!action) {
    return;
  }

  if (!performAction(action)) {
    return;
  }

  event.preventDefault();
});

renderHelp();
renderMermaid();
