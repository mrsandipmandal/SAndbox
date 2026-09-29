# Sandbox Registry Preview

## Reproduce Artifacts

The registry binary must be built before running:

```bash
cd registry && cargo build
```

This produces `registry/target/debug/registry`. No `.env` or external dependencies are needed — SQLite is embedded and the DB auto-creates at `registry/registry-data/registry.db`.

## Run the Server

The registry binary must be built first (see above). Then launch detached in a
`screen` session — plain `nohup`/`setsid ... &` background jobs get reaped by
the tool runner on this machine, so use screen:

```bash
cd /media/sandip/Pro/SAndbox
screen -dmS sandbox-registry sh -c 'cd /media/sandip/Pro/SAndbox && exec registry/target/debug/registry > .freebuff/preview.log 2>&1'
```

Check it: `screen -ls | grep sandbox-registry` (session),
`pgrep -f 'target/debug/registry'` (pid), `curl localhost:3000/health` (200 = OK).
Stop with: `screen -S sandbox-registry -X quit`

The server listens on `http://localhost:3000` by default.

### Apache proxy (configured 2026-09-16)

`/etc/apache2/conf-available/sandbox-registry.conf` (source copy: `.freebuff/sandbox-registry.conf`)
reverse-proxies `/registry` -> `127.0.0.1:3000` on ALL port-80 vhosts. `mod_proxy` +
`mod_proxy_http` enabled. Dashboard HTML uses relative URLs so it works at both `/`
(direct :3000) and `/registry/` (proxied).

- Dashboard: http://localhost/registry (or http://localhost:3000)
- API via proxy: http://localhost/registry/api/v1/
- CLI: `registry_url = "http://localhost/registry"` in sandbox.toml, or direct :3000

To undo: `sudo a2disconf sandbox-registry && sudo systemctl reload apache2`

Configure via env vars:

- `REGISTRY_DB` — SQLite database path (default: `registry-data/registry.db`)
- `REGISTRY_ADDR` — bind address (default: `0.0.0.0:3000`)
- `REGISTRY_RATE_LIMIT` — max requests per window (default: 60)
- `REGISTRY_RATE_WINDOW` — rate limit window in seconds (default: 60)
- `REGISTRY_MAX_UPLOAD_MB` — max upload size in MB (default: 50)

Health check: `curl http://localhost:3000/health`
