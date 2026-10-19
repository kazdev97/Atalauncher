const DEMO = !window.__TAURI__;

const $ = (id) => document.getElementById(id);
const status = (id, msg) => { const el = $(id); if (el) el.textContent = msg || ""; };
function showPage(page) {
  const btn = document.querySelector(`.nav-item[data-page="${page}"]`);
  if (btn) btn.click();
}
const demoStore = { accounts: [], selected_id: "", mode: "offline", offline_username: "Juanito67" };
const demoInstances = [
  { name: "Vanilla 1.21.4", version_id: "1.21.4", loader: "vanilla", game_version: "1.21.4", loader_version: "", source: "local", manifest_url: "", manifest_type: "", server_version: "", actualizacion: true, installed: true, instance_dir: "C:\\Documentos\\Atalauncher\\instancias\\Vanilla 1.21.4", last_launched: "2026-09-14 18:00", session_based: false, remote: false },
  { name: "Server de Kaz", version_id: "1.21.4-fabric-0.16.14", loader: "fabric", game_version: "1.21.4", loader_version: "0.16.14", source: "remote", manifest_url: "https://drive.google.com/uc?export=download&id=abc123", manifest_type: "pau", server_version: "2026-09-13", actualizacion: true, installed: true, instance_dir: "C:\\Documentos\\Atalauncher\\instancias\\Server de Kaz", last_launched: "2026-09-13 21:30", session_based: false, remote: true },
  { name: "Pack de Test", version_id: "", loader: "fabric", game_version: "1.20.1", loader_version: "", source: "remote", manifest_url: "https://drive.google.com/uc?export=download&id=xyz789", manifest_type: "", server_version: "", actualizacion: true, installed: false, instance_dir: "C:\\Documentos\\Atalauncher\\instancias\\Pack de Test", last_launched: "", session_based: false },
];
const demoJava = [
  { path: "C:\\Documentos\\Atalauncher\\runtime\\jdk-21\\bin\\java.exe", major: 21, is_valid: true },
  { path: "C:\\Program Files\\Java\\jdk-17.0.10\\bin\\java.exe", major: 17, is_valid: true },
];
const demoVersions = [
  { id: "1.21.4", kind: "release", release_time: "2024-12-03T18:21:00+00:00" },
  { id: "1.20.1", kind: "release", release_time: "2023-06-12T12:11:00+00:00" },
];
const demoSettings = { ram_mb: 4096, max_ram_mb: 0, java_path_override: "", streamer_mode: true, close_launcher_after_launch: false, game_res_width: 0, game_res_height: 0, fullscreen: false, last_username: "", source_url: "https://raw.githubusercontent.com/kazdev97/Hallogeey/main/index.json", auto_update: true };
const demoSource = [
  { id: "paucalipsis", name: "Paucalipsis 2", description: "Carpeta del pack en la repo (sync directo de GitHub)", game_version: "1.21.4", loader: "fabric", loader_version: "0.16.14", manifest_url: "", revision: "latest", installed: true, github_repo: "kazdev97/Hallogeey", github_path: "pack", github_branch: "main" },
  { id: "escuela", name: "Escuela 2026", description: "Pack educativo ligero para el aula", game_version: "1.20.1", loader: "fabric", loader_version: "0.16.10", manifest_url: "https://raw.githubusercontent.com/pau/mis-packs/main/escuela/manifest.json", revision: "2026-09-01", installed: false },
  { id: "evento", name: "Evento Hallowen 2026", description: "Modpack temporal del evento", game_version: "1.21.4", loader: "vanilla", loader_version: "", manifest_url: "https://raw.githubusercontent.com/pau/mis-packs/main/evento/manifest.json", revision: "2026-09-13", installed: false },
];
const demoDiff = { revision: "20260914T120000Z", up_to_date: false, missing: ["mods/jei-1.21.4.jar", "config/server.properties"], different: ["mods/usefulmod.jar"], extra: ["mods/lag_fix.jar", "config/old_setting.toml"], change_count: 5, has_archive: true };
const demoSyncResult = { applied_revision: "20260914T120000Z", replaced: ["mods/usefulmod.jar"], downloaded: ["mods/jei-1.21.4.jar", "config/server.properties"], removed: ["mods/lag_fix.jar"], backup_dir: "C:\\Documentos\\Atalauncher\\instancias\\Server de Kaz\\backups\\20260914T120000Z", errors: [] };

