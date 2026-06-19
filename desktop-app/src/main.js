// Whatszara Desktop App - Main
import QRCode from "qrcode";

// ── Navigation, Themes, Shortcuts ──
const views = ["dashboard", "chat", "providers", "permissions", "actions", "settings", "guide"];
const viewActions = {
  dashboard: () => { stopChatPolling(); refreshDashboard(); },
  permissions: () => { stopChatPolling(); refreshContactsTable(); },
  chat: () => { refreshChatContacts(); startChatPolling(); },
  actions: () => { stopChatPolling(); refreshActionLog(); },
  providers: () => { stopChatPolling(); refreshModels(); },
  settings: () => { stopChatPolling(); loadSettingsUI(); },
  guide: () => { stopChatPolling(); },
};

function showView(view) {
  if (!views.includes(view)) return;
  document.querySelectorAll(".nav-item").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.view === view);
  });
  document.querySelectorAll(".view").forEach((viewEl) => viewEl.classList.add("hidden"));
  document.getElementById(`view-${view}`)?.classList.remove("hidden");
  viewActions[view]?.();
}

document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => showView(btn.dataset.view));
});

document.addEventListener("click", (event) => {
  const btn = event.target.closest("button[data-view]");
  if (!btn || btn.classList.contains("nav-item")) return;
  showView(btn.dataset.view);
});

function applyTheme(theme) {
  const nextTheme = ["dark", "light", "vibrant"].includes(theme) ? theme : "dark";
  document.documentElement.dataset.theme = nextTheme;
  localStorage.setItem("whatszara-theme", nextTheme);
  document.querySelectorAll(".theme-btn").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.themeChoice === nextTheme);
  });
}

document.querySelectorAll(".theme-btn").forEach((btn) => {
  btn.addEventListener("click", () => applyTheme(btn.dataset.themeChoice));
});

function isTypingTarget(target) {
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target?.tagName) || target?.isContentEditable;
}

function getCurrentView() {
  return document.querySelector(".view:not(.hidden)")?.id?.replace("view-", "") || "dashboard";
}

function focusSearchOrReply() {
  const currentView = getCurrentView();
  const candidates = [
    currentView === "chat" && selectedChatJid ? "#chat-reply-input" : null,
    currentView === "chat" ? "#chat-search" : null,
    currentView === "permissions" ? "#contacts-search" : null,
    ".view:not(.hidden) input[type='text']",
    ".view:not(.hidden) textarea",
  ].filter(Boolean);
  const target = candidates.map((selector) => document.querySelector(selector)).find((el) => el && !el.closest(".hidden"));
  if (target) {
    target.focus();
    target.select?.();
  }
}

document.addEventListener("keydown", (event) => {
  const withCommand = event.metaKey || event.ctrlKey;
  const key = event.key.toLowerCase();

  if (withCommand && /^[1-7]$/.test(key)) {
    event.preventDefault();
    showView(views[Number(key) - 1]);
    return;
  }

  if (withCommand && key === "k") {
    event.preventDefault();
    focusSearchOrReply();
    return;
  }

  if (withCommand && key === "j") {
    event.preventDefault();
    showView("chat");
    return;
  }

  if (withCommand && key === "g") {
    event.preventDefault();
    showView("guide");
    return;
  }

  if (event.key === "?" && !isTypingTarget(event.target)) {
    event.preventDefault();
    showView("guide");
    return;
  }

  if (event.key === "Escape" && isTypingTarget(event.target)) {
    event.target.value = "";
    event.target.blur();
  }
});

// ── Tauri invoke helper ──
async function invoke(cmd, args = {}) {
  if (window.__TAURI_INTERNALS__) {
    const { invoke } = window.__TAURI_INTERNALS__;
    return invoke(cmd, args);
  }
  console.warn("Tauri not available, returning mock");
  return JSON.stringify({ success: false, error: "Not running in Tauri" });
}

