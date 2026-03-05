/**
 * BOING ANIMATED BACKGROUND ENGINE
 * Version 2.0 — Aquatic-Space System
 *
 * A single reusable Canvas + CSS engine that renders the full
 * bioluminescent aquatic-space background. Accepts a config object
 * to produce distinct variants per site and per page.
 *
 * Usage:
 *   const bg = new BoingBackground(document.getElementById('bg-canvas'), CONFIG.express.landing);
 *   bg.start();
 *
 * All opacity values are intentionally low (0.05–0.20) to guarantee
 * content readability. The base background is always near-black.
 */

class BoingBackground {
  constructor(canvas, config) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.cfg = this._mergeConfig(config);
    this.raf = null;
    this.t = 0; // global time counter (seconds)
    this.last = null;

    // Element pools
    this.stars = [];
    this.shootingStars = [];
    this.bubbles = [];
    this.jellyfish = [];
    this.coral = [];
    this.fish = [];
    this.particles = []; // finance data particles

    this._resize();
    this._init();
    window.addEventListener('resize', () => { this._resize(); this._init(); });
  }

  // ─── DEFAULT CONFIG ────────────────────────────────────────────────────────
  _mergeConfig(cfg) {
    const defaults = {
      // Base gradient
      baseBg: ['#020408', '#050c18', '#060f1e'],
      accentColor: '#00e8c8',
      accentColor2: null,

      // Stars
      starCount: 120,
      starColor: '#ffffff',
      starOpacityMin: 0.15,
      starOpacityMax: 0.65,

      // Nebula
      nebulaEnabled: true,
      nebulaClouds: [
        { x: 0.15, y: 0.12, r: 280, color: '#00e8c8', opacity: 0.055 },
        { x: 0.75, y: 0.08, r: 320, color: '#7c3aed', opacity: 0.045 },
        { x: 0.50, y: 0.20, r: 200, color: '#0096c7', opacity: 0.035 },
      ],

      // Shooting stars
      shootingStarEnabled: true,
      shootingStarColor: '#00e8c8',
      shootingStarColor2: null,
      shootingStarFrequency: 6000, // ms between spawns
      shootingStarCount: 3,

      // Waterline
      waterlineEnabled: true,
      waterlineY: 0.32,        // fraction of canvas height
      waterlineOpacity: 0.18,
      waterlineColor: '#00e8c8',
      waterlineWaveAmp: 6,
      waterlineWaveFreq: 0.008,

      // Bubbles
      bubblesEnabled: true,
      bubbleCount: 22,
      bubbleColor: '#00e8c8',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.13,
      bubbleSizeMin: 3,
      bubbleSizeMax: 18,

      // Jellyfish
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#00e8c8', '#48cae4', '#7c3aed'],
      jellyfishOpacity: 0.16,
      jellyfishSizeMin: 45,
      jellyfishSizeMax: 90,

      // Coral
      coralEnabled: true,
      coralCount: 7,
      coralColors: ['#00e8c8', '#0096c7', '#7c3aed'],
      coralOpacity: 0.10,
      coralHeightFraction: 0.20,

      // Fish
      fishEnabled: true,
      fishCount: 5,
      fishColor: '#48cae4',
      fishOpacity: 0.07,

      // Finance data particles
      particlesEnabled: false,
      particleCount: 40,
      particleColor: '#00e5ff',
      particleOpacity: 0.06,

      // Grid overlay (finance)
      gridEnabled: false,
      gridColor: '#00e5ff',
      gridOpacity: 0.022,
      gridSize: 60,
    };
    return Object.assign({}, defaults, cfg);
  }

  // ─── RESIZE ────────────────────────────────────────────────────────────────
  _resize() {
    this.W = this.canvas.width = window.innerWidth;
    this.H = this.canvas.height = window.innerHeight;
  }

  // ─── INITIALISE ALL ELEMENT POOLS ─────────────────────────────────────────
  _init() {
    const { W, H, cfg } = this;

    // Stars
    this.stars = Array.from({ length: cfg.starCount }, () => ({
      x: Math.random() * W,
      y: Math.random() * H * 0.55, // stars only in upper 55%
      r: 0.4 + Math.random() * 1.4,
      baseOpacity: cfg.starOpacityMin + Math.random() * (cfg.starOpacityMax - cfg.starOpacityMin),
      twinklePeriod: 3 + Math.random() * 6,
      twinkleOffset: Math.random() * Math.PI * 2,
    }));

    // Shooting stars — initialised as inactive, spawned by timer
    this.shootingStars = [];
    this._shootingStarTimer = 0;

    // Bubbles
    this.bubbles = Array.from({ length: cfg.bubbleCount }, () => this._newBubble(true));

    // Jellyfish
    this.jellyfish = Array.from({ length: cfg.jellyfishCount }, (_, i) => ({
      x: (0.15 + (i / cfg.jellyfishCount) * 0.7) * W + (Math.random() - 0.5) * 80,
      y: (0.45 + Math.random() * 0.35) * H,
      r: cfg.jellyfishSizeMin + Math.random() * (cfg.jellyfishSizeMax - cfg.jellyfishSizeMin),
      color: cfg.jellyfishColors[i % cfg.jellyfishColors.length],
      bobOffset: Math.random() * Math.PI * 2,
      bobPeriod: 4 + Math.random() * 3,
      driftSpeed: (Math.random() - 0.5) * 0.12,
      tentaclePhase: Math.random() * Math.PI * 2,
    }));

    // Coral
    this.coral = Array.from({ length: cfg.coralCount }, (_, i) => ({
      x: (i / (cfg.coralCount - 1)) * W * 1.1 - W * 0.05,
      baseY: H,
      height: cfg.coralHeightFraction * H * (0.5 + Math.random() * 0.8),
      color: cfg.coralColors[i % cfg.coralColors.length],
      type: Math.random() > 0.5 ? 'branch' : 'anemone',
      swayOffset: Math.random() * Math.PI * 2,
      swayPeriod: 3 + Math.random() * 3,
      swayAmp: 0.04 + Math.random() * 0.06,
      seed: Math.random() * 1000,
    }));

    // Fish
    this.fish = Array.from({ length: cfg.fishCount }, () => this._newFish());

    // Data particles (finance)
    if (cfg.particlesEnabled) {
      this.particles = Array.from({ length: cfg.particleCount }, () => ({
        x: Math.random() * W,
        y: Math.random() * H,
        vx: (Math.random() - 0.5) * 0.3,
        vy: (Math.random() - 0.5) * 0.3,
        r: 1.5 + Math.random() * 2,
        opacity: cfg.particleOpacityMin || 0.04,
      }));
    }
  }

  // ─── FACTORY HELPERS ──────────────────────────────────────────────────────
  _newBubble(randomY = false) {
    const { W, H, cfg } = this;
    return {
      x: Math.random() * W,
      y: randomY ? H * (0.4 + Math.random() * 0.6) : H + 20,
      r: cfg.bubbleSizeMin + Math.random() * (cfg.bubbleSizeMax - cfg.bubbleSizeMin),
      opacity: cfg.bubbleOpacityMin + Math.random() * (cfg.bubbleOpacityMax - cfg.bubbleOpacityMin),
      riseSpeed: 0.25 + Math.random() * 0.55,
      wobbleAmp: 0.4 + Math.random() * 0.8,
      wobblePeriod: 2 + Math.random() * 3,
      wobbleOffset: Math.random() * Math.PI * 2,
    };
  }

  _newFish() {
    const { W, H } = this;
    const dir = Math.random() > 0.5 ? 1 : -1;
    return {
      x: dir === 1 ? -60 : W + 60,
      y: H * (0.38 + Math.random() * 0.45),
      size: 8 + Math.random() * 14,
      speed: (0.4 + Math.random() * 0.6) * dir,
      color: this.cfg.fishColor,
      opacity: this.cfg.fishOpacity,
      wobbleAmp: 0.8 + Math.random() * 1.2,
      wobblePeriod: 1.5 + Math.random() * 2,
      wobbleOffset: Math.random() * Math.PI * 2,
    };
  }

  _spawnShootingStar() {
    const { W, H, cfg } = this;
    const angle = -(Math.PI / 6) + (Math.random() - 0.5) * 0.4;
    const speed = 380 + Math.random() * 280;
    const length = 90 + Math.random() * 120;
    const color1 = cfg.shootingStarColor2 || cfg.shootingStarColor;
    return {
      x: Math.random() * W * 0.8,
      y: Math.random() * H * 0.35,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      length,
      opacity: 0.7 + Math.random() * 0.3,
      color: cfg.shootingStarColor,
      color2: color1,
      life: 0,
      maxLife: (length / speed) + 0.3,
    };
  }

  // ─── START / STOP ─────────────────────────────────────────────────────────
  start() {
    if (this.raf) return;
    const loop = (ts) => {
      if (!this.last) this.last = ts;
      const dt = Math.min((ts - this.last) / 1000, 0.05);
      this.last = ts;
      this.t += dt;
      this._update(dt);
      this._draw();
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  stop() {
    if (this.raf) { cancelAnimationFrame(this.raf); this.raf = null; }
  }

  // ─── UPDATE ───────────────────────────────────────────────────────────────
  _update(dt) {
    const { W, H, cfg, t } = this;

    // Shooting stars
    if (cfg.shootingStarEnabled) {
      this._shootingStarTimer -= dt * 1000;
      if (this._shootingStarTimer <= 0 && this.shootingStars.length < cfg.shootingStarCount) {
        this.shootingStars.push(this._spawnShootingStar());
        this._shootingStarTimer = cfg.shootingStarFrequency * (0.7 + Math.random() * 0.6);
      }
      this.shootingStars = this.shootingStars.filter(s => {
        s.x += s.vx * dt;
        s.y += s.vy * dt;
        s.life += dt;
        return s.life < s.maxLife;
      });
    }

    // Bubbles
    if (cfg.bubblesEnabled) {
      this.bubbles.forEach(b => {
        b.y -= b.riseSpeed;
        b.x += Math.sin(t * (Math.PI * 2 / b.wobblePeriod) + b.wobbleOffset) * b.wobbleAmp * dt;
        if (b.y + b.r < 0) Object.assign(b, this._newBubble(false));
      });
    }

    // Jellyfish drift
    if (cfg.jellyfishEnabled) {
      this.jellyfish.forEach(j => {
        j.x += j.driftSpeed;
        if (j.x > W + j.r * 2) j.x = -j.r * 2;
        if (j.x < -j.r * 2) j.x = W + j.r * 2;
      });
    }

    // Fish
    if (cfg.fishEnabled) {
      this.fish.forEach((f, i) => {
        f.x += f.speed;
        if ((f.speed > 0 && f.x > W + 80) || (f.speed < 0 && f.x < -80)) {
          this.fish[i] = this._newFish();
        }
      });
    }

    // Data particles
    if (cfg.particlesEnabled) {
      this.particles.forEach(p => {
        p.x += p.vx;
        p.y += p.vy;
        if (p.x < 0) p.x = W;
        if (p.x > W) p.x = 0;
        if (p.y < 0) p.y = H;
        if (p.y > H) p.y = 0;
      });
    }
  }

  // ─── DRAW ─────────────────────────────────────────────────────────────────
  _draw() {
    const { ctx, W, H, cfg, t } = this;
    ctx.clearRect(0, 0, W, H);

    this._drawBase();
    if (cfg.gridEnabled) this._drawGrid();
    if (cfg.nebulaEnabled) this._drawNebula();
    this._drawStars();
    if (cfg.shootingStarEnabled) this._drawShootingStars();
    if (cfg.waterlineEnabled) this._drawWaterline();
    if (cfg.coralEnabled) this._drawCoral();
    if (cfg.bubblesEnabled) this._drawBubbles();
    if (cfg.fishEnabled) this._drawFish();
    if (cfg.jellyfishEnabled) this._drawJellyfish();
    if (cfg.particlesEnabled) this._drawParticles();
  }

  // ─── DRAW: BASE GRADIENT ──────────────────────────────────────────────────
  _drawBase() {
    const { ctx, W, H, cfg } = this;
    const grad = ctx.createLinearGradient(0, 0, 0, H);
    grad.addColorStop(0, cfg.baseBg[0] || '#020408');
    grad.addColorStop(0.45, cfg.baseBg[1] || '#050c18');
    grad.addColorStop(1, cfg.baseBg[2] || '#060f1e');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, W, H);
  }

  // ─── DRAW: GRID (finance) ─────────────────────────────────────────────────
  _drawGrid() {
    const { ctx, W, H, cfg } = this;
    ctx.save();
    ctx.strokeStyle = cfg.gridColor;
    ctx.globalAlpha = cfg.gridOpacity;
    ctx.lineWidth = 0.5;
    const g = cfg.gridSize;
    for (let x = 0; x < W; x += g) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
    }
    for (let y = 0; y < H; y += g) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke();
    }
    ctx.restore();
  }

  // ─── DRAW: NEBULA ─────────────────────────────────────────────────────────
  _drawNebula() {
    const { ctx, W, H, cfg, t } = this;
    ctx.save();
    cfg.nebulaClouds.forEach((cloud, i) => {
      const drift = Math.sin(t * 0.04 + i * 1.3) * 30;
      const cx = cloud.x * W + drift;
      const cy = cloud.y * H + Math.cos(t * 0.03 + i) * 15;
      const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, cloud.r);
      grad.addColorStop(0, cloud.color + 'cc');
      grad.addColorStop(0.5, cloud.color + '44');
      grad.addColorStop(1, 'transparent');
      ctx.globalAlpha = cloud.opacity;
      ctx.fillStyle = grad;
      ctx.beginPath();
      ctx.arc(cx, cy, cloud.r, 0, Math.PI * 2);
      ctx.fill();
    });
    ctx.restore();
  }

  // ─── DRAW: STARS ──────────────────────────────────────────────────────────
  _drawStars() {
    const { ctx, cfg, t } = this;
    ctx.save();
    this.stars.forEach(s => {
      const twinkle = 0.5 + 0.5 * Math.sin(t * (Math.PI * 2 / s.twinklePeriod) + s.twinkleOffset);
      const opacity = s.baseOpacity * (0.4 + 0.6 * twinkle);
      ctx.globalAlpha = opacity;
      ctx.fillStyle = cfg.starColor;
      ctx.beginPath();
      ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
      ctx.fill();
      // Subtle cross-sparkle for brighter stars
      if (s.baseOpacity > 0.45 && twinkle > 0.7) {
        ctx.globalAlpha = opacity * 0.4;
        ctx.strokeStyle = cfg.starColor;
        ctx.lineWidth = 0.5;
        const len = s.r * 3;
        ctx.beginPath(); ctx.moveTo(s.x - len, s.y); ctx.lineTo(s.x + len, s.y); ctx.stroke();
        ctx.beginPath(); ctx.moveTo(s.x, s.y - len); ctx.lineTo(s.x, s.y + len); ctx.stroke();
      }
    });
    ctx.restore();
  }

  // ─── DRAW: SHOOTING STARS ─────────────────────────────────────────────────
  _drawShootingStars() {
    const { ctx } = this;
    ctx.save();
    this.shootingStars.forEach(s => {
      const progress = s.life / s.maxLife;
      const fade = progress < 0.2 ? progress / 0.2 : 1 - (progress - 0.2) / 0.8;
      const tailX = s.x - (s.vx / Math.hypot(s.vx, s.vy)) * s.length;
      const tailY = s.y - (s.vy / Math.hypot(s.vx, s.vy)) * s.length;
      const grad = ctx.createLinearGradient(s.x, s.y, tailX, tailY);
      grad.addColorStop(0, s.color);
      grad.addColorStop(0.3, s.color2 + 'aa');
      grad.addColorStop(1, 'transparent');
      ctx.globalAlpha = s.opacity * fade;
      ctx.strokeStyle = grad;
      ctx.lineWidth = 1.5;
      ctx.lineCap = 'round';
      ctx.beginPath();
      ctx.moveTo(s.x, s.y);
      ctx.lineTo(tailX, tailY);
      ctx.stroke();
      // Head glow
      const glow = ctx.createRadialGradient(s.x, s.y, 0, s.x, s.y, 6);
      glow.addColorStop(0, s.color);
      glow.addColorStop(1, 'transparent');
      ctx.globalAlpha = s.opacity * fade * 0.8;
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(s.x, s.y, 6, 0, Math.PI * 2);
      ctx.fill();
    });
    ctx.restore();
  }

  // ─── DRAW: WATERLINE ──────────────────────────────────────────────────────
  _drawWaterline() {
    const { ctx, W, H, cfg, t } = this;
    const baseY = cfg.waterlineY * H;
    ctx.save();

    // Refraction shimmer below waterline
    const shimmerGrad = ctx.createLinearGradient(0, baseY, 0, baseY + H * 0.12);
    shimmerGrad.addColorStop(0, cfg.waterlineColor + '18');
    shimmerGrad.addColorStop(1, 'transparent');
    ctx.fillStyle = shimmerGrad;
    ctx.beginPath();
    ctx.moveTo(0, baseY);
    for (let x = 0; x <= W; x += 4) {
      const y = baseY + Math.sin(x * cfg.waterlineWaveFreq + t * 1.2) * cfg.waterlineWaveAmp;
      ctx.lineTo(x, y);
    }
    ctx.lineTo(W, H);
    ctx.lineTo(0, H);
    ctx.closePath();
    ctx.fill();

    // Waterline itself
    ctx.globalAlpha = cfg.waterlineOpacity;
    ctx.strokeStyle = cfg.waterlineColor;
    ctx.lineWidth = 1.2;
    ctx.shadowColor = cfg.waterlineColor;
    ctx.shadowBlur = 8;
    ctx.beginPath();
    for (let x = 0; x <= W; x += 4) {
      const y = baseY + Math.sin(x * cfg.waterlineWaveFreq + t * 1.2) * cfg.waterlineWaveAmp;
      x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
    ctx.restore();
  }

  // ─── DRAW: BUBBLES ────────────────────────────────────────────────────────
  _drawBubbles() {
    const { ctx, cfg } = this;
    ctx.save();
    this.bubbles.forEach(b => {
      ctx.globalAlpha = b.opacity;
      // Bubble ring
      ctx.strokeStyle = cfg.bubbleColor;
      ctx.lineWidth = 0.8;
      ctx.shadowColor = cfg.bubbleColor;
      ctx.shadowBlur = 4;
      ctx.beginPath();
      ctx.arc(b.x, b.y, b.r, 0, Math.PI * 2);
      ctx.stroke();
      // Specular highlight
      ctx.globalAlpha = b.opacity * 0.5;
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.arc(b.x - b.r * 0.3, b.y - b.r * 0.3, b.r * 0.22, 0, Math.PI * 2);
      ctx.fill();
    });
    ctx.restore();
  }

  // ─── DRAW: JELLYFISH ──────────────────────────────────────────────────────
  _drawJellyfish() {
    const { ctx, cfg, t } = this;
    ctx.save();
    this.jellyfish.forEach(j => {
      const bobY = j.y + Math.sin(t * (Math.PI * 2 / j.bobPeriod) + j.bobOffset) * 12;

      // Glow halo
      const halo = ctx.createRadialGradient(j.x, bobY, 0, j.x, bobY, j.r * 2.2);
      halo.addColorStop(0, j.color + '30');
      halo.addColorStop(1, 'transparent');
      ctx.globalAlpha = cfg.jellyfishOpacity * 0.6;
      ctx.fillStyle = halo;
      ctx.beginPath();
      ctx.arc(j.x, bobY, j.r * 2.2, 0, Math.PI * 2);
      ctx.fill();

      // Bell body
      ctx.globalAlpha = cfg.jellyfishOpacity;
      const bellGrad = ctx.createRadialGradient(j.x, bobY - j.r * 0.3, j.r * 0.1, j.x, bobY, j.r);
      bellGrad.addColorStop(0, j.color + 'cc');
      bellGrad.addColorStop(0.6, j.color + '55');
      bellGrad.addColorStop(1, j.color + '11');
      ctx.fillStyle = bellGrad;
      ctx.beginPath();
      ctx.ellipse(j.x, bobY, j.r, j.r * 0.65, 0, Math.PI, 0);
      ctx.fill();

      // Bell rim
      ctx.strokeStyle = j.color + '88';
      ctx.lineWidth = 1;
      ctx.shadowColor = j.color;
      ctx.shadowBlur = 10;
      ctx.beginPath();
      ctx.ellipse(j.x, bobY, j.r, j.r * 0.65, 0, Math.PI, 0);
      ctx.stroke();

      // Tentacles
      const tentacleCount = 7;
      ctx.lineWidth = 0.8;
      ctx.shadowBlur = 6;
      for (let k = 0; k < tentacleCount; k++) {
        const tx = j.x - j.r * 0.8 + (k / (tentacleCount - 1)) * j.r * 1.6;
        const phase = j.tentaclePhase + k * 0.5 + t * 1.2;
        const waveAmp = j.r * 0.25;
        const len = j.r * (1.2 + Math.random() * 0.0); // stable per frame
        ctx.globalAlpha = cfg.jellyfishOpacity * 0.7;
        ctx.strokeStyle = j.color + 'aa';
        ctx.beginPath();
        ctx.moveTo(tx, bobY);
        for (let seg = 1; seg <= 8; seg++) {
          const sy = bobY + (seg / 8) * len;
          const sx = tx + Math.sin(phase + seg * 0.7) * waveAmp * (seg / 8);
          seg === 1 ? ctx.moveTo(tx, bobY) : ctx.lineTo(sx, sy);
        }
        ctx.stroke();
      }
    });
    ctx.restore();
  }

  // ─── DRAW: CORAL ──────────────────────────────────────────────────────────
  _drawCoral() {
    const { ctx, cfg, t, H } = this;
    ctx.save();
    this.coral.forEach(c => {
      const sway = Math.sin(t * (Math.PI * 2 / c.swayPeriod) + c.swayOffset) * c.swayAmp;
      ctx.globalAlpha = cfg.coralOpacity;
      ctx.strokeStyle = c.color;
      ctx.shadowColor = c.color;
      ctx.shadowBlur = 8;
      ctx.lineWidth = 1.5;
      if (c.type === 'branch') {
        this._drawBranch(ctx, c.x, H, c.height, -Math.PI / 2 + sway, 5, c.color, c.seed);
      } else {
        this._drawAnemone(ctx, c.x, H - c.height * 0.3, c.height * 0.4, sway, c.color);
      }
    });
    ctx.restore();
  }

  _drawBranch(ctx, x, y, len, angle, depth, color, seed) {
    if (depth === 0 || len < 4) return;
    const ex = x + Math.cos(angle) * len;
    const ey = y + Math.sin(angle) * len;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(ex, ey);
    ctx.stroke();
    const spread = 0.35 + ((seed * depth) % 0.2);
    this._drawBranch(ctx, ex, ey, len * 0.65, angle - spread, depth - 1, color, seed * 1.3);
    this._drawBranch(ctx, ex, ey, len * 0.65, angle + spread, depth - 1, color, seed * 0.7);
  }

  _drawAnemone(ctx, x, y, r, sway, color) {
    const petals = 8;
    for (let i = 0; i < petals; i++) {
      const angle = (i / petals) * Math.PI * 2 + sway;
      const px = x + Math.cos(angle) * r;
      const py = y + Math.sin(angle) * r * 0.5;
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.quadraticCurveTo(
        x + Math.cos(angle + 0.3) * r * 0.6,
        y + Math.sin(angle + 0.3) * r * 0.3,
        px, py
      );
      ctx.stroke();
    }
    // Centre dot
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y, 3, 0, Math.PI * 2);
    ctx.fill();
  }

  // ─── DRAW: FISH ───────────────────────────────────────────────────────────
  _drawFish() {
    const { ctx, cfg, t } = this;
    ctx.save();
    this.fish.forEach(f => {
      const wobble = Math.sin(t * (Math.PI * 2 / f.wobblePeriod) + f.wobbleOffset) * f.wobbleAmp;
      ctx.globalAlpha = f.opacity;
      ctx.fillStyle = f.color;
      ctx.shadowColor = f.color;
      ctx.shadowBlur = 6;
      ctx.save();
      ctx.translate(f.x, f.y + wobble);
      if (f.speed < 0) ctx.scale(-1, 1);
      // Body
      ctx.beginPath();
      ctx.ellipse(0, 0, f.size, f.size * 0.45, 0, 0, Math.PI * 2);
      ctx.fill();
      // Tail
      ctx.beginPath();
      ctx.moveTo(-f.size * 0.85, 0);
      ctx.lineTo(-f.size * 1.5, -f.size * 0.5);
      ctx.lineTo(-f.size * 1.5, f.size * 0.5);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
    });
    ctx.restore();
  }

  // ─── DRAW: DATA PARTICLES (finance) ───────────────────────────────────────
  _drawParticles() {
    const { ctx, cfg } = this;
    ctx.save();
    const connectionDist = 100;
    // Connections
    for (let i = 0; i < this.particles.length; i++) {
      for (let j = i + 1; j < this.particles.length; j++) {
        const dx = this.particles[i].x - this.particles[j].x;
        const dy = this.particles[i].y - this.particles[j].y;
        const dist = Math.hypot(dx, dy);
        if (dist < connectionDist) {
          ctx.globalAlpha = cfg.particleOpacity * (1 - dist / connectionDist) * 0.5;
          ctx.strokeStyle = cfg.particleColor;
          ctx.lineWidth = 0.5;
          ctx.beginPath();
          ctx.moveTo(this.particles[i].x, this.particles[i].y);
          ctx.lineTo(this.particles[j].x, this.particles[j].y);
          ctx.stroke();
        }
      }
    }
    // Dots
    this.particles.forEach(p => {
      ctx.globalAlpha = cfg.particleOpacity * 1.5;
      ctx.fillStyle = cfg.particleColor;
      ctx.shadowColor = cfg.particleColor;
      ctx.shadowBlur = 4;
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      ctx.fill();
    });
    ctx.restore();
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// SITE & PAGE CONFIGS
// ═══════════════════════════════════════════════════════════════════════════

const BOING_BG_CONFIGS = {

  // ─── BOING.EXPRESS — "AQUA PERSONAL" ──────────────────────────────────────
  express: {
    landing: {
      baseBg: ['#020810', '#041220', '#051828'],
      accentColor: '#00e8c8',
      starCount: 110,
      starColor: '#e0f7ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.60,
      nebulaClouds: [
        { x: 0.10, y: 0.08, r: 260, color: '#00e8c8', opacity: 0.052 },
        { x: 0.80, y: 0.06, r: 300, color: '#0096c7', opacity: 0.040 },
        { x: 0.45, y: 0.18, r: 180, color: '#48cae4', opacity: 0.032 },
      ],
      shootingStarColor: '#00e8c8',
      shootingStarColor2: '#00b4d8',
      shootingStarFrequency: 5500,
      shootingStarCount: 3,
      waterlineEnabled: true,
      waterlineY: 0.34,
      waterlineOpacity: 0.16,
      waterlineColor: '#00e8c8',
      waterlineWaveAmp: 7,
      waterlineWaveFreq: 0.007,
      bubblesEnabled: true,
      bubbleCount: 24,
      bubbleColor: '#48cae4',
      bubbleOpacityMin: 0.05,
      bubbleOpacityMax: 0.14,
      bubbleSizeMin: 3,
      bubbleSizeMax: 16,
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#00e8c8', '#48cae4', '#7c3aed'],
      jellyfishOpacity: 0.15,
      jellyfishSizeMin: 50,
      jellyfishSizeMax: 85,
      coralEnabled: true,
      coralCount: 8,
      coralColors: ['#00e8c8', '#0096c7', '#7c3aed', '#48cae4'],
      coralOpacity: 0.10,
      coralHeightFraction: 0.22,
      fishEnabled: true,
      fishCount: 5,
      fishColor: '#48cae4',
      fishOpacity: 0.07,
      particlesEnabled: false,
      gridEnabled: false,
    },
    wallet: {
      baseBg: ['#020810', '#030e1a', '#041220'],
      accentColor: '#00e8c8',
      starCount: 80,
      starColor: '#e0f7ff',
      starOpacityMin: 0.10,
      starOpacityMax: 0.45,
      nebulaClouds: [
        { x: 0.20, y: 0.10, r: 200, color: '#00e8c8', opacity: 0.040 },
        { x: 0.75, y: 0.15, r: 240, color: '#0096c7', opacity: 0.032 },
      ],
      shootingStarColor: '#00e8c8',
      shootingStarColor2: '#00b4d8',
      shootingStarFrequency: 8000,
      shootingStarCount: 2,
      waterlineEnabled: true,
      waterlineY: 0.28,
      waterlineOpacity: 0.12,
      waterlineColor: '#00b4d8',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.009,
      bubblesEnabled: true,
      bubbleCount: 16,
      bubbleColor: '#48cae4',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.10,
      bubbleSizeMin: 2,
      bubbleSizeMax: 12,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#00e8c8', '#48cae4'],
      jellyfishOpacity: 0.11,
      jellyfishSizeMin: 40,
      jellyfishSizeMax: 65,
      coralEnabled: true,
      coralCount: 5,
      coralColors: ['#00e8c8', '#0096c7'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.18,
      fishEnabled: true,
      fishCount: 3,
      fishColor: '#48cae4',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    security: {
      baseBg: ['#020810', '#031018', '#041520'],
      accentColor: '#2dd4bf',
      starCount: 90,
      starColor: '#e0f7ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.50,
      nebulaClouds: [
        { x: 0.15, y: 0.10, r: 220, color: '#2dd4bf', opacity: 0.045 },
        { x: 0.70, y: 0.08, r: 260, color: '#fbbf24', opacity: 0.025 },
        { x: 0.45, y: 0.20, r: 160, color: '#0096c7', opacity: 0.030 },
      ],
      shootingStarColor: '#2dd4bf',
      shootingStarColor2: '#00e8c8',
      shootingStarFrequency: 7000,
      shootingStarCount: 2,
      waterlineEnabled: true,
      waterlineY: 0.38,
      waterlineOpacity: 0.13,
      waterlineColor: '#2dd4bf',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.006,
      bubblesEnabled: true,
      bubbleCount: 18,
      bubbleColor: '#2dd4bf',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.11,
      bubbleSizeMin: 3,
      bubbleSizeMax: 14,
      jellyfishEnabled: false,
      coralEnabled: true,
      coralCount: 6,
      coralColors: ['#2dd4bf', '#0096c7', '#fbbf24'],
      coralOpacity: 0.09,
      coralHeightFraction: 0.20,
      fishEnabled: true,
      fishCount: 4,
      fishColor: '#2dd4bf',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    docs: {
      baseBg: ['#020608', '#030c14', '#040f1a'],
      accentColor: '#00b4d8',
      starCount: 60,
      starColor: '#e0f7ff',
      starOpacityMin: 0.08,
      starOpacityMax: 0.35,
      nebulaClouds: [
        { x: 0.20, y: 0.12, r: 180, color: '#00b4d8', opacity: 0.028 },
        { x: 0.80, y: 0.10, r: 200, color: '#0096c7', opacity: 0.022 },
      ],
      shootingStarColor: '#00b4d8',
      shootingStarColor2: '#48cae4',
      shootingStarFrequency: 10000,
      shootingStarCount: 1,
      waterlineEnabled: false,
      bubblesEnabled: true,
      bubbleCount: 10,
      bubbleColor: '#00b4d8',
      bubbleOpacityMin: 0.03,
      bubbleOpacityMax: 0.08,
      bubbleSizeMin: 2,
      bubbleSizeMax: 10,
      jellyfishEnabled: false,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: false,
      gridEnabled: false,
    },
    faucet: {
      baseBg: ['#020810', '#041220', '#051828'],
      accentColor: '#00e8c8',
      starCount: 95,
      starColor: '#e0f7ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.55,
      nebulaClouds: [
        { x: 0.25, y: 0.10, r: 240, color: '#00e8c8', opacity: 0.048 },
        { x: 0.70, y: 0.12, r: 200, color: '#7c3aed', opacity: 0.035 },
      ],
      shootingStarColor: '#00e8c8',
      shootingStarColor2: '#00b4d8',
      shootingStarFrequency: 6000,
      shootingStarCount: 2,
      waterlineEnabled: true,
      waterlineY: 0.36,
      waterlineOpacity: 0.14,
      waterlineColor: '#00e8c8',
      waterlineWaveAmp: 8,
      waterlineWaveFreq: 0.008,
      bubblesEnabled: true,
      bubbleCount: 30,
      bubbleColor: '#00e8c8',
      bubbleOpacityMin: 0.05,
      bubbleOpacityMax: 0.15,
      bubbleSizeMin: 4,
      bubbleSizeMax: 20,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#00e8c8', '#48cae4'],
      jellyfishOpacity: 0.13,
      jellyfishSizeMin: 45,
      jellyfishSizeMax: 70,
      coralEnabled: true,
      coralCount: 6,
      coralColors: ['#00e8c8', '#48cae4'],
      coralOpacity: 0.09,
      coralHeightFraction: 0.18,
      fishEnabled: true,
      fishCount: 6,
      fishColor: '#48cae4',
      fishOpacity: 0.08,
      particlesEnabled: false,
      gridEnabled: false,
    },
  },

  // ─── BOING.FINANCE — "DEEP TRADE" ─────────────────────────────────────────
  finance: {
    landing: {
      baseBg: ['#020408', '#030810', '#040c18'],
      accentColor: '#00e5ff',
      starCount: 100,
      starColor: '#e0f8ff',
      starOpacityMin: 0.10,
      starOpacityMax: 0.55,
      nebulaClouds: [
        { x: 0.08, y: 0.10, r: 240, color: '#00e5ff', opacity: 0.045 },
        { x: 0.85, y: 0.08, r: 280, color: '#00ff88', opacity: 0.032 },
        { x: 0.50, y: 0.22, r: 160, color: '#8b5cf6', opacity: 0.028 },
      ],
      shootingStarColor: '#00e5ff',
      shootingStarColor2: '#00ff88',
      shootingStarFrequency: 4500,
      shootingStarCount: 4,
      waterlineEnabled: true,
      waterlineY: 0.30,
      waterlineOpacity: 0.12,
      waterlineColor: '#00e5ff',
      waterlineWaveAmp: 4,
      waterlineWaveFreq: 0.010,
      bubblesEnabled: true,
      bubbleCount: 16,
      bubbleColor: '#00e5ff',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.10,
      bubbleSizeMin: 2,
      bubbleSizeMax: 12,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#00e5ff', '#00ff88'],
      jellyfishOpacity: 0.12,
      jellyfishSizeMin: 45,
      jellyfishSizeMax: 75,
      coralEnabled: true,
      coralCount: 5,
      coralColors: ['#00e5ff', '#00ff88', '#8b5cf6'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.16,
      fishEnabled: false,
      particlesEnabled: true,
      particleCount: 45,
      particleColor: '#00e5ff',
      particleOpacity: 0.055,
      gridEnabled: true,
      gridColor: '#00e5ff',
      gridOpacity: 0.020,
      gridSize: 60,
    },
    trade: {
      baseBg: ['#010306', '#020508', '#03070c'],
      accentColor: '#00e5ff',
      starCount: 60,
      starColor: '#e0f8ff',
      starOpacityMin: 0.08,
      starOpacityMax: 0.35,
      nebulaClouds: [
        { x: 0.15, y: 0.08, r: 180, color: '#00e5ff', opacity: 0.030 },
        { x: 0.80, y: 0.12, r: 200, color: '#00ff88', opacity: 0.022 },
      ],
      shootingStarColor: '#00e5ff',
      shootingStarColor2: '#00ff88',
      shootingStarFrequency: 8000,
      shootingStarCount: 2,
      waterlineEnabled: false,
      bubblesEnabled: false,
      jellyfishEnabled: false,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: true,
      particleCount: 55,
      particleColor: '#00e5ff',
      particleOpacity: 0.050,
      gridEnabled: true,
      gridColor: '#00e5ff',
      gridOpacity: 0.025,
      gridSize: 60,
    },
    analytics: {
      baseBg: ['#010306', '#020508', '#030810'],
      accentColor: '#00e5ff',
      starCount: 70,
      starColor: '#e0f8ff',
      starOpacityMin: 0.08,
      starOpacityMax: 0.40,
      nebulaClouds: [
        { x: 0.20, y: 0.10, r: 200, color: '#00e5ff', opacity: 0.035 },
        { x: 0.75, y: 0.08, r: 220, color: '#8b5cf6', opacity: 0.025 },
      ],
      shootingStarColor: '#00e5ff',
      shootingStarColor2: '#8b5cf6',
      shootingStarFrequency: 9000,
      shootingStarCount: 2,
      waterlineEnabled: false,
      bubblesEnabled: false,
      jellyfishEnabled: false,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: true,
      particleCount: 50,
      particleColor: '#00e5ff',
      particleOpacity: 0.055,
      gridEnabled: true,
      gridColor: '#00e5ff',
      gridOpacity: 0.022,
      gridSize: 60,
    },
    governance: {
      baseBg: ['#020408', '#030810', '#050c18'],
      accentColor: '#8b5cf6',
      starCount: 90,
      starColor: '#f0e8ff',
      starOpacityMin: 0.10,
      starOpacityMax: 0.50,
      nebulaClouds: [
        { x: 0.15, y: 0.10, r: 260, color: '#8b5cf6', opacity: 0.048 },
        { x: 0.78, y: 0.08, r: 220, color: '#00e5ff', opacity: 0.030 },
        { x: 0.50, y: 0.20, r: 160, color: '#ec4899', opacity: 0.022 },
      ],
      shootingStarColor: '#8b5cf6',
      shootingStarColor2: '#00e5ff',
      shootingStarFrequency: 6000,
      shootingStarCount: 3,
      waterlineEnabled: false,
      bubblesEnabled: true,
      bubbleCount: 12,
      bubbleColor: '#8b5cf6',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.10,
      bubbleSizeMin: 3,
      bubbleSizeMax: 14,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#8b5cf6', '#ec4899'],
      jellyfishOpacity: 0.12,
      jellyfishSizeMin: 40,
      jellyfishSizeMax: 65,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: true,
      particleCount: 35,
      particleColor: '#8b5cf6',
      particleOpacity: 0.050,
      gridEnabled: true,
      gridColor: '#8b5cf6',
      gridOpacity: 0.018,
      gridSize: 60,
    },
    portfolio: {
      baseBg: ['#010306', '#020508', '#030810'],
      accentColor: '#00ff88',
      starCount: 75,
      starColor: '#e0fff0',
      starOpacityMin: 0.08,
      starOpacityMax: 0.42,
      nebulaClouds: [
        { x: 0.20, y: 0.10, r: 200, color: '#00ff88', opacity: 0.035 },
        { x: 0.75, y: 0.08, r: 220, color: '#00e5ff', opacity: 0.025 },
      ],
      shootingStarColor: '#00ff88',
      shootingStarColor2: '#00e5ff',
      shootingStarFrequency: 7000,
      shootingStarCount: 2,
      waterlineEnabled: false,
      bubblesEnabled: false,
      jellyfishEnabled: false,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: true,
      particleCount: 40,
      particleColor: '#00ff88',
      particleOpacity: 0.050,
      gridEnabled: true,
      gridColor: '#00ff88',
      gridOpacity: 0.018,
      gridSize: 60,
    },
  },

  // ─── BOING.NETWORK — "COSMIC FOUNDATION" ──────────────────────────────────
  network: {
    landing: {
      baseBg: ['#020408', '#04060e', '#060818'],
      accentColor: '#7c3aed',
      starCount: 160,
      starColor: '#f0e8ff',
      starOpacityMin: 0.15,
      starOpacityMax: 0.75,
      nebulaClouds: [
        { x: 0.12, y: 0.08, r: 320, color: '#7c3aed', opacity: 0.060 },
        { x: 0.80, y: 0.06, r: 360, color: '#c026d3', opacity: 0.045 },
        { x: 0.45, y: 0.18, r: 240, color: '#06b6d4', opacity: 0.038 },
        { x: 0.65, y: 0.30, r: 180, color: '#7c3aed', opacity: 0.028 },
      ],
      shootingStarColor: '#a78bfa',
      shootingStarColor2: '#06b6d4',
      shootingStarFrequency: 5000,
      shootingStarCount: 4,
      waterlineEnabled: true,
      waterlineY: 0.40,
      waterlineOpacity: 0.10,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.006,
      bubblesEnabled: true,
      bubbleCount: 18,
      bubbleColor: '#a78bfa',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.12,
      bubbleSizeMin: 3,
      bubbleSizeMax: 16,
      jellyfishEnabled: true,
      jellyfishCount: 4,
      jellyfishColors: ['#7c3aed', '#c026d3', '#06b6d4', '#a78bfa'],
      jellyfishOpacity: 0.14,
      jellyfishSizeMin: 55,
      jellyfishSizeMax: 100,
      coralEnabled: true,
      coralCount: 7,
      coralColors: ['#7c3aed', '#06b6d4', '#a78bfa', '#c026d3'],
      coralOpacity: 0.09,
      coralHeightFraction: 0.22,
      fishEnabled: true,
      fishCount: 4,
      fishColor: '#a78bfa',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    pillars: {
      baseBg: ['#020408', '#04060e', '#060818'],
      accentColor: '#7c3aed',
      starCount: 130,
      starColor: '#f0e8ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.65,
      nebulaClouds: [
        { x: 0.15, y: 0.10, r: 280, color: '#7c3aed', opacity: 0.052 },
        { x: 0.78, y: 0.08, r: 300, color: '#c026d3', opacity: 0.038 },
        { x: 0.50, y: 0.20, r: 200, color: '#06b6d4', opacity: 0.030 },
      ],
      shootingStarColor: '#a78bfa',
      shootingStarColor2: '#06b6d4',
      shootingStarFrequency: 6000,
      shootingStarCount: 3,
      waterlineEnabled: true,
      waterlineY: 0.42,
      waterlineOpacity: 0.09,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 4,
      waterlineWaveFreq: 0.006,
      bubblesEnabled: true,
      bubbleCount: 15,
      bubbleColor: '#a78bfa',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.10,
      bubbleSizeMin: 3,
      bubbleSizeMax: 14,
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#7c3aed', '#c026d3', '#06b6d4'],
      jellyfishOpacity: 0.12,
      jellyfishSizeMin: 50,
      jellyfishSizeMax: 85,
      coralEnabled: true,
      coralCount: 6,
      coralColors: ['#7c3aed', '#06b6d4', '#a78bfa'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.20,
      fishEnabled: true,
      fishCount: 3,
      fishColor: '#a78bfa',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    tokenomics: {
      baseBg: ['#020408', '#030610', '#050818'],
      accentColor: '#06b6d4',
      starCount: 120,
      starColor: '#e8f4ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.60,
      nebulaClouds: [
        { x: 0.20, y: 0.08, r: 260, color: '#06b6d4', opacity: 0.048 },
        { x: 0.75, y: 0.10, r: 280, color: '#7c3aed', opacity: 0.038 },
        { x: 0.48, y: 0.22, r: 180, color: '#fbbf24', opacity: 0.020 },
      ],
      shootingStarColor: '#06b6d4',
      shootingStarColor2: '#a78bfa',
      shootingStarFrequency: 6500,
      shootingStarCount: 3,
      waterlineEnabled: true,
      waterlineY: 0.38,
      waterlineOpacity: 0.11,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.007,
      bubblesEnabled: true,
      bubbleCount: 16,
      bubbleColor: '#06b6d4',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.11,
      bubbleSizeMin: 3,
      bubbleSizeMax: 15,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#06b6d4', '#7c3aed'],
      jellyfishOpacity: 0.12,
      jellyfishSizeMin: 50,
      jellyfishSizeMax: 80,
      coralEnabled: true,
      coralCount: 5,
      coralColors: ['#06b6d4', '#7c3aed'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.18,
      fishEnabled: false,
      particlesEnabled: false,
      gridEnabled: false,
    },
    roadmap: {
      baseBg: ['#020408', '#030610', '#050818'],
      accentColor: '#7c3aed',
      starCount: 140,
      starColor: '#f0e8ff',
      starOpacityMin: 0.14,
      starOpacityMax: 0.70,
      nebulaClouds: [
        { x: 0.10, y: 0.08, r: 300, color: '#7c3aed', opacity: 0.055 },
        { x: 0.82, y: 0.06, r: 340, color: '#c026d3', opacity: 0.042 },
        { x: 0.50, y: 0.15, r: 220, color: '#06b6d4', opacity: 0.032 },
        { x: 0.30, y: 0.28, r: 160, color: '#a78bfa', opacity: 0.025 },
      ],
      shootingStarColor: '#a78bfa',
      shootingStarColor2: '#06b6d4',
      shootingStarFrequency: 4500,
      shootingStarCount: 4,
      waterlineEnabled: false,
      bubblesEnabled: true,
      bubbleCount: 14,
      bubbleColor: '#a78bfa',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.11,
      bubbleSizeMin: 3,
      bubbleSizeMax: 14,
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#7c3aed', '#c026d3', '#a78bfa'],
      jellyfishOpacity: 0.13,
      jellyfishSizeMin: 55,
      jellyfishSizeMax: 90,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: false,
      gridEnabled: false,
    },
    developers: {
      baseBg: ['#010306', '#020508', '#030810'],
      accentColor: '#06b6d4',
      starCount: 90,
      starColor: '#e8f4ff',
      starOpacityMin: 0.10,
      starOpacityMax: 0.50,
      nebulaClouds: [
        { x: 0.18, y: 0.10, r: 220, color: '#06b6d4', opacity: 0.038 },
        { x: 0.78, y: 0.08, r: 240, color: '#7c3aed', opacity: 0.030 },
      ],
      shootingStarColor: '#06b6d4',
      shootingStarColor2: '#a78bfa',
      shootingStarFrequency: 8000,
      shootingStarCount: 2,
      waterlineEnabled: false,
      bubblesEnabled: false,
      jellyfishEnabled: false,
      coralEnabled: false,
      fishEnabled: false,
      particlesEnabled: false,
      gridEnabled: true,
      gridColor: '#06b6d4',
      gridOpacity: 0.018,
      gridSize: 60,
    },
    /* Testnet portal & hub backgrounds */
    portal: {
      baseBg: ['#020408', '#04060e', '#060818'],
      accentColor: '#7c3aed',
      starCount: 150,
      starColor: '#f0e8ff',
      starOpacityMin: 0.14,
      starOpacityMax: 0.70,
      nebulaClouds: [
        { x: 0.12, y: 0.08, r: 300, color: '#7c3aed', opacity: 0.055 },
        { x: 0.82, y: 0.06, r: 320, color: '#c026d3', opacity: 0.042 },
        { x: 0.50, y: 0.18, r: 220, color: '#06b6d4', opacity: 0.035 },
      ],
      shootingStarColor: '#a78bfa',
      shootingStarColor2: '#06b6d4',
      shootingStarFrequency: 5500,
      shootingStarCount: 4,
      waterlineEnabled: true,
      waterlineY: 0.38,
      waterlineOpacity: 0.11,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.007,
      bubblesEnabled: true,
      bubbleCount: 20,
      bubbleColor: '#a78bfa',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.12,
      bubbleSizeMin: 3,
      bubbleSizeMax: 16,
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#7c3aed', '#c026d3', '#06b6d4'],
      jellyfishOpacity: 0.13,
      jellyfishSizeMin: 50,
      jellyfishSizeMax: 90,
      coralEnabled: true,
      coralCount: 6,
      coralColors: ['#7c3aed', '#06b6d4', '#a78bfa'],
      coralOpacity: 0.09,
      coralHeightFraction: 0.20,
      fishEnabled: true,
      fishCount: 4,
      fishColor: '#a78bfa',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    users: {
      baseBg: ['#020408', '#04060e', '#060818'],
      accentColor: '#06b6d4',
      starCount: 140,
      starColor: '#f0e8ff',
      starOpacityMin: 0.13,
      starOpacityMax: 0.68,
      nebulaClouds: [
        { x: 0.15, y: 0.09, r: 280, color: '#7c3aed', opacity: 0.050 },
        { x: 0.78, y: 0.07, r: 300, color: '#c026d3', opacity: 0.038 },
        { x: 0.48, y: 0.20, r: 200, color: '#fbbf24', opacity: 0.022 },
      ],
      shootingStarColor: '#a78bfa',
      shootingStarColor2: '#06b6d4',
      shootingStarFrequency: 6000,
      shootingStarCount: 3,
      waterlineEnabled: true,
      waterlineY: 0.40,
      waterlineOpacity: 0.10,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.006,
      bubblesEnabled: true,
      bubbleCount: 22,
      bubbleColor: '#a78bfa',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.11,
      bubbleSizeMin: 3,
      bubbleSizeMax: 15,
      jellyfishEnabled: true,
      jellyfishCount: 3,
      jellyfishColors: ['#7c3aed', '#06b6d4', '#fbbf24'],
      jellyfishOpacity: 0.12,
      jellyfishSizeMin: 48,
      jellyfishSizeMax: 85,
      coralEnabled: true,
      coralCount: 6,
      coralColors: ['#7c3aed', '#06b6d4', '#a78bfa'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.20,
      fishEnabled: true,
      fishCount: 4,
      fishColor: '#a78bfa',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: false,
    },
    operators: {
      baseBg: ['#020408', '#030610', '#050818'],
      accentColor: '#06b6d4',
      starCount: 120,
      starColor: '#e8f4ff',
      starOpacityMin: 0.12,
      starOpacityMax: 0.58,
      nebulaClouds: [
        { x: 0.20, y: 0.08, r: 260, color: '#06b6d4', opacity: 0.045 },
        { x: 0.75, y: 0.10, r: 280, color: '#7c3aed', opacity: 0.035 },
        { x: 0.50, y: 0.22, r: 180, color: '#a78bfa', opacity: 0.025 },
      ],
      shootingStarColor: '#06b6d4',
      shootingStarColor2: '#a78bfa',
      shootingStarFrequency: 6500,
      shootingStarCount: 3,
      waterlineEnabled: true,
      waterlineY: 0.36,
      waterlineOpacity: 0.10,
      waterlineColor: '#06b6d4',
      waterlineWaveAmp: 5,
      waterlineWaveFreq: 0.007,
      bubblesEnabled: true,
      bubbleCount: 16,
      bubbleColor: '#06b6d4',
      bubbleOpacityMin: 0.04,
      bubbleOpacityMax: 0.10,
      bubbleSizeMin: 3,
      bubbleSizeMax: 14,
      jellyfishEnabled: true,
      jellyfishCount: 2,
      jellyfishColors: ['#06b6d4', '#7c3aed'],
      jellyfishOpacity: 0.11,
      jellyfishSizeMin: 45,
      jellyfishSizeMax: 75,
      coralEnabled: true,
      coralCount: 5,
      coralColors: ['#06b6d4', '#7c3aed'],
      coralOpacity: 0.08,
      coralHeightFraction: 0.18,
      fishEnabled: true,
      fishCount: 3,
      fishColor: '#a78bfa',
      fishOpacity: 0.06,
      particlesEnabled: false,
      gridEnabled: true,
      gridColor: '#06b6d4',
      gridOpacity: 0.016,
      gridSize: 60,
    },
  },
};

// ESM export for Astro/Vite
export { BoingBackground, BOING_BG_CONFIGS };