function demoCall(cmd) {
  const map = {
    login_get_state: () => demoStore,
    login_set_offline: () => undefined,
    login_set_selected: () => undefined,
    login_remove_account: () => undefined,
    instances_list: () => demoInstances,
    java_get_bundled: () => demoJava.filter((j) => j.path.toLowerCase().includes("runtime")),
    java_get_system: () => demoJava.filter((j) => !j.path.toLowerCase().includes("runtime")),
    install_vanilla_versions: () => demoVersions,
    install_fabric_loaders: () => ["0.16.14", "0.16.10"],
    sync_list_backups: () => ["20260914_120000", "20260910_090000", "20260901_140000"],
    sync_check: () => demoDiff,
    sync_apply: () => demoSyncResult,
    settings_get: () => demoSettings,
    system_info: () => ({ cpu_cores: 16, total_ram_mb: 32768 }),
    ram_recommend: () => ({ total_ram_gb: 32, min_mb: 1536, max_mb: 8192 }),
    source_fetch: () => demoSource,
    source_install: () => undefined,
    auto_sync_check: () => [
      { name: "Server de Kaz", up_to_date: false, change_count: 5, revision: "20260914T120000Z", error: null },
      { name: "Vanilla 1.21.4", up_to_date: true, change_count: 0, revision: "", error: null },
    ],
    auto_sync_apply: () => ({ applied: ["Server de Kaz (revisión 20260914T120000Z)"], already_up_to_date: ["Vanilla 1.21.4"], errors: [] }),
    java_resolve: () => ({ path: "C:\\Documentos\\Atalauncher\\runtime\\jdk-21\\bin\\java.exe", major: 21, is_valid: true }),
    launch_game: () => 4321,
    launch_is_running: () => false,
    update_check: () => ({ current: "0.1.0", latest: "0.2.0", available: true, notes: "Mejoras de compatibilidad", download_url: "" }),
    update_apply: () => undefined,
  };
  const fn = map[cmd];
  return Promise.resolve(fn ? fn() : undefined);
}

const demoEvent = {
  listen: (_name, _cb) => Promise.resolve(() => {}),
};

const consoleBuf = [];
const CONSOLE_MAX_LINES = 4000;

function appendLog(line, stream) {
  const pre = $("console");
  const wasAtBottom = pre.scrollHeight - pre.scrollTop - pre.clientHeight < 24;
  consoleBuf.push((stream === "stderr" ? "[err] " : "") + line);
  if (consoleBuf.length > CONSOLE_MAX_LINES) {
    consoleBuf.splice(0, consoleBuf.length - Math.floor(CONSOLE_MAX_LINES / 2));
  }
  pre.textContent = consoleBuf.join("\n") + "\n";
  if (wasAtBottom) pre.scrollTop = pre.scrollHeight;
}

function copyConsole() {
  const text = $("console").textContent;
  const done = () => status("game-state", "Consola copiada al portapapeles.");
  const fallback = () => {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); done(); }
    catch (e) { status("game-state", "No se pudo copiar la consola."); }
    document.body.removeChild(ta);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(done).catch(fallback);
  } else {
    fallback();
  }
}

$("btn-copy-console").addEventListener("click", copyConsole);
$("btn-clear-console").addEventListener("click", () => {
  consoleBuf.length = 0;
  $("console").textContent = "";
  status("game-state", "Consola limpiada.");
});

const EVENT = DEMO ? demoEvent : window.__TAURI__.event;

async function call(cmd, args) {
  if (DEMO) return demoCall(cmd, args);
  try {
    return await window.__TAURI__.core.invoke(cmd, args || {});
  } catch (e) {
    throw new Error(typeof e === "string" ? e : (e && e.message) || "error desconocido");
  }
}
document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach((b) => b.classList.remove("active"));
    document.querySelectorAll(".page").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    $("page-" + btn.dataset.page).classList.add("active");
    if (btn.dataset.page === "instances") loadSource();
    if (btn.dataset.page === "settings") { loadSettings(); setupRam(); fillRollbackForm(); loadAntilag(); }
    call("discord_set_page", { page: btn.dataset.page }).catch(() => {});
  });
});
async function refreshLogin() {
  try {
    const store = await call("login_get_state", {});
    const offlineName = (store.mode === "offline" && (store.offline_username || "").trim())
      || "";
    const offlineToggle = $("offline-mode");
    if (offlineToggle) offlineToggle.checked = !!offlineName;
    if (offlineName) $("offline-username").value = store.offline_username;

    if (offlineName) {
      $("nav-user").textContent = offlineName + " (no premium)";
      const navAvatar = $("nav-user-avatar");
      if (navAvatar) navAvatar.classList.add("hidden");
      $("login-box").classList.remove("hidden");
      $("login-ok").classList.add("hidden");
      status("login-status", "");
      return;
    }

    if (store.accounts && store.accounts.length > 0) {
      const sel = store.accounts.find((a) => a.id === store.selected_id) || store.accounts[0];
      const username = sel.name || sel.id;
      const uuid = sel.id || "";

      $("login-name").textContent = username;
      if ($("login-uuid")) $("login-uuid").textContent = uuid;
      const needsRelogin = !!sel.needs_relogin;
      const statusEl = $("account-status");
      if (statusEl) {
        statusEl.textContent = needsRelogin
          ? "● Sesión cerrada. Vuelve a iniciar sesión."
          : "● Conectado a Microsoft";
        statusEl.classList.remove("account-status-online", "account-status-warning");
        statusEl.classList.add(needsRelogin ? "account-status-warning" : "account-status-online");
      }
      const reloginBtn = $("btn-relogin");
      if (reloginBtn) {
        if (needsRelogin) reloginBtn.classList.remove("hidden");
        else reloginBtn.classList.add("hidden");
      }
      const skinBody = $("login-skin-body");
      if (skinBody) {
        skinBody.src = `https://mc-heads.net/body/${encodeURIComponent(username)}/240`;
        skinBody.onerror = () => {
          if (uuid) skinBody.src = `https://crafatar.com/renders/body/${uuid}?overlay=true`;
        };
      }
      const avatarHead = $("login-avatar-head");
      if (avatarHead) {
        avatarHead.src = `https://mc-heads.net/avatar/${encodeURIComponent(username)}/64`;
        avatarHead.onerror = () => {
          avatarHead.src = `https://minotar.net/helm/${encodeURIComponent(username)}/64`;
        };
      }
      $("nav-user").textContent = username;
      const navAvatar = $("nav-user-avatar");
      if (navAvatar) {
        navAvatar.src = `https://mc-heads.net/avatar/${encodeURIComponent(username)}/64`;
        navAvatar.classList.remove("hidden");
        navAvatar.onerror = () => {
          navAvatar.src = `https://minotar.net/helm/${encodeURIComponent(username)}/64`;
        };
      }

      $("login-box").classList.add("hidden");
      $("login-ok").classList.remove("hidden");
    } else {
      $("login-box").classList.remove("hidden");
      $("login-ok").classList.add("hidden");
      $("nav-user").textContent = "Sin sesión";
      const navAvatar = $("nav-user-avatar");
      if (navAvatar) navAvatar.classList.add("hidden");
    }
  } catch (e) {
    status("login-status", "Error al leer cuentas: " + e.message);
  }
}