// ── Bridge Polling ──
let bridgePollInterval = null;

async function pollBridge() {
  try {
    const raw = await invoke("check_bridge");
    const result = JSON.parse(raw);
    updateBridgeUI(result);
  } catch {
    updateBridgeUI({ status: "error", error: "Failed to check bridge status" });
  }
  refreshAutoReadStatus();
}

let lastQrCode = "";

function updateBridgeUI(status) {
  const badge = document.getElementById("status-badge");
  const indicator = document.getElementById("bridge-indicator");
  const spinner = document.getElementById("bridge-spinner");
  const icon = document.getElementById("bridge-icon");
  const statusText = document.getElementById("bridge-status-text");
  const errorDetail = document.getElementById("bridge-error-detail");
  const waStatus = document.getElementById("wa-status");
  const qrContainer = document.getElementById("qr-container");
  const qrCanvas = document.getElementById("qr-canvas");

  const stepBridge = document.getElementById("step-bridge");
  const stepProvider = document.getElementById("step-provider");
  const stepAllowlist = document.getElementById("step-allowlist");

  if (badge) {
    const labels = { stopped: "stopped", running: "starting\u2026", awaiting_scan: "scan QR", connected: "connected", error: "error" };
    badge.textContent = labels[status.status] || status.status;
    badge.classList.toggle("connected", status.status === "connected");
    badge.classList.toggle("error", status.status === "error");
  }

  if (waStatus) {
    const labels = { stopped: "Bridge Stopped", running: "Connecting\u2026", awaiting_scan: "Scan QR Code", connected: "Connected", error: "Bridge Error" };
    waStatus.textContent = labels[status.status] || "Unknown";
    waStatus.style.color = status.status === "connected" ? "var(--green)" : status.status === "error" ? "var(--red)" : "var(--yellow)";
  }

  if (indicator) {
    indicator.className = "step-indicator";
    indicator.classList.add(`step-${status.status}`);
  }
  if (spinner) spinner.classList.toggle("hidden", status.status !== "running" && status.status !== "starting" && status.status !== "awaiting_scan");
  if (icon) {
    const showIcon = status.status !== "running" && status.status !== "starting" && status.status !== "awaiting_scan";
    icon.classList.toggle("hidden", !showIcon);
    if (showIcon) {
      const icons = { stopped: "\u2715", connected: "\u2713", error: "\u2715" };
      icon.textContent = icons[status.status] || "?";
    }
  }

  if (status.status === "awaiting_scan" && status.qr && status.qr !== lastQrCode) {
    lastQrCode = status.qr;
    if (qrContainer) qrContainer.classList.remove("hidden");
    if (qrCanvas) {
      QRCode.toCanvas(qrCanvas, status.qr, { width: 280, margin: 2, color: { dark: "#000", light: "#fff" } });
    }
  }

  if (statusText) {
    const texts = {
      stopped: "Bridge is not running. Try restarting the app.",
      running: "Bridge process is running, waiting for QR code from WhatsApp\u2026",
      awaiting_scan: "Scan the QR code below with WhatsApp on your phone.",
      connected: "Bridge is connected to WhatsApp!",
      error: `Bridge failed: ${status.error || "Unknown error"}`,
    };
    statusText.textContent = texts[status.status] || "Unknown status";
  }

  if (errorDetail) {
    if (status.status === "error" && status.error) {
      errorDetail.textContent = status.error;
      errorDetail.classList.remove("hidden");
    } else {
      errorDetail.classList.add("hidden");
    }
  }

  if (stepBridge) stepBridge.classList.toggle("completed", status.status === "connected");
  if (stepProvider) stepProvider.classList.toggle("active", status.status === "connected");

  const logoutDiv = document.getElementById("bridge-logout");
  if (logoutDiv) logoutDiv.classList.toggle("hidden", status.status !== "connected");
}

