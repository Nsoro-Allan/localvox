import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ---------- Types ----------

interface HardwareInfo {
  total_ram_gb: number;
  cpu_cores: number;
  is_apple_silicon: boolean;
  gpu_name: string | null;
  tier: string;
}

interface ModelInfo {
  id: string;
  display_name: string;
  size_mb: number;
  family: string;
  description: string;
  languages: string;
  recommended: boolean;
  downloaded: boolean;
  backend_ready: boolean;
}

interface ReplacementRule {
  find: string;
  replace: string;
}

interface HistoryEntry {
  id: string;
  text: string;
  created_at: string;
  source: string;
  file_path: string | null;
  starred: boolean;
}

interface PostProcessConfig {
  endpoint: string;
  model: string;
  prompt_template: string;
  enabled: boolean;
}

// ---------- Tabs ----------

document.querySelectorAll<HTMLButtonElement>(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.remove("active"));
    btn.classList.add("active");
    const panel = document.getElementById(`tab-${btn.dataset.tab}`);
    panel?.classList.add("active");
    if (btn.dataset.tab === "history") loadHistory();
  });
});

// ---------- Status ----------

const statusDot = document.querySelector<HTMLSpanElement>("#status-dot")!;
const statusText = document.querySelector<HTMLSpanElement>("#status-text")!;

function setStatus(state: string, label: string) {
  statusDot.className = `status-dot ${state}`;
  statusText.textContent = label;
}

listen<string>("pipeline-status", (event) => {
  const payload = event.payload;
  let state = "Running";
  if (payload.includes("listening")) state = "listening";
  else if (payload.includes("transcribing") || payload.includes("post-processing")) state = "transcribing";
  setStatus(state, payload);
});

const warningBanner = document.querySelector<HTMLDivElement>("#warning-banner")!;
const warningText = document.querySelector<HTMLSpanElement>("#warning-text")!;
document.querySelector("#dismiss-warning")!.addEventListener("click", () => {
  warningBanner.classList.add("hidden");
});
listen<string>("pipeline-warning", (event) => {
  warningText.textContent = event.payload;
  warningBanner.classList.remove("hidden");
});

// ---------- Hardware ----------

const hardwareInfo = document.querySelector<HTMLDListElement>("#hardware-info")!;

async function loadHardware() {
  const info = await invoke<HardwareInfo>("get_hardware_info");
  const gpu = info.gpu_name ?? (info.is_apple_silicon ? "Apple Silicon (unified memory)" : "None detected");
  hardwareInfo.innerHTML = `
    <dt>RAM</dt><dd>${info.total_ram_gb.toFixed(1)} GB</dd>
    <dt>CPU cores</dt><dd>${info.cpu_cores}</dd>
    <dt>GPU</dt><dd>${gpu}</dd>
    <dt>Recommended tier</dt><dd>${info.tier}</dd>
  `;
}

// ---------- Models ----------

const modelList = document.querySelector<HTMLDivElement>("#model-list")!;
const modelProgress = document.querySelector<HTMLParagraphElement>("#model-progress")!;

async function loadModels() {
  const [models, current] = await Promise.all([
    invoke<ModelInfo[]>("list_models"),
    invoke<string>("get_current_model"),
  ]);

  modelList.innerHTML = "";
  for (const model of models) {
    const row = document.createElement("div");
    row.className = "model-row";
    row.style.flexDirection = "column";
    row.style.alignItems = "flex-start";
    row.style.gap = "6px";

    const top = document.createElement("div");
    top.style.display = "flex";
    top.style.width = "100%";
    top.style.justifyContent = "space-between";
    top.style.alignItems = "center";

    const label = document.createElement("div");
    const title = document.createElement("div");
    title.textContent = model.display_name;
    if (model.recommended) {
      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = "recommended";
      title.appendChild(badge);
    }
    const meta = document.createElement("div");
    meta.className = "model-meta";
    meta.textContent = `${model.family} · ${model.size_mb} MB · ${model.languages}${model.downloaded ? "" : " · not downloaded"}`;
    label.append(title, meta);

    const button = document.createElement("button");
    if (model.id === current) {
      button.textContent = "Active";
      button.disabled = true;
    } else {
      button.textContent = model.downloaded ? "Switch" : "Download & switch";
      button.addEventListener("click", () => switchModel(model.id));
    }

    top.append(label, button);
    row.append(top);

    const desc = document.createElement("div");
    desc.className = "model-meta";
    desc.textContent = model.description;
    row.append(desc);

    modelList.append(row);
  }
}

async function switchModel(modelId: string) {
  modelProgress.textContent = "Starting…";
  try {
    await invoke("switch_model", { modelId });
    modelProgress.textContent = "";
    await loadModels();
  } catch (e) {
    modelProgress.textContent = `Failed: ${e}`;
  }
}

