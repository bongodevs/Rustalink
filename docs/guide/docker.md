# Docker

The recommended, zero-hassle way to deploy Rustalink.

## `docker-compose.yml`

```yaml

services:
  rustalink:
    image: ghcr.io/appujet/rustalink:latest
    container_name: rustalink
    restart: unless-stopped
    ports:
      - "2333:2333"
    environment:
      - RUST_LOG=info
    volumes:
      - ./config.toml:/app/config.toml
```

## Start the Container

```bash
docker-compose up -d
```