// ── Auto-Read ──
async function toggleAutoRead(enabled) {
  if (enabled) {
    await invoke("start_auto_read");
  } else {
    await invoke("stop_auto_read");
  }
}

async function refreshAutoReadStatus() {
  try {
    const raw = await invoke("get_auto_read_status");
    const status = JSON.parse(raw);
    const pill = document.getElementById("auto-read-pill");
    const toggle = document.getElementById("auto-read-toggle");
    const statusText = document.getElementById("auto-read-status");
    if (pill) {
      pill.textContent = status.enabled ? "Running" : "Off";
      pill.className = "pill" + (status.enabled ? " pill-active" : "");
    }
    if (toggle && toggle.checked !== status.enabled) {
      toggle.checked = status.enabled;
    }
    if (statusText) {
      statusText.textContent = `Last rowid: ${status.last_rowid || 0}`;
    }
  } catch {}
}

document.getElementById("auto-read-toggle")?.addEventListener("change", async (e) => {
  await toggleAutoRead(e.target.checked);
  refreshAutoReadStatus();
});

// ── Dashboard ──
async function refreshDashboard() {
  try {
    const raw = await invoke("get_status");
    const status = JSON.parse(raw);
    document.getElementById("llm-status").textContent = status.active_provider || "none";
    document.getElementById("actions-count").textContent = status.journal_entries || 0;
    // Update auto-read toggle from status
    const toggle = document.getElementById("auto-read-toggle");
    if (toggle && toggle.checked !== status.auto_read_enabled) {
      toggle.checked = status.auto_read_enabled;
    }
  } catch {}
  pollBridge();
  refreshAutoReadStatus();
}

// ── Policy Management (Contacts Table) ──
let allContacts = [];
let cachedPolicy = null;
let contactsSearchTerm = "";

async function refreshContactsTable() {
  const tbody = document.getElementById("contacts-table-body");
  const status = document.getElementById("contacts-status");
  if (!tbody) return;
  try {
    const [contactsRaw, policyRaw] = await Promise.all([
      invoke("list_contacts"),
      invoke("get_policy"),
    ]);
    const contacts = JSON.parse(contactsRaw);
    cachedPolicy = JSON.parse(policyRaw);
    allContacts = contacts;
    renderContactsTable(contacts, cachedPolicy);
    if (status) status.textContent = `${contacts.length} contacts loaded`;
  } catch (e) {
    console.error("Failed to load contacts", e);
    if (status) status.textContent = "Failed to load contacts";
  }
}

function renderContactsTable(contacts, policy) {
  const tbody = document.getElementById("contacts-table-body");
  const allowlist = policy.allowlist || [];
  const contactModes = policy.contact_modes || {};
  tbody.innerHTML = contacts
    .filter((c) => {
      if (!contactsSearchTerm) return true;
      const q = contactsSearchTerm.toLowerCase();
      return c.jid.toLowerCase().includes(q) || c.name.toLowerCase().includes(q);
    })
    .map(
      (c) => `
    <tr>
      <td>${escHtml(c.name || "Unknown")}</td>
      <td style="font-size:0.8rem;color:var(--text-muted);">${escHtml(c.jid)}</td>
      <td>
        <label class="toggle" style="background:none;border:none;padding:0;margin:0;">
          <input type="checkbox" ${allowlist.includes(c.jid) ? "checked" : ""} data-jid="${escHtml(c.jid)}" class="allowlist-toggle" />
        </label>
      </td>
      <td>
        <select class="contact-mode-select" data-jid="${escHtml(c.jid)}">
          <option value="assistant" ${contactModes[c.jid] === "assistant" ? "selected" : ""}>Assistant</option>
          <option value="chat" ${contactModes[c.jid] === "chat" ? "selected" : ""}>Chat Only</option>
          <option value="summarize" ${(contactModes[c.jid] || "summarize") === "summarize" ? "selected" : ""}>Summarize</option>
          <option value="blocked" ${contactModes[c.jid] === "blocked" ? "selected" : ""}>Blocked</option>
        </select>
      </td>
    </tr>`
    )
    .join("");
}

