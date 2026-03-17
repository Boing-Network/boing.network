# Boing Design Kit — Astro Components

Reusable components matching the [Boing Design Kit](https://github.com/boing-network/boing.network). Use with Tailwind (see project root `tailwind.config.mjs`) and global styles `src/styles/boing-design-kit.css`.

## Components

| Component | Import | Props |
|-----------|--------|-------|
| **Button** | `import Button from '../components/design-kit/Button.astro'` | `variant` (primary \| secondary \| ghost), `size` (sm \| md \| lg), `disabled`, `type`, `href`, `className` |
| **Card** | `import Card from '../components/design-kit/Card.astro'` | `title`, `description`, `className`, `as` (div \| article \| section) |
| **Input** | `import Input from '../components/design-kit/Input.astro'` | `type`, `name`, `id`, `placeholder`, `value`, `required`, `disabled`, `className`, etc. |
| **Badge** | `import Badge from '../components/design-kit/Badge.astro'` | `variant` (default \| success \| warning \| error), `className` |
| **GlowingText** | `import GlowingText from '../components/design-kit/GlowingText.astro'` | `intensity` (low \| medium \| high), `color` (cyan \| blue \| green), `as` (span \| strong \| em), `className` |
| **FeatureGrid** | `import FeatureGrid from '../components/design-kit/FeatureGrid.astro'` | `features` (array of `{ title, description }`), `columns` (2 \| 3 \| 4), `className` |
| **TestimonialCard** | `import TestimonialCard from '../components/design-kit/TestimonialCard.astro'` | `quote`, `author`, `role`, `avatar?`, `className` |

## Example

```astro
---
import Button from '../components/design-kit/Button.astro';
import Card from '../components/design-kit/Card.astro';
import Badge from '../components/design-kit/Badge.astro';
---
<Card title="Security" description="Safety first.">
  <Badge variant="success">Live</Badge>
  <Button href="/testnet" variant="primary" size="lg">Join Testnet</Button>
</Card>
```
