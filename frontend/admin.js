// Pizzería Los Herrera - JavaScript para Administrador (CRUD de Productos y Pedidos)

const API_BASE = '/api';
const TOKEN_KEY = 'admin_token';
const ORDERS_REFRESH_MS = 20000;

let adminState = {
  products: [],
  orders: [],
  pendingCompleteOrderId: null,
  pendingDeleteOrderId: null,
  orderSearchQuery: '',
  activeOrderTab: 'pending',
  knownOrderIds: new Set(),
  ordersTimer: null,
  lastAnnouncedCount: 0
};

document.addEventListener('DOMContentLoaded', () => {
  initLogin();
  initAdminTabs();
  initOrderSubTabs();
  initModals();
  initAdminForms();
  initImageUploadAreas();
  initOrdersSearch();
  initSafeActions();

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
    stopOrdersPolling();
    showLoginView();
    throw new Error('Sesión expirada. Vuelve a iniciar sesión.');
  }
  return res;
}

function showLoginView() {
  stopOrdersPolling();
  document.getElementById('admin-login-view').hidden = false;
  document.getElementById('admin-panel').hidden = true;
}

function showAdminPanel() {
  document.getElementById('admin-login-view').hidden = true;
  document.getElementById('admin-panel').hidden = false;
  loadAdminProducts();
  loadAdminOrders();
  startOrdersPolling();
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
      adminState.knownOrderIds = new Set();
      adminState.lastAnnouncedCount = 0;
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
    stopOrdersPolling();
    showLoginView();
    showToast('Sesión cerrada');
  });
}

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

/* ------------------------------------------------------------------
 * Delegación de eventos segura.
 *
 * Antes los botones usaban atributos onclick="" con datos del pedido
 * interpolados dentro de la cadena. Como `escapeHtml` convierte `'` en
 * `&#39;` y el parser HTML vuelve a decodificar esa entidad a `'`, un
 * cliente podía cerrar la cadena de JavaScript con su nombre y ejecutar
 * código en el navegador del admin (XSS almacenado).
 *
 * Ahora los botones solo llevan un `data-id` (UUID) y el nombre se busca
 * en `adminState.orders`, por lo que ningún dato del cliente llega al HTML.
 * ------------------------------------------------------------------ */