function escHtml(s) {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

document.getElementById("contacts-search")?.addEventListener("input", (e) => {
  contactsSearchTerm = e.target.value;
  if (cachedPolicy) renderContactsTable(allContacts, cachedPolicy);
});

document.getElementById("contacts-table-body")?.addEventListener("change", async (e) => {
  const target = e.target;
  if (target.classList.contains("allowlist-toggle")) {
    const jid = target.dataset.jid;
    await invoke("update_allowlist", { action: target.checked ? "add" : "remove", jid });
  } else if (target.classList.contains("contact-mode-select")) {
    const jid = target.dataset.jid;
    await invoke("update_contact_mode", { jid, mode: target.value });
  }
  const raw = await invoke("get_policy");
  cachedPolicy = JSON.parse(raw);
  renderContactsTable(allContacts, cachedPolicy);
});

document.querySelectorAll("[data-perm]").forEach((cb) => {
  cb.addEventListener("change", async () => {
    const perm = cb.dataset.perm;
    const args = {};
    args[perm] = cb.checked;
    await invoke("update_permissions", args);
  });
});

// ── Providers ──
async function refreshModels() {
  const container = document.getElementById("models-container");
  if (!container) return;
  container.innerHTML = '<p class="text-muted">Loading models...</p>';
  try {
    const raw = await invoke("list_models");
    const providers = JSON.parse(raw);
    container.innerHTML = providers
      .map(
        ([name, models, current]) => `
      <div class="model-provider-card">
        <div class="model-provider-header">
          <h4>${escHtml(name)}</h4>
          <span class="model-count">${models.length} model${models.length !== 1 ? "s" : ""}</span>
        </div>
        <select class="model-select" data-provider="${escHtml(name)}">
          ${models.map((m) => `<option value="${escHtml(m)}" ${m === current ? "selected" : ""}>${escHtml(m)}</option>`).join("")}
        </select>
      </div>`
      )
      .join("");
  } catch {
    container.innerHTML = '<p class="text-muted">Failed to fetch models</p>';
  }
}

document.getElementById("refresh-models")?.addEventListener("click", refreshModels);

document.getElementById("models-container")?.addEventListener("change", async (e) => {
  const select = e.target.closest(".model-select");
  if (!select) return;
  const provider = select.dataset.provider;
  const model = select.value;
  await invoke("set_model", { provider, model });
});

document.getElementById("active-provider-select")?.addEventListener("change", async (e) => {
  await invoke("set_active_provider", { name: e.target.value });
  refreshModels();
});

// ── Chat View ──
let chatContacts = [];
let chatAllowlist = [];
let selectedChatJid = null;
let chatPollInterval = null;
let pendingActionsPollInterval = null;

async function refreshChatContacts() {
  try {
    const [contactsRaw, policyRaw] = await Promise.all([
      invoke("list_contacts"),
      invoke("get_policy"),
    ]);
    const contacts = JSON.parse(contactsRaw);
    const policy = JSON.parse(policyRaw);
    chatAllowlist = policy.allowlist || [];
    chatContacts = contacts;
    renderChatContacts(contacts);
  } catch (e) {
    console.error("Failed to load chat contacts", e);
  }
}

function renderChatContacts(contacts) {
  const list = document.getElementById("chat-contact-list");
  const search = (document.getElementById("chat-search")?.value || "").toLowerCase();
  const sorted = [...contacts].sort((a, b) => {
    const aAllowed = chatAllowlist.includes(a.jid) ? 1 : 0;
    const bAllowed = chatAllowlist.includes(b.jid) ? 1 : 0;
    return bAllowed - aAllowed;
  });
  list.innerHTML = sorted
    .filter((c) => {
      if (!search) return true;
      return c.jid.toLowerCase().includes(search) || c.name.toLowerCase().includes(search);
    })
    .map(
      (c) => `
    <button class="chat-contact-item ${selectedChatJid === c.jid ? "active" : ""}" data-jid="${escHtml(c.jid)}">
      <div class="avatar">${(c.name || "?")[0].toUpperCase()}${chatAllowlist.includes(c.jid) ? '<span class="allowlisted-dot"></span>' : ""}</div>
      <div class="contact-info">
        <div class="contact-name">${escHtml(c.name || "Unknown")}</div>
        <div class="contact-jid-sm">${escHtml(c.jid)}</div>
      </div>
      ${chatAllowlist.includes(c.jid) ? '<span class="allowlisted-badge">Allowlisted</span>' : ""}
    </button>`
    )
    .join("");
}

document.getElementById("chat-search")?.addEventListener("input", () => {
  renderChatContacts(chatContacts);
});

document.getElementById("chat-contact-list")?.addEventListener("click", async (e) => {
  const item = e.target.closest(".chat-contact-item");
  if (!item) return;
  selectedChatJid = item.dataset.jid;
  renderChatContacts(chatContacts);
  const name = item.querySelector(".contact-name")?.textContent || selectedChatJid;
  const isAllowlisted = chatAllowlist.includes(selectedChatJid);
  document.getElementById("chat-contact-name").textContent = name;
  document.getElementById("chat-contact-jid").textContent = selectedChatJid;
  document.getElementById("chat-placeholder").classList.add("hidden");
  document.getElementById("chat-conversation").classList.remove("hidden");
  const replyArea = document.getElementById("chat-reply-area");
  if (replyArea) replyArea.classList.toggle("hidden", !isAllowlisted);
  lastMessageCount = 0;
  await loadMessages(selectedChatJid);
  await refreshPendingActions();
});

document.getElementById("chat-refresh")?.addEventListener("click", async () => {
  await refreshChatContacts();
  if (selectedChatJid) await loadMessages(selectedChatJid);
  await refreshPendingActions();
});

async function sendAIReply() {
  const input = document.getElementById("chat-reply-input");
  const text = input?.value.trim();
  if (!text || !selectedChatJid) return;
  input.value = "";
  input.disabled = true;
  const sendBtn = document.getElementById("chat-reply-send");
  if (sendBtn) sendBtn.disabled = true;
  try {
    const raw = await invoke("send_reply", { jid: selectedChatJid, message: text });
    const result = JSON.parse(raw);
    if (result.reply) {
      await loadMessages(selectedChatJid);
    }
    if (result.has_pending_actions) {
      await refreshPendingActions();
    }
  } catch (e) {
    console.error("Reply failed", e);
  }
  input.disabled = false;
  if (sendBtn) sendBtn.disabled = false;
  input.focus();
}

document.getElementById("chat-reply-send")?.addEventListener("click", sendAIReply);
document.getElementById("chat-reply-input")?.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendAIReply();
  }
});

