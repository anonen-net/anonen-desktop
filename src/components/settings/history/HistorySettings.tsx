import React, { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readFile } from "@tauri-apps/plugin-fs";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  FolderOpen,
  RotateCcw,
  Star,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  commands,
  events,
  type HistoryEntry,
  type HistoryUpdatePayload,
} from "@/bindings";
import { useOsType } from "@/hooks/useOsType";
import { formatDateTime } from "@/utils/dateFormat";
import {
  isActiveSubscriptionStatus,
  useAnonenCloudStore,
} from "@/stores/anonenCloudStore";
import { useModelStore } from "@/stores/modelStore";
import { retryFailureToast } from "@/lib/failureNotice";
import { AudioPlayer, AudioPlayerGroup } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";

const IconButton: React.FC<{
  onClick: () => void;
  title: string;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, disabled, active, children }) => (
  <button
    onClick={onClick}
    disabled={disabled}
    className={`p-1.5 rounded-md flex items-center justify-center transition-colors cursor-pointer disabled:cursor-not-allowed disabled:text-muted/40 ${
      active
        ? "text-logo-primary hover:text-logo-primary/80"
        : "text-muted hover:text-logo-primary"
    }`}
    title={title}
  >
    {children}
  </button>
);

const PAGE_SIZE = 30;

interface OpenRecordingsButtonProps {
  onClick: () => void;
  label: string;
}

const OpenRecordingsButton: React.FC<OpenRecordingsButtonProps> = ({
  onClick,
  label,
}) => (
  <Button
    onClick={onClick}
    variant="secondary"
    size="sm"
    className="flex items-center gap-2"
    title={label}
  >
    <FolderOpen className="w-4 h-4" />
    <span>{label}</span>
  </Button>
);

