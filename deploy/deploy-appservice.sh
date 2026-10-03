#!/bin/bash
set -euo pipefail

# ============================================================
#  Pizzería Los Herrera - Deploy a Azure App Service
# ============================================================
#  Uso: ./deploy/deploy-appservice.sh [--ssh <key>] [--skip-tests]
#
#  Qué hace:
#    1. Verifica formato, clippy y tests
#    2. Construye la imagen Docker y la sube al ACR
#    3. Apunta el Web App a la imagen recien subida
#
#  Requisitos: az CLI autenticado y credenciales del ACR en el
#  entorno o en ~/.azure-pizzeria.env (ver más abajo).
# ============================================================

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

RESOURCE_GROUP="rg-pizzeria"
LOCATION="eastus"
ACR_NAME="cac7aa06a97facr"
IMAGE_NAME="pizzeria-api"
WEB_APP="pizzas-los-herrera-web"
APP_SERVICE_PLAN="pizzeria-plan"
MOUNT_PATH="/mnt/pizzeria-data"

SSH_KEY="${AZURE_SSH_KEY:-}"
SKIP_TESTS=false

while [ $# -gt 0 ]; do
  case "$1" in
    --ssh)       SSH_KEY="$2"; shift 2 ;;
    --skip-tests) SKIP_TESTS=true; shift ;;
    -h|--help)   sed -n '3,16p' "$0"; exit 0 ;;
    *) echo "Opción desconocida: $1"; exit 1 ;;
  esac
done

ENV_FILE="$HOME/.azure-pizzeria.env"
if [ -f "$ENV_FILE" ]; then
  echo "Cargando credenciales de $ENV_FILE"
  set -a; source "$ENV_FILE"; set +a
fi

: "${ACR_USERNAME:?Falta ACR_USERNAME (ver $ENV_FILE)}"
: "${ACR_PASSWORD:?Falta ACR_PASSWORD (ver $ENV_FILE)}"
: "${ADMIN_PASSWORD:?Falta ADMIN_PASSWORD (ver $ENV_FILE)}"
ADMIN_USER="${ADMIN_USER:-admin}"
WHATSAPP_NUMBER="${PIZZERIA_WHATSAPP:-5350722776}"
ALLOWED_ORIGINS="${ALLOWED_ORIGINS:-}"

echo "============================================"
echo "  PIZZERIA LOS HERRERA - DEPLOY APPSERVICE"
echo "============================================"
echo "Proyecto:   ${PROJECT_DIR}"
echo "Web App:    ${WEB_APP}"
echo "Registro:   ${ACR_NAME}"
echo ""

# --- 1. Verificación ---
if [ "$SKIP_TESTS" = false ]; then
  echo "[1/4] Verificando código..."
  cd "${PROJECT_DIR}/backend"
  cargo fmt -- --check
  cargo clippy --all-targets -- -D warnings
  cargo test --quiet
  echo "OK: fmt, clippy y tests correctos"
  cd "${PROJECT_DIR}"
else
  echo "[1/4] Verificación OMITIDA (--skip-tests)"
fi
echo ""

# --- 2. Login al registro ---
echo "[2/4] Autenticando en Azure Container Registry..."
if [ -n "$SSH_KEY" ]; then
  if [ ! -f "$SSH_KEY" ]; then
    echo "ERROR: no se encontró la clave SSH '$SSH_KEY'"
    exit 1
  fi
  cat "$SSH_KEY" | docker login "$ACR_NAME.azurecr.io" --username "$ACR_USERNAME" --password-stdin
else
  echo "$ACR_PASSWORD" | docker login "$ACR_NAME.azurecr.io" --username "$ACR_USERNAME" --password-stdin
fi
echo "OK: login correcto"
echo ""

# --- 3. Build y push ---
echo "[3/4] Construyendo imagen y subiendo al registro..."
TAG="$(git -C "${PROJECT_DIR}" rev-parse --short HEAD 2>/dev/null || echo 'local')"
IMAGE="$ACR_NAME.azurecr.io/$IMAGE_NAME"
docker build -t "$IMAGE:$TAG" -t "$IMAGE:latest" "${PROJECT_DIR}"
docker push "$IMAGE:$TAG"
docker push "$IMAGE:latest"
echo "OK: imagen $IMAGE:$TAG subida"
echo ""

# --- 4. Actualizar el Web App ---
echo "[4/4] Actualizando el Web App..."
az webapp config appsettings set \
  --name "$WEB_APP" --resource-group "$RESOURCE_GROUP" \
  --settings \
    DOCKER_REGISTRY_SERVER_URL="https://$ACR_NAME.azurecr.io" \
    DOCKER_REGISTRY_SERVER_USERNAME="$ACR_USERNAME" \
    DOCKER_REGISTRY_SERVER_PASSWORD="$ACR_PASSWORD" \
    DOCKER_USERNAME="$ACR_USERNAME" \
    DOCKER_PASSWORD="$ACR_PASSWORD" \
    ADMIN_USER="$ADMIN_USER" \
    ADMIN_PASSWORD="$ADMIN_PASSWORD" \
    PIZZERIA_WHATSAPP="$WHATSAPP_NUMBER" \
    ALLOWED_ORIGINS="$ALLOWED_ORIGINS" \
    RUST_LOG="info" \
    DATABASE_URL="sqlite:///$MOUNT_PATH/pizzeria.db?mode=rwc" \
    UPLOADS_DIR="$MOUNT_PATH/uploads" \
  --output none

az webapp update \
  --name "$WEB_APP" --resource-group "$RESOURCE_GROUP" \
  --deployment-container-image-name "$IMAGE:$TAG" \
  --output none

az webapp restart --name "$WEB_APP" --resource-group "$RESOURCE_GROUP" --output none
echo "OK: Web App actualizado"
echo ""

FQDN="$(az webapp show --name "$WEB_APP" --resource-group "$RESOURCE_GROUP" \
  --query defaultHostName --output tsv)"

echo "============================================"
echo "  DEPLOY COMPLETADO"
echo "============================================"
echo "URL:      https://${FQDN}"
echo "Admin:    https://${FQDN}/admin"
echo ""

echo "Comprobando que la app responde..."
if curl --fail --retry 12 --retry-delay 10 --retry-all-errors \
     "https://${FQDN}/api/health" >/dev/null; then
  echo "OK: health check correcto"
else
  echo "AVISO: el health check falló. Revisa con:"
  echo "  az webapp log tail --name ${WEB_APP} --resource-group ${RESOURCE_GROUP}"
  exit 1
fi
echo ""

echo "SIGUIENTE PASO: si cambiaste la contraseña, actualízala con"
echo "  az webapp config appsettings set --name ${WEB_APP} --resource-group ${RESOURCE_GROUP} \\"
echo "    --settings ADMIN_PASSWORD='TU_NUEVA_CLAVE' --output none"
echo "============================================"