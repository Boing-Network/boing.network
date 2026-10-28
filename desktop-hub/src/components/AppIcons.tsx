const sizeProps = (size: number) => ({ width: size, height: size, viewBox: "0 0 24 24" });
const stroke = { fill: "none" as const, stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

export function AppIcon({ appId, size = 18, ariaHidden = true }: { appId: string; size?: number; ariaHidden?: boolean }) {
  const s = sizeProps(size);
  const p = { ...s, ...stroke, ...(ariaHidden ? { "aria-hidden": true } : {}) };

  switch (appId) {
    case "observer":
      return (
        <svg {...p}>
          <rect x="3" y="3" width="7" height="7" rx="1" />
          <rect x="14" y="3" width="7" height="7" rx="1" />
          <rect x="3" y="14" width="7" height="7" rx="1" />
          <rect x="14" y="14" width="7" height="7" rx="1" />
        </svg>
      );
    case "express":
      return (
        <svg {...p}>
          <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4" />
          <path d="M3 5v14a2 2 0 0 0 2 2h16v-5" />
          <path d="M18 12a2 2 0 0 0 0 4h4v-4h-4z" />
        </svg>
      );
    case "finance":
      return (
        <svg {...p}>
          <line x1="12" y1="1" x2="12" y2="23" />
          <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5.5a3.5 3.5 0 0 1 0 7H6" />
        </svg>
      );
    case "network":
      /* Monochrome Arc Slab: same geometry as the Boing logo (public/favicon.svg), scaled to the 24-unit icon grid. */
      return (
        <svg {...p}>
          <rect x="3" y="3" width="18" height="18" rx="5" />
          <path d="M5.4 16.5 C7.5 16.5 8.2 14.8 9.6 12.4 C11 9.9 12.3 8.2 14.1 7.5 C15.8 6.8 17.2 7.8 17.9 9.2" />
          <circle cx="17.9" cy="9.2" r="1.5" fill="currentColor" stroke="none" />
        </svg>
      );
    case "qa":
      return (
        <svg {...p}>
          <path d="M9 11l3 3L22 4" />
          <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
        </svg>
      );
    case "home":
      return (
        <svg {...p}>
          <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
          <polyline points="9 22 9 12 15 12 15 22" />
        </svg>
      );
    default:
      return <AppIcon appId="network" size={size} ariaHidden={ariaHidden} />;
  }
}
