// Pizzería Los Herrera - JavaScript para Administrador (CRUD de Productos y Pedidos)

const API_BASE = '/api';
const TOKEN_KEY = 'admin_token';

let adminState = {
  products: [],
  orders: [],
  pendingCompleteOrderId: null,
  pendingDeleteOrderId: null,
  orderSearchQuery: '',
  activeOrderTab: 'pending'
};

document.addEventListener('DOMContentLoaded', () => {
  initLogin();
  initAdminTabs();
  initOrderSubTabs();
  initModals();
  initAdminForms();
  initImageUploadAreas();
  initOrdersSearch();

  if (getToken()) {
    showAdminPanel();
  } else {
    showLoginView();
  }
});

function getToken() {
  return sessionStorage.getItem(TOKEN_KEY);
}

function setToken(token) {
  sessionStorage.setItem(TOKEN_KEY, token);
}

function clearToken() {
  sessionStorage.removeItem(TOKEN_KEY);
}

function authHeaders(extra = {}) {
  const headers = { ...extra };
  const token = getToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
}

async function adminFetch(url, options = {}) {
  const opts = { ...options };
  opts.headers = authHeaders(opts.headers || {});
  const res = await fetch(url, opts);
  if (res.status === 401) {
    clearToken();
    showLoginView();
    throw new Error('Sesión expirada. Vuelve a iniciar sesión.');
  }
  return res;
}

function showLoginView() {
  document.getElementById('admin-login-view').hidden = false;
  document.getElementById('admin-panel').hidden = true;
}

function showAdminPanel() {
  document.getElementById('admin-login-view').hidden = true;
  document.getElementById('admin-panel').hidden = false;
  loadAdminProducts();
  loadAdminOrders();
}

