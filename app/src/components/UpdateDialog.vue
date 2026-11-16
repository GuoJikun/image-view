<script setup lang="ts">
/**
 * 检查更新弹窗（460 px）— tauri-plugin-updater + 自定义 Rust 命令。
 * 状态机：idle → checking → [up-to-date | available | error]
 *          available → downloading → [ready | error | available(取消)]
 *          ready → install（安装并退出）
 */
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { t } from "../i18n";
import * as ipc from "../services/ipc";
import { renderMarkdown } from "../services/markdown";
import AppScrollbar from "./AppScrollbar.vue";
import type { UpdateInfo, UpdateProgress } from "../types/ipc";

type Phase = "idle" | "checking" | "up-to-date" | "available" | "downloading" | "ready" | "error";

const emit = defineEmits<{ (event: "close"): void }>();
const closeButton = ref<HTMLButtonElement | null>(null);

const phase = ref<Phase>("idle");
const info = ref<UpdateInfo | null>(null);
const errorMessage = ref("");
const progress = reactive<UpdateProgress>({ received: 0, total: null });
/** 当前版本取自二进制（构建时嵌入的 `tauri.conf.json`），与更新检查用的是同一个值。 */
const appVersion = ref("");
/** Release 正文是 Markdown，渲染成受限 HTML 后再插入。 */
const notesHtml = computed(() => renderMarkdown(info.value?.body ?? ""));

let unlistenProgress: (() => void) | null = null;

async function loadVersion(): Promise<void> {
  try {
    appVersion.value = await getVersion();
  } catch {
    // 纯浏览器预览没有 Tauri runtime，留空即可。
  }
}

onMounted(async () => {
  closeButton.value?.focus();
  void loadVersion();
  try {
    unlistenProgress = await ipc.onUpdateProgress((payload) => {
      progress.received = payload.received;
      progress.total = payload.total;
    });
  } catch {
    // 浏览器预览没有事件系统。
  }
  void check();
});

onBeforeUnmount(() => unlistenProgress?.());

async function check(): Promise<void> {
  phase.value = "checking";
  errorMessage.value = "";
  info.value = null;
  try {
    const result = await ipc.checkUpdate();
    if (result) {
      info.value = result;
      phase.value = "available";
    } else {
      phase.value = "up-to-date";
    }
  } catch (caught) {
    errorMessage.value = ipc.normalizeError(caught).message;
    phase.value = "error";
  }
}

async function download(): Promise<void> {
  phase.value = "downloading";
  progress.received = 0;
  progress.total = null;
  try {
    await ipc.downloadUpdate();
    phase.value = "ready";
  } catch (caught) {
    const err = ipc.normalizeError(caught);
    if (err.code === "update_cancelled") {
      phase.value = "available";
    } else {
      errorMessage.value = err.message;
      phase.value = "error";
    }
  }
}

async function cancelDownload(): Promise<void> {
  try {
    await ipc.cancelUpdateDownload();
  } catch {
    // 忽略取消失败；download 命令会自行收尾。
  }
}

async function install(): Promise<void> {
  try {
    await ipc.installUpdate();
  } catch (caught) {
    errorMessage.value = ipc.normalizeError(caught).message;
    phase.value = "error";
  }
}

function progressPercent(): number {
  if (!progress.total || progress.total <= 0) return 0;
  return Math.min(100, Math.round((progress.received / progress.total) * 100));
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div class="dialog-backdrop">
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
          <strong>{{ appVersion }}</strong>
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
            <AppScrollbar direction="vertical" class="update-notes-scroll">
              <div class="markdown" v-html="notesHtml" />
            </AppScrollbar>
          </div>
        </template>

        <!-- 下载中 -->
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
            {{ formatBytes(progress.received) }}
            <template v-if="progress.total"> / {{ formatBytes(progress.total) }}</template>
          </p>
        </template>

        <!-- 就绪 -->
        <div v-else-if="phase === 'ready'" class="update-status">
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

        <!-- 下载中：取消 -->
        <template v-else-if="phase === 'downloading'">
          <button type="button" class="btn btn-secondary" @click="cancelDownload">
            {{ t("update.cancel") }}
          </button>
        </template>

        <!-- 就绪：安装并重启 -->
        <template v-else-if="phase === 'ready'">
          <button type="button" class="btn btn-secondary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="install">
            {{ t("update.install") }}
          </button>
        </template>

        <!-- 可用：下载 + 关闭 -->
        <template v-else-if="phase === 'available'">
          <button type="button" class="btn btn-secondary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="download">
            {{ t("update.download") }}
          </button>
        </template>

        <!-- idle / checking / up-to-date：关闭 -->
        <template v-else>
          <button ref="closeButton" type="button" class="btn btn-primary" @click="emit('close')">
            {{ t("dialog.close") }}
          </button>
        </template>
      </footer>
    </section>
  </div>
</template>
