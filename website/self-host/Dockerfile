# Build the site then run the minimal self-host server (static + /api/health).
# Build from repo root: docker build -f website/self-host/Dockerfile .
# For full portal API wire the Functions with your own DB — see README.md.

FROM node:20-alpine AS builder
WORKDIR /app
COPY website/package.json website/package-lock.json* ./
RUN npm ci
COPY website ./
RUN npm run build

FROM node:20-alpine
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY website/self-host/server.js ./server.js
ENV NODE_ENV=production
ENV PORT=3000
ENV DIST_PATH=/app/dist
EXPOSE 3000
CMD ["node", "server.js"]
