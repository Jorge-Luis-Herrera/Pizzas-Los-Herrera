// Pizzería Los Herrera - JavaScript para Clientes (Landing, Pedidos, GPS & WhatsApp)

const API_BASE = '/api';
const CART_KEY = 'pizzeria_cart';

// App State
let state = {
  products: [],
  cart: JSON.parse(localStorage.getItem(CART_KEY) || '[]'),
  selectedCategory: 'all',
  pendingOrderPayload: null
};

// Initialize Client App
document.addEventListener('DOMContentLoaded', () => {
  initCategoryPills();
  initCartModal();
  initConfirmModal();
  initCheckoutForm();
  initGPS();
  initImageModal();

  loadProducts();
  updateCartUI();
});

function showToast(message) {
  const toast = document.getElementById('toast-msg');
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(showToast._timer);
  showToast._timer = setTimeout(() => {
    toast.classList.remove('show');
  }, 3000);
}

// 1. Category Filters
function initCategoryPills() {
  const pills = document.querySelectorAll('.pill-btn');
  pills.forEach(pill => {
    pill.addEventListener('click', () => {
      pills.forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      state.selectedCategory = pill.getAttribute('data-category');
      renderProducts();
    });
  });
}

// 2. Load & Render Client Menu Products
async function loadProducts() {
  const container = document.getElementById('products-container');
  try {
    const res = await fetch(`${API_BASE}/products`);
    if (!res.ok) throw new Error('Error al cargar productos');
    state.products = await res.json();
    renderProducts();
    reconcileCartWithMenu();
  } catch (err) {
    console.error(err);
    if (container) {
      container.innerHTML = `
        <div class="empty-state" style="grid-column: 1 / -1;">
          <div class="empty-state-icon">⚠️</div>
          <p>No se pudo cargar el menú en este momento. Inténtalo más tarde.</p>
        </div>
      `;
    }
  }
}

function renderProducts() {
  const container = document.getElementById('products-container');
  if (!container) return;

  const filtered = state.selectedCategory === 'all'
    ? state.products
    : state.products.filter(p => p.category === state.selectedCategory);

  if (filtered.length === 0) {
    container.innerHTML = `
      <div class="empty-state" style="grid-column: 1 / -1;">
        <div class="empty-state-icon">🍕</div>
        <p>No hay productos disponibles en esta categoría.</p>
      </div>
    `;
    return;
  }

  container.innerHTML = filtered.map(p => `
    <div class="product-card ${p.image ? 'has-image' : ''}" ${p.image ? `style="background-image: url('/uploads/${encodeURIComponent(p.image)}');"` : ''} ${p.image ? `data-image="/uploads/${encodeURIComponent(p.image)}"` : ''}>
      ${p.image ? '<div class="product-card-overlay"></div>' : ''}
      <span class="product-badge ${p.available ? 'badge-available' : 'badge-unavailable'}">
        ${p.available ? 'Disponible' : 'Agotado'}
      </span>
      <div ${p.image ? 'class="product-card-content"' : ''}>
        <span class="product-category">${escapeHtml(p.category)}</span>
        <h3 class="product-title">${escapeHtml(p.name)}</h3>
        <p class="product-desc">${escapeHtml(p.description || 'Delicioso ingrediente artesanal.')}</p>
      </div>
      <div class="product-footer" ${p.image ? 'class="product-card-content"' : ''}>
        <span class="product-price">$${p.price.toFixed(2)}</span>
        <button
          class="btn-add-cart"
          data-product-id="${escapeHtml(p.id)}"
          ${!p.available ? 'disabled' : ''}
        >
          Agregar
        </button>
      </div>
    </div>
  `).join('');

  container.querySelectorAll('.product-card[data-image]').forEach(card => {
    card.addEventListener('click', () => openImageModal(card.getAttribute('data-image')));
  });

  container.querySelectorAll('.btn-add-cart').forEach(btn => {
    btn.addEventListener('click', (event) => {
      event.stopPropagation();
      addToCart(btn.getAttribute('data-product-id'));
    });
  });
}

function openImageModal(src) {
  const modal = document.getElementById('modal-view-image');
  const img = document.getElementById('modal-image-src');
  img.src = src;
  modal.classList.add('active');
}

