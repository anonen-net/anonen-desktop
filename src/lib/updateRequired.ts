import { emit } from "@tauri-apps/api/event";
import { commands } from "@/bindings";
import type { ConfirmOptions } from "../components/ui/ConfirmDialog";

export const UPDATE_REQUIRED_EVENT = "update-required";

export const UPDATE_REQUIRED_INSTALL_EVENT = "update-required-install";

export const DOWNLOAD_PAGE_URL = "https://anonen.net/download/";

export const updateRequiredDialog = (
  translate: (key: string) => string,
): ConfirmOptions => ({
  title: translate("errors.notice.seal_outdated"),
  body: translate("updateRequired.reason"),
  okLabel: translate("updateRequired.update"),
  cancelLabel: translate("updateRequired.close"),
  align: "center",
});

export const serverUpdateRequired = async (): Promise<boolean> => {
  try {
    return await commands.serverUpdateRequired();
  } catch {
    return false;
  }
};

export const askToUpdate = async (
  confirmInApp: (options: ConfirmOptions) => Promise<boolean>,
  translate: (key: string) => string,
): Promise<void> => {
  if (await confirmInApp(updateRequiredDialog(translate))) {
    await emit(UPDATE_REQUIRED_INSTALL_EVENT);
  }
};
