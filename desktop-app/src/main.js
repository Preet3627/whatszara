// Whatszara Desktop App - Main

// ── Navigation ──
document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    const view = btn.dataset.view;
    document.querySelectorAll(".view").forEach((v) => v.classList.add("hidden"));
    const target = document.getElementById(`view-${view}`);
    if (target) target.classList.remove("hidden");
    if (view === "dashboard") refreshDashboard();
    if (view === "permissions") refreshPolicy();
    if (view === "actions") refreshActionLog();
    if (view === "providers") refreshModels();
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

function updateBridgeUI(status) {
  const badge = document.getElementById("status-badge");
  const indicator = document.getElementById("bridge-indicator");
  const spinner = document.getElementById("bridge-spinner");
  const icon = document.getElementById("bridge-icon");
  const statusText = document.getElementById("bridge-status-text");
  const errorDetail = document.getElementById("bridge-error-detail");
  const waStatus = document.getElementById("wa-status");

  const stepBridge = document.getElementById("step-bridge");
  const stepProvider = document.getElementById("step-provider");
  const stepAllowlist = document.getElementById("step-allowlist");

  // Update sidebar badge
  if (badge) {
    const labels = { stopped: "stopped", running: "starting…", connected: "connected", error: "error" };
    badge.textContent = labels[status.status] || status.status;
    badge.classList.toggle("connected", status.status === "connected");
    badge.classList.toggle("error", status.status === "error");
  }

  // Update WhatsApp status stat
  if (waStatus) {
    const labels = { stopped: "Bridge Stopped", running: "Connecting…", connected: "Connected", error: "Bridge Error" };
    waStatus.textContent = labels[status.status] || "Unknown";
    waStatus.style.color = status.status === "connected" ? "var(--green)" : status.status === "error" ? "var(--red)" : "var(--yellow)";
  }

  // Update step indicator
  if (indicator) {
    indicator.className = "step-indicator";
    indicator.classList.add(`step-${status.status}`);
  }
  if (spinner) spinner.classList.toggle("hidden", status.status !== "running" && status.status !== "starting");
  if (icon) {
    const showIcon = status.status !== "running" && status.status !== "starting";
    icon.classList.toggle("hidden", !showIcon);
    if (showIcon) {
      const icons = { stopped: "✕", connected: "✓", error: "✕" };
      icon.textContent = icons[status.status] || "?";
    }
  }

  // Status text
  if (statusText) {
    const texts = {
      stopped: "Bridge is not running. Try restarting the app.",
      running: "Bridge process is running. Waiting for WhatsApp connection… (scan the QR code in the terminal)",
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

// ── Policy Management ──
async function refreshPolicy() {
  const raw = await invoke("get_policy");
  try {
    const policy = JSON.parse(raw);
    document.getElementById("allowlist-display").textContent =
      JSON.stringify(policy.allowlist || [], null, 2);
    document.getElementById("contact-modes-display").textContent =
      JSON.stringify(policy.contact_modes || {}, null, 2);
    document.getElementById("perm-shell").checked = policy.tool_permissions?.shell ?? false;
    document.getElementById("perm-file-access").checked = policy.tool_permissions?.file_access ?? true;
    document.getElementById("perm-media-control").checked = policy.tool_permissions?.media_control ?? true;
    document.getElementById("perm-app-launching").checked = policy.tool_permissions?.app_launching ?? true;
    document.getElementById("perm-whatsapp").checked = policy.tool_permissions?.whatsapp ?? true;
  } catch (e) {
    console.error("Failed to load policy", e);
  }
}

document.querySelectorAll("[data-perm]").forEach((cb) => {
  cb.addEventListener("change", async () => {
    const perm = cb.dataset.perm;
    const args = {};
    args[perm] = cb.checked;
    await invoke("update_permissions", args);
  });
});

document.getElementById("allowlist-add")?.addEventListener("click", async () => {
  const jid = document.getElementById("allowlist-jid").value.trim();
  if (!jid) return;
  await invoke("update_allowlist", { action: "add", jid });
  document.getElementById("allowlist-jid").value = "";
  refreshPolicy();
});

document.getElementById("allowlist-remove")?.addEventListener("click", async () => {
  const jid = document.getElementById("allowlist-jid").value.trim();
  if (!jid) return;
  await invoke("update_allowlist", { action: "remove", jid });
  document.getElementById("allowlist-jid").value = "";
  refreshPolicy();
});

document.getElementById("contact-mode-set")?.addEventListener("click", async () => {
  const jid = document.getElementById("contact-jid").value.trim();
  const mode = document.getElementById("contact-mode-select").value;
  if (!jid) return;
  await invoke("update_contact_mode", { jid, mode });
  document.getElementById("contact-jid").value = "";
  refreshPolicy();
});

// ── Providers ──
async function refreshModels() {
  try {
    const raw = await invoke("list_models");
    document.getElementById("models-list").textContent =
      JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    document.getElementById("models-list").textContent = "Failed to fetch models";
  }
}

document.getElementById("active-provider-select")?.addEventListener("change", async (e) => {
  await invoke("set_active_provider", { name: e.target.value });
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
});
