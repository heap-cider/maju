import type { CSSProperties } from "react";

import { cn } from "@/shared/lib/cn";

type ShimmerProps = {
  children: string;
  className?: string;
};

export function Shimmer({ children, className }: ShimmerProps) {
  return (
    <span
      className={cn("maju-shimmer", className)}
      style={
        { "--maju-shimmer-spread": `${children.length * 2}px` } as CSSProperties
      }
    >
      {children}
      {/* Visual-only highlight copy; the sibling text node above is the sole
          accessible content. */}
      <span aria-hidden="true" className="maju-shimmer-overlay">
        {children}
      </span>
    </span>
  );
}
