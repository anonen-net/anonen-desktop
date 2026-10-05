export const networkFailureKey = (audioSaved: boolean) =>
  audioSaved
    ? "errors.networkTranscriptionFailed"
    : "errors.networkTranscriptionFailedNotSaved";

export const audioTooLongGuideKey = (audioSaved: boolean) =>
  audioSaved
    ? "anonenCloud.toast.audioTooLongGuide"
    : "anonenCloud.toast.audioTooLongGuideNotSaved";

export const capRecordingKey = (audioSaved: boolean) =>
  audioSaved
    ? "anonenCloud.toast.capRecordingKept"
    : "anonenCloud.toast.capRecordingNotKept";

export const subscriptionToastWanted = (
  onboardingStep: string | null,
  historyOpenFromSignIn: boolean,
): boolean => {
  if (onboardingStep === null || onboardingStep === "accessibility") {
    return false;
  }
  if (onboardingStep === "auth") return historyOpenFromSignIn;
  return true;
};

export const NO_ACTIVE_SUBSCRIPTION = "no_active_subscription";

export const subscriptionWallKey = (lookupError: string | null): string =>
  lookupError === null || lookupError === NO_ACTIVE_SUBSCRIPTION
    ? "authOnboarding.noSubscription"
    : "authOnboarding.subscriptionCheckFailed";

const FAILURE_NOTICE_KEYS: Record<string, string> = {
  seal_unavailable: "errors.notice.seal_unavailable",
  seal_outdated: "errors.notice.seal_outdated",
  seal_failed: "errors.notice.seal_failed",
  seal_key_lost: "errors.notice.seal_key_lost",
  seal_outdated_after_send: "errors.notice.seal_outdated_after_send",
  seal_response_broken: "errors.notice.seal_response_broken",
  seal_refused: "errors.notice.seal_refused",
  plaintext_refused: "errors.notice.plaintext_refused",
  auth_failed: "errors.notice.auth_failed",
};

export const FAILURE_NOTICE_IDS = Object.keys(FAILURE_NOTICE_KEYS);

export const failureNoticeKey = (
  notice: string | null | undefined,
): string | null =>
  notice && Object.prototype.hasOwnProperty.call(FAILURE_NOTICE_KEYS, notice)
    ? FAILURE_NOTICE_KEYS[notice]
    : null;

export const ALREADY_NOTIFIED = "already-notified";

export const NOTICE_PREFIX = "notice:";

export const retryFailureToast = (
  error: unknown,
  fallback: string,
  translate: (key: string) => string,
): string | null => {
  const message =
    error instanceof Error ? error.message.trim() : String(error ?? "").trim();
  if (message === ALREADY_NOTIFIED) return null;
  if (message.startsWith(NOTICE_PREFIX)) {
    const key = failureNoticeKey(message.slice(NOTICE_PREFIX.length));
    return key ? translate(key) : fallback;
  }
  return message.length > 0 ? message : fallback;
};

export const SIGNED_OUT_CHECK_MS = 5_000;

export function withinMs<T, F>(
  promise: Promise<T>,
  ms: number,
  fallback: F,
): Promise<T | F> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(fallback), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      () => {
        clearTimeout(timer);
        resolve(fallback);
      },
    );
  });
}

export function capToastKeys(
  suspended: boolean,
  hasResetsAt: boolean,
): { title: string; timing: string } {
  if (suspended) {
    return {
      title: "anonenCloud.toast.capSuspended",
      timing: hasResetsAt
        ? "anonenCloud.toast.capResumesAt"
        : "anonenCloud.toast.capSuspendedWait",
    };
  }
  return {
    title: "anonenCloud.toast.capReached",
    timing: hasResetsAt
      ? "anonenCloud.toast.capResetsAt"
      : "anonenCloud.toast.capWait",
  };
}

let sessionEndedNote = false;

export function noteSessionEnded(): void {
  sessionEndedNote = true;
}

export function takeSessionEndedNote(): boolean {
  const noted = sessionEndedNote;
  sessionEndedNote = false;
  return noted;
}
