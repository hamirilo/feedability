/**
 * The pictures of the 画像付き layout (docs/LIST_LAYOUT.md), asked of Rust
 * (`thumbnail_data_url`) only for the rows in sight, a few at a time. A row
 * scrolled away before its turn takes its request back, so a fast scroll
 * through thousands of articles doesn't leave a queue behind it.
 */
import { api } from "./api";

/** Data URLs, or null for a picture that couldn't be had (not asked again). */
const ready = new Map<string, string | null>();
const MAX_CACHED = 400;
const PARALLEL = 4;

interface Job {
  url: string;
  started: boolean;
  listeners: Set<(data: string | null) => void>;
}
const jobs = new Map<string, Job>();
const queue: Job[] = [];
let active = 0;

function remember(url: string, data: string | null) {
  ready.delete(url);
  if (ready.size >= MAX_CACHED) ready.delete(ready.keys().next().value!);
  ready.set(url, data);
}

function pump() {
  while (active < PARALLEL && queue.length > 0) {
    const job = queue.shift()!;
    job.started = true;
    active++;
    void api
      .thumbnailDataUrl(job.url)
      .catch(() => null)
      .then((data) => {
        active--;
        jobs.delete(job.url);
        remember(job.url, data);
        for (const l of job.listeners) l(data);
        pump();
      });
  }
}

/** What is known of `url` now: a data URL, null (none), or undefined (not yet). */
export function cachedThumb(url: string): string | null | undefined {
  const data = ready.get(url);
  // Recently used: kept longest.
  if (data !== undefined) remember(url, data);
  return data;
}

/** Asks for `url`'s picture; `onReady` gets it. Returns the call that takes the request back. */
export function wantThumb(url: string, onReady: (data: string | null) => void): () => void {
  let job = jobs.get(url);
  if (!job) {
    job = { url, started: false, listeners: new Set() };
    jobs.set(url, job);
    queue.push(job);
  }
  const j = job;
  j.listeners.add(onReady);
  pump();
  return () => {
    j.listeners.delete(onReady);
    if (j.listeners.size === 0 && !j.started) {
      jobs.delete(url);
      queue.splice(queue.indexOf(j), 1);
    }
  };
}