const navUserContainer = $("nav-user-container");
if (navUserContainer) {
  navUserContainer.addEventListener("click", () => {
    const btn = document.querySelector('.nav-item[data-page="login"]');
    if (btn) btn.click();
  });
}

$("btn-login").addEventListener("click", async () => {
  status("login-status", "Iniciando sesión... Complete el login en el navegador.");
  try {
    const acc = await call("login_start", {});
    status("login-status", "Sesión iniciada como " + (acc.name || acc.id));
    refreshLogin();
  } catch (e) {
    status("login-status", (e.message || e) === "__MINECRAFT_NOT_OWNED__"
      ? "Esta cuenta de Microsoft no tiene Minecraft comprado."
      : "Login cancelado o fallido: " + e.message);
  }
});

$("btn-relogin").addEventListener("click", async () => {
  status("login-status", "Reiniciando sesión... Complete el login en el navegador si se abre.");
  try {
    const acc = await call("login_start", {});
    status("login-status", "Sesión renovada como " + (acc.name || acc.id));
    refreshLogin();
  } catch (e) {
    status("login-status", (e.message || e) === "__MINECRAFT_NOT_OWNED__"
      ? "Esta cuenta de Microsoft no tiene Minecraft comprado."
      : "Relogin cancelado o fallido: " + e.message);
  }
});

$("btn-logout").addEventListener("click", async () => {
  try {
    const store = await call("login_get_state", {});
    if (store.selected_id) await call("login_remove_account", { accountId: store.selected_id });
    refreshLogin();
  } catch (e) { status("login-status", e.message); }
});
$("btn-save-offline").addEventListener("click", async () => {
  const name = $("offline-username").value.trim();
  if (!name) { status("offline-status", "Escribe un nombre de usuario."); return; }
  try {
    await call("login_set_offline", { username: name });
    status("offline-status", `Modo No premium activo como '${name}'.`);
    refreshLogin();
  } catch (e) { status("offline-status", e.message); }
});

$("offline-mode").addEventListener("change", async () => {
  if ($("offline-mode").checked) {
    const name = $("offline-username").value.trim();
    if (!name) {
      $("offline-mode").checked = false;
      status("offline-status", "Escribe un nombre de usuario primero.");
      return;
    }
    try {
      await call("login_set_offline", { username: name });
      status("offline-status", "Modo No premium activado.");
      refreshLogin();
    } catch (e) {
      $("offline-mode").checked = false;
      status("offline-status", e.message);
    }
  } else {
    try {
      const store = await call("login_get_state", {});
      if (store.accounts && store.accounts.length > 0) {
        await call("login_set_selected", { accountId: store.accounts[0].id });
        status("offline-status", "Cuenta de Microsoft seleccionada.");
      } else {
        status("offline-status", "No hay cuentas premium guardadas. Inicia sesión con Microsoft.");
      }
      refreshLogin();
    } catch (e) { status("offline-status", e.message); }
  }
});
let instances = [];
let currentInstance = null;

async function loadInstances() {
  try {
    instances = await call("instances_list", {});
  } catch (e) {
    status("instances-status", e.message);
    return;
  }
  const list = $("instances-list");
  list.innerHTML = "";
  if (instances.length === 0) {
    list.innerHTML = '<p class="muted">Sin instancias instaladas. Instala una de las disponibles arriba.</p>';
    return;
  }
  instances.forEach((inst) => list.appendChild(instanceCard(inst)));
  fillRollbackForm();
  loadSource();
  runAutoSync(false);
}

let settingsState = null;
async function getSettings() {
  if (!settingsState) settingsState = await call("settings_get", {});
  return settingsState;
}

async function loadSource() {
  renderSourceStatus("Consultando instancias disponibles...");
  let url = "";
  try {
    const s = await getSettings();
    url = (s.source_url || "").trim();
    if (!url) {
      renderAvailable([]);
      status("instances-status", "No hay fuente configurada.");
      return;
    }
    const list = await call("source_fetch", { url });
    renderAvailable(list || []);
    status("instances-status", (list && list.length)
      ? `${list.length} instancias disponibles en la fuente.`
      : "No hay instancias disponibles en la fuente.");
  } catch (e) {
    const msg = (e && e.message) ? e.message : String(e);
    renderSourceStatus("No se pudo consultar la fuente: " + msg);
    status("instances-status", "Error al consultar " + url + ": " + msg);
  }
}