function initSafeActions() {
  document.addEventListener('click', (event) => {
    const actionEl = event.target.closest('[data-action]');
    if (!actionEl) return;

    const id = actionEl.getAttribute('data-id');
    switch (actionEl.getAttribute('data-action')) {
      case 'edit-product':
        openEditProductModal(id);
        break;
      case 'toggle-product':
        toggleProductAvailability(id, actionEl.getAttribute('data-available') === 'true');
        break;
      case 'delete-product':
        deleteProduct(id);
        break;
      case 'change-status':
        updateOrderStatus(id, actionEl.getAttribute('data-status'));
        break;
      case 'complete-order':
        completeOrder(id);
        break;
      case 'delete-order':
        deleteOrder(id);
        break;
    }
  });
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
    <div class="product-card ${p.image ? 'has-image' : ''}" ${p.image ? `style="background-image: url('/uploads/${encodeURIComponent(p.image)}');"` : ''}>
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
          <button type="button" class="btn-action admin-product-action" data-action="edit-product" data-id="${escapeHtml(p.id)}">
            ✏️ Editar
          </button>
          <button type="button" class="btn-action admin-product-action ${p.available ? 'is-pause' : 'is-activate'}" data-action="toggle-product" data-id="${escapeHtml(p.id)}" data-available="${!p.available}">
            ${p.available ? '⏸️ Pausar' : '▶️ Activar'}
          </button>
        </div>

        <button type="button" class="btn-action admin-product-action is-delete" data-action="delete-product" data-id="${escapeHtml(p.id)}">
          🗑️ Eliminar Producto
        </button>
      </div>
    </div>
  `).join('');
}

async function toggleProductAvailability(id, newStatus) {
  try {
    const res = await adminFetch(`${API_BASE}/products/${encodeURIComponent(id)}`, {
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

  const clearBox = document.getElementById('edit-prod-clear-image');
  if (clearBox) {
    clearBox.checked = false;
    clearBox.disabled = !product.image;
    clearBox.closest('label')?.style.setProperty('display', product.image ? 'flex' : 'none');
  }

  const preview = document.getElementById('edit-image-preview');
  if (product.image) {
    preview.innerHTML = `<img src="/uploads/${encodeURIComponent(product.image)}" style="width: 100%; height: 160px; object-fit: cover; border-radius: 8px;">`;
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
    const res = await adminFetch(`${API_BASE}/products/${encodeURIComponent(id)}`, {
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

  const file = fileInput.files[0];
  if (file.size > 5 * 1024 * 1024) {
    return Promise.reject(new Error('La imagen no puede superar los 5 MB'));
  }

  const formData = new FormData();
  formData.append('image', file);

  return adminFetch(`${API_BASE}/products/${encodeURIComponent(productId)}/image`, {
    method: 'POST',
    body: formData
  }).then(res => {
    if (!res.ok) return res.text().then(t => { throw new Error(t || 'Error al subir imagen'); });
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

function resetImagePreview(previewId) {
  const preview = document.getElementById(previewId);
  if (!preview) return;
  preview.innerHTML = `
    <span style="font-size: 1.5rem;">📷</span>
    <span>Seleccionar imagen</span>
    <span style="font-size: 0.75rem; color: var(--text-dim);">JPG, PNG, WebP (max 5MB)</span>
  `;
  preview.classList.add('image-upload-placeholder');
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
      if (!Number.isFinite(price) || price <= 0) {
        throw new Error('El precio debe ser un número mayor que cero');
      }

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

      createForm.reset();
      resetImagePreview('create-image-preview');
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
    const submitButton = e.target.querySelector('button[type="submit"]');
    if (submitButton) submitButton.disabled = true;

    const id = document.getElementById('edit-prod-id').value;
    const name = document.getElementById('edit-prod-name').value.trim();
    const price = parseFloat(document.getElementById('edit-prod-price').value);
    const category = document.getElementById('edit-prod-category').value;
    const description = document.getElementById('edit-prod-desc').value.trim();
    const available = document.getElementById('edit-prod-available').checked;
    const clearImage = document.getElementById('edit-prod-clear-image')?.checked || false;

    try {
      if (!Number.isFinite(price) || price <= 0) {
        throw new Error('El precio debe ser un número mayor que cero');
      }

      const res = await adminFetch(`${API_BASE}/products/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, price, category, description, available, clear_image: clearImage })
      });

      if (!res.ok) throw new Error('Error al actualizar producto');

      if (clearImage) {
        resetImagePreview('edit-image-preview');
      } else if (document.getElementById('edit-prod-image').files.length) {
        await uploadProductImage(id, 'edit-prod-image');
      }

      document.getElementById('modal-edit-product').classList.remove('active');
      showToast('Producto actualizado correctamente');
      loadAdminProducts();
    } catch (err) {
      alert(`Error: ${err.message}`);
    } finally {
      if (submitButton) submitButton.disabled = false;
    }
  });
}