function initLogin() {
  document.getElementById('form-admin-login').addEventListener('submit', async (e) => {
    e.preventDefault();
    const errorEl = document.getElementById('login-error');
    errorEl.hidden = true;

    const username = document.getElementById('login-username').value.trim();
    const password = document.getElementById('login-password').value;

    try {
      const res = await fetch(`${API_BASE}/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password })
      });

      if (!res.ok) {
        const msg = await res.text();
        throw new Error(msg || 'Credenciales incorrectas');
      }

      const data = await res.json();
      setToken(data.token);
      document.getElementById('form-admin-login').reset();
      showAdminPanel();
      showToast('Sesión iniciada');
    } catch (err) {
      errorEl.textContent = err.message || 'No se pudo iniciar sesión';
      errorEl.hidden = false;
    }
  });

  document.getElementById('btn-logout')?.addEventListener('click', async () => {
    try {
      await fetch(`${API_BASE}/auth/logout`, {
        method: 'POST',
        headers: authHeaders()
      });
    } catch (_) { /* ignore */ }
    clearToken();
    showLoginView();
    showToast('Sesión cerrada');
  });
}

function showToast(message) {
  const toast = document.getElementById('toast-msg');
  if (!toast) return;
  toast.textContent = message;
  toast.classList.add('show');
  setTimeout(() => {
    toast.classList.remove('show');
  }, 3000);
}

function initAdminTabs() {
  const tabBtns = document.querySelectorAll('.tab-btn');
  const sections = document.querySelectorAll('.view-section');

  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const targetTab = btn.getAttribute('data-tab');

      tabBtns.forEach(b => b.classList.remove('active'));
      sections.forEach(s => s.classList.remove('active'));

      btn.classList.add('active');
      document.getElementById(targetTab).classList.add('active');

      if (targetTab === 'tab-orders') {
        loadAdminOrders();
      } else if (targetTab === 'tab-products') {
        loadAdminProducts();
      }
    });
  });

  document.getElementById('btn-refresh-orders')?.addEventListener('click', () => {
    loadAdminOrders();
  });
}

function initOrderSubTabs() {
  const pills = document.querySelectorAll('#order-tab-pills .pill-btn');
  pills.forEach(pill => {
    pill.addEventListener('click', () => {
      pills.forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      adminState.activeOrderTab = pill.getAttribute('data-order-tab');
      renderAdminOrders();
    });
  });
}

function initModals() {
  const createModal = document.getElementById('modal-create-product');
  const btnOpenCreate = document.getElementById('btn-open-create-modal');
  const btnCloseCreate = document.getElementById('btn-close-create-modal');

  if (btnOpenCreate && createModal) {
    btnOpenCreate.addEventListener('click', () => createModal.classList.add('active'));
  }
  if (btnCloseCreate && createModal) {
    btnCloseCreate.addEventListener('click', () => createModal.classList.remove('active'));
  }

  const editModal = document.getElementById('modal-edit-product');
  const btnCloseEdit = document.getElementById('btn-close-edit-modal');

  if (btnCloseEdit && editModal) {
    btnCloseEdit.addEventListener('click', () => editModal.classList.remove('active'));
  }

  const completeModal = document.getElementById('modal-complete-order');
  const closeComplete = () => {
    completeModal?.classList.remove('active');
    adminState.pendingCompleteOrderId = null;
  };

  document.getElementById('btn-close-complete')?.addEventListener('click', closeComplete);
  document.getElementById('btn-cancel-complete')?.addEventListener('click', closeComplete);
  document.getElementById('btn-final-complete')?.addEventListener('click', async () => {
    const orderId = adminState.pendingCompleteOrderId;
    if (!orderId) return;
    closeComplete();
    await updateOrderStatus(orderId, 'Entregado');
  });

  const deleteModal = document.getElementById('modal-delete-order');
  const closeDelete = () => {
    deleteModal?.classList.remove('active');
    adminState.pendingDeleteOrderId = null;
  };

  document.getElementById('btn-close-delete')?.addEventListener('click', closeDelete);
  document.getElementById('btn-cancel-delete')?.addEventListener('click', closeDelete);
  document.getElementById('btn-final-delete')?.addEventListener('click', async () => {
    const orderId = adminState.pendingDeleteOrderId;
    if (!orderId) return;
    closeDelete();
    await executeDeleteOrder(orderId);
  });

  [createModal, editModal, completeModal, deleteModal].forEach(m => {
    if (m) {
      m.addEventListener('click', (e) => {
        if (e.target === m) {
          m.classList.remove('active');
          if (m === completeModal) adminState.pendingCompleteOrderId = null;
          if (m === deleteModal) adminState.pendingDeleteOrderId = null;
        }
      });
    }
  });
}

async function loadAdminProducts() {
  const container = document.getElementById('admin-products-container');
  try {
    const res = await fetch(`${API_BASE}/products`);
    if (!res.ok) throw new Error('Error al cargar productos');
    adminState.products = await res.json();
    renderAdminProducts();
  } catch (err) {
    console.error(err);
    if (container) {
      container.innerHTML = `
        <div class="empty-state" style="grid-column: 1 / -1;">
          <div class="empty-state-icon">⚠️</div>
          <p>Error al conectar con la base de datos de productos.</p>
        </div>
      `;
    }
  }
}

function renderAdminProducts() {
  const container = document.getElementById('admin-products-container');
  if (!container) return;

  if (adminState.products.length === 0) {
    container.innerHTML = `
      <div class="empty-state" style="grid-column: 1 / -1;">
        <div class="empty-state-icon">🍕</div>
        <p>No hay productos registrados en el menú. Haz clic en "Crear Nuevo Producto".</p>
      </div>
    `;
    return;
  }

  container.innerHTML = adminState.products.map(p => `
    <div class="product-card ${p.image ? 'has-image' : ''}" ${p.image ? `style="background-image: url('/uploads/${escapeHtml(p.image)}');"` : ''}>
      ${p.image ? '<div class="product-card-overlay"></div>' : ''}
      <span class="product-badge ${p.available ? 'badge-available' : 'badge-unavailable'}">
        ${p.available ? 'Disponible' : 'Agotado'}
      </span>

      <div ${p.image ? 'class="product-card-content"' : ''}>
        <span class="product-category">${escapeHtml(p.category)}</span>
        <h3 class="product-title">${escapeHtml(p.name)}</h3>
        <p class="product-desc">${escapeHtml(p.description || 'Sin descripción.')}</p>
        <p style="font-size: 1.3rem; font-weight: 700; color: var(--text-main); margin-bottom: 0.75rem;">$${p.price.toFixed(2)}</p>
      </div>

      <div class="admin-product-actions">
        <div class="admin-product-actions-grid">
          <button class="btn-action admin-product-action" onclick="openEditProductModal('${p.id}')">
            ✏️ Editar
          </button>
          <button class="btn-action admin-product-action ${p.available ? 'is-pause' : 'is-activate'}" onclick="toggleProductAvailability('${p.id}', ${!p.available})">
            ${p.available ? '⏸️ Pausar' : '▶️ Activar'}
          </button>
        </div>

        <button class="btn-action admin-product-action is-delete" onclick="deleteProduct('${p.id}')">
          🗑️ Eliminar Producto
        </button>
      </div>
    </div>
  `).join('');
}

async function toggleProductAvailability(id, newStatus) {
  try {
    const res = await adminFetch(`${API_BASE}/products/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ available: newStatus })
    });

    if (!res.ok) throw new Error('Error al actualizar disponibilidad');

    showToast(`📌 Estado actualizado a ${newStatus ? '"Disponible"' : '"Agotado"'}`);
    loadAdminProducts();
  } catch (err) {
    alert(`Error: ${err.message}`);
  }
}

