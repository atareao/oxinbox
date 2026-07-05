# oxinbox — Despliegue en VPS

## Prerrequisitos

- Docker + Docker Compose v2 en el VPS
- Dominio apuntando al VPS (para HTTPS)
- Un proxy reverso (nginx, Caddy, Traefik) para terminar TLS

## Pasos

### 1. Clonar la configuración

```sh
mkdir -p /opt/oxinbox
cd /opt/oxinbox
# Copiar deploy/ a /opt/oxinbox:
#   deploy/docker-compose.yml
#   deploy/.env.prod → renombrar a .env
```

### 2. Configurar variables de entorno

```sh
cp .env.prod .env
# Editar .env con tus valores reales:
#   DB_PASSWORD      → contraseña segura
#   RP_ID            → dominio (ej: oxinbox.tudominio.com)
#   RP_ORIGIN        → https://oxinbox.tudominio.com
#   AI_API_KEY       → API key de OpenAI (opcional)
```

### 3. Configurar imagen del backend

Editar `docker-compose.yml` y cambiar `tu-usuario/oxinbox` por tu usuario de GitHub.

### 4. Iniciar

```sh
docker compose up -d
```

### 5. Proxy reverso (ejemplo con Caddy)

```caddyfile
oxinbox.tudominio.com {
    reverse_proxy localhost:3300
}
```

```sh
caddy run
```

### 6. Verificar

```sh
curl https://oxinbox.tudominio.com/health
# → {"status":"ok"}
```

## Actualizar

Cuando el CI/CD publique una nueva imagen (push a main):

```sh
cd /opt/oxinbox
docker compose pull
docker compose up -d
```

O automatizar con un webhook + watchtower.