// Pizzería Los Herrera - JavaScript para Clientes (Landing, Pedidos, GPS & WhatsApp)

const API_BASE = '/api';

// App State
let state = {
  products: [],
  cart: JSON.parse(localStorage.getItem('pizzeria_cart') || '[]'),
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

// Helper: Toast Notifications
function showToast(message) {
  const toast = document.getElementById('toast-msg');
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add('show');
  setTimeout(() => {
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
    <div class="product-card ${p.image ? 'has-image' : ''}" ${p.image ? `style="background-image: url('/uploads/${escapeHtml(p.image)}');"` : ''} ${p.image ? `onclick="openImageModal('/uploads/${escapeHtml(p.image)}')"` : ''}>
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
          onclick="event.stopPropagation(); addToCart('${p.id}')"
          ${!p.available ? 'disabled' : ''}
        >
          Agregar
        </button>
      </div>
    </div>
  `).join('');
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
  }

  saveCart();
  updateCartUI();
}

function saveCart() {
  localStorage.setItem('pizzeria_cart', JSON.stringify(state.cart));
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
            <button class="btn-qty" onclick="updateCartQuantity('${item.product_id}', -1)">-</button>
            <span style="font-weight: 600; min-width: 20px; text-align: center;">${item.quantity}</span>
            <button class="btn-qty" onclick="updateCartQuantity('${item.product_id}', 1)">+</button>
          </div>
        </div>
      `).join('');
    }
  }
}

// 4. Cart Modal Events
function initCartModal() {
  const modal = document.getElementById('modal-cart');
  const btnOpen = document.getElementById('btn-open-cart');
  const btnClose = document.getElementById('btn-close-cart');

  if (btnOpen && modal) {
    btnOpen.addEventListener('click', () => modal.classList.add('active'));
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
    const deliveryAddress = document.getElementById('cust-address').value.trim();
    const latVal = document.getElementById('cust-lat').value;
    const lngVal = document.getElementById('cust-lng').value;
    const notes = document.getElementById('cust-notes').value.trim();

    const customerIdNumber = document.getElementById('cust-id-number').value.trim();

    state.pendingOrderPayload = {
      customer_name: customerName,
      customer_id_number: customerIdNumber,
      customer_phone: '',
      delivery_address: deliveryAddress,
      latitude: latVal ? parseFloat(latVal) : null,
      longitude: lngVal ? parseFloat(lngVal) : null,
      notes: notes || null,
      items: state.cart.map(item => ({
        product_id: item.product_id,
        quantity: item.quantity
      }))
    };

    // Render Order Confirmation Summary Box
    const totalAmount = state.cart.reduce((sum, item) => sum + (item.unit_price * item.quantity), 0);
    const summaryBox = document.getElementById('confirm-summary-box');
    if (summaryBox) {
      summaryBox.innerHTML = `
        <p><strong>Cliente:</strong> ${escapeHtml(customerName)}</p>
        <p><strong>Entrega:</strong> ${escapeHtml(deliveryAddress)}</p>
        <p><strong>Ítems (${state.cart.length}):</strong> ${state.cart.map(i => `${i.quantity}x ${escapeHtml(i.product_name)}`).join(', ')}</p>
        ${latVal ? '<p style="color: var(--accent-green);"><strong>GPS:</strong> Ubicación adjunta ✅</p>' : ''}
        <p style="font-size: 1.1rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total a Pagar: $${totalAmount.toFixed(2)}</p>
      `;
    }

    // Open Security Confirmation Modal
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

      try {
        // PARALELO: 1. Guardar en Base de Datos (Panel Admin) + 2. Abrir WhatsApp Pizzería
        const res = await fetch(`${API_BASE}/orders`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(state.pendingOrderPayload)
        });

        if (!res.ok) {
          const errText = await res.text();
          throw new Error(errText || 'Error al guardar el pedido');
        }

        const orderData = await res.json();

        // 1. Abrir WhatsApp dirigido a la pizzería en paralelo
        if (orderData.whatsapp_url) {
          window.open(orderData.whatsapp_url, '_blank');
        }

        // 2. Limpiar carrito y cerrar ventanas
        state.cart = [];
        saveCart();
        updateCartUI();

        confirmModal.classList.remove('active');
        document.getElementById('modal-cart').classList.remove('active');
        document.getElementById('form-checkout').reset();

        showToast('🚀 ¡Pedido confirmado! Registrado en la pizzería y enviado por WhatsApp.');

      } catch (err) {
        alert(`Error al confirmar el pedido: ${err.message}`);
      } finally {
        btnFinalConfirm.disabled = false;
        btnFinalConfirm.textContent = '✅ Sí, Confirmar';
      }
    });
  }
}

// Utility: Escape HTML
function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/[&<>"']/g, match => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  }[match]));
}