function openEditProductModal(id) {
  const product = adminState.products.find(p => p.id === id);
  if (!product) return;

  document.getElementById('edit-prod-id').value = product.id;
  document.getElementById('edit-prod-name').value = product.name;
  document.getElementById('edit-prod-price').value = product.price;
  document.getElementById('edit-prod-category').value = product.category;
  document.getElementById('edit-prod-desc').value = product.description;
  document.getElementById('edit-prod-available').checked = product.available;

  const preview = document.getElementById('edit-image-preview');
  if (product.image) {
    preview.innerHTML = `<img src="/uploads/${escapeHtml(product.image)}" style="width: 100%; height: 160px; object-fit: cover; border-radius: 8px;">`;
    preview.classList.remove('image-upload-placeholder');
  } else {
    preview.innerHTML = `
      <span style="font-size: 1.5rem;">📷</span>
      <span>Seleccionar imagen</span>
      <span style="font-size: 0.75rem; color: var(--text-dim);">JPG, PNG, WebP (max 5MB)</span>
    `;
    preview.classList.add('image-upload-placeholder');
  }

  document.getElementById('modal-edit-product').classList.add('active');
}

async function deleteProduct(id) {
  const product = adminState.products.find(item => item.id === id);
  const productName = product ? product.name : 'este producto';

  if (!confirm(`¿Estás seguro de eliminar el producto "${productName}" del menú?`)) {
    return;
  }

  try {
    const res = await adminFetch(`${API_BASE}/products/${id}`, {
      method: 'DELETE'
    });

    if (!res.ok) throw new Error('Error al eliminar producto');

    showToast(`🗑️ Producto "${productName}" eliminado`);
    loadAdminProducts();
  } catch (err) {
    alert(`Error: ${err.message}`);
  }
}

function uploadProductImage(productId, fileInputId) {
  const fileInput = document.getElementById(fileInputId);
  if (!fileInput || !fileInput.files.length) return Promise.resolve();

  const formData = new FormData();
  formData.append('image', fileInput.files[0]);

  return adminFetch(`${API_BASE}/products/${productId}/image`, {
    method: 'POST',
    body: formData
  }).then(res => {
    if (!res.ok) throw new Error('Error al subir imagen');
    return res.json();
  });
}

function initImageUploadAreas() {
  const createArea = document.getElementById('create-image-upload-area');
  const createInput = document.getElementById('prod-image');
  const createPreview = document.getElementById('create-image-preview');

  if (createArea && createInput) {
    createArea.addEventListener('click', () => createInput.click());
    createInput.addEventListener('change', () => {
      if (createInput.files.length) {
        const url = URL.createObjectURL(createInput.files[0]);
        createPreview.innerHTML = `<img src="${url}" style="width: 100%; height: 160px; object-fit: cover; border-radius: 8px;">`;
        createPreview.classList.remove('image-upload-placeholder');
      }
    });
  }

  const editArea = document.getElementById('edit-image-upload-area');
  const editInput = document.getElementById('edit-prod-image');
  const editPreview = document.getElementById('edit-image-preview');

  if (editArea && editInput) {
    editArea.addEventListener('click', () => editInput.click());
    editInput.addEventListener('change', () => {
      if (editInput.files.length) {
        const url = URL.createObjectURL(editInput.files[0]);
        editPreview.innerHTML = `<img src="${url}" style="width: 100%; height: 160px; object-fit: cover; border-radius: 8px;">`;
        editPreview.classList.remove('image-upload-placeholder');
      }
    });
  }
}