function renderSourceStatus(msg) {
  $("available-list").innerHTML = `<p class="muted">${escapeHtml(msg)}</p>`;
}

function renderAvailable(list) {
  const el = $("available-list");
  el.innerHTML = "";
  if (list.length === 0) {
    el.innerHTML = '<p class="muted">No hay instancias publicadas todavía.</p>';
    return;
  }
  list.forEach((item) => {
    const card = document.createElement("div");
    card.className = "instance-card";
    const loader = item.loader || "vanilla";
    const meta = `${escapeHtml(loader)} ${escapeHtml(item.game_version || "?")}`
      + (item.loader_version ? " · " + escapeHtml(item.loader_version) : "")
      + (item.revision ? " · rev " + escapeHtml(item.revision) : "");
    card.innerHTML = `
      <div class="icon">${escapeHtml((item.name || "?").charAt(0).toUpperCase())}</div>
      <div class="info">
        <b>${escapeHtml(item.name)}</b>
        ${item.installed ? '<span class="badge install">instalada</span>' : ""}
        <div class="sub">${meta}</div>
        ${item.description ? `<div class="sub">${escapeHtml(item.description)}</div>` : ""}
      </div>
      <div class="actions">
        ${item.installed
          ? '<button class="btn ghost small" disabled>instalado</button>'
          : '<button class="btn mint small" data-install>Instalar</button>'}
      </div>`;
    const btn = card.querySelector("[data-install]");
    if (btn) btn.addEventListener("click", () => installFromSource(item, btn));
    el.appendChild(card);
  });
}

async function installFromSource(item, btn) {
  if (DEMO) return installFromSourceDemo(item);
  status("instances-status", `Instalando '${item.name}' desde el source...`);
  setInstallProgress(1, `Instalando '${item.name}'...`);
  if (btn) { btn.disabled = true; btn.textContent = "Instalando…"; }
  try {
    await call("source_install", {
      name: item.name,
      gameVersion: item.game_version || "",
      loader: item.loader || "vanilla",
      loaderVersion: item.loader_version || "",
      manifestUrl: item.manifest_url || "",
      githubRepo: item.github_repo || "",
      githubPath: item.github_path || "",
      githubBranch: item.github_branch || "",
    });
    setInstallProgress(100, "Instalada.");
    status("instances-status", `'${item.name}' instalada correctamente.`);
    loadInstances();
  } catch (e) {
    hideInstallProgress();
    if (btn) { btn.disabled = false; btn.textContent = "Instalar"; }
    status("instances-status", "Error instalando: " + e.message);
  }
}

async function installFromSourceDemo(item) {
  status("instances-status", `Instalando '${item.name}' desde el source...`);
  const steps = [
    [5, "Obteniendo manifiesto..."],
    [15, "Descargando el jar del juego..."],
    [40, "Descargando assets..."],
    [62, "Descargando librerías..."],
    [85, "Descargando mods..."],
    [99, "Finalizando..."],
  ];
  for (const [pct, txt] of steps) {
    await new Promise((r) => setTimeout(r, 450));
    setInstallProgress(pct, txt);
  }
  await new Promise((r) => setTimeout(r, 250));
  hideInstallProgress();
  status("instances-status", `'${item.name}' instalada correctamente.`);
  loadInstances();
}

async function runAutoSync(autoApply) {
  let states;
  try {
    states = await call("auto_sync_check", {});
  } catch (e) {
    return;
  }
  const pending = (states || []).filter((s) =>
    !s.error && !s.up_to_date && s.change_count > 0);
  const banner = $("updates-banner");
  if (pending.length > 0) {
    banner.innerHTML = `
      <div class="row">
        <b>Actualizaciones disponibles:</b>
        <span class="muted">${pending.map((s) => escapeHtml(s.name)).join(", ")}</span>
        <button class="btn mint small" id="btn-apply-all">Actualizar ahora</button>
      </div>`;
    banner.classList.remove("hidden");
    $("btn-apply-all").addEventListener("click", async () => {
      banner.classList.add("hidden");
      status("instances-status", "Aplicando actualizaciones...");
      try {
        const res = await call("auto_sync_apply", {});
        const msg = [
          res.applied.length ? "Actualizado: " + res.applied.join(", ") : "Todo al día.",
          res.errors.length ? "Errores: " + res.errors.join("; ") : "",
        ].filter(Boolean).join("\n");
        status("instances-status", msg);
        loadInstances();
      } catch (e) { status("instances-status", "Error: " + e.message); }
    });
  } else {
    banner.classList.add("hidden");
  }
  if (autoApply && pending.length > 0) {
    try {
      const s = await getSettings();
      if (s.auto_update) {
        status("instances-status", "Buscando actualizaciones (auto)...");
        const res = await call("auto_sync_apply", {});
        banner.classList.add("hidden");
        status("instances-status",
          (res.applied.length ? "Actualizado: " + res.applied.join(", ") : "Todo al día.")
          + (res.errors.length ? " · Errores: " + res.errors.join("; ") : ""));
        loadInstances();
      }
    } catch (e) { status("instances-status", "No se pudo auto-actualizar: " + e.message); }
  }
}