function initImageModal() {
  const modal = document.getElementById('modal-view-image');
  const btnClose = document.getElementById('btn-close-image');
  if (btnClose) btnClose.addEventListener('click', () => modal.classList.remove('active'));
  if (modal) modal.addEventListener('click', (e) => {
    if (e.target === modal) modal.classList.remove('active');
  });
}

// 3. Cart Operations
function addToCart(productId) {
  const product = state.products.find(p => p.id === productId);
  if (!product || !product.available) return;

  const existingItem = state.cart.find(item => item.product_id === productId);
  const currentQty = existingItem ? existingItem.quantity : 0;

  if (currentQty >= 50) {
    showToast('Has alcanzado el máximo de 50 unidades por producto');
    return;
  }

  if (existingItem) {
    existingItem.quantity += 1;
  } else {
    state.cart.push({
      product_id: product.id,
      product_name: product.name,
      unit_price: product.price,
      quantity: 1
    });
  }

  saveCart();
  updateCartUI();
  showToast(`✅ "${product.name}" agregado al carrito`);
}

function updateCartQuantity(productId, delta) {
  const item = state.cart.find(i => i.product_id === productId);
  if (!item) return;

  item.quantity += delta;
  if (item.quantity <= 0) {
    state.cart = state.cart.filter(i => i.product_id !== productId);
  } else if (item.quantity > 50) {
    item.quantity = 50;
    showToast('Has alcanzado el máximo de 50 unidades por producto');
  }

  saveCart();
  updateCartUI();
}

function saveCart() {
  localStorage.setItem(CART_KEY, JSON.stringify(state.cart));
}

/**
 * Sincroniza el carrito guardado con el menú actual.
 *
 * El carrito vive en localStorage, así que puede quedar desactualizado si el
 * admin cambia precios o agota un producto. Antes el error solo aparecía al
 * confirmar el pedido; aquí se avisa al momento de abrir el carrito.
 */
function reconcileCartWithMenu() {
  if (state.cart.length === 0 || state.products.length === 0) return;

  const cambios = [];

  state.cart = state.cart.filter(item => {
    const product = state.products.find(p => p.id === item.product_id);
    if (!product) {
      cambios.push(`"${item.product_name}" ya no está en el menú`);
      return false;
    }
    if (!product.available) {
      cambios.push(`"${product.name}" se agotó`);
      return false;
    }
    if (product.price !== item.unit_price) {
      cambios.push(`el precio de "${product.name}" cambió a $${product.price.toFixed(2)}`);
      item.unit_price = product.price;
    }
    return true;
  });

  if (cambios.length === 0) return;

  saveCart();
  updateCartUI();
  showToast(`🛒 Carrito actualizado: ${cambios[0]}`);
}

function updateCartUI() {
  const totalCount = state.cart.reduce((sum, item) => sum + item.quantity, 0);
  const totalAmount = state.cart.reduce((sum, item) => sum + (item.unit_price * item.quantity), 0);

  const cartCountEl = document.getElementById('cart-count');
  const cartTotalEl = document.getElementById('cart-total-amount');

  if (cartCountEl) cartCountEl.textContent = totalCount;
  if (cartTotalEl) cartTotalEl.textContent = `$${totalAmount.toFixed(2)}`;

  const container = document.getElementById('cart-items-container');
  if (container) {
    if (state.cart.length === 0) {
      container.innerHTML = `
        <div class="empty-state">
          <div class="empty-state-icon">🛒</div>
          <p>Tu carrito está vacío</p>
        </div>
      `;
    } else {
      container.innerHTML = state.cart.map(item => `
        <div class="cart-item">
          <div class="cart-item-info">
            <h4>${escapeHtml(item.product_name)}</h4>
            <p>$${item.unit_price.toFixed(2)} c/u</p>
          </div>
          <div class="cart-item-controls">
            <button type="button" class="btn-qty" data-action="dec" data-product-id="${escapeHtml(item.product_id)}" aria-label="Quitar uno">-</button>
            <span style="font-weight: 600; min-width: 20px; text-align: center;">${item.quantity}</span>
            <button type="button" class="btn-qty" data-action="inc" data-product-id="${escapeHtml(item.product_id)}" aria-label="Añadir uno">+</button>
          </div>
        </div>
      `).join('');

      container.querySelectorAll('.btn-qty').forEach(btn => {
        btn.addEventListener('click', () => {
          const delta = btn.getAttribute('data-action') === 'inc' ? 1 : -1;
          updateCartQuantity(btn.getAttribute('data-product-id'), delta);
        });
      });
    }
  }
}

