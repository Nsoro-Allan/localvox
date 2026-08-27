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

loadHardware();
loadModels();