function instanceCard(inst) {
  const el = document.createElement("div");
  el.className = "instance-card";
  const badge = inst.installed ? (inst.last_launched ? "" : "")
    : '<span class="badge outdated">no instalada</span>';
  el.innerHTML = `
    <div class="icon">${inst.name.charAt(0).toUpperCase()}</div>
    <div class="info">
      <b>${escapeHtml(inst.name)}</b>${badge}
      <div class="sub">${escapeHtml(inst.loader)} ${escapeHtml(inst.game_version)}
        ${inst.loader_version ? "· " + escapeHtml(inst.loader_version) : ""}
        ${inst.manifest_url ? "· manifest: ajustado" : "· manifest: no configurado"}
        ${inst.last_launched ? "· último: " + escapeHtml(inst.last_launched) : ""}
      </div>
    </div>
    <div class="actions">
      ${!inst.installed ? '<button class="btn mint small" data-act="install">Instalar</button>' : ''}
      <button class="btn lila small" data-act="check">Actualizar</button>
      <button class="btn lila small" data-act="launch">Jugar</button>
      <button class="btn ghost small" data-act="delete">Eliminar</button>
    </div>`;
  const btnInstall = el.querySelector('[data-act="install"]');
  if (btnInstall) btnInstall.addEventListener("click", () => installInstance(inst));
  el.querySelector('[data-act="check"]').addEventListener("click", () => checkUpdates(inst));
  el.querySelector('[data-act="launch"]').addEventListener("click", () => launchInstance(inst));
  el.querySelector('[data-act="delete"]').addEventListener("click", async () => {
    if (!confirm(`¿Eliminar la instancia '${inst.name}' y todos sus archivos?`)) return;
    try {
      await call("instances_delete", { name: inst.name });
      loadInstances();
    } catch (e) { status("instances-status", e.message); }
  });
  return el;
}

