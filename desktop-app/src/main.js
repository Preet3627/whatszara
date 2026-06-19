// Whatszara Desktop App - Main
import QRCode from "qrcode";

// ── Navigation ──
document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    const view = btn.dataset.view;
    document.querySelectorAll(".view").forEach((v) => v.classList.add("hidden"));
    const target = document.getElementById(`view-${view}`);
    if (target) target.classList.remove("hidden");
    if (view === "dashboard") { stopChatPolling(); refreshDashboard(); }
    if (view === "permissions") { stopChatPolling(); refreshContactsTable(); }
    if (view === "chat") { refreshChatContacts(); startChatPolling(); }
    if (view === "actions") { stopChatPolling(); refreshActionLog(); }
    if (view === "providers") { stopChatPolling(); refreshModels(); }
    if (view === "settings") { stopChatPolling(); }
  });
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

  // Update sidebar badge
  if (badge) {
    const labels = { stopped: "stopped", running: "starting…", awaiting_scan: "scan QR", connected: "connected", error: "error" };
    badge.textContent = labels[status.status] || status.status;
    badge.classList.toggle("connected", status.status === "connected");
    badge.classList.toggle("error", status.status === "error");
  }

  // Update WhatsApp status stat
  if (waStatus) {
    const labels = { stopped: "Bridge Stopped", running: "Connecting…", awaiting_scan: "Scan QR Code", connected: "Connected", error: "Bridge Error" };
    waStatus.textContent = labels[status.status] || "Unknown";
    waStatus.style.color = status.status === "connected" ? "var(--green)" : status.status === "error" ? "var(--red)" : "var(--yellow)";
  }

  // Update step indicator
  if (indicator) {
    indicator.className = "step-indicator";
    indicator.classList.add(`step-${status.status}`);
  }
  if (spinner) spinner.classList.toggle("hidden", status.status !== "running" && status.status !== "starting" && status.status !== "awaiting_scan");
  if (icon) {
    const showIcon = status.status !== "running" && status.status !== "starting" && status.status !== "awaiting_scan";
    icon.classList.toggle("hidden", !showIcon);
    if (showIcon) {
      const icons = { stopped: "✕", connected: "✓", error: "✕" };
      icon.textContent = icons[status.status] || "?";
    }
  }

  // QR code display
  if (status.status === "awaiting_scan" && status.qr && status.qr !== lastQrCode) {
    lastQrCode = status.qr;
    if (qrContainer) qrContainer.classList.remove("hidden");
    if (qrCanvas) {
      QRCode.toCanvas(qrCanvas, status.qr, {
        width: 280,
        margin: 2,
        color: { dark: "#000000", light: "#ffffff" },
      });
    }
  }

  // Status text
  if (statusText) {
    const texts = {
      stopped: "Bridge is not running. Try restarting the app.",
      running: "Bridge process is running, waiting for QR code from WhatsApp…",
      awaiting_scan: "Scan the QR code below with WhatsApp on your phone.",
      connected: "Bridge is connected to WhatsApp!",
      error: `Bridge failed: ${status.error || "Unknown error"}`,
    };
    statusText.textContent = texts[status.status] || "Unknown status";
  }

  // Error detail
  if (errorDetail) {
    if (status.status === "error" && status.error) {
      errorDetail.textContent = status.error;
      errorDetail.classList.remove("hidden");
    } else {
      errorDetail.classList.add("hidden");
    }
  }

  // Step progression
  if (stepBridge) stepBridge.classList.toggle("completed", status.status === "connected");
  if (stepProvider) stepProvider.classList.toggle("active", status.status === "connected");

  // Logout button visibility
  const logoutDiv = document.getElementById("bridge-logout");
  if (logoutDiv) logoutDiv.classList.toggle("hidden", status.status !== "connected");
}

// ── Dashboard ──
async function refreshDashboard() {
  try {
    const raw = await invoke("get_status");
    const status = JSON.parse(raw);
    document.getElementById("llm-status").textContent = status.active_provider || "none";
    document.getElementById("actions-count").textContent = status.journal_entries || 0;
  } catch {
    // ignore
  }
  pollBridge();
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
});

document.getElementById("chat-refresh")?.addEventListener("click", async () => {
  await refreshChatContacts();
  if (selectedChatJid) await loadMessages(selectedChatJid);
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
  const actionsList = document.getElementById("chat-actions-list");
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
          <div class="message-meta">${m.timestamp || ""}${m.media_type ? " · " + escHtml(m.media_type) : ""}</div>
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
  actionsList.innerHTML = '<p class="text-muted">No pending actions</p>';
}

function startChatPolling() {
  if (chatPollInterval) return;
  chatPollInterval = setInterval(async () => {
    await refreshChatContacts();
    if (selectedChatJid) await loadMessages(selectedChatJid);
  }, 3000);
}

function stopChatPolling() {
  if (chatPollInterval) {
    clearInterval(chatPollInterval);
    chatPollInterval = null;
  }
}

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
  } catch {
    // ignore
  }
}

document.getElementById("refresh-log")?.addEventListener("click", refreshActionLog);

// ── Settings ──
document.getElementById("save-settings")?.addEventListener("click", () => {
  const settings = {
    bridgeUrl: document.getElementById("bridge-url")?.value,
    apiKey: document.getElementById("api-key")?.value,
    ollamaEndpoint: document.getElementById("ollama-endpoint")?.value,
    activeProvider: document.getElementById("active-provider-select")?.value,
  };
  localStorage.setItem("whatszara-settings", JSON.stringify(settings));
  alert("Settings saved (local only for now)");
});

// ── Setup Wizard Nav ──
document.querySelectorAll("#setup-wizard .btn[data-view]").forEach((btn) => {
  btn.addEventListener("click", () => {
    const view = btn.dataset.view;
    const navItem = document.querySelector(`.nav-item[data-view="${view}"]`);
    if (navItem) navItem.click();
  });
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
  const saved = localStorage.getItem("whatszara-settings");
  if (saved) {
    const settings = JSON.parse(saved);
    if (document.getElementById("bridge-url"))
      document.getElementById("bridge-url").value = settings.bridgeUrl || "http://localhost:8080";
    if (document.getElementById("ollama-endpoint"))
      document.getElementById("ollama-endpoint").value = settings.ollamaEndpoint || "http://localhost:11434";
    if (document.getElementById("active-provider-select"))
      document.getElementById("active-provider-select").value = settings.activeProvider || "ollama";
  }
  pollBridge();
  bridgePollInterval = setInterval(pollBridge, 3000);
  setTimeout(refreshDashboard, 500);
});

window.addEventListener("beforeunload", () => {
  if (bridgePollInterval) clearInterval(bridgePollInterval);
  stopChatPolling();
});
