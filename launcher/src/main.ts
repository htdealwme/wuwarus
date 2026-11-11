import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface Status {
  game_path: string | null;
  game_ok: boolean;
  installed_version: string | null;
  installed_ok: boolean;
  risk_accepted: boolean;
  repo: string;
}

interface Manifest {
  version: string;
  game_version: string | null;
  notes: string | null;
  files: { name: string; size: number }[];
}

interface Progress {
  file: string;
  done: number;
  total: number;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

const ui = {
  repo: $("repo"),
  risk: $("risk"),
  acceptRisk: $<HTMLButtonElement>("accept-risk"),
  gamePath: $("game-path"),
  installed: $("installed"),
  latest: $("latest"),
  notes: $("notes"),
  detect: $<HTMLButtonElement>("detect"),
  pick: $<HTMLButtonElement>("pick"),
  refresh: $<HTMLButtonElement>("refresh"),
  progress: $("progress"),
  bar: $("bar"),
  progressText: $("progress-text"),
  msg: $("msg"),
  uninstall: $<HTMLButtonElement>("uninstall"),
  install: $<HTMLButtonElement>("install"),
  installLabel: $("install-label"),
  play: $<HTMLButtonElement>("play"),
};

let status: Status | null = null;
let manifest: Manifest | null = null;
let busy = false;

const mb = (n: number) => (n / 1024 / 1024).toFixed(1) + " МБ";

function say(text: string, bad = false) {
  ui.msg.textContent = text;
  ui.msg.classList.toggle("is-bad", bad);
}

function setValue(el: HTMLElement, text: string, tone: "" | "is-bad" | "is-warn" = "") {
  el.textContent = text;
  el.title = text;
  el.classList.remove("is-bad", "is-warn");
  if (tone) el.classList.add(tone);
}

function render() {
  const s = status;
  if (!s) return;

  ui.repo.textContent = `github.com/${s.repo}`;
  ui.risk.hidden = s.risk_accepted;

  if (!s.game_path) setValue(ui.gamePath, "не указана", "is-warn");
  else if (!s.game_ok) setValue(ui.gamePath, `${s.game_path} — игры там больше нет`, "is-bad");
  else setValue(ui.gamePath, s.game_path);

  if (!s.installed_version) setValue(ui.installed, "ничего");
  else if (!s.installed_ok) setValue(ui.installed, `${s.installed_version} — файлы пропали, переустановите`, "is-warn");
  else setValue(ui.installed, s.installed_version);

  const upToDate =
    !!manifest && s.installed_ok && s.installed_version === manifest.version;
  ui.installLabel.textContent = !s.installed_version
    ? "Установить перевод"
    : upToDate
      ? "Переустановить"
      : "Обновить перевод";

  const installing = busy && ui.install.dataset.running === "1";
  ui.install.disabled = (busy && !installing) || !s.game_ok || !s.risk_accepted || !manifest;
  ui.uninstall.disabled = busy || !s.game_ok || !s.installed_version;
  ui.play.disabled = busy || !s.game_ok;
  ui.detect.disabled = ui.pick.disabled = ui.refresh.disabled = busy;
  ui.install.setAttribute("aria-busy", String(installing));
}

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  if (busy) return;
  busy = true;
  render();
  try {
    return await fn();
  } catch (e) {
    say(String(e), true);
  } finally {
    busy = false;
    render();
  }
}

async function loadManifest() {
  setValue(ui.latest, "проверяю…");
  ui.notes.textContent = "";
  try {
    manifest = await invoke<Manifest>("fetch_manifest");
    const size = manifest.files.reduce((a, f) => a + f.size, 0);
    const game = manifest.game_version ? ` · для игры ${manifest.game_version}` : "";
    setValue(ui.latest, `${manifest.version}${game} · ${mb(size)}`);
    ui.notes.textContent = manifest.notes ?? "";
    ui.notes.title = manifest.notes ?? "";
  } catch (e) {
    manifest = null;
    setValue(ui.latest, String(e), "is-warn");
  }
  render();
}

ui.acceptRisk.addEventListener("click", () =>
  run(async () => {
    status = await invoke<Status>("accept_risk");
  }),
);

ui.detect.addEventListener("click", () =>
  run(async () => {
    status = await invoke<Status>("detect_game");
    say("Игра найдена");
  }),
);

ui.pick.addEventListener("click", () =>
  run(async () => {
    const dir = await open({ directory: true, title: "Папка Wuthering Waves" });
    if (typeof dir !== "string") return;
    status = await invoke<Status>("set_game_path", { path: dir });
    say("Папка игры сохранена");
  }),
);

ui.refresh.addEventListener("click", () => run(loadManifest));

ui.install.addEventListener("click", () =>
  run(async () => {
    ui.install.dataset.running = "1";
    ui.progress.hidden = false;
    ui.bar.style.width = "0";
    ui.progressText.textContent = "Скачиваю…";
    say("");
    try {
      status = await invoke<Status>("install_translation");
      say(`Перевод ${status.installed_version} установлен. Язык текста в игре: English.`);
    } finally {
      ui.install.dataset.running = "";
      ui.progress.hidden = true;
    }
  }),
);

ui.uninstall.addEventListener("click", () =>
  run(async () => {
    status = await invoke<Status>("uninstall_translation");
    say("Перевод убран, игра снова на оригинальном языке");
  }),
);

ui.play.addEventListener("click", () =>
  run(async () => {
    await invoke("launch_game");
    say("Игра запускается…");
  }),
);

listen<Progress>("install-progress", ({ payload: p }) => {
  const pct = p.total ? (p.done / p.total) * 100 : 0;
  ui.bar.style.width = `${pct.toFixed(1)}%`;
  ui.progressText.textContent = `${p.file} · ${mb(p.done)} из ${mb(p.total)}`;
});

(async () => {
  status = await invoke<Status>("get_status");
  render();
  if (!status.game_path) {
    try {
      status = await invoke<Status>("detect_game");
    } catch {
      /* not found — the user picks the folder manually */
    }
    render();
  }
  await loadManifest();
})();
