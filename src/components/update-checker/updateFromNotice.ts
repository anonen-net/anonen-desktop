export const updateFromNotice = async <Update>({
  updateChecksEnabled,
  check,
  offer,
  openDownloadPage,
}: {
  updateChecksEnabled: boolean;

  check: () => Promise<Update | null>;

  offer: (update: Update) => Promise<void>;
  openDownloadPage: () => Promise<void>;
}): Promise<void> => {
  if (!updateChecksEnabled) {
    await openDownloadPage();
    return;
  }
  try {
    const update = await check();
    if (!update) {
      await openDownloadPage();
      return;
    }
    await offer(update);
  } catch (error) {
    console.error("Failed to start the update from the notice:", error);
    await openDownloadPage();
  }
};