let lastMessageCount = 0;

async function loadMessages(jid) {
  const container = document.getElementById("chat-messages");
  const wasAtBottom = container.scrollTop + container.clientHeight >= container.scrollHeight - 50;
  try {
    const raw = await invoke("list_messages", { jid, limit: 50 });
    const msgs = JSON.parse(raw);
    if (!Array.isArray(msgs)) throw new Error("Invalid response");
    const hasNewMessages = msgs.length !== lastMessageCount;
    if (hasNewMessages || container.innerHTML === '<p class="text-muted">Loading messages...</p>') {
      container.innerHTML = msgs.length
        ? msgs
            .map(
              (m) => `
        <div class="message-bubble ${m.sender === m.chat_jid ? "incoming" : "outgoing"}">
          <div>${escHtml(m.content || "(media)")}</div>
          <div class="message-meta">${m.timestamp || ""}${m.media_type ? " \u00b7 " + escHtml(m.media_type) : ""}</div>
        </div>`
            )
            .join("")
        : '<p class="text-muted">No messages yet</p>';
      lastMessageCount = msgs.length;
    }
    if (wasAtBottom || hasNewMessages) {
      container.scrollTop = container.scrollHeight;
    }
  } catch (e) {
    if (container.innerHTML === '<p class="text-muted">Loading messages...</p>') {
      container.innerHTML = '<p class="text-muted">Failed to load messages</p>';
    }
  }
}

