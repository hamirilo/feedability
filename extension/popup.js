// The popup: the open page's feeds to subscribe to, and the page itself into
// あとで読む (src-tauri/src/extension.rs). The page is read only when the
// popup opens (activeTab), never in the background.
import { call, openConnectPage } from "./shared.js";

const $ = (id) => document.getElementById(id);
const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
const web = /^https?:/.test(tab?.url ?? "");

function result(text, kind) {
  const r = $("result");
  r.hidden = false;
  r.className = `status ${kind}`;
  r.textContent = text;
}

/** In the page: the feeds it names (`<link rel="alternate">`), as absolute URLs. */
function feedLinks() {
  const types = ["application/rss+xml", "application/atom+xml", "application/feed+json", "application/json"];
  return [...document.querySelectorAll('link[rel~="alternate"][href]')]
    .filter((l) => types.includes((l.type || "").toLowerCase()))
    .map((l) => ({ url: new URL(l.getAttribute("href"), document.baseURI).href, title: l.title || "" }))
    .filter((f, i, all) => all.findIndex((g) => g.url === f.url) === i);
}

/** In the page: what it shows, for あとで読む. */
function pageHtml() {
  return { url: location.href, title: document.title, html: document.documentElement.outerHTML };
}

async function inPage(func) {
  const [r] = await chrome.scripting.executeScript({ target: { tabId: tab.id }, func });
  return r?.result;
}

async function subscribe(url, button) {
  button.disabled = true;
  try {
    const { title } = await call("/subscribe", { method: "POST", params: { url } });
    result(`「${title}」を購読しました`, "ok");
    button.textContent = "購読済み";
  } catch (e) {
    result(e.message, "error");
    button.disabled = false;
  }
}

async function showFeeds() {
  const feeds = web ? ((await inPage(feedLinks).catch(() => [])) ?? []) : [];
  const list = $("feeds");
  list.replaceChildren();
  for (const f of feeds) {
    const li = document.createElement("li");
    const name = document.createElement("div");
    name.className = "name";
    const t = document.createElement("span");
    t.textContent = f.title || f.url;
    const u = document.createElement("small");
    u.textContent = f.url;
    name.append(t, u);
    const b = document.createElement("button");
    b.className = "btn primary";
    b.textContent = "購読";
    b.onclick = () => subscribe(f.url, b);
    li.append(name, b);
    list.append(li);
  }
  $("no-feeds").hidden = feeds.length > 0;
  $("try-page").disabled = !web;
  $("try-page").onclick = () => subscribe(tab.url, $("try-page"));
}

$("later").disabled = !web;
$("later").onclick = async () => {
  $("later").disabled = true;
  try {
    const page = await inPage(pageHtml);
    await call("/later", { method: "POST", params: { url: page.url, title: page.title }, body: page.html });
    result("あとで読むに入れました", "ok");
  } catch (e) {
    result(e.message, "error");
    $("later").disabled = false;
  }
};

$("connect").onclick = () => {
  openConnectPage();
  window.close();
};

async function connect() {
  const status = $("status");
  try {
    await call("/info");
    status.className = "status ok";
    status.textContent = "Feedability と接続中";
  } catch (e) {
    status.className = "status error";
    status.textContent = e.status === 401 ? "Feedability とまだ接続されていません" : e.message;
    $("connect").hidden = e.status !== 401;
  }
}

connect();
showFeeds();