listen<string>("model-switch-progress", (event) => {
  modelProgress.textContent = event.payload;
});

// ---------- Hotkey ----------

const modCtrl = document.querySelector<HTMLInputElement>("#mod-ctrl")!;
const modAlt = document.querySelector<HTMLInputElement>("#mod-alt")!;
const modShift = document.querySelector<HTMLInputElement>("#mod-shift")!;
const triggerKey = document.querySelector<HTMLSelectElement>("#trigger-key")!;
const saveHotkeyBtn = document.querySelector<HTMLButtonElement>("#save-hotkey")!;
const hotkeyStatus = document.querySelector<HTMLParagraphElement>("#hotkey-status")!;

function applyComboToInputs(combo: string) {
  const parts = combo.split("+");
  const key = parts[parts.length - 1];
  const mods = parts.slice(0, -1).map((m) => m.toUpperCase());
  modCtrl.checked = mods.includes("CTRL");
  modAlt.checked = mods.includes("ALT");
  modShift.checked = mods.includes("SHIFT");
  triggerKey.value = key;
}

function buildCombo(): string {
  const mods: string[] = [];
  if (modCtrl.checked) mods.push("Ctrl");
  if (modAlt.checked) mods.push("Alt");
  if (modShift.checked) mods.push("Shift");
  mods.push(triggerKey.value);
  return mods.join("+");
}

async function loadHotkey() {
  const combo = await invoke<string>("get_hotkey");
  applyComboToInputs(combo);
}

saveHotkeyBtn.addEventListener("click", async () => {
  const combo = buildCombo();
  hotkeyStatus.textContent = "Saving…";
  try {
    await invoke("set_hotkey", { combo });
    hotkeyStatus.textContent = `Now listening for ${combo}`;
  } catch (e) {
    hotkeyStatus.textContent = `Failed: ${e}`;
  }
});

// ---------- Vocabulary ----------

const vocabInput = document.querySelector<HTMLTextAreaElement>("#vocab-input")!;
const saveVocabBtn = document.querySelector<HTMLButtonElement>("#save-vocab")!;

async function loadVocabulary() {
  const words = await invoke<string[]>("get_vocabulary");
  vocabInput.value = words.join("\n");
}

saveVocabBtn.addEventListener("click", async () => {
  const words = vocabInput.value.split("\n").map((w) => w.trim()).filter((w) => w.length > 0);
  await invoke("set_vocabulary", { words });
});

// ---------- Corrections ----------

const replacementRows = document.querySelector<HTMLDivElement>("#replacement-rows")!;
const addRuleBtn = document.querySelector<HTMLButtonElement>("#add-rule")!;
const saveRulesBtn = document.querySelector<HTMLButtonElement>("#save-rules")!;

function addRuleRow(find = "", replace = "") {
  const row = document.createElement("div");
  row.className = "hotkey-row";
  row.style.marginBottom = "6px";
  const findInput = document.createElement("input");
  findInput.type = "text";
  findInput.placeholder = "find";
  findInput.value = find;
  findInput.style.flex = "1";
  const replaceInput = document.createElement("input");
  replaceInput.type = "text";
  replaceInput.placeholder = "replace";
  replaceInput.value = replace;
  replaceInput.style.flex = "1";
  const remove = document.createElement("button");
  remove.textContent = "×";
  remove.addEventListener("click", () => row.remove());
  row.append(findInput, replaceInput, remove);
  replacementRows.append(row);
}

addRuleBtn.addEventListener("click", () => addRuleRow());

async function loadReplacements() {
  const rules = await invoke<ReplacementRule[]>("get_replacements");
  replacementRows.innerHTML = "";
  if (rules.length === 0) addRuleRow();
  else rules.forEach((r) => addRuleRow(r.find, r.replace));
}

saveRulesBtn.addEventListener("click", async () => {
  const rules: ReplacementRule[] = [];
  replacementRows.querySelectorAll("div").forEach((row) => {
    const inputs = row.querySelectorAll("input");
    if (inputs.length >= 2 && inputs[0].value.trim()) {
      rules.push({ find: inputs[0].value.trim(), replace: inputs[1].value });
    }
  });
  await invoke("set_replacements", { rules });
});

// ---------- History ----------

const historyList = document.querySelector<HTMLDivElement>("#history-list")!;

