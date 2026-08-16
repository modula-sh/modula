import { LargeButton } from "../LargeButton";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

export function CreateUser({ onNext, onBack }: { onNext: () => void; onBack?: () => void }) {
  return (
    <>
      <OnboardingTitle>Sign in</OnboardingTitle>
      <OnboardingActions onBack={onBack} className="mt-8">
        <LargeButton onClick={onNext}>Continue as guest</LargeButton>
      </OnboardingActions>
    </>
  );
}
