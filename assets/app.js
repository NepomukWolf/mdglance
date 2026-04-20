const mermaidScript = document.createElement("script");
mermaidScript.text = window.__MDVIEW_MERMAID_SOURCE;
document.head.appendChild(mermaidScript);

const mermaid = globalThis.mermaid;
mermaid.initialize({ startOnLoad: false, securityLevel: "loose" });

const content = document.getElementById("content");
const searchBar = document.getElementById("search-bar");
const searchInput = document.getElementById("search-input");
const searchStatus = document.getElementById("search-status");
const helpOverlay = document.getElementById("help-overlay");

let searchQuery = "";
let searchHits = [];
let currentHit = -1;

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

searchInput.addEventListener("input", () => {
  highlightSearch(searchInput.value);
});

searchInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    closeSearch(false);
  } else if (event.key === "Escape") {
    event.preventDefault();
    closeSearch(true);
  }
});

window.addEventListener("keydown", (event) => {
  if (event.metaKey && !event.altKey) {
    if (!event.ctrlKey && ["w", "q"].includes(event.key.toLocaleLowerCase())) {
      event.preventDefault();
      window.ipc.postMessage("close");
      return;
    }

  }

  if (event.metaKey || event.ctrlKey || event.altKey) {
    return;
  }

  if (!helpOverlay.classList.contains("hidden")) {
    if (event.key === "Escape" || event.key === "?") {
      event.preventDefault();
      toggleHelp(false);
    }
    return;
  }

  if (!searchBar.classList.contains("hidden") || isTypingTarget(event.target)) {
    return;
  }

  const line = Math.max(48, Math.round(window.innerHeight * 0.08));
  const page = Math.max(120, Math.round(window.innerHeight * 0.82));

  switch (event.key) {
    case "/":
      event.preventDefault();
      openSearch();
      break;
    case "j":
      event.preventDefault();
      window.scrollBy({ top: line, behavior: "instant" });
      break;
    case "k":
      event.preventDefault();
      window.scrollBy({ top: -line, behavior: "instant" });
      break;
    case "h":
      event.preventDefault();
      window.scrollBy({ left: -line, behavior: "instant" });
      break;
    case "l":
      event.preventDefault();
      window.scrollBy({ left: line, behavior: "instant" });
      break;
    case "d":
      event.preventDefault();
      window.scrollBy({ top: page / 2, behavior: "instant" });
      break;
    case "u":
      event.preventDefault();
      window.scrollBy({ top: -page / 2, behavior: "instant" });
      break;
    case " ":
      event.preventDefault();
      window.scrollBy({ top: page, behavior: "instant" });
      break;
    case "g":
      event.preventDefault();
      window.scrollTo({ top: 0, behavior: "instant" });
      break;
    case "G":
      event.preventDefault();
      window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
      break;
    case "n":
      if (searchQuery) {
        event.preventDefault();
        setCurrentHit(currentHit + 1);
      }
      break;
    case "N":
      if (searchQuery) {
        event.preventDefault();
        setCurrentHit(currentHit - 1);
      }
      break;
    case "?":
      event.preventDefault();
      toggleHelp(true);
      break;
    case "Escape":
      event.preventDefault();
      closeSearch(true);
      toggleHelp(false);
      break;
    case "q":
      event.preventDefault();
      window.ipc.postMessage("close");
      break;
  }
});

renderMermaid();
