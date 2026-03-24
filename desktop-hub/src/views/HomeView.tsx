import type { HubView } from "../config";

const CARDS: { id: HubView; title: string; description: string }[] = [
  {
    id: "observer",
    title: "Observer",
    description: "Block explorer — browse blocks, transactions, and accounts on Boing Network.",
  },
  {
    id: "express",
    title: "Wallet",
    description: "Same wallet as the Chrome extension — send BOING, faucet, stake, and connect to dApps.",
  },
  {
    id: "finance",
    title: "Finance",
    description: "DEX & DeFi — swap, liquidity, bridge, and portfolio on Boing.",
  },
  {
    id: "network",
    title: "Testnet",
    description: "Testnet ecosystem — register, faucet, quests, and developer resources. Later: network hub for everyone.",
  },
];

type Props = { onNavigate: (view: HubView) => void; lastUsedAppId: HubView | null };

export function HomeView({ onNavigate, lastUsedAppId }: Props) {
  return (
    <div className="home-root">
      <div className="home-bg" aria-hidden="true">
        <div className="home-bg-gradient" />
        <div className="home-bg-grid" />
        <div className="home-bg-orb home-bg-orb--a" />
        <div className="home-bg-orb home-bg-orb--b" />
        <div className="home-bg-orb home-bg-orb--c" />
        <div className="home-bg-shimmer" />
      </div>

      <div className="home">
        <header className="home-header">
          <div className="home-kicker">Desktop hub</div>
          <h1 className="home-title">Boing Network Hub</h1>
          <p className="home-lead">
            One place for the Boing ecosystem: explore the chain, use the same wallet as the Chrome extension, and
            access testnet — all inside this desktop app.
          </p>
        </header>

        <div className="home-cards">
          {CARDS.map((card, index) => (
            <div
              key={card.id}
              className={`home-card ${lastUsedAppId === card.id ? "home-card-last-used" : ""}`}
              style={{ animationDelay: `${80 + index * 55}ms` }}
              role="button"
              tabIndex={0}
              onClick={() => onNavigate(card.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onNavigate(card.id);
                }
              }}
            >
              <div className="home-card-head">
                <h3>{card.title}</h3>
                {lastUsedAppId === card.id && (
                  <span className="home-card-badge" aria-label="Last used app">
                    Last used
                  </span>
                )}
              </div>
              <p>{card.description}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
