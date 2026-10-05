import { useEffect, useState, useRef, type ReactNode } from "react";
import { toast, Toaster } from "sonner";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import MicClippingNotice from "./components/MicClippingNotice";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import {
  ModelStateEvent,
  RecordingErrorEvent,
  TranscriptionErrorEvent,
} from "./lib/types/events";
import {
  failureNoticeKey,
  networkFailureKey,
  NO_ACTIVE_SUBSCRIPTION,
  noteSessionEnded,
  SIGNED_OUT_CHECK_MS,
  subscriptionToastWanted,
  withinMs,
} from "./lib/failureNotice";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import { ModelDataPolicyDialog } from "./components/model-data-policy/ModelDataPolicyDialog";
import Onboarding, {
  AccessibilityOnboarding,
  AuthOnboarding,
  FirstRunOnboarding,
  ShortcutOnboarding,
} from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { HistorySettings } from "./components/settings";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { useModelStore } from "./stores/modelStore";
import {
  isActiveSubscriptionStatus,
  notifyCloudModelRetired,
  useAnonenCloudStore,
  registerFailureListeners,
  setSubscriptionToastOn,
} from "./stores/anonenCloudStore";
import { commands } from "@/bindings";
import { getLanguageDirection, initializeRTL } from "@/lib/utils/rtl";
import { markTutorialSeen, tutorialSeen } from "@/lib/tutorialSeen";
import {
  keepOnboardingStepValid,
  nextOnboardingStep,
  type OnboardingStep,
} from "@/lib/onboardingFlow";

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const { t, i18n } = useTranslation();
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );

  const shortcutShownRef = useRef(false);
  const [showHistoryFromAuth, setShowHistoryFromAuth] = useState(false);
  const [currentSection, setCurrentSection] =
    useState<SidebarSection>("models");
  const { settings, updateSetting } = useSettings();
  const direction = getLanguageDirection(i18n.language);
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);

  const contentScrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  useEffect(() => {
    registerFailureListeners();
  }, []);

  useEffect(() => {
    setSubscriptionToastOn(
      subscriptionToastWanted(onboardingStep, showHistoryFromAuth),
    );
  }, [onboardingStep, showHistoryFromAuth]);

  useEffect(() => {
    initializeRTL(i18n.language);
  }, [i18n.language]);

  useEffect(() => {
    if (platform() === "macos") return;
    Promise.all([
      commands.initializeEnigo(),
      commands.initializeShortcuts(),
    ]).catch((e) => console.warn("Failed to initialize shortcuts early:", e));
  }, []);

  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => {
        console.warn("Failed to initialize:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();

      useAnonenCloudStore
        .getState()
        .initialize()
        .catch((e) => {
          console.warn("Failed to initialize Anonen Cloud store:", e);
        });
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  useEffect(() => {
    contentScrollRef.current?.scrollTo({ top: 0 });
  }, [currentSection]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "not_signed_in") {
        toast.error(t("errors.notSignedInTitle"), {
          description: t("errors.notSignedIn"),
        });
        setCurrentSection("models");
      } else if (error_type === "no_active_subscription") {
        toast.error(t("errors.noActiveSubscriptionTitle"), {
          description: t("errors.noActiveSubscription"),
        });
        setCurrentSection("models");
      } else if (error_type === "no_model_selected") {
        toast.error(t("errors.noModelSelectedTitle"), {
          description: t("errors.noModelSelected"),
        });
        setCurrentSection("models");
      } else if (error_type === "cloud_model_unavailable") {
        notifyCloudModelRetired(null);
        setCurrentSection("models");
      } else if (error_type === "microphone_permission_denied") {
        const currentPlatform = platform();
        const platformKey = `errors.micPermissionDenied.${currentPlatform}`;
        const description = t(platformKey, {
          defaultValue: t("errors.micPermissionDenied.generic"),
        });
        toast.error(t("errors.micPermissionDeniedTitle"), { description });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else if (error_type === "network_unavailable") {
        toast.error(t("errors.networkUnavailableTitle"), {
          description: t("errors.networkUnavailable"),
        });
      } else {
        if (detail) console.error("Recording failed:", detail);
        toast.error(t("errors.recordingFailed"));
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  useEffect(() => {
    const unlisten = listen<TranscriptionErrorEvent>(
      "transcription-error",
      (event) => {
        console.error("Transcription error:", event.payload.message);
        const noticeKey = failureNoticeKey(event.payload.notice);
        if (event.payload.is_network_error) {
          toast.error(t("errors.networkUnavailableTitle"), {
            description: t(networkFailureKey(event.payload.audio_saved)),
          });
        } else if (noticeKey) {
          toast.error(t("errors.transcriptionFailedTitle"), {
            description: t(noticeKey),
            duration: 10000,
          });
        } else {
          toast.error(t("errors.transcriptionFailedTitle"), {
            description: t("errors.transcriptionFailed"),
          });
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        if (event.payload.error)
          console.error("Model load failed:", event.payload.error);
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: t("errors.modelLoadFailedDescription"),
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  useEffect(() => {
    const handleFocus = () => {
      useAnonenCloudStore.getState().refreshAuthStatus();
    };
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, []);

  const revealMainWindowForPermissions = async () => {
    try {
      await commands.showMainWindowCommand();
    } catch (e) {
      console.warn("Failed to show main window for permission onboarding:", e);
    }
  };

  const isSignedInWithValidSubscription = async (): Promise<boolean> => {
    try {
      const auth = await commands.anonenCloudAuthStatus();
      if (auth.status !== "ok") return false;
      if (!auth.data.signed_in) {
        if (auth.data.email) noteSessionEnded();
        return false;
      }
      const cached = await commands.anonenCloudCurrentUsage();
      if (cached.status === "ok" && cached.data?.subscription) {
        return isActiveSubscriptionStatus(cached.data.subscription.status);
      }
      const fresh = await commands.anonenCloudFetchUsage();
      if (fresh.status === "ok") {
        return isActiveSubscriptionStatus(
          fresh.data?.subscription?.status ?? null,
        );
      }

      if (fresh.error !== NO_ACTIVE_SUBSCRIPTION) {
        const again = await withinMs(
          commands.anonenCloudAuthStatus(),
          SIGNED_OUT_CHECK_MS,
          null,
        );
        if (again !== null && again.status === "ok" && !again.data.signed_in) {
          noteSessionEnded();
          return false;
        }
      }

      const persisted = await commands.anonenCloudCachedSubscriptionStatus();
      if (persisted.status === "ok") {
        return isActiveSubscriptionStatus(persisted.data);
      }
      return false;
    } catch {
      return false;
    }
  };

  const needsPermissions = async (): Promise<boolean> => {
    const currentPlatform = platform();
    if (currentPlatform === "macos") {
      try {
        const [hasA, hasM] = await Promise.all([
          checkAccessibilityPermission(),
          checkMicrophonePermission(),
        ]);
        return !hasA || !hasM;
      } catch {
        return false;
      }
    }
    if (currentPlatform === "windows") {
      try {
        const mic = await commands.getWindowsMicrophonePermissionStatus();
        return mic.supported && mic.overall_access === "denied";
      } catch {
        return false;
      }
    }
    return false;
  };

  const hasSelectedModel = async (): Promise<boolean> => {
    try {
      const result = await commands.getCurrentModel();
      return result.status === "ok" && result.data !== "";
    } catch {
      return false;
    }
  };

  const stepFor = (facts: {
    needsPermissions?: boolean;
    signedIn: boolean;
    modelSelected: boolean;
  }): OnboardingStep =>
    nextOnboardingStep({
      needsPermissions: facts.needsPermissions ?? false,
      signedIn: facts.signedIn,
      modelSelected: facts.modelSelected,
      tutorialSeen: tutorialSeen(),
      shortcutShown: shortcutShownRef.current,
    });

  const checkOnboardingStatus = async () => {
    try {
      const permissionsNeeded = await needsPermissions();

      const authOk =
        !permissionsNeeded && (await isSignedInWithValidSubscription());

      const step = stepFor({
        needsPermissions: permissionsNeeded,
        signedIn: authOk,
        modelSelected: await hasSelectedModel(),
      });
      if (step === "accessibility") {
        await revealMainWindowForPermissions();
      }
      setOnboardingStep(step);
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("auth");
    }
  };

  const signedIn = useAnonenCloudStore((s) => s.authStatus.signed_in);
  const signedInEmail = useAnonenCloudStore((s) => s.authStatus.email);
  const storeInitialized = useAnonenCloudStore((s) => s.initialized);

  useEffect(() => {
    if (onboardingStep === "done" && storeInitialized && !signedIn) {
      if (signedInEmail) noteSessionEnded();
      setOnboardingStep("auth");
    }
  }, [onboardingStep, signedIn, signedInEmail, storeInitialized]);

  const currentModel = useModelStore((s) => s.currentModel);
  const modelStoreReady = useModelStore((s) => s.initialized);
  useEffect(() => {
    if (onboardingStep === null || !modelStoreReady) return;
    const valid = keepOnboardingStepValid(onboardingStep, currentModel !== "");
    if (valid !== onboardingStep) setOnboardingStep(valid);
  }, [onboardingStep, currentModel, modelStoreReady]);

  const handleAccessibilityComplete = async () => {
    const authOk = await isSignedInWithValidSubscription();
    setOnboardingStep(
      stepFor({ signedIn: authOk, modelSelected: await hasSelectedModel() }),
    );
  };

  const handleAuthComplete = async () => {
    await useAnonenCloudStore.getState().refreshAuthStatus();

    setOnboardingStep(
      stepFor({ signedIn: true, modelSelected: await hasSelectedModel() }),
    );
  };

  const handleModelSelected = () => {
    setOnboardingStep((step) =>
      step === "model"
        ? stepFor({ signedIn: true, modelSelected: true })
        : step,
    );
  };

  const handleModelSkipped = () => {
    setOnboardingStep("done");
  };

  const handleShortcutComplete = async () => {
    shortcutShownRef.current = true;

    setOnboardingStep(
      stepFor({ signedIn: true, modelSelected: await hasSelectedModel() }),
    );
  };

  const handleFirstRunComplete = () => {
    markTutorialSeen();
    setOnboardingStep("done");
  };

  if (onboardingStep === null) {
    return null;
  }

  const toaster = (
    <Toaster
      theme="system"
      toastOptions={{
        unstyled: true,
        classNames: {
          toast:
            "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
          title: "font-medium",
          description: "text-mid-gray",
          actionButton:
            "shrink-0 rounded-md border border-mid-gray/20 px-2 py-1 text-xs font-medium hover:bg-mid-gray/10",
        },
      }}
    />
  );

  let content: ReactNode;

  if (onboardingStep === "accessibility") {
    content = (
      <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />
    );
  } else if (onboardingStep === "auth") {
    content = showHistoryFromAuth ? (
      <div
        dir={direction}
        className="h-screen flex flex-col select-none cursor-default"
      >
        <div className="p-3 border-b border-border shrink-0">
          <button
            onClick={() => setShowHistoryFromAuth(false)}
            className="text-sm text-muted hover:text-text transition-colors"
          >
            ← {t("anonenCloud.back")}
          </button>
        </div>
        <div className="flex-1 overflow-y-auto">
          <div className="flex flex-col items-center p-4 gap-4">
            <HistorySettings />
          </div>
        </div>
      </div>
    ) : (
      <AuthOnboarding
        onComplete={handleAuthComplete}
        onViewHistory={() => setShowHistoryFromAuth(true)}
      />
    );
  } else if (onboardingStep === "model") {
    content = (
      <Onboarding
        onModelSelected={handleModelSelected}
        onSkip={handleModelSkipped}
      />
    );
  } else if (onboardingStep === "shortcut") {
    content = <ShortcutOnboarding onComplete={handleShortcutComplete} />;
  } else if (onboardingStep === "tryIt") {
    content = <FirstRunOnboarding onComplete={handleFirstRunComplete} />;
  } else {
    content = (
      <div
        dir={direction}
        className="h-screen flex flex-col select-none cursor-default"
      >
        <div className="flex-1 flex overflow-hidden min-h-0">
          <Sidebar
            activeSection={currentSection}
            onSectionChange={setCurrentSection}
          />

          <div className="flex-1 flex flex-col overflow-hidden min-h-0">
            <div
              ref={contentScrollRef}
              className="flex-1 overflow-y-auto min-h-0"
            >
              <div className="flex flex-col items-center p-4 gap-4">
                <AccessibilityPermissions />

                <MicClippingNotice />
                {renderSettingsContent(currentSection)}
              </div>
            </div>
          </div>
        </div>

        <Footer />
      </div>
    );
  }

  return (
    <>
      {toaster}
      {content}

      <ModelDataPolicyDialog />
    </>
  );
}

export default App;