function initAdminForms() {
  const createForm = document.getElementById('form-create-product');
  const createSubmitButton = createForm.querySelector('button[type="submit"]');

  createForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    if (createForm.dataset.submitting === 'true') return;

    createForm.dataset.submitting = 'true';
    createForm.setAttribute('aria-busy', 'true');
    createSubmitButton.disabled = true;
    createSubmitButton.dataset.originalText = createSubmitButton.textContent;
    createSubmitButton.textContent = 'Guardando producto...';

    const name = document.getElementById('prod-name').value.trim();
    const price = parseFloat(document.getElementById('prod-price').value);
    const category = document.getElementById('prod-category').value;
    const description = document.getElementById('prod-desc').value.trim();

    try {
      const res = await adminFetch(`${API_BASE}/products`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, price, category, description, available: true })
      });

      if (!res.ok) throw new Error('Error al crear producto');

      const product = await res.json();

      if (document.getElementById('prod-image').files.length) {
        await uploadProductImage(product.id, 'prod-image');
      }

      document.getElementById('form-create-product').reset();
      document.getElementById('create-image-preview').innerHTML = `
        <span style="font-size: 1.5rem;">📷</span>
        <span>Seleccionar imagen</span>
        <span style="font-size: 0.75rem; color: var(--text-dim);">JPG, PNG, WebP (max 5MB)</span>
      `;
      document.getElementById('create-image-preview').classList.add('image-upload-placeholder');
      document.getElementById('modal-create-product').classList.remove('active');
      showToast('Producto creado con éxito');
      loadAdminProducts();
    } catch (err) {
      alert(`Error: ${err.message}`);
    } finally {
      createForm.dataset.submitting = 'false';
      createForm.removeAttribute('aria-busy');
      createSubmitButton.disabled = false;
      createSubmitButton.textContent = createSubmitButton.dataset.originalText;
    }
  });

  document.getElementById('form-edit-product').addEventListener('submit', async (e) => {
    e.preventDefault();

    const id = document.getElementById('edit-prod-id').value;
    const name = document.getElementById('edit-prod-name').value.trim();
    const price = parseFloat(document.getElementById('edit-prod-price').value);
    const category = document.getElementById('edit-prod-category').value;
    const description = document.getElementById('edit-prod-desc').value.trim();
    const available = document.getElementById('edit-prod-available').checked;

    try {
      const res = await adminFetch(`${API_BASE}/products/${id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, price, category, description, available })
      });

      if (!res.ok) throw new Error('Error al actualizar producto');

      if (document.getElementById('edit-prod-image').files.length) {
        await uploadProductImage(id, 'edit-prod-image');
      }

      document.getElementById('modal-edit-product').classList.remove('active');
      showToast('Producto actualizado correctamente');
      loadAdminProducts();
    } catch (err) {
      alert(`Error: ${err.message}`);
    }
  });
}

async function loadAdminOrders() {
  const container = document.getElementById('admin-orders-container');
  if (!container) return;

  try {
    const res = await adminFetch(`${API_BASE}/orders`);
    if (!res.ok) throw new Error('Error al obtener pedidos');
    adminState.orders = await res.json();
    renderAdminOrders();
  } catch (err) {
    console.error(err);
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">⚠️</div>
        <p>Error al cargar la lista de pedidos.</p>
      </div>
    `;
  }
}

function initOrdersSearch() {
  const input = document.getElementById('orders-search-id');
  const clearBtn = document.getElementById('btn-clear-orders-search');
  if (!input) return;

  const syncClear = () => {
    if (clearBtn) clearBtn.hidden = !input.value.trim();
  };

  input.addEventListener('input', () => {
    adminState.orderSearchQuery = input.value.trim();
    syncClear();
    renderAdminOrders();
  });

  clearBtn?.addEventListener('click', () => {
    input.value = '';
    adminState.orderSearchQuery = '';
    syncClear();
    renderAdminOrders();
    input.focus();
  });
}

function normalizeOrderIdQuery(q) {
  return q.toLowerCase().replace(/[^a-f0-9]/g, '');
}

function normalizeDigits(q) {
  return q.replace(/\D/g, '');
}

function getFilteredOrders() {
  let filtered = adminState.orders;

  const tab = adminState.activeOrderTab;
  if (tab === 'pending') {
    filtered = filtered.filter(o => o.status === 'Pendiente' || o.status === 'En Preparación');
  } else if (tab === 'transit') {
    filtered = filtered.filter(o => o.status === 'En Camino');
  } else if (tab === 'delivered') {
    filtered = filtered.filter(o => o.status === 'Entregado');
  }

  const q = adminState.orderSearchQuery.trim();
  if (!q) return filtered;

  const lower = q.toLowerCase();
  const digitsNeedle = normalizeDigits(q);
  const looksLikeId = /^[a-f0-9-]+$/i.test(q);

  return filtered.filter(o => {
    const nameMatch = (o.customer_name || '').toLowerCase().includes(lower);

    const ci = o.customer_id_number || '';
    const ciMatch = ci.toLowerCase().includes(lower)
      || (digitsNeedle.length >= 3 && normalizeDigits(ci).includes(digitsNeedle));

    let idMatch = false;
    if (looksLikeId) {
      const idNeedle = normalizeOrderIdQuery(q);
      idMatch = idNeedle.length > 0 && (
        normalizeOrderIdQuery(o.id).includes(idNeedle)
        || o.id.toLowerCase().includes(lower)
        || o.id.slice(0, 8).toLowerCase().includes(lower)
      );
    }

    return idMatch || nameMatch || ciMatch;
  });
}

function renderAdminOrders() {
  const container = document.getElementById('admin-orders-container');
  if (!container) return;

  if (adminState.orders.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">📋</div>
        <p>No hay pedidos registrados aún.</p>
      </div>
    `;
    return;
  }

  const filtered = getFilteredOrders();
  const query = adminState.orderSearchQuery.trim();
  const tabLabels = { pending: 'recibidos/elaborando', transit: 'en camino', delivered: 'entregados' };

  if (filtered.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">🔍</div>
        <p>${query
          ? `No hay pedidos que coincidan con <strong>${escapeHtml(query)}</strong> en ${tabLabels[adminState.activeOrderTab]}.`
          : `No hay pedidos ${tabLabels[adminState.activeOrderTab]}.`
        }</p>
      </div>
    `;
    return;
  }

  container.innerHTML = filtered.map(o => {
    // API aplana OrderWithItems (serde flatten): campos del pedido están en la raíz
    const items = o.items || [];

    const statusClass = {
      'Pendiente': 'status-pendiente',
      'En Preparación': 'status-preparacion',
      'En Camino': 'status-camino',
      'Entregado': 'status-entregado'
    }[o.status] || 'status-pendiente';

    const itemsSummary = items.map(i => `${i.quantity}x ${escapeHtml(i.product_name)} ($${i.subtotal.toFixed(2)})`).join(', ');

    return `
      <div class="order-card">
        <div class="order-card-header">
          <div>
            <span class="order-id">Pedido #${o.id.slice(0, 8)}</span>
            <span class="order-date"> • ${new Date(o.created_at).toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}</span>
          </div>
          <span class="status-badge ${statusClass}">${escapeHtml(o.status)}</span>
        </div>

        <div>
          <p><strong>Cliente:</strong> ${escapeHtml(o.customer_name)}${o.customer_phone ? ` (${escapeHtml(o.customer_phone)})` : ''}</p>
          <p><strong>CI:</strong> ${escapeHtml(o.customer_id_number || '—')}</p>
          <p><strong>Dirección:</strong> ${escapeHtml(o.delivery_address)}</p>
          <p><strong>Detalle:</strong> ${itemsSummary}</p>
          ${o.notes ? `<p style="color: var(--accent-amber); font-size: 0.85rem; margin-top: 0.2rem;">📝 ${escapeHtml(o.notes)}</p>` : ''}
          <p style="font-size: 1.15rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total: $${o.total_amount.toFixed(2)}</p>
        </div>

        <div class="order-actions">
          <select class="form-select" style="width: auto; padding: 0.35rem 0.7rem; font-size: 0.85rem;" onchange="updateOrderStatus('${o.id}', this.value)">
            <option value="Pendiente" ${o.status === 'Pendiente' ? 'selected' : ''}>Pendiente</option>
            <option value="En Preparación" ${o.status === 'En Preparación' ? 'selected' : ''}>En Preparación</option>
            <option value="En Camino" ${o.status === 'En Camino' ? 'selected' : ''}>En Camino</option>
            <option value="Entregado" ${o.status === 'Entregado' ? 'selected' : ''}>Entregado</option>
          </select>

          ${o.status !== 'Entregado' ? `
            <button type="button" class="btn-action btn-complete" onclick="completeOrder('${o.id}')">
              Completar pedido
            </button>
          ` : `
            <button type="button" class="btn-delete-order" onclick="deleteOrder('${o.id}', '${escapeHtml(o.customer_name)}')">
              🗑️ Eliminar
            </button>
          `}

          ${o.whatsapp_url ? `
            <a href="${o.whatsapp_url}" target="_blank" class="btn-action btn-whatsapp">
              💬 Abrir WhatsApp
            </a>
          ` : ''}

          ${o.google_maps_url ? `
            <a href="${o.google_maps_url}" target="_blank" class="btn-action btn-maps">
              📍 Ver en Mapa
            </a>
          ` : ''}
        </div>
      </div>
    `;
  }).join('');
}

async function updateOrderStatus(orderId, newStatus) {
  try {
    const res = await adminFetch(`${API_BASE}/orders/${orderId}/status`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ status: newStatus })
    });

    if (!res.ok) throw new Error('Error al actualizar estado');

    showToast(`📌 Estado del pedido cambiado a "${newStatus}"`);
    loadAdminOrders();
  } catch (err) {
    alert(`Error: ${err.message}`);
  }
}

