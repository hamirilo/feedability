/** Dates as the list and the reader show them. Pure, so the same everywhere. */

const pad = (n: number) => String(n).padStart(2, "0");

/**
 * The list's date: the time for today, "昨日 HH:MM", the date this year,
 * and the full date before that. `now` is for the caller's clock.
 */
export function listDate(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const today = new Date(now);
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (ms >= start) return time;
  if (ms >= start - 86_400_000) return `昨日 ${time}`;
  if (d.getFullYear() === today.getFullYear()) return `${d.getMonth() + 1}/${d.getDate()}`;
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
}

/** The reader's date: always in full. */
export function fullDate(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** A day: "2026年10月17日". */
export function dayOnly(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日`;
}

/** "3 分前" for the sidebar's last refresh. */
export function ago(ms: number, now = Date.now()): string {
  const min = Math.floor((now - ms) / 60_000);
  if (min < 1) return "たった今";
  if (min < 60) return `${min} 分前`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} 時間前`;
  return `${Math.floor(h / 24)} 日前`;
}
