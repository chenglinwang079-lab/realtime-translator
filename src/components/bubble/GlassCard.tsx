import type { ReactNode } from "react";

interface GlassCardProps {
  children: ReactNode;
  className?: string;
  padding?: boolean;
}

export function GlassCard({
  children,
  className = "",
  padding = true,
}: GlassCardProps) {
  return (
    <div className={`glass-card ${padding ? "glass-card--padded" : ""} ${className}`}>
      {children}
    </div>
  );
}
