import { Rss } from "lucide-react";
import { useStore } from "../store";

/**
 * A feed's icon (docs/FAVICON.md), or the generic feed mark when it has none
 * (yet). Both are `size` px square, so a row doesn't shift when the icon
 * arrives.
 */
export function FeedIcon({ feedId, size, className = "" }: { feedId: number; size: number; className?: string }) {
  const icon = useStore((s) => s.icons[feedId]);
  if (!icon) return <Rss size={size} className={`shrink-0 text-dim ${className}`} />;
  return (
    <img
      src={icon}
      alt=""
      draggable={false}
      width={size}
      height={size}
      className={`shrink-0 rounded-sm object-contain ${className}`}
      style={{ width: size, height: size }}
    />
  );
}