// 4. Cart Modal Events
function initCartModal() {
  const modal = document.getElementById('modal-cart');
  const btnOpen = document.getElementById('btn-open-cart');
  const btnClose = document.getElementById('btn-close-cart');

  if (btnOpen && modal) {
    btnOpen.addEventListener('click', () => {
      reconcileCartWithMenu();
      modal.classList.add('active');
    });
  }
  if (btnClose && modal) {
    btnClose.addEventListener('click', () => modal.classList.remove('active'));
  }
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) modal.classList.remove('active');
    });
  }
}

// 5. GPS Geolocation Helper
function initGPS() {
  const btnGps = document.getElementById('btn-get-gps');
  if (!btnGps) return;

  btnGps.addEventListener('click', () => {
    if (!navigator.geolocation) {
      alert('Tu dispositivo o navegador no soporta geolocalización.');
      return;
    }

    btnGps.textContent = '⏳ Obteniendo ubicación GPS...';
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        document.getElementById('cust-lat').value = pos.coords.latitude;
        document.getElementById('cust-lng').value = pos.coords.longitude;
        btnGps.textContent = `✅ GPS Capturado (${pos.coords.latitude.toFixed(4)}, ${pos.coords.longitude.toFixed(4)})`;
        btnGps.style.background = 'rgba(46, 196, 182, 0.25)';
        showToast('📍 Ubicación GPS agregada al pedido');
      },
      (err) => {
        console.error(err);
        btnGps.textContent = '📍 Capturar Mi Ubicación GPS Actual';
        alert('No se pudo obtener la ubicación GPS automática. Por favor escribe tu dirección exacta.');
      }
    );
  });
}

// 6. Checkout Form Submission & Confirmation Security Modal
function initCheckoutForm() {
  const form = document.getElementById('form-checkout');
  if (!form) return;

  form.addEventListener('submit', (e) => {
    e.preventDefault();

    if (state.cart.length === 0) {
      alert('Tu carrito está vacío. Agrega al menos un producto.');
      return;
    }

    const customerName = document.getElementById('cust-name').value.trim();
    const customerPhone = document.getElementById('cust-phone').value.trim();
    const deliveryAddress = document.getElementById('cust-address').value.trim();
    const latVal = document.getElementById('cust-lat').value;
    const lngVal = document.getElementById('cust-lng').value;
    const notes = document.getElementById('cust-notes').value.trim();
    const customerIdNumber = document.getElementById('cust-id-number').value.trim();

    if (!customerName || !deliveryAddress) {
      alert('Completa tu nombre y la dirección de entrega.');
      return;
    }

    // El teléfono es lo que permite al local llamar al cliente para confirmar.
    if (!customerPhone) {
      alert('Indica un teléfono de contacto para que podamos llamarte si hace falta.');
      return;
    }

    state.pendingOrderPayload = {
      customer_name: customerName,
      customer_id_number: customerIdNumber,
      customer_phone: customerPhone,
      delivery_address: deliveryAddress,
      latitude: latVal ? parseFloat(latVal) : null,
      longitude: lngVal ? parseFloat(lngVal) : null,
      notes: notes || null,
      items: state.cart.map(item => ({
        product_id: item.product_id,
        quantity: item.quantity
      }))
    };

    // El total que se muestra se calcula con los precios guardados en el
    // carrito. El servidor es la autoridad: al confirmar devuelve el total
    // definitivo y, si ha cambiado, se avisa al cliente.
    const totalAmount = state.cart.reduce((sum, item) => sum + (item.unit_price * item.quantity), 0);
    const summaryBox = document.getElementById('confirm-summary-box');
    if (summaryBox) {
      summaryBox.innerHTML = `
        <p><strong>Cliente:</strong> ${escapeHtml(customerName)}</p>
        <p><strong>Teléfono:</strong> ${escapeHtml(customerPhone)}</p>
        <p><strong>Entrega:</strong> ${escapeHtml(deliveryAddress)}</p>
        <p><strong>Ítems (${state.cart.length}):</strong> ${state.cart.map(i => `${i.quantity}x ${escapeHtml(i.product_name)}`).join(', ')}</p>
        ${latVal ? '<p style="color: var(--accent-green);"><strong>GPS:</strong> Ubicación adjunta ✅</p>' : ''}
        <p style="font-size: 1.1rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total a Pagar: $${totalAmount.toFixed(2)}</p>
      `;
    }

    document.getElementById('modal-confirm-order').classList.add('active');
  });
}