async function loadAdminOrders() {
  const container = document.getElementById('admin-orders-container');
  if (!container) return;
  // Si el usuario está escribiendo una búsqueda, no se recarga la lista:
  // perder el foco mientras se teclea es muy molesto.
  if (document.activeElement?.id === 'orders-search-id') return;

  try {
    const res = await adminFetch(`${API_BASE}/orders?limit=200`);
    if (!res.ok) throw new Error('Error al obtener pedidos');
    adminState.orders = await res.json();
    announceNewOrders();
    renderAdminOrders();
  } catch (err) {
    console.error(err);
    if (err.message.includes('Sesión expirada')) return;
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">⚠️</div>
        <p>Error al cargar la lista de pedidos.</p>
      </div>
    `;
  }
}

/* ------------------------------------------------------------------
 * Actualización automática de pedidos.
 *
 * Antes había que pulsar "Actualizar" a mano, y en un negocio real eso
 * significaba pedidos que nadie veía hasta que el cliente reclamaba.
 * ------------------------------------------------------------------ */

function startOrdersPolling() {
  stopOrdersPolling();
  adminState.ordersTimer = setInterval(() => {
    if (!getToken()) return;
    if (document.hidden) return;
    loadAdminOrders();
  }, ORDERS_REFRESH_MS);
}

function stopOrdersPolling() {
  if (adminState.ordersTimer) {
    clearInterval(adminState.ordersTimer);
    adminState.ordersTimer = null;
  }
}

function announceNewOrders() {
  const active = adminState.orders.filter(o => o.status === 'Pendiente');
  const ids = new Set(adminState.orders.map(o => o.id));

  // Primera carga tras abrir el panel: se memoriza sin avisar.
  if (adminState.knownOrderIds.size === 0) {
    adminState.knownOrderIds = ids;
    adminState.lastAnnouncedCount = active.length;
    return;
  }

  const nuevos = active.filter(o => !adminState.knownOrderIds.has(o.id));

  if (nuevos.length > 0) {
    const plural = nuevos.length === 1 ? 'Hay 1 pedido nuevo' : `Hay ${nuevos.length} pedidos nuevos`;
    showToast(`🔔 ${plural} — revisa la pestaña Pedidos`);
    playNewOrderSound();

    if (document.getElementById('tab-orders') && !document.getElementById('tab-orders').classList.contains('active')) {
      const badge = document.getElementById('orders-badge');
      if (badge) {
        badge.textContent = nuevos.length;
        badge.hidden = false;
      }
    }
  }

  adminState.knownOrderIds = ids;
  adminState.lastAnnouncedCount = active.length;
}

function playNewOrderSound() {
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.frequency.value = 880;
    osc.type = 'sine';
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.2, ctx.currentTime + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.35);
    osc.start();
    osc.stop(ctx.currentTime + 0.36);
    setTimeout(() => ctx.close(), 600);
  } catch (_) { /* el audio es un extra, nunca debe romper nada */ }
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

  const badge = document.getElementById('orders-badge');
  if (badge && !badge.hidden) {
    const nuevosPendientes = adminState.orders.filter(
      o => o.status === 'Pendiente' && !adminState.knownOrderIds.has(o.id)
    );
    if (nuevosPendientes.length === 0) badge.hidden = true;
  }

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
          ? `No hay pedidos que coincidan con <strong>${escapeHtml(query)}</strong> en ${escapeHtml(tabLabels[adminState.activeOrderTab])}.`
          : `No hay pedidos ${escapeHtml(tabLabels[adminState.activeOrderTab])}.`
        }</p>
      </div>
    `;
    return;
  }

  container.innerHTML = filtered.map(o => {
    // API aplana OrderWithItems (serde flatten): campos del pedido están en la raíz
    const items = o.items || [];
    const id = escapeHtml(o.id);

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
            <span class="order-id">Pedido #${escapeHtml(o.id.slice(0, 8))}</span>
            <span class="order-date"> • ${escapeHtml(formatDate(o.created_at))}</span>
          </div>
          <span class="status-badge ${statusClass}">${escapeHtml(o.status)}</span>
        </div>

        <div>
          <p><strong>Cliente:</strong> ${escapeHtml(o.customer_name)}</p>
          <p><strong>Teléfono:</strong> ${o.customer_phone
            ? `<a href="tel:${escapeHtml(o.customer_phone)}" style="color: var(--primary);">${escapeHtml(o.customer_phone)}</a>`
            : '<span style="color: var(--text-dim);">no indicado</span>'}</p>
          <p><strong>CI:</strong> ${escapeHtml(o.customer_id_number || '—')}</p>
          <p><strong>Dirección:</strong> ${escapeHtml(o.delivery_address)}</p>
          <p><strong>Detalle:</strong> ${itemsSummary}</p>
          ${o.notes ? `<p style="color: var(--accent-amber); font-size: 0.85rem; margin-top: 0.2rem;">📝 ${escapeHtml(o.notes)}</p>` : ''}
          <p style="font-size: 1.15rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total: $${o.total_amount.toFixed(2)}</p>
        </div>

        <div class="order-actions">
          <select class="form-select" style="width: auto; padding: 0.35rem 0.7rem; font-size: 0.85rem;" data-action="change-status" data-id="${id}" data-status="${escapeHtml(o.status)}">
            ${['Pendiente', 'En Preparación', 'En Camino', 'Entregado'].map(s =>
              `<option value="${escapeHtml(s)}" ${o.status === s ? 'selected' : ''}>${escapeHtml(s)}</option>`
            ).join('')}
          </select>

          ${o.status !== 'Entregado' ? `
            <button type="button" class="btn-action btn-complete" data-action="complete-order" data-id="${id}">
              Completar pedido
            </button>
          ` : `
            <button type="button" class="btn-delete-order" data-action="delete-order" data-id="${id}">
              🗑️ Eliminar
            </button>
          `}

          ${o.customer_phone ? `
            <a href="https://wa.me/${encodeURIComponent(o.customer_phone.replace(/[^0-9]/g, ''))}" target="_blank" rel="noopener noreferrer" class="btn-action btn-whatsapp">
              💬 WhatsApp cliente
            </a>
          ` : ''}

          ${o.google_maps_url ? `
            <a href="${escapeHtml(o.google_maps_url)}" target="_blank" rel="noopener noreferrer" class="btn-action btn-maps">
              📍 Ver en Mapa
            </a>
          ` : ''}
        </div>
      </div>
    `;
  }).join('');
}

function formatDate(iso) {
  try {
    return new Date(iso).toLocaleString([], {
      day: '2-digit', month: '2-digit', year: '2-digit',
      hour: '2-digit', minute: '2-digit'
    });
  } catch (_) {
    return iso;
  }
}

async function updateOrderStatus(orderId, newStatus) {
  if (!newStatus) return;
  try {
    const res = await adminFetch(`${API_BASE}/orders/${encodeURIComponent(orderId)}/status`, {
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

function renderOrderSummaryBox(boxId, order, includeStatus) {
  const itemsSummary = (order.items || [])
    .map(i => `${i.quantity}x ${escapeHtml(i.product_name)}`)
    .join(', ');

  const box = document.getElementById(boxId);
  if (!box) return;

  box.innerHTML = `
    <p><strong>Pedido:</strong> #${escapeHtml(order.id.slice(0, 8))}</p>
    <p><strong>Cliente:</strong> ${escapeHtml(order.customer_name)}</p>
    <p><strong>Teléfono:</strong> ${order.customer_phone ? escapeHtml(order.customer_phone) : '—'}</p>
    <p><strong>CI:</strong> ${escapeHtml(order.customer_id_number || '—')}</p>
    <p><strong>Dirección:</strong> ${escapeHtml(order.delivery_address)}</p>
    <p><strong>Detalle:</strong> ${itemsSummary || '—'}</p>
    ${includeStatus ? `<p><strong>Estado actual:</strong> ${escapeHtml(order.status)}</p>` : ''}
    <p style="font-size: 1.05rem; font-weight: 700; color: var(--primary); margin-top: 0.4rem;">Total: $${Number(order.total_amount).toFixed(2)}</p>
  `;
}

function completeOrder(orderId) {
  const order = adminState.orders.find(o => o.id === orderId);
  if (!order) return;

  adminState.pendingCompleteOrderId = orderId;
  renderOrderSummaryBox('complete-summary-box', order, true);
  document.getElementById('modal-complete-order')?.classList.add('active');
}

function deleteOrder(orderId) {
  const order = adminState.orders.find(o => o.id === orderId);
  if (!order) return;

  adminState.pendingDeleteOrderId = orderId;
  renderOrderSummaryBox('delete-summary-box', order, false);
  document.getElementById('modal-delete-order')?.classList.add('active');
}

async function executeDeleteOrder(orderId) {
  try {
    const res = await adminFetch(`${API_BASE}/orders/${encodeURIComponent(orderId)}`, {
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
  if (str === null || str === undefined) return '';
  return String(str).replace(/[&<>"']/g, match => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  }[match]));
}