async function completeOrder(orderId) {
  const order = adminState.orders.find(o => o.id === orderId);
  if (!order) return;

  adminState.pendingCompleteOrderId = orderId;

  const items = order.items || [];
  const itemsSummary = items
    .map(i => `${i.quantity}x ${escapeHtml(i.product_name)}`)
    .join(', ');

  const summaryBox = document.getElementById('complete-summary-box');
  if (summaryBox) {
    summaryBox.innerHTML = `
      <p><strong>Pedido:</strong> #${escapeHtml(order.id.slice(0, 8))}</p>
      <p><strong>Cliente:</strong> ${escapeHtml(order.customer_name)}${order.customer_phone ? ` (${escapeHtml(order.customer_phone)})` : ''}</p>
      <p><strong>CI:</strong> ${escapeHtml(order.customer_id_number || '—')}</p>
      <p><strong>Dirección:</strong> ${escapeHtml(order.delivery_address)}</p>
      <p><strong>Detalle:</strong> ${itemsSummary || '—'}</p>
      <p><strong>Estado actual:</strong> ${escapeHtml(order.status)}</p>
      <p style="font-size: 1.05rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total: $${Number(order.total_amount).toFixed(2)}</p>
    `;
  }

  document.getElementById('modal-complete-order')?.classList.add('active');
}