function initConfirmModal() {
  const confirmModal = document.getElementById('modal-confirm-order');
  const btnClose = document.getElementById('btn-close-confirm');
  const btnCancel = document.getElementById('btn-cancel-confirm');
  const btnFinalConfirm = document.getElementById('btn-final-confirm');

  const closeFn = () => confirmModal.classList.remove('active');

  if (btnClose) btnClose.addEventListener('click', closeFn);
  if (btnCancel) btnCancel.addEventListener('click', closeFn);

  if (btnFinalConfirm) {
    btnFinalConfirm.addEventListener('click', async () => {
      if (!state.pendingOrderPayload) return;

      btnFinalConfirm.disabled = true;
      btnFinalConfirm.textContent = '⏳ Procesando...';

      // Ventana abierta de forma SÍNCRONA, dentro del clic.
      // Si se abriera después del fetch (dentro del then), el navegador la
      // bloquearía por no haber gesto del usuario y el cliente nunca vería el
      // WhatsApp, aunque el pedido sí quedara registrado.
      const waWindow = window.open('', '_blank');

      try {
        const res = await fetch(`${API_BASE}/orders`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(state.pendingOrderPayload)
        });

        if (!res.ok) {
          const errText = await res.text();
          if (waWindow) waWindow.close();
          throw new Error(errText || 'Error al guardar el pedido');
        }

        const orderData = await res.json();

        if (orderData.whatsapp_url) {
          if (waWindow) {
            waWindow.location.href = orderData.whatsapp_url;
          } else {
            // Si el navegador bloqueó la ventana, al menos queda un enlace
            // visible para que el cliente abra el chat manualmente.
            showWhatsAppFallbackLink(orderData.whatsapp_url);
          }
        } else if (waWindow) {
          waWindow.close();
        }

        // El total que devuelve el servidor es el definitivo: si el precio
        // cambió entre que se armó el carrito y se confirmó, se avisa.
        const serverTotal = Number(orderData.total_amount);
        const cartTotal = state.cart.reduce(
          (sum, item) => sum + (item.unit_price * item.quantity), 0
        );

        state.cart = [];
        saveCart();
        updateCartUI();

        confirmModal.classList.remove('active');
        document.getElementById('modal-cart').classList.remove('active');
        document.getElementById('form-checkout').reset();

        if (Math.abs(serverTotal - cartTotal) > 0.009) {
          showToast(`⚠️ El precio cambió: el total final es $${serverTotal.toFixed(2)}`);
        } else {
          showToast('🚀 ¡Pedido confirmado! Registrado y enviado por WhatsApp.');
        }

      } catch (err) {
        alert(`Error al confirmar el pedido: ${err.message}`);
      } finally {
        btnFinalConfirm.disabled = false;
        btnFinalConfirm.textContent = '✅ Sí, Confirmar';
      }
    });
  }
}

/** Enlace visible por si el navegador bloqueó la ventana de WhatsApp. */
function showWhatsAppFallbackLink(url) {
  const existing = document.getElementById('wa-fallback');
  if (existing) existing.remove();

  const box = document.createElement('div');
  box.id = 'wa-fallback';
  box.className = 'modal-overlay active';
  box.innerHTML = `
    <div class="modal-content" style="max-width: 440px;">
      <div class="modal-header">
        <h3 class="modal-title">📱 Tu pedido está registrado</h3>
        <button class="btn-close" aria-label="Cerrar">&times;</button>
      </div>
      <p style="color: var(--text-muted); line-height: 1.6;">
        Tu pedido se guardó correctamente, pero el navegador bloqueó la apertura
        automática de WhatsApp. Pulsa el botón para enviarlo a la pizzería.
      </p>
      <a href="${escapeHtml(url)}" target="_blank" rel="noopener noreferrer" class="btn-submit" style="display:block; text-align:center; margin-top:1rem;">
        💬 Abrir WhatsApp
      </a>
    </div>
  `;

  const close = () => box.remove();
  box.querySelector('.btn-close').addEventListener('click', close);
  box.addEventListener('click', (e) => { if (e.target === box) close(); });
  document.body.appendChild(box);
}

// Utility: Escape HTML
function escapeHtml(str) {
  if (str === null || str === undefined) return '';
  return String(str).replace(/[&<>"']/g, match => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  }[match]));
}