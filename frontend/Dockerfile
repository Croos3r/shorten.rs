# syntax=docker/dockerfile:1

# ---- Builder ----
# Bun installs/builds (the repo ships a bun.lock); `--bun` targets the Bun
# runtime, so the self-contained server under .output/ must also run on Bun.
FROM oven/bun:1 AS builder
WORKDIR /app

COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

COPY . .
RUN bun --bun run build

# ---- Runtime ----
FROM oven/bun:1-slim AS runtime
WORKDIR /app

COPY --from=builder /app/.output ./.output

ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000

EXPOSE 3000
CMD ["bun", ".output/server/index.mjs"]