export const HistorySettings: React.FC = () => {
  useEffect(() => {
    commands.prefetchEnclaveKey().catch(() => {});
  }, []);

  const { t } = useTranslation();
  const osType = useOsType();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);

  const [savedOnly, setSavedOnly] = useState(false);
  const savedOnlyRef = useRef(savedOnly);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);

  const generationRef = useRef(0);

  const signedIn = useAnonenCloudStore((s) => s.authStatus.signed_in);
  const subscriptionStatus = useAnonenCloudStore(
    (s) => s.usage?.subscription?.status ?? null,
  );
  const [persistedStatus, setPersistedStatus] = useState<string | null>(null);
  useEffect(() => {
    commands
      .anonenCloudCachedSubscriptionStatus()
      .then((result) => {
        if (result.status === "ok") setPersistedStatus(result.data);
      })
      .catch(() => {});
  }, []);
  const canRetranscribe =
    signedIn &&
    isActiveSubscriptionStatus(subscriptionStatus ?? persistedStatus);

  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    savedOnlyRef.current = savedOnly;
  }, [savedOnly]);

  const loadPage = useCallback(
    async (cursor?: number) => {
      const isFirstPage = cursor === undefined;
      if (!isFirstPage && loadingRef.current) return;
      if (isFirstPage) generationRef.current += 1;
      const generation = generationRef.current;
      loadingRef.current = true;

      if (isFirstPage) setLoading(true);

      try {
        const result = await commands.getHistoryEntries(
          cursor ?? null,
          PAGE_SIZE,
          savedOnly,
        );
        if (generation !== generationRef.current) return;
        if (result.status === "ok") {
          const { entries: newEntries, has_more } = result.data;
          setEntries((prev) =>
            isFirstPage ? newEntries : [...prev, ...newEntries],
          );
          setHasMore(has_more);
        }
      } catch (error) {
        console.error("Failed to load history entries:", error);
      } finally {
        if (generation === generationRef.current) {
          setLoading(false);
          loadingRef.current = false;
        }
      }
    },
    [savedOnly],
  );

  useEffect(() => {
    loadPage();
  }, [loadPage]);

  useEffect(() => {
    if (loading) return;

    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;

    const observer = new IntersectionObserver(
      (observerEntries) => {
        const first = observerEntries[0];
        if (first.isIntersecting) {
          const lastEntry = entriesRef.current[entriesRef.current.length - 1];
          if (lastEntry) {
            loadPage(lastEntry.id);
          }
        }
      },
      { threshold: 0 },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loading, hasMore, loadPage]);

  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen((event) => {
      const payload: HistoryUpdatePayload = event.payload;
      if (payload.action === "added") {
        if (savedOnlyRef.current && !payload.entry.saved) return;
        setEntries((prev) => [payload.entry, ...prev]);
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const toggleSaved = async (id: number) => {
    setEntries((prev) =>
      prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
    );
    try {
      const result = await commands.toggleHistoryEntrySaved(id);
      if (result.status !== "ok") {
        setEntries((prev) =>
          prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
        );
      }
    } catch (error) {
      console.error("Failed to toggle saved status:", error);

      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
      );
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const getAudioUrl = useCallback(
    async (fileName: string) => {
      try {
        const result = await commands.getAudioFilePath(fileName);
        if (result.status === "ok") {
          if (osType === "linux") {
            const fileData = await readFile(result.data);
            const blob = new Blob([fileData], { type: "audio/wav" });
            return URL.createObjectURL(blob);
          }
          return convertFileSrc(result.data, "asset");
        }
        return null;
      } catch (error) {
        console.error("Failed to get audio file path:", error);
        return null;
      }
    },
    [osType],
  );

  const deleteAudioEntry = async (id: number) => {
    setEntries((prev) => prev.filter((e) => e.id !== id));
    try {
      const result = await commands.deleteHistoryEntry(id);
      if (result.status !== "ok") {
        loadPage();
      }
    } catch (error) {
      console.error("Failed to delete entry:", error);
      loadPage();
    }
  };

  const retryHistoryEntry = async (id: number) => {
    const result = await commands.retryHistoryEntryTranscription(id);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }
  };

  const openRecordingsFolder = async () => {
    try {
      const result = await commands.openRecordingsFolder();
      if (result.status !== "ok") {
        throw new Error(String(result.error));
      }
    } catch (error) {
      console.error("Failed to open recordings folder:", error);
    }
  };

  const handleClearAll = async () => {
    const confirmed = await ask(t("settings.history.clearAllConfirm"), {
      title: t("settings.history.clearAllTitle"),
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      const result = await commands.clearAllHistory();
      if (result.status === "ok") {
        generationRef.current += 1;
        loadingRef.current = false;
        setLoading(false);
        setEntries([]);
        setHasMore(false);
      } else {
        throw new Error(String(result.error));
      }
    } catch (error) {
      console.error("Failed to clear all history:", error);
      toast.error(t("settings.history.clearAllError"));
    }
  };

  let content: React.ReactNode;

  if (loading) {
    content = (
      <div className="px-4 py-3 text-center text-muted">
        {t("settings.history.loading")}
      </div>
    );
  } else if (entries.length === 0) {
    content = (
      <div className="px-4 py-3 text-center text-muted">
        {savedOnly
          ? t("settings.history.favoritesEmpty")
          : t("settings.history.empty")}
      </div>
    );
  } else {
    content = (
      <>
        <AudioPlayerGroup>
          <div className="divide-y divide-mid-gray/20">
            {entries.map((entry) => (
              <HistoryEntryComponent
                key={entry.id}
                entry={entry}
                onToggleSaved={() => toggleSaved(entry.id)}
                onCopyText={() =>
                  copyToClipboard(
                    entry.post_processed_text ?? entry.transcription_text,
                  )
                }
                getAudioUrl={getAudioUrl}
                deleteAudio={deleteAudioEntry}
                retryTranscription={retryHistoryEntry}
                canRetranscribe={canRetranscribe}
              />
            ))}
          </div>
        </AudioPlayerGroup>

        <div ref={sentinelRef} className="h-1" />
      </>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-2">
      <div className="px-4 flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-3 shrink-0">
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.history.title")}
          </h2>

          <Button
            onClick={() => setSavedOnly((on) => !on)}
            variant={savedOnly ? "primary-soft" : "secondary"}
            size="sm"
            className="flex items-center gap-2"
            aria-pressed={savedOnly}
            title={t("settings.history.favoritesOnly")}
          >
            <Star
              className={`w-4 h-4 ${savedOnly ? "text-logo-primary" : ""}`}
              fill={savedOnly ? "currentColor" : "none"}
            />
            <span>{t("settings.history.favoritesOnly")}</span>
          </Button>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <OpenRecordingsButton
            onClick={openRecordingsFolder}
            label={t("settings.history.openFolder")}
          />
          <Button
            onClick={handleClearAll}
            variant="secondary"
            size="sm"
            className="flex items-center gap-2"
            title={t("settings.history.clearAll")}
          >
            <Trash2 className="w-4 h-4" />
            <span>{t("settings.history.clearAll")}</span>
          </Button>
        </div>
      </div>
      <div className="bg-surface border-2 border-border rounded-md shadow-hard-sm overflow-visible">
        {content}
      </div>
    </div>
  );
};

interface HistoryEntryProps {
  entry: HistoryEntry;
  onToggleSaved: () => void;
  onCopyText: () => void;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
  retryTranscription: (id: number) => Promise<void>;

  canRetranscribe: boolean;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  onToggleSaved,
  onCopyText,
  getAudioUrl,
  deleteAudio,
  retryTranscription,
  canRetranscribe,
}) => {
  const { t, i18n } = useTranslation();
  const [showCopied, setShowCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);

  const hasTranscription = entry.transcription_text.trim().length > 0;

  const displayText = entry.post_processed_text ?? entry.transcription_text;

  const getModelInfo = useModelStore((s) => s.getModelInfo);
  const modelLabel = (id: string) => getModelInfo(id)?.name ?? id;

  const pages = React.useMemo(
    () => [
      {
        text: displayText,
        model: entry.model,
        at: entry.transcribed_at ?? entry.timestamp,
      },
      ...[...entry.revisions].reverse().map((r) => ({
        text: r.text,
        model: r.model,
        at: r.transcribed_at ?? entry.timestamp,
      })),
    ],
    [
      displayText,
      entry.model,
      entry.revisions,
      entry.transcribed_at,
      entry.timestamp,
    ],
  );
  const [page, setPage] = useState(0);

  useEffect(() => {
    setPage(0);
  }, [entry.revisions.length]);
  const shown = pages[Math.min(page, pages.length - 1)];

  const handleLoadAudio = useCallback(
    () => getAudioUrl(entry.file_name),
    [getAudioUrl, entry.file_name],
  );

  const handleCopyText = () => {
    if (!hasTranscription) {
      return;
    }

    onCopyText();
    setShowCopied(true);
    setTimeout(() => setShowCopied(false), 2000);
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("settings.history.deleteError"));
    }
  };

  const handleRetranscribe = async () => {
    try {
      setRetrying(true);
      await retryTranscription(entry.id);
    } catch (error) {
      console.error("Failed to re-transcribe:", error);
      const message = retryFailureToast(
        error,
        t("settings.history.retranscribeError"),
        t,
      );
      if (message !== null) toast.error(message);
    } finally {
      setRetrying(false);
    }
  };

  const formattedDate = formatDateTime(String(shown.at), i18n.language);

  return (
    <div className="px-4 py-2 pb-5 flex flex-col gap-3">
      <div className="flex justify-between items-center">
        <div className="flex items-center gap-2 min-w-0">
          <p className="text-sm font-medium whitespace-nowrap">
            {formattedDate}
          </p>

          {shown.model && (
            <span
              className="text-[10px] text-muted truncate"
              title={shown.model}
            >
              {modelLabel(shown.model)}
            </span>
          )}
          {entry.post_processed_text && (
            <span className="text-[10px] font-medium uppercase text-logo-primary">
              {t("settings.history.postProcessed")}
            </span>
          )}
        </div>
        <div className="flex items-center">
          <IconButton
            onClick={handleCopyText}
            disabled={!hasTranscription || retrying}
            title={t("settings.history.copyToClipboard")}
          >
            {showCopied ? (
              <Check width={16} height={16} />
            ) : (
              <Copy width={16} height={16} />
            )}
          </IconButton>
          <IconButton
            onClick={onToggleSaved}
            disabled={retrying}
            active={entry.saved}
            title={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
          >
            <Star
              width={16}
              height={16}
              fill={entry.saved ? "currentColor" : "none"}
            />
          </IconButton>
          {canRetranscribe && entry.has_audio && (
            <>
              <IconButton
                onClick={handleRetranscribe}
                disabled={retrying}
                title={t("settings.history.retranscribe")}
              >
                <RotateCcw
                  width={16}
                  height={16}
                  style={
                    retrying
                      ? { animation: "spin 1s linear infinite reverse" }
                      : undefined
                  }
                />
              </IconButton>
            </>
          )}
          <IconButton
            onClick={handleDeleteEntry}
            disabled={retrying}
            title={t("settings.history.delete")}
          >
            <Trash2 width={16} height={16} />
          </IconButton>
        </div>
      </div>

      <p
        className={`italic text-sm pb-2 ${
          retrying
            ? ""
            : hasTranscription
              ? "text-text/90 select-text cursor-text whitespace-pre-wrap break-words"
              : "text-muted/80"
        }`}
        style={
          retrying
            ? { animation: "transcribe-pulse 3s ease-in-out infinite" }
            : undefined
        }
      >
        {retrying && (
          <style>{`
            @keyframes transcribe-pulse {
              0%, 100% { color: color-mix(in srgb, var(--color-text) 40%, transparent); }
              50% { color: color-mix(in srgb, var(--color-text) 90%, transparent); }
            }
          `}</style>
        )}
        {retrying
          ? t("settings.history.transcribing")
          : hasTranscription
            ? shown.text
            : t("settings.history.transcriptionFailed")}
      </p>

      {pages.length > 1 && !retrying && (
        <div className="flex items-center gap-2 pb-2 text-xs text-muted">
          <IconButton
            onClick={() => setPage((p) => Math.min(p + 1, pages.length - 1))}
            disabled={page >= pages.length - 1}
            title={t("settings.history.revisionOlder")}
          >
            <ChevronLeft width={14} height={14} />
          </IconButton>
          <span className="tabular-nums">
            {t("settings.history.revisionPosition", {
              index: page + 1,
              total: pages.length,
            })}
          </span>
          <IconButton
            onClick={() => setPage((p) => Math.max(p - 1, 0))}
            disabled={page === 0}
            title={t("settings.history.revisionNewer")}
          >
            <ChevronRight width={14} height={14} />
          </IconButton>
        </div>
      )}

      {entry.has_audio ? (
        <AudioPlayer onLoadRequest={handleLoadAudio} className="w-full" />
      ) : (
        <p className="text-xs text-muted pb-1">
          {t("settings.history.noAudio")}
        </p>
      )}
    </div>
  );
};
