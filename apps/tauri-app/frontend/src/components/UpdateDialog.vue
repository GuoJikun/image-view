<script setup lang="ts">
/**
 * 检查更新弹窗（460 px）— tauri-plugin-updater。
 * 状态机：idle → checking → [up-to-date | available | error]
 *          available → downloading → [done | error]
 *          done → relaunch
 */
import { onMounted, reactive, ref } from "vue";
import { t } from "../i18n";
import { APP_VERSION } from "../constants";
import * as ipc from "../services/ipc";

type Phase = "idle" | "checking" | "up-to-date" | "available" | "downloading" | "done" | "error";

interface UpdateMeta {
  version: string;
  body: string | null;
  date: string | null;
}

const emit = defineEmits<{ (event: "close"): void }>();
const closeButton = ref<HTMLButtonElement | null>(null);

const phase = ref<Phase>("idle");
const info = ref<UpdateMeta | null>(null);
const errorMessage = ref("");
const progress = reactive({ downloaded: 0, total: 0 });

onMounted(() => {
  closeButton.value?.focus();
  void check();
});

async function check(): Promise<void> {
  phase.value = "checking";
  errorMessage.value = "";
  info.value = null;
  try {
    const result = await ipc.checkUpdate();
    if (result) {
      info.value = {
        version: result.version,
        body: result.body ?? null,
        date: result.date ?? null,
      };
      phase.value = "available";
    } else {
      phase.value = "up-to-date";
    }
  } catch (caught) {
    errorMessage.value = ipc.normalizeError(caught).message;
    phase.value = "error";
  }
}

async function downloadAndInstall(): Promise<void> {
  phase.value = "downloading";
  progress.downloaded = 0;
  progress.total = 0;
  try {
    await ipc.downloadAndInstallUpdate((event) => {
      if (event.event === "Started") {
        progress.total = event.data?.contentLength ?? 0;
      } else if (event.event === "Progress") {
        progress.downloaded += event.data?.chunkLength ?? 0;
      }
    });
    phase.value = "done";
  } catch (caught) {
    errorMessage.value = ipc.normalizeError(caught).message;
    phase.value = "error";
  }
}

async function relaunch(): Promise<void> {
  try {
    await ipc.relaunchApp();
  } catch {
    // 回退：退出应用，用户手动重启。
    await ipc.exitApp(0).catch(() => undefined);
    emit("close");
  }
}

function progressPercent(): number {
  if (!progress.total || progress.total <= 0) return 0;
  return Math.min(100, Math.round((progress.downloaded / progress.total) * 100));
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div class="dialog-backdrop" @pointerdown.self="emit('close')">
    <section
      class="dialog update-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-title"
    >
      <header class="dialog-header">
        <h2 id="update-title">{{ t("dialog.update") }}</h2>
      </header>

      <div class="dialog-body">
        <!-- 当前版本 -->
        <p class="update-version">
          <span>{{ t("dialog.version") }}</span>
          <strong>{{ APP_VERSION }}</strong>
        </p>

        <!-- 检查中 -->
        <div v-if="phase === 'checking'" class="update-status">
          <span class="update-spinner" aria-hidden="true" />
          <span>{{ t("update.checking") }}</span>
        </div>

        <!-- 已最新 -->
        <div v-else-if="phase === 'up-to-date'" class="update-status">
          <span class="update-check-mark" aria-hidden="true">✓</span>
          <span>{{ t("update.upToDate") }}</span>
        </div>

        <!-- 发现新版本 -->
        <template v-else-if="phase === 'available' && info">
          <div class="update-status update-new">
            <strong>{{ t("update.available") }} — {{ info.version }}</strong>
          </div>
          <div v-if="info.body" class="update-notes">
            <h3>{{ t("update.notes") }}</h3>
            <p>{{ info.body }}</p>
          </div>
        </template>

        <!-- 下载安装中 -->
        <template v-else-if="phase === 'downloading'">
          <div class="update-status">
            <span class="update-spinner" aria-hidden="true" />
            <span>{{ t("update.downloading") }}</span>
            <span v-if="progress.total" class="update-percent">
              {{ progressPercent() }}%
            </span>
          </div>
          <div class="update-progress-track">
            <div
              class="update-progress-fill"
              :style="{ width: `${progressPercent()}%` }"
            />
          </div>
          <p class="update-progress-detail">
            {{ formatBytes(progress.downloaded) }}
            <template v-if="progress.total"> / {{ formatBytes(progress.total) }}</template>
          </p>
        </template>

        <!-- 完成 -->
        <div v-else-if="phase === 'done'" class="update-status">
          <span class="update-check-mark" aria-hidden="true">✓</span>
          <div>
            <strong>{{ t("update.ready") }}</strong>
            <p class="update-ready-hint">{{ t("update.readyHint") }}</p>
          </div>
        </div>

        <!-- 错误 -->
        <div v-else-if="phase === 'error'" class="update-status update-error">
          <span class="update-error-mark" aria-hidden="true">⚠</span>
          <div>
            <strong>{{ t("update.error") }}</strong>
            <p>{{ errorMessage }}</p>
          </div>
        </div>
      </div>

      <footer class="dialog-actions">
        <!-- 错误：重试 + 关闭 -->
        <template v-if="phase === 'error'">
          <button type="button" class="btn btn-secondary" @click="check">
            {{ t("update.retry") }}
          </button>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
        </template>

        <!-- 完成：重启 -->
        <template v-else-if="phase === 'done'">
          <button type="button" class="btn btn-secondary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="relaunch">
            {{ t("update.install") }}
          </button>
        </template>

        <!-- 可用：下载安装 + 关闭 -->
        <template v-else-if="phase === 'available'">
          <button type="button" class="btn btn-secondary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="downloadAndInstall">
            {{ t("update.download") }}
          </button>
        </template>

        <!-- idle / checking / up-to-date / downloading：关闭 -->
        <template v-else>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
        </template>
      </footer>
    </section>
  </div>
</template>