function escapeHtml(s) {
  return (s || "").replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

async function installInstance(inst) {
  status("instances-status", `Instalando '${inst.name}' (descarga de versiones y librerías)...`);
  try {
    await call("install_instance", { name: inst.name });
    status("instances-status", `'${inst.name}' instalada correctamente.`);
    loadInstances();
  } catch (e) {
    status("instances-status", "Error instalando: " + e.message);
  }
}

async function checkUpdates(inst) {
  currentInstance = inst;
  status("instances-status", "Comprobando actualizaciones...");
  try {
    const diff = await call("sync_check", { name: inst.name });
    showDiffModal(inst, diff);
  } catch (e) {
    showModal(`Error: ${e.message}`);
  }
}

function showDiffModal(inst, diff) {
  const lines = [];
  lines.push(`<div class="group">Revisión del servidor: ${escapeHtml(diff.revision || "sin fecha")}</div>`);
  if (diff.up_to_date && diff.change_count === 0) {
    lines.push('<div class="group">✔ La instancia está al día.</div>');
  } else {
    if (diff.missing.length) {
      lines.push('<div class="group">Añadir (' + diff.missing.length + '):</div>');
      diff.missing.forEach((f) => lines.push(`<div class="item add">${escapeHtml(f)}</div>`));
    }
    if (diff.different.length) {
      lines.push('<div class="group">Reemplazar (' + diff.different.length + '):</div>');
      diff.different.forEach((f) => lines.push(`<div class="item">${escapeHtml(f)}</div>`));
    }
    if (diff.extra.length) {
      lines.push('<div class="group">Eliminar sobrantes (' + diff.extra.length + '):</div>');
      diff.extra.forEach((f) => lines.push(`<div class="item extra">${escapeHtml(f)}</div>`));
    }
  }
  const modal = showModal(
    `<h2>Actualizaciones disponibles</h2><div class="diff">${lines.join("")}</div>`,
    diff.up_to_date && diff.change_count === 0 ? null : "Aplicar cambios"
  );
  if (diff.up_to_date && diff.change_count === 0) return;
  modal.querySelector(".btn.primary")?.addEventListener("click", async () => {
    modal.remove();
    status("instances-status", "Aplicando... (se hace backup automático)");
    try {
      const res = await call("sync_apply", { name: inst.name });
      const msg = [`Aplicado (revisión ${res.applied_revision || "n/a"}).`,
        `Añadidos: ${res.downloaded.length}, reemplazados: ${res.replaced.length}, eliminados: ${res.removed.length}.`,
        res.backup_dir ? `Backup: ${res.backup_dir}` : ""].join("\n");
      showModal(`<h2>Actualización aplicada</h2><pre class="mono small">${escapeHtml(msg)}</pre>`);
      if (res.errors && res.errors.length) showModal(`<h2>Errores parciales</h2><pre class="mono">${escapeHtml(res.errors.join("\n"))}</pre>`);
      loadInstances();
    } catch (e) { status("instances-status", "Error: " + e.message); }
  });
}

function showModal(html, okLabel, okHandler) {
  const backdrop = document.createElement("div");
  backdrop.className = "modal-backdrop";
  backdrop.innerHTML = `<div class="modal">${html}</div>`;
  const modal = backdrop.firstChild;
  if (okLabel) {
    const btn = document.createElement("button");
    btn.className = "btn primary";
    btn.textContent = okLabel;
    modal.appendChild(btn);
    if (okHandler) btn.addEventListener("click", okHandler);
  }
  backdrop.addEventListener("click", (e) => { if (e.target === backdrop) backdrop.remove(); });
  document.body.appendChild(backdrop);
  return backdrop;
}
function setInstallProgress(pct, text) {
  const card = $("install-progress");
  if (!card) return;
  $("install-fill").style.width = pct + "%";
  $("install-text").textContent = (text || "") + (pct > 0 ? ` (${pct}%)` : "");
  if (pct >= 100) card.classList.add("hidden");
  else card.classList.remove("hidden");
}
function hideInstallProgress() {
  const card = $("install-progress");
  if (card) card.classList.add("hidden");
}
async function loadSettings() {
  try {
    const s = await getSettings();
    $("set-java").value = s.java_path_override || "";
    $("set-streamer").checked = !!s.streamer_mode;
    $("set-auto").checked = s.auto_update !== undefined ? !!s.auto_update : true;
  } catch (e) { status("settings-status", e.message); }
}
async function loadAntilag() {
  try {
    const st = await call("antilag_status", {});
    $("antilag-enable").checked = !!st.enabled;
    $("antilag-host").value = st.host || "";
    const hint = $("antilag-hint");
    hint.style.color = "";
    if (st.installed && st.tasks_registered) {
      hint.textContent = st.tunnel_active ? "Túnel activo ahora mismo." : "";
      $("btn-antilag-install").textContent = "Reinstalar modo antilag";
    } else if (st.installed) {
      hint.textContent = "Faltan las tareas de administrador: pulsa Instalar y acepta el permiso.";
    } else {
      hint.textContent = "No instalado. Descarga un motor de ~15 MB y crea una cuenta WARP free.";
    }
  } catch (e) { status("antilag-status", e.message); }
}

function bindAntilag() {
  const installBtn = $("btn-antilag-install");
  installBtn.addEventListener("click", async () => {
    installBtn.disabled = true;
    installBtn.textContent = "Instalando…";
    status("antilag-status", "Instalando modo antilag…");
    try {
      const st = await call("antilag_install", {});
      status("antilag-status", st.tunnel_active
        ? "Modo antilag instalado."
        : "Modo antilag instalado. Actívalo con el interruptor.");
      await loadAntilag();
    } catch (e) {
      status("antilag-status", e.message || String(e));
    } finally {
      installBtn.disabled = false;
    }
  });
  $("antilag-enable").addEventListener("change", async () => {
    try {
      await call("antilag_set_enabled", { enabled: $("antilag-enable").checked });
      status("antilag-status", $("antilag-enable").checked
        ? "Activado: al jugar, la conexión al servidor saldrá por WARP."
        : "Desactivado.");
    } catch (e) { status("antilag-status", e.message || String(e)); }
  });
  let hostTimer = null;
  $("antilag-host").addEventListener("input", (e) => {
    clearTimeout(hostTimer);
    const value = e.target.value.trim();
    hostTimer = setTimeout(async () => {
      try {
        await call("antilag_set_host", { host: value });
        status("antilag-status", "Dirección del servidor guardada.");
      } catch (err) { status("antilag-status", err.message || String(err)); }
    }, 600);
  });
}

let ramRec = { min_mb: 1024, max_mb: 4096, total_ram_gb: 16 };

function populateRamInstanceSelect() {
  const sel = $("ram-instance");
  sel.innerHTML = "";
  instances.forEach((i) => {
    const o = document.createElement("option");
    o.value = i.name;
    o.textContent = i.name;
    sel.appendChild(o);
  });
  if (sel.childElementCount === 0) {
    const o = document.createElement("option");
    o.value = "";
    o.textContent = "No hay instancias";
    sel.appendChild(o);
  }
}

function updateRamFeedback(valueGb, auto) {
  $("ram-value").textContent = (auto ? "Auto · " : "") + valueGb + " GB";
  const fb = $("ram-feedback");
  const recMaxGb = Math.max(1, Math.round((ramRec.max_mb || 4096) / 1024));
  const recMinGb = Math.max(0, Math.round((ramRec.min_mb || 0) / 1024));
  fb.style.color = "var(--menta)";
  if (auto) {
    fb.textContent = `Auto: mínimo ${recMinGb} GB · máximo ${recMaxGb} GB (para esta instancia).`;
    return;
  }
  if (valueGb <= 1) {
    fb.textContent = "Riesgoso: puede que no sea suficiente para iniciar.";
    fb.style.color = "#E23D28";
  } else if (valueGb < recMaxGb - 1) {
    fb.textContent = `Bajo: esta instancia recomienda ${recMinGb}–${recMaxGb} GB.`;
    fb.style.color = "#F8B339";
  } else if (valueGb <= recMaxGb + 1) {
    fb.textContent = `Óptimo para esta instancia (recomendado: ${recMinGb}–${recMaxGb} GB).`;
  } else if (valueGb >= 10) {
    fb.textContent = "Excesivo: a partir de 10 GB puede causar micro-cortes y ralentizar Windows.";
    fb.style.color = "#F8B339";
  } else if (valueGb <= Math.max(2, Math.round(ramRec.total_ram_gb * 0.75))) {
    fb.textContent = "Aceptable: solo si el modpack es muy pesado.";
    fb.style.color = "var(--lila)";
  } else {
    fb.textContent = "Excesivo: puede causar micro-cortes y ralentizar Windows.";
    fb.style.color = "#F8B339";
  }
}

async function setupRam() {
  populateRamInstanceSelect();
  try {
    const info = await call("system_info", {}).catch(() => null);
    ramRec.total_ram_gb = info ? Math.max(1, Math.round((info.total_ram_mb || 8192) / 1024)) : 16;
    if (ramRec.max_mb > ramRec.total_ram_gb * 1024) ramRec.max_mb = ramRec.total_ram_gb * 1024;
    const name = $("ram-instance").value;
    if (name) {
      try {
        const r = await call("ram_recommend", { name });
        ramRec.min_mb = r.min_mb || ramRec.min_mb;
        ramRec.max_mb = r.max_mb || ramRec.max_mb;
        ramRec.total_ram_gb = r.total_ram_gb || ramRec.total_ram_gb;
      } catch (e) {}
    }
    const s = await getSettings();
    const slider = $("ram-slider");
    slider.max = Math.max(1, ramRec.total_ram_gb);
    const auto = !(s.max_ram_mb > 0);
    $("ram-auto").checked = auto;
    let val = Math.max(1, Math.min(Math.round((ramRec.max_mb || 4096) / 1024), slider.max));
    if (!auto && s.max_ram_mb > 0) {
      val = Math.max(1, Math.min(Math.round(s.max_ram_mb / 1024), slider.max));
    }
    slider.value = val;
    $("ram-info").textContent = info
      ? `Tu equipo: ${info.cpu_cores} núcleos · ${ramRec.total_ram_gb} GB de RAM.`
      : `${ramRec.total_ram_gb} GB de RAM detectados.`;
    updateRamFeedback(val, auto);
  } catch (e) {
    $("ram-info").textContent = "No se pudo calcular la recomendación.";
  }
}

$("ram-instance").addEventListener("change", () => setupRam());
$("ram-slider").addEventListener("input", () => {
  $("ram-auto").checked = false;
  updateRamFeedback(parseInt($("ram-slider").value, 10) || 1, false);
});
$("ram-auto").addEventListener("change", () => {
  if (!$("ram-auto").checked) return;
  const slider = $("ram-slider");
  slider.value = Math.max(1, Math.min(Math.round((ramRec.max_mb || 4096) / 1024), slider.max));
  updateRamFeedback(parseInt(slider.value, 10), true);
});

$("btn-save-settings").addEventListener("click", async () => {
  try {
    const s = await getSettings();
    const auto = $("ram-auto").checked;
    const ramOverrideMb = auto ? 0 : (parseInt($("ram-slider").value, 10) || 1) * 1024;
    const sourceUrl = (s.source_url || "").trim();
    await call("settings_save", {
      javaPathOverride: $("set-java").value.trim(),
      streamerMode: $("set-streamer").checked,
      ramOverrideMb,
      sourceUrl,
      autoUpdate: $("set-auto").checked,
      discordRpcEnabled: true,
    });
    settingsState.max_ram_mb = ramOverrideMb;
    status("settings-status", "Ajustes guardados.");
  } catch (e) { status("settings-status", e.message); }
});

bindAntilag();

function fillRollbackForm() {
  const sel = $("rb-instance");
  sel.innerHTML = "";
  instances.forEach((i) => {
    const o = document.createElement("option");
    o.value = i.name;
    o.textContent = i.name;
    sel.appendChild(o);
  });
  if (instances.length) loadBackups(instances[0].name);
  $("rb-instance").addEventListener("change", () => loadBackups($("rb-instance").value));
}

async function loadBackups(name) {
  try {
    const revs = await call("sync_list_backups", { name });
    const sel = $("rb-revision");
    sel.innerHTML = "";
    revs.forEach((r) => {
      const o = document.createElement("option");
      o.value = r;
      o.textContent = r;
      sel.appendChild(o);
    });
  } catch (e) {}
}

$("btn-rollback").addEventListener("click", async () => {
  const name = $("rb-instance").value;
  const rev = $("rb-revision").value;
  if (!name || !rev) { status("settings-status", "No hay revisiones."); return; }
  if (!confirm(`Restaurar la revisión ${rev} de '${name}'? Se sobrescribirá el estado actual.`)) return;
  try {
    await call("sync_rollback", { name, revision: rev });
    status("settings-status", "Rollback completado.");
    loadInstances();
  } catch (e) { status("settings-status", "Error: " + e.message); }
});
async function launchInstance(inst) {
  showPage("console");
  status("game-state", "Preparando lanzamiento de '" + inst.name + "'...");
  const settings = await call("settings_get", {}).catch(() => null) || {};
  let javaPath = (settings.java_path_override || "").trim();
  if (!javaPath) {
    status("game-state", "Buscando el Java que necesita " + (inst.game_version || "?") + "...");
    awaitingJava = true;
    try {
      const j = await call("java_resolve", { name: inst.name });
      javaPath = j.path;
    } catch (e) {
      status("game-state", "No se pudo obtener Java: " + e.message);
      return;
    } finally {
      awaitingJava = false;
    }
  }
  if (settings.auto_update && inst.remote) {
    try {
      const diff = await call("sync_check", { name: inst.name });
      if (!diff.up_to_date && diff.change_count > 0) {
        status("game-state", "Actualizando antes de jugar...");
        await call("sync_apply", { name: inst.name });
        status("game-state", `Actualizado (revisión ${diff.revision}). Lanzando...`);
      }
    } catch (e) {
      status("game-state", "Aviso: no se pudo auto-actualizar (" + e.message + ")");
    }
  }
  status("game-state", "Lanzando...");
  try {
    const pid = await call("launch_game", { name: inst.name, javaPath });
    status("game-state", `Juego lanzado (PID ${pid}).`);
    if (DEMO) {
      const lines = [
        "[main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.16.14",
        "[main/INFO]: Launched version " + inst.name,
        "[main/INFO]: 45 mods loaded (12 new, 2 updated)",
        "[main/WARN]: Some legacy options were deprecated",
        "[main/INFO]: Backend library: Atalauncher/0.1 (Minecraft client 1.21.4)",
        "[Render thread/INFO]: Setting user: elusuario_de_prueba",
        "[Render thread/INFO]: Backend library: connecting to play.kazserver.gg...",
        "[Render thread/INFO]: Connected to play.kazserver.gg:25565",
      ];
      lines.forEach((l, i) => setTimeout(() => appendLog(l, null), i * 350));
    }
  } catch (e) {
    status("game-state", "Error al lanzar: " + e.message);
    refreshLogin();
  }
}

$("btn-launch").addEventListener("click", async () => {
  if (!currentInstance && instances.length > 0) currentInstance = instances[0];
  if (!currentInstance) { status("game-state", "Crea una instancia primero."); return; }
  launchInstance(currentInstance);
});

$("btn-kill").addEventListener("click", async () => {
  try { await call("launch_kill", {}); status("game-state", "Juego detenido."); }
  catch (e) { status("game-state", e.message); }
});

consoleBuf.push("— consola vacía —");
$("console").textContent = "— consola vacía —\n";
EVENT.listen("launch/line", (data) => {
  appendLog(data.payload.line, data.payload.stream);
});
EVENT.listen("launch/exit", () => {
  appendLog("— el juego terminó —", null);
  status("game-state", "El juego terminó.");
});
EVENT.listen("launch/error", (data) => status("game-state", data.payload));
EVENT.listen("sync/status", (data) => status("instances-status", data.payload));
EVENT.listen("sync/progress", (data) => setInstallProgress(data.payload.percent || 0, data.payload.status || ""));
let awaitingJava = false;
EVENT.listen("java/progress", (data) => {
  status("settings-status", data.payload);
  if (awaitingJava) status("game-state", data.payload);
});
const donateBtn = $("donate-btn");
if (donateBtn) {
  donateBtn.addEventListener("click", () => {
    const url = "https://ko-fi.com/nexuscreatorsjr";
    if (DEMO) {
      window.open(url, "_blank");
    } else {
      call("open_url", { url }).catch((e) => {
        try { window.open(url, "_blank"); } catch (_) {}
      });
    }
  });
}
function initTheme() {
  try {
    const saved = localStorage.getItem("ata_theme");
    const prefersDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
    if (saved === "dark" || (!saved && prefersDark)) {
      document.body.classList.add("dark");
    } else {
      document.body.classList.remove("dark");
    }
  } catch (_) {}
}

const themeToggle = $("theme-toggle");
if (themeToggle) {
  themeToggle.addEventListener("click", () => {
    const isDark = document.body.classList.toggle("dark");
    try {
      localStorage.setItem("ata_theme", isDark ? "dark" : "light");
    } catch (_) {}
  });
}
function checkLauncherUpdate() {
  const chip = $("update-chip");
  if (!chip) return;
  call("update_check", {}).then(async (info) => {
    chip.textContent = "";
    const ver = document.createElement("span");
    ver.textContent = `Atalauncher v${info.current || "?"}`;
    chip.appendChild(ver);
    if (info.available && info.latest && info.latest !== info.current) {
      const sep = document.createElement("span");
      sep.textContent = " · ";
      const link = document.createElement("a");
      link.href = "#";
      link.className = "up";
      link.textContent = `actualización disponible (v${info.latest})`;
      const apply = async () => {
        if (DEMO) { showModal(`<h2>Actualización v${escapeHtml(info.latest)}</h2><div class="group">Demo: se descargaría, instalaría y reiniciaría automáticamente.</div>`); return; }
        chip.textContent = "descargando actualización…";
        link.classList.remove("clickable");
        try {
          await call("update_apply", {});
          chip.textContent = "instalando y reiniciando…";
        } catch (err) {
          chip.textContent = "";
          chip.append(ver);
          showModal(`<h2>Error al actualizar</h2><pre class="mono">${escapeHtml(err.message)}</pre>`);
        }
      };
      link.addEventListener("click", async (e) => {
        e.preventDefault();
        e.stopPropagation();
        await apply();
      });
      chip.classList.add("clickable");
      chip.appendChild(sep);
      chip.appendChild(link);
      try {
        const s = await getSettings();
        if (s.auto_update) {
          chip.textContent = "";
          chip.append(ver);
          const note = document.createElement("span");
          note.textContent = " · " + (DEMO ? "actualización detectada" : "descargando actualización…");
          chip.appendChild(note);
          await apply();
        }
      } catch (_) {}
    }
  }).catch((err) => {
    chip.textContent = `Atalauncher v${DEMO ? "0.1.0" : "?"} · sin conexión`;
    if (chip.title) chip.title = err.message;
  });
}
initTheme();
refreshLogin();
loadInstances();
runAutoSync(true);
checkLauncherUpdate();
call("discord_set_page", { page: "login" }).catch(() => {});