function startChatPolling() {
  if (chatPollInterval) return;
  chatPollInterval = setInterval(async () => {
    await refreshChatContacts();
    if (selectedChatJid) await loadMessages(selectedChatJid);
  }, 3000);
  if (!pendingActionsPollInterval) {
    pendingActionsPollInterval = setInterval(refreshPendingActions, 3000);
  }
}

function stopChatPolling() {
  if (chatPollInterval) {
    clearInterval(chatPollInterval);
    chatPollInterval = null;
  }
  if (pendingActionsPollInterval) {
    clearInterval(pendingActionsPollInterval);
    pendingActionsPollInterval = null;
  }
}

// ── Pending Actions (Risk/Approval) ──
async function refreshPendingActions() {
  try {
    const raw = await invoke("get_pending_actions");
    const actions = JSON.parse(raw);
    renderPendingActions(actions);
  } catch {}
}

function renderPendingActions(actions) {
  const list = document.getElementById("chat-actions-list");
  const countBadge = document.getElementById("pending-actions-count");
  if (!list) return;

  const contactActions = actions.filter((a) => a.contact_jid === selectedChatJid);

  if (countBadge) {
    if (contactActions.length > 0) {
      countBadge.textContent = contactActions.length;
      countBadge.style.display = "inline";
    } else {
      countBadge.style.display = "none";
    }
  }

  if (!contactActions.length) {
    list.innerHTML = '<p class="text-muted">No pending actions</p>';
    return;
  }

  list.innerHTML = contactActions
    .map(
      (a) => `
    <div class="action-card" data-action-id="${escHtml(a.id)}">
      <div class="action-info">
        <div class="action-name">${escHtml(a.action)}</div>
        <div class="action-detail">${escHtml(JSON.stringify(a.params))} \u00b7 Risk: ${escHtml(a.risk_level)}</div>
      </div>
      <div class="action-buttons">
        <button class="btn btn-small btn-success approve-btn">Approve</button>
        <button class="btn btn-small btn-danger reject-btn">Reject</button>
      </div>
    </div>`
    )
    .join("");
}

document.getElementById("chat-actions-list")?.addEventListener("click", async (e) => {
  const card = e.target.closest(".action-card");
  if (!card) return;
  const id = card.dataset.actionId;
  if (e.target.classList.contains("approve-btn")) {
    await invoke("approve_action", { id });
    await loadMessages(selectedChatJid);
    await refreshPendingActions();
  } else if (e.target.classList.contains("reject-btn")) {
    await invoke("reject_action", { id });
    await refreshPendingActions();
  }
});

// ── Action Log ──
async function refreshActionLog() {
  try {
    const raw = await invoke("get_status");
    const status = JSON.parse(raw);
    const tbody = document.getElementById("action-log-body");
    if (tbody) {
      tbody.innerHTML =
        `<tr><td>-</td><td>Journal: ${status.journal_entries || 0} entries</td><td>-</td><td>${status.reversible_actions || 0} reversible</td></tr>`;
    }
  } catch {}
}

document.getElementById("refresh-log")?.addEventListener("click", refreshActionLog);

