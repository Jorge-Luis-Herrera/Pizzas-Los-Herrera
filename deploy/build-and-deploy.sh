#!/bin/bash
set -e

# ============================================================
#  Pizzería Los Herrera - Build & Deploy Script (Rocky Linux)
# ============================================================
#  Uso: ./deploy/build-and-deploy.sh <IP_SERVIDOR> <USUARIO_SSH>
#
#  Ejemplo:
#    ./deploy/build-and-deploy.sh 20.123.45.67 Jorge
# ============================================================

if [ $# -lt 2 ]; then
  echo "Uso: $0 <IP_SERVIDOR> <USUARIO_SSH>"
  echo "Ejemplo: $0 20.123.45.67 Jorge"
  exit 1
fi

SERVER_IP="$1"
SSH_USER="$2"
REMOTE_DIR="/var/pizzeria"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "============================================"
echo "  PIZZERIA LOS HERRERA - DEPLOY"
echo "============================================"
echo "Servidor: ${SSH_USER}@${SERVER_IP}"
echo "Directorio remoto: ${REMOTE_DIR}"
echo ""

# --- Paso 1: Compilar release ---
echo "[1/5] Compilando release..."
cd "${PROJECT_DIR}/backend"
cargo build --release
echo "OK: Binario compilado ($(du -h target/release/pizzeria-api | cut -f1))"
echo ""

# --- Paso 2: Instalar dependencias en el servidor ---
echo "[2/5] Instalando dependencias en el servidor..."
ssh "${SSH_USER}@${SERVER_IP}" << 'REMOTE_SCRIPT'
  sudo dnf install -y nginx
  sudo systemctl enable --now nginx
  sudo systemctl start nginx
REMOTE_SCRIPT
echo "OK: Nginx instalado"
echo ""

# --- Paso 3: Crear directorios y copiar archivos ---
echo "[3/5] Copiando archivos al servidor..."
ssh "${SSH_USER}@${SERVER_IP}" "sudo mkdir -p ${REMOTE_DIR}/{frontend,uploads} && sudo chown -R ${SSH_USER}:${SSH_USER} ${REMOTE_DIR}"

# Binario
scp target/release/pizzeria-api "${SSH_USER}@${SERVER_IP}:${REMOTE_DIR}/pizzeria-api"

# Frontend
scp -r "${PROJECT_DIR}/frontend/"* "${SSH_USER}@${SERVER_IP}:${REMOTE_DIR}/frontend/"

# Archivos de deploy
scp "${PROJECT_DIR}/deploy/pizzeria.service" "${SSH_USER}@${SERVER_IP}:~/pizzeria.service"
scp "${PROJECT_DIR}/deploy/nginx-pizzeria.conf" "${SSH_USER}@${SERVER_IP}:~/pizzeria.conf"

echo "OK: Archivos copiados"
echo ""

# --- Paso 4: Configurar servicio systemd ---
echo "[4/5] Configurando servicio y nginx..."
ssh "${SSH_USER}@${SERVER_IP}" << 'REMOTE_SCRIPT'
  # Hacer ejecutable el binario
  chmod +x /var/pizzeria/pizzeria-api

  # Instalar servicio systemd
  sudo cp ~/pizzeria.service /etc/systemd/system/pizzeria.service
  sudo systemctl daemon-reload
  sudo systemctl enable pizzeria

  # Configurar nginx (Rocky Linux usa /etc/nginx/conf.d/)
  sudo cp ~/pizzeria.conf /etc/nginx/conf.d/pizzeria.conf
  sudo rm -f /etc/nginx/conf.d/default.conf
  sudo nginx -t && sudo systemctl restart nginx

  # Abrir puertos en firewalld
  sudo firewall-cmd --permanent --add-service=http
  sudo firewall-cmd --permanent --add-service=https
  sudo firewall-cmd --reload

  # Limpiar archivos temporales
  rm -f ~/pizzeria.service ~/pizzeria.conf
REMOTE_SCRIPT
echo "OK: Servicio y nginx configurados"
echo ""

# --- Paso 5: Iniciar servicio ---
echo "[5/5] Iniciando servicio..."
ssh "${SSH_USER}@${SERVER_IP}" "sudo systemctl restart pizzeria && sleep 1 && sudo systemctl status pizzeria --no-pager"
echo ""

echo "============================================"
echo "  DEPLOY COMPLETADO"
echo "============================================"
echo "URL: http://${SERVER_IP}"
echo "Admin: http://${SERVER_IP}/admin"
echo ""
echo "SIGUIENTE PASO: Configurar HTTPS con certbot"
echo "  sudo dnf install -y certbot python3-certbot-nginx"
echo "  sudo certbot --nginx -d TU_DOMINIO"
echo "============================================"
