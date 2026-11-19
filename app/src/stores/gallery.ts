/**
 * 图库状态：索引快照、筛选条件、分页与选中项。
 * 只读：所有操作经 `services/ipc.ts` 调用后端，不直接 invoke。
 */
import { computed, reactive, ref } from "vue";
import { defineStore } from "pinia";
import type { CommandError, GalleryEntry, GalleryFilter, GallerySnapshot } from "../types/ipc";
import * as ipc from "../services/ipc";

const PAGE_SIZE = 500;

function defaultFilter(): GalleryFilter {
  return {
    query: "",
    tags: [],
    format: "",
    minWidth: 0,
    maxWidth: 2147483647,
    fromDate: null,
    toDate: null,
    sort: "name",
  };
}

export const useGalleryStore = defineStore("gallery", () => {
  const dir = ref("");
  const entries = ref<GalleryEntry[]>([]);
  const total = ref(0);
  const offset = ref(0);
  const indexed = ref(0);
  const indexReady = ref(false);
  const degraded = ref(false);
  const loading = ref(false);
  const error = ref<CommandError | null>(null);
  const filter = reactive<GalleryFilter>(defaultFilter());
  /** 图库中选中的图片 id，用于「从图库打开」与对比槽位。 */
  const selectedIds = ref<string[]>([]);

  const hasMore = computed(() => entries.value.length < total.value);

  function applySnapshot(snapshot: GallerySnapshot, append: boolean): void {
    dir.value = snapshot.dir;
    total.value = snapshot.total;
    offset.value = snapshot.offset + snapshot.entries.length;
    indexed.value = snapshot.indexed;
    indexReady.value = snapshot.indexReady;
    degraded.value = snapshot.degraded;
    entries.value = append ? [...entries.value, ...snapshot.entries] : snapshot.entries;
    Object.assign(filter, snapshot.filter);
  }

  async function open(path: string): Promise<void> {
    loading.value = true;
    error.value = null;
    selectedIds.value = [];
    try {
      applySnapshot(await ipc.openGallery(path, { ...filter }), false);
    } catch (e) {
      error.value = e as CommandError;
    } finally {
      loading.value = false;
    }
  }

  async function refresh(): Promise<void> {
    if (!dir.value) return;
    loading.value = true;
    try {
      applySnapshot(await ipc.refreshGallery(), false);
    } catch (e) {
      error.value = e as CommandError;
    } finally {
      loading.value = false;
    }
  }

  async function search(): Promise<void> {
    loading.value = true;
    try {
      applySnapshot(await ipc.setGalleryFilter({ ...filter }), false);
    } catch (e) {
      error.value = e as CommandError;
    } finally {
      loading.value = false;
    }
  }

  async function loadMore(): Promise<void> {
    if (loading.value || !hasMore.value) return;
    loading.value = true;
    try {
      applySnapshot(await ipc.galleryPage(offset.value, PAGE_SIZE), true);
    } catch (e) {
      error.value = e as CommandError;
    } finally {
      loading.value = false;
    }
  }

  function toggleSelected(id: string, additive: boolean): void {
    if (!additive) {
      selectedIds.value = [id];
      return;
    }
    selectedIds.value = selectedIds.value.includes(id)
      ? selectedIds.value.filter((value) => value !== id)
      : [...selectedIds.value, id];
  }

  async function addTag(id: string, tag: string): Promise<void> {
    try {
      await ipc.addGalleryTag(id, tag);
      await refresh();
    } catch (e) {
      error.value = e as CommandError;
    }
  }

  async function removeTag(id: string, tag: string): Promise<void> {
    try {
      await ipc.removeGalleryTag(id, tag);
      await refresh();
    } catch (e) {
      error.value = e as CommandError;
    }
  }

  return {
    dir,
    entries,
    total,
    offset,
    indexed,
    indexReady,
    degraded,
    loading,
    error,
    filter,
    selectedIds,
    hasMore,
    open,
    refresh,
    search,
    loadMore,
    toggleSelected,
    addTag,
    removeTag,
  };
});
