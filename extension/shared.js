// Talking to the Feedability app (src-tauri/src/extension.rs). Copied from
// stockroom's extension (extension/shared.js) and cut down to what this one
// needs: the connection, its key, and calling the app.

const DEFAULT_PORT = 41630;

export class AppError extends Error {
  constructor(message, status) {
    super(message);
    this.status = status;
  }
}

const NOT_RUNNING =
  "Feedability に接続できません。アプリを起動して、設定 › ブラウザ拡張 で受け付けをオンにしてください";
export const NOT_CONNECTED =
  "この拡張機能は Feedability と接続されていません。下の「アプリと接続」を押してください";

/**
 * Where and how to connect. Keys to try, in order: the one the app gave when
 * the user allowed this extension ("アプリと接続"), then the one in
 * `config.json`, which the app writes into the copy it sets up.
 */
export async function getConfig() {
  let file = {};
  try {
    const res = await fetch(chrome.runtime.getURL("config.json"), { cache: "no-store" });
    if (res.ok) file = await res.json();
  } catch {
    /* not the copy the app set up */
  }
  const stored = await chrome.storage.local.get(["token"]);
  const tokens = [...new Set([stored.token, file.token].filter(Boolean).map(String))];
  return { port: Number(file.port) || DEFAULT_PORT, tokens };
}

async function request(port, path, init) {
  try {
    return await fetch(`http://127.0.0.1:${port}${path}`, { cache: "no-store", ...init });
  } catch {
    throw new AppError(NOT_RUNNING, 0);
  }
}

/** Calls the app. Throws AppError with a message to show (status 401 = not connected). */
export async function call(path, { method = "GET", params, body } = {}) {
  const { port, tokens } = await getConfig();
  if (!tokens.length) throw new AppError(NOT_CONNECTED, 401);
  const qs = params ? `?${new URLSearchParams(params)}` : "";
  let res;
  for (const token of tokens) {
    // A refused key is checked before the body is read, so sending again is safe.
    res = await request(port, path + qs, { method, body, headers: { Authorization: `Bearer ${token}` } });
    if (res.status !== 401) break;
  }
  const json = await res.json().catch(() => ({}));
  if (!res.ok) throw new AppError(json.error || `エラー（${res.status}）`, res.status);
  return json;
}

/**
 * Asks the app to let this extension in: the user sees `code` in the app and
 * presses 許可する. Resolves once they answer (up to about two minutes).
 */
export async function pair(code) {
  const { port } = await getConfig();
  const res = await request(port, `/pair?code=${code}`, { method: "POST" });
  const json = await res.json().catch(() => ({}));
  if (!res.ok || !json.token) throw new AppError(json.error || `エラー（${res.status}）`, res.status);
  await chrome.storage.local.set({ token: json.token });
}

/** Opens the page that connects the extension to the app. */
export function openConnectPage() {
  return chrome.tabs.create({ url: chrome.runtime.getURL("connect.html") });
}

/**
 * The app through its `feed:` link (docs/OPEN_WITH.md), for when it doesn't
 * answer: the OS starts it, or brings it up. The tab stays where it is; the
 * browser only asks whether to open the app.
 */
function openFeedLink(link, tabId) {
  return chrome.tabs.update(tabId, { url: link });
}

/** Starts Feedability (a `feed:` link it ignores: nothing to subscribe to). */
export function launchApp(tabId) {
  return openFeedLink("feed:open", tabId);
}

/** Starts Feedability with 「フィードを追加」 filled in with `url`. */
export function openInApp(url, tabId) {
  return openFeedLink(`feed:${url}`, tabId);
}