async function loadHistory() {
  const entries = await invoke<HistoryEntry[]>("get_history", { limit: 50 });
  historyList.innerHTML = "";
  if (entries.length === 0) {
    historyList.innerHTML = `<p class="hint">No transcripts yet.</p>`;
    return;
  }
  for (const e of entries) {
    const item = document.createElement("div");
    item.className = "history-item";
    const meta = document.createElement("div");
    meta.className = "history-meta";
    const when = new Date(e.created_at).toLocaleString();
    meta.innerHTML = `<span>${when} · ${e.source}${e.starred ? " · ★" : ""}</span>`;
    const text = document.createElement("div");
    text.textContent = e.text;
    const actions = document.createElement("div");
    actions.className = "history-actions";
    const starBtn = document.createElement("button");
    starBtn.textContent = e.starred ? "Unstar" : "Star";
    starBtn.addEventListener("click", async () => {
      await invoke("star_history", { id: e.id, starred: !e.starred });
      loadHistory();
    });
    const delBtn = document.createElement("button");
    delBtn.textContent = "Delete";
    delBtn.className = "danger";
    delBtn.addEventListener("click", async () => {
      await invoke("delete_history", { id: e.id });
      loadHistory();
    });
    const copyBtn = document.createElement("button");
    copyBtn.textContent = "Copy";
    copyBtn.addEventListener("click", () => navigator.clipboard.writeText(e.text));
    actions.append(starBtn, copyBtn, delBtn);
    item.append(meta, text, actions);
    historyList.append(item);
  }
}

document.querySelector("#refresh-history")!.addEventListener("click", () => loadHistory());
document.querySelector("#clear-history")!.addEventListener("click", async () => {
  await invoke("clear_history");
  loadHistory();
});

// ---------- Post-processing ----------

const ppEnabled = document.querySelector<HTMLInputElement>("#pp-enabled")!;
const ppEndpoint = document.querySelector<HTMLInputElement>("#pp-endpoint")!;
const ppModel = document.querySelector<HTMLInputElement>("#pp-model")!;
const ppPrompt = document.querySelector<HTMLTextAreaElement>("#pp-prompt")!;
const ppStatus = document.querySelector<HTMLParagraphElement>("#pp-status")!;

async function loadPostprocess() {
  const cfg = await invoke<PostProcessConfig>("get_postprocess");
  ppEnabled.checked = cfg.enabled;
  ppEndpoint.value = cfg.endpoint;
  ppModel.value = cfg.model;
  ppPrompt.value = cfg.prompt_template;
}

document.querySelector("#pp-save")!.addEventListener("click", async () => {
  const config: PostProcessConfig = {
    enabled: ppEnabled.checked,
    endpoint: ppEndpoint.value.trim(),
    model: ppModel.value.trim(),
    prompt_template: ppPrompt.value,
  };
  try {
    await invoke("set_postprocess", { config });
    ppStatus.textContent = "Saved.";
  } catch (e) {
    ppStatus.textContent = `Failed: ${e}`;
  }
});

document.querySelector("#pp-ping")!.addEventListener("click", async () => {
  ppStatus.textContent = "Checking…";
  const ok = await invoke<boolean>("ping_ollama", { endpoint: ppEndpoint.value.trim() });
  ppStatus.textContent = ok ? "Ollama is reachable." : "Could not reach endpoint.";
});

// ---------- File transcription ----------

// File browse (fills the path input; keeps manual typing)
const filePathInput = document.querySelector<HTMLInputElement>("#file-path")!;
const fileUpload = document.querySelector<HTMLInputElement>("#file-upload")!;
document.querySelector("#file-browse")!.addEventListener("click", () => fileUpload.click());
fileUpload.addEventListener("change", () => {
  const file = fileUpload.files?.[0] as File & { path?: string } | undefined;
  if (!file) return;
  // Tauri / Chromium expose a real filesystem path on File.path when available
  filePathInput.value = file.path || file.name;
});

document.querySelector("#transcribe-file")!.addEventListener("click", async () => {
  const path = (document.querySelector("#file-path") as HTMLInputElement).value.trim();
  const resultEl = document.querySelector<HTMLParagraphElement>("#file-result")!;
  const outputEl = document.querySelector<HTMLPreElement>("#file-output")!;
  if (!path) {
    resultEl.textContent = "Enter a path.";
    return;
  }
  resultEl.textContent = "Transcribing…";
  outputEl.classList.add("hidden");
  try {
    const text = await invoke<string>("transcribe_file", { path });
    resultEl.textContent = "Done.";
    outputEl.textContent = text;
    outputEl.classList.remove("hidden");
  } catch (e) {
    resultEl.textContent = `Failed: ${e}`;
  }
});

// ---------- Boot ----------

async function boot() {
  await Promise.all([
    loadHardware(),
    loadModels(),
    loadHotkey(),
    loadVocabulary(),
    loadReplacements(),
    loadPostprocess(),
  ]);
}

listen("settings-ready", () => {
  boot();
});

// Also try immediately in case settings-ready already fired
boot().catch(console.error);
