#!/bin/bash
set -e

# 1. Configurar puerto de NGINX basado en la variable de entorno $PORT (Default 8080)
PORT="${PORT:-8080}"
sed -i "s/PORT_PLACEHOLDER/$PORT/g" /etc/nginx/conf.d/default.conf

echo "🚀 Iniciando App Rust (REST: 3000, gRPC: 50051)..."
# Iniciamos la app en background
/app/service &
APP_PID=$!

# Esperamos un momento para que la app arranque (opcional, Nginx reintentará si falla al inicio)
sleep 1

echo "🚀 Iniciando NGINX Proxy en puerto $PORT..."
# Iniciamos Nginx en foreground, pero lo mandamos a background para esperar señales
nginx -g "daemon off;" &
NGINX_PID=$!

# Función para propagar señales
_term() { 
  echo "🛑 Caught SIGTERM signal!" 
  kill -TERM "$APP_PID" 2>/dev/null
  kill -TERM "$NGINX_PID" 2>/dev/null
}

trap _term SIGTERM SIGINT

# Esperar a que cualquiera de los procesos termine
wait $APP_PID $NGINX_PID