async function deleteOrder(orderId, customerName) {
  const order = adminState.orders.find(o => o.id === orderId);
  if (!order) return;

  adminState.pendingDeleteOrderId = orderId;

  const items = order.items || [];
  const itemsSummary = items
    .map(i => `${i.quantity}x ${escapeHtml(i.product_name)}`)
    .join(', ');

  const summaryBox = document.getElementById('delete-summary-box');
  if (summaryBox) {
    summaryBox.innerHTML = `
      <p><strong>Pedido:</strong> #${escapeHtml(order.id.slice(0, 8))}</p>
      <p><strong>Cliente:</strong> ${escapeHtml(order.customer_name)}</p>
      <p><strong>Detalle:</strong> ${itemsSummary || '—'}</p>
      <p style="font-size: 1.05rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total: $${Number(order.total_amount).toFixed(2)}</p>
    `;
  }

  document.getElementById('modal-delete-order')?.classList.add('active');
}

async function executeDeleteOrder(orderId) {
  try {
    const res = await adminFetch(`${API_BASE}/orders/${orderId}`, {
      method: 'DELETE'
    });

    if (!res.ok) throw new Error('Error al eliminar el pedido');

    showToast('🗑️ Pedido eliminado permanentemente');
    loadAdminOrders();
  } catch (err) {
    alert(`Error: ${err.message}`);
  }
}

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
