export type OnboardingStep =
  | "accessibility"
  | "auth"
  | "model"
  | "shortcut"
  | "tryIt"
  | "done";

export interface OnboardingFacts {
  needsPermissions: boolean;

  signedIn: boolean;

  modelSelected: boolean;

  tutorialSeen: boolean;

  shortcutShown: boolean;
}

export const nextOnboardingStep = (facts: OnboardingFacts): OnboardingStep => {
  if (facts.needsPermissions) return "accessibility";
  if (!facts.signedIn) return "auth";
  if (facts.tutorialSeen) return "done";
  if (!facts.modelSelected) return "model";
  if (!facts.shortcutShown) return "shortcut";
  return "tryIt";
};

export const keepOnboardingStepValid = (
  step: OnboardingStep,
  modelSelected: boolean,
): OnboardingStep => (step === "tryIt" && !modelSelected ? "model" : step);
