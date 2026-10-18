// "アプリと接続": asks the app for its key (POST /pair). The user allows it in
// the app after checking that both show the same number. A tab rather than the
// popup, which would close as soon as the user clicks into the app.
// Copied from stockroom's extension (extension/connect.js).
import { call, pair } from "./shared.js";

const $ = (id) => document.getElementById(id);

function show(text, kind = "", { busy = false, retry = false, close = false } = {}) {
  const m = $("message");
  m.className = kind;
  m.replaceChildren();
  if (busy) {
    const s = document.createElement("span");
    s.className = "spinner";
    m.append(s);
  }
  $("code").classList.toggle("ok", kind === "ok");
  m.append(text);
  $("retry").hidden = !retry;
  $("close").hidden = !close;
}

/** The number, one box per digit. */
function showCode(code) {
  $("code").replaceChildren(
    ...[...code].map((c) => Object.assign(document.createElement("span"), { textContent: c })),
  );
}

async function connect() {
  const code = String(crypto.getRandomValues(new Uint16Array(1))[0] % 10000).padStart(4, "0");
  showCode(code);
  show("アプリの確認を待っています…（Feedability の画面を見てください）", "", { busy: true });
  try {
    await pair(code);
    show("接続しました。このタブは閉じて構いません。", "ok", { close: true });
  } catch (e) {
    show(e.message, "error", { retry: true });
  }
}

$("retry").onclick = connect;
$("close").onclick = () => window.close();

// Already connected (e.g. opened twice): say so instead of asking again.
try {
  await call("/info");
  showCode("✓");
  show("すでに接続されています。", "ok", { close: true });
} catch (e) {
  if (e.status === 401) connect();
  else show(e.message, "error", { retry: true });
}
