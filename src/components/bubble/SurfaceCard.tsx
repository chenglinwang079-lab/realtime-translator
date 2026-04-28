import type { ReactNode } from "react";

interface SurfaceCardProps {
  children: ReactNode;
  className?: string;
  padding?: boolean;
}

export function SurfaceCard({
  children,
  className = "",
  padding = true,
}: SurfaceCardProps) {
  return (
    <div className={`surface-card ${padding ? "surface-card--padded" : ""} ${className}`}>
      {children}
    </div>
  );
}
