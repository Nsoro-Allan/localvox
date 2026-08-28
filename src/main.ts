import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface HardwareInfo {
  total_ram_gb: number;
  cpu_cores: number;
  is_apple_silicon: boolean;
  gpu_name: string | null;
  tier: string;
}

interface ModelInfo {
  id: string;
  size_mb: number;
  downloaded: boolean;
}

const statusDot = document.querySelector<HTMLSpanElement>("#status-dot")!;
const statusText = document.querySelector<HTMLSpanElement>("#status-text")!;
const hardwareInfo = document.querySelector<HTMLDListElement>("#hardware-info")!;
const modelList = document.querySelector<HTMLDivElement>("#model-list")!;
const modelProgress = document.querySelector<HTMLParagraphElement>("#model-progress")!;

function setStatus(state: string, label: string) {
  statusDot.className = `status-dot ${state}`;
  statusText.textContent = label;
}

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

async function loadModels() {
  const [models, current] = await Promise.all([
    invoke<ModelInfo[]>("list_models"),
    invoke<string>("get_current_model"),
  ]);

  modelList.innerHTML = "";
  for (const model of models) {
    const row = document.createElement("div");
    row.className = "model-row";

    const label = document.createElement("span");
    label.textContent = `${model.id} — ${(model.size_mb / 1000).toFixed(1)} GB${model.downloaded ? "" : " (not downloaded)"}`;

    const button = document.createElement("button");
    if (model.id === current) {
      button.textContent = "Active";
      button.disabled = true;
    } else {
      button.textContent = model.downloaded ? "Switch" : "Download & switch";
      button.addEventListener("click", () => switchModel(model.id));
    }

    row.append(label, button);
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

listen<string>("pipeline-status", (event) => {
  const payload = event.payload;
  let state = "idle";
  if (payload.includes("listening")) state = "listening";
  else if (payload.includes("transcribing")) state = "transcribing";
  setStatus(state, payload);
});

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

interface ReplacementRule { find: string; replace: string; }

const vocabInput = document.querySelector<HTMLTextAreaElement>("#vocab-input")!;
const saveVocabBtn = document.querySelector<HTMLButtonElement>("#save-vocab")!;
const replacementRows = document.querySelector<HTMLDivElement>("#replacement-rows")!;
const addRuleBtn = document.querySelector<HTMLButtonElement>("#add-rule")!;
const saveRulesBtn = document.querySelector<HTMLButtonElement>("#save-rules")!;

async function loadVocabulary() {
  const words = await invoke<string[]>("get_vocabulary");
  vocabInput.value = words.join("\n");
}

saveVocabBtn.addEventListener("click", async () => {
  const words = vocabInput.value.split("\n").map((w) => w.trim()).filter((w) => w.length > 0);
  await invoke("set_vocabulary", { words });
});

function addRuleRow(find = "", replace = "") {
  const row = document.createElement("div");
  row.className = "replacement-row";

  const findInput = document.createElement("input");
  findInput.type = "text";
  findInput.placeholder = "heard as...";
  findInput.value = find;

  const arrow = document.createElement("span");
  arrow.className = "arrow";
  arrow.textContent = "→";

  const replaceInput = document.createElement("input");
  replaceInput.type = "text";
  replaceInput.placeholder = "correct to...";
  replaceInput.value = replace;

  const removeBtn = document.createElement("button");
  removeBtn.type = "button";
  removeBtn.className = "remove-rule";
  removeBtn.textContent = "✕";
  removeBtn.addEventListener("click", () => row.remove());

  row.append(findInput, arrow, replaceInput, removeBtn);
  replacementRows.append(row);
}

async function loadReplacements() {
  const rules = await invoke<ReplacementRule[]>("get_replacements");
  replacementRows.innerHTML = "";
  for (const rule of rules) addRuleRow(rule.find, rule.replace);
}

addRuleBtn.addEventListener("click", () => addRuleRow());

saveRulesBtn.addEventListener("click", async () => {
  const rows = Array.from(replacementRows.querySelectorAll<HTMLDivElement>(".replacement-row"));
  const rules: ReplacementRule[] = rows
    .map((row) => {
      const inputs = row.querySelectorAll<HTMLInputElement>("input[type=text]");
      return { find: inputs[0].value.trim(), replace: inputs[1].value.trim() };
    })
    .filter((r) => r.find.length > 0);
  await invoke("set_replacements", { rules });
});

loadVocabulary();
loadReplacements();
loadHotkey();
loadHardware();
loadModels();