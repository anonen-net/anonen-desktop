export const TUTORIAL_SEEN_KEY = "anonen.tutorialSeen";

export const tutorialSeen = (): boolean => {
  try {
    return localStorage.getItem(TUTORIAL_SEEN_KEY) === "1";
  } catch {
    return true;
  }
};

export const markTutorialSeen = (): void => {
  try {
    localStorage.setItem(TUTORIAL_SEEN_KEY, "1");
  } catch {}
};
