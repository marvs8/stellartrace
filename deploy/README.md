# Deployment Notes

This directory holds deployment assets for running StellarTrace outside of `cargo run`.

## Docker

```bash
docker compose up --build
```

See [`../Dockerfile`](../Dockerfile) and [`../docker-compose.yml`](../docker-compose.yml). The image builds a release binary of `stellartrace-api` only — run the Soroban contract's own build/deploy separately (see [`../contracts/flagged_accounts/scripts`](../contracts/flagged_accounts/scripts)).

## systemd

`stellartrace-api.service` is a template unit for running the API as a systemd service on a Linux host:

```bash
sudo useradd --system --home /opt/stellartrace stellartrace
sudo mkdir -p /opt/stellartrace/data
sudo cp target/release/stellartrace-api /opt/stellartrace/
sudo cp .env /opt/stellartrace/.env
sudo cp deploy/stellartrace-api.service /etc/systemd/system/
sudo chown -R stellartrace:stellartrace /opt/stellartrace
sudo systemctl daemon-reload
sudo systemctl enable --now stellartrace-api
```

The unit applies basic hardening (`ProtectSystem=strict`, `NoNewPrivileges`, a scoped `ReadWritePaths`) — review and adjust for your environment before relying on it in production, especially the `ReadWritePaths` if you change `STELLARTRACE_DATA_DIR`.

## Reverse proxy / TLS

Neither the Docker image nor the systemd unit terminates TLS. Put a reverse proxy (nginx, Caddy, an cloud load balancer) in front and terminate TLS there — see [docs/threat-model.md](../docs/threat-model.md#out-of-scope-for-this-reference-implementation).
