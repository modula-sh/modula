import { LargeButton } from "../LargeButton";
import { RecommendedToolsList } from "../recommended-tools/RecommendedToolsList";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

export function RequiredTools({ onNext, onBack }: { onNext: () => void; onBack?: () => void }) {
  return (
    <>
      <OnboardingTitle>Recommended tools</OnboardingTitle>
      <p className="text-fg-muted text-sm font-inter -mt-2">
        Tools we recommend installing for the best experience.
      </p>
      <RecommendedToolsList className="w-[32rem] font-inter" />
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={onNext}>Continue</LargeButton>
      </OnboardingActions>
    </>
  );
}