// ── Settings ──
function loadSettingsUI() {
  const saved = localStorage.getItem("whatszara-settings");
  if (saved) {
    const s = JSON.parse(saved);
    if (document.getElementById("bridge-url"))
      document.getElementById("bridge-url").value = s.bridgeUrl || "http://localhost:8080";
    if (document.getElementById("ollama-endpoint"))
      document.getElementById("ollama-endpoint").value = s.ollamaEndpoint || "http://localhost:11434";
    if (document.getElementById("active-provider-select"))
      document.getElementById("active-provider-select").value = s.activeProvider || "ollama";
    if (document.getElementById("api-key-claude"))
      document.getElementById("api-key-claude").value = s.claudeKey || "";
    if (document.getElementById("api-key-groq"))
      document.getElementById("api-key-groq").value = s.groqKey || "";
    if (document.getElementById("api-key-xai"))
      document.getElementById("api-key-xai").value = s.xaiKey || "";
    if (document.getElementById("api-key-gemini"))
      document.getElementById("api-key-gemini").value = s.geminiKey || "";
  }
}

document.getElementById("save-settings")?.addEventListener("click", () => {
  const settings = {
    bridgeUrl: document.getElementById("bridge-url")?.value,
    ollamaEndpoint: document.getElementById("ollama-endpoint")?.value,
    activeProvider: document.getElementById("active-provider-select")?.value,
    claudeKey: document.getElementById("api-key-claude")?.value,
    groqKey: document.getElementById("api-key-groq")?.value,
    xaiKey: document.getElementById("api-key-xai")?.value,
    geminiKey: document.getElementById("api-key-gemini")?.value,
  };
  localStorage.setItem("whatszara-settings", JSON.stringify(settings));
});

document.getElementById("apply-ollama-endpoint")?.addEventListener("click", async () => {
  const endpoint = document.getElementById("ollama-endpoint")?.value;
  if (endpoint) {
    await invoke("set_ollama_endpoint", { endpoint });
    refreshModels();
  }
});

document.getElementById("apply-api-keys")?.addEventListener("click", async () => {
  const keys = {
    claude: document.getElementById("api-key-claude")?.value,
    groq: document.getElementById("api-key-groq")?.value,
    xai: document.getElementById("api-key-xai")?.value,
    gemini: document.getElementById("api-key-gemini")?.value,
  };
  for (const [provider, key] of Object.entries(keys)) {
    if (key) {
      await invoke("set_api_key", { provider, key });
    }
  }
  refreshModels();
});

document.getElementById("save-config-to-keychain")?.addEventListener("click", async () => {
  await invoke("save_config");
  alert("Config saved to macOS Keychain");
});

document.getElementById("load-config-from-keychain")?.addEventListener("click", async () => {
  const raw = await invoke("load_config");
  const result = JSON.parse(raw);
  if (result.success) {
    await refreshContactsTable();
    alert("Config loaded from Keychain");
  } else {
    alert("No saved config found in Keychain");
  }
});

document.getElementById("clear-config-from-keychain")?.addEventListener("click", async () => {
  if (!confirm("Clear saved config from Keychain?")) return;
  await invoke("clear_config");
  alert("Config cleared from Keychain");
});

// ── Logout ──
document.getElementById("logout-bridge")?.addEventListener("click", async () => {
  if (!confirm("Logout from WhatsApp? This will disconnect your session and require re-authentication via QR code.")) return;
  try {
    await invoke("logout_bridge");
    lastQrCode = "";
    const qrContainer = document.getElementById("qr-container");
    if (qrContainer) qrContainer.classList.add("hidden");
    pollBridge();
  } catch (e) {
    console.error("Logout failed", e);
  }
});

// ── Init ──
window.addEventListener("DOMContentLoaded", () => {
  applyTheme(localStorage.getItem("whatszara-theme") || "dark");
  loadSettingsUI();
  pollBridge();
  bridgePollInterval = setInterval(pollBridge, 3000);
  setTimeout(refreshDashboard, 500);
});

window.addEventListener("beforeunload", () => {
  if (bridgePollInterval) clearInterval(bridgePollInterval);
  stopChatPolling();